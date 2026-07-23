//! Native window hosting for the incremental UI runtime.

#[cfg(target_arch = "wasm32")]
use std::sync::{Arc, Mutex};
use std::{any::Any, collections::HashMap};

use astrelis_app::{App, AppContext};
use astrelis_compositor::{CompositionStats, ViewOptions, ViewRenderTarget};
use astrelis_core::{
    color::Color,
    geometry::{LogicalPoint, LogicalSize, Size},
};
use astrelis_gpu::{SurfaceFrameStatus, TextureViewDescriptor};
use astrelis_paint::CompositorViewId;
use astrelis_paint_gpu::{ExternalImage, RenderStats, RenderTarget};
use astrelis_platform::{
    Clipboard, DeviceId, ElementState, Key, Modifiers, PointerButton, ScrollDelta, Window,
    WindowEvent,
};
use astrelis_ui_next::{
    AccessibilityUpdate, ClipboardOperation, NodeId, SemanticAction, SemanticNode, UiInput, UiRoot,
};

use super::{
    GpuState, GraphicsContext, HostError, HostStatus, HostUpdate, WindowHost, WindowHostOptions,
    initialize_gpu,
};

/// One platform accessibility request targeting the incremental retained tree.
#[derive(Clone, Debug, PartialEq)]
pub struct NextAccessibilityRequest {
    /// Stable retained semantic target.
    pub target: NodeId,
    /// Backend-neutral operation.
    pub action: SemanticAction,
}

/// Platform accessibility bridge for [`NextWindowHost`].
pub trait NextAccessibilityAdapter {
    /// Observes a platform event before retained input routing.
    fn handle_window_event(
        &mut self,
        _window: &Window,
        _event: &WindowEvent,
    ) -> Result<(), HostError> {
        Ok(())
    }

    /// Drains operations requested by the platform.
    fn drain_requests(&mut self) -> Vec<NextAccessibilityRequest> {
        Vec::new()
    }

    /// Publishes the current semantic snapshot and its latest delta.
    fn update(
        &mut self,
        window: &Window,
        snapshot: &[SemanticNode],
        delta: &AccessibilityUpdate,
    ) -> Result<(), HostError>;
}

/// Incremental retained UI tree connected to a platform window and GPU surface.
pub struct NextWindowHost {
    window: Window,
    clipboard: Clipboard,
    gpu: Option<GpuState>,
    #[cfg(target_arch = "wasm32")]
    pending: Arc<Mutex<Option<Result<GpuState, HostError>>>>,
    failed: Option<HostError>,
    ui: UiRoot,
    clear_color: Color,
    modifiers: Modifiers,
    pointer_positions: HashMap<DeviceId, LogicalPoint>,
    actions: Vec<Box<dyn Any>>,
    accessibility: Option<Box<dyn NextAccessibilityAdapter>>,
}

impl NextWindowHost {
    /// Creates and registers a window for an incremental UI tree.
    pub fn open<A: App>(
        context: &mut AppContext<'_, '_, A>,
        graphics: &GraphicsContext,
        ui: UiRoot,
        options: WindowHostOptions,
    ) -> Result<Self, HostError> {
        let window = context
            .create_window(options.window)
            .map_err(HostError::from_display)?;
        let clipboard = context.clipboard();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let result = pollster::block_on(initialize_gpu(
                graphics.clone(),
                window.clone(),
                options.renderer,
            ));
            let gpu = match result {
                Ok(gpu) => gpu,
                Err(error) => {
                    context.unregister_window(window.id());
                    return Err(error);
                }
            };
            let mut host = Self {
                window,
                clipboard,
                gpu: Some(gpu),
                failed: None,
                ui,
                clear_color: options.clear_color,
                modifiers: Modifiers::default(),
                pointer_positions: HashMap::new(),
                actions: Vec::new(),
                accessibility: None,
            };
            host.sync_viewport();
            host.update_passes()?;
            Ok(host)
        }

        #[cfg(target_arch = "wasm32")]
        {
            let pending = Arc::new(Mutex::new(None));
            let completion = pending.clone();
            let graphics = graphics.clone();
            let initialization_window = window.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let result =
                    initialize_gpu(graphics, initialization_window.clone(), options.renderer).await;
                *completion
                    .lock()
                    .expect("host initialization state poisoned") = Some(result);
                initialization_window.request_redraw();
            });
            let mut host = Self {
                window,
                clipboard,
                gpu: None,
                pending,
                failed: None,
                ui,
                clear_color: options.clear_color,
                modifiers: Modifiers::default(),
                pointer_positions: HashMap::new(),
                actions: Vec::new(),
                accessibility: None,
            };
            host.sync_viewport();
            host.update_passes()?;
            Ok(host)
        }
    }

    /// Returns the platform window.
    pub const fn window(&self) -> &Window {
        &self.window
    }

    /// Returns the incremental retained UI tree.
    pub const fn ui(&self) -> &UiRoot {
        &self.ui
    }

    /// Returns the incremental retained UI tree for application updates.
    pub fn ui_mut(&mut self) -> &mut UiRoot {
        &mut self.ui
    }

    /// Returns current GPU initialization state.
    pub fn status(&mut self) -> HostStatus {
        self.sync_initialization();
        if self.gpu.as_ref().is_some_and(|gpu| gpu.device.is_lost()) {
            HostStatus::DeviceLost
        } else if self.gpu.is_some() {
            HostStatus::Ready
        } else if self.failed.is_some() {
            HostStatus::Failed
        } else {
            HostStatus::Initializing
        }
    }

    /// Drains actions emitted by retained elements.
    pub fn drain_actions(&mut self) -> impl Iterator<Item = Box<dyn Any>> + '_ {
        self.actions.drain(..)
    }

    /// Installs a platform accessibility adapter and publishes the current tree.
    pub fn set_accessibility_adapter(
        &mut self,
        mut adapter: impl NextAccessibilityAdapter + 'static,
    ) -> Result<(), HostError> {
        adapter.update(
            &self.window,
            &self.ui.semantic_snapshot(),
            &AccessibilityUpdate::default(),
        )?;
        self.accessibility = Some(Box::new(adapter));
        Ok(())
    }

    /// Removes and returns the platform accessibility adapter.
    pub fn take_accessibility_adapter(&mut self) -> Option<Box<dyn NextAccessibilityAdapter>> {
        self.accessibility.take()
    }

    /// Routes a platform event and updates invalidated retained passes.
    pub fn handle_event(&mut self, event: &WindowEvent) -> Result<HostUpdate, HostError> {
        self.sync_initialization();
        if let Some(accessibility) = &mut self.accessibility {
            accessibility.handle_window_event(&self.window, event)?;
        }
        if matches!(event, WindowEvent::CloseRequested) {
            return Ok(HostUpdate {
                close_requested: true,
                ..HostUpdate::default()
            });
        }
        let redraw = match event {
            WindowEvent::Resized(size) => {
                self.configure(size.width, size.height)?;
                true
            }
            WindowEvent::ScaleFactorChanged { inner_size, .. } => {
                self.configure(inner_size.width, inner_size.height)?;
                true
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = *modifiers;
                false
            }
            WindowEvent::PointerMoved {
                device_id,
                position,
            } => {
                let point = self.logical_point(position.x, position.y);
                self.pointer_positions.insert(*device_id, point);
                self.dispatch(UiInput::PointerMoved(point))?;
                true
            }
            WindowEvent::PointerButton {
                device_id,
                button: PointerButton::Primary,
                state,
            } => {
                if let Some(point) = self.pointer_positions.get(device_id).copied() {
                    self.dispatch(match state {
                        ElementState::Pressed => UiInput::PointerPressed(point),
                        ElementState::Released => UiInput::PointerReleased(point),
                    })?;
                }
                true
            }
            WindowEvent::PointerWheel {
                device_id, delta, ..
            } => {
                if let Some(position) = self.pointer_positions.get(device_id).copied() {
                    let scale = self.window.scale_factor().max(f64::EPSILON);
                    let delta = match delta {
                        ScrollDelta::Lines { x, y } => LogicalPoint::new(-x * 40.0, -y * 40.0),
                        ScrollDelta::Pixels(point) => {
                            LogicalPoint::new((-point.x / scale) as f32, (-point.y / scale) as f32)
                        }
                    };
                    self.dispatch(UiInput::PointerWheel { position, delta })?;
                }
                true
            }
            WindowEvent::KeyboardInput(input) => {
                let paste = input.state == ElementState::Pressed
                    && (self.modifiers.control || self.modifiers.super_key)
                    && matches!(
                        &input.logical_key,
                        Key::Character(value) if value.eq_ignore_ascii_case("v")
                    );
                self.dispatch(UiInput::Keyboard {
                    input: input.clone(),
                    modifiers: self.modifiers,
                })?;
                if paste
                    && self.clipboard.capabilities().read_text
                    && let Some(text) = self
                        .clipboard
                        .read_text()
                        .map_err(HostError::from_display)?
                {
                    self.dispatch(UiInput::Paste(text))?;
                }
                true
            }
            WindowEvent::Ime(event) => {
                self.dispatch(UiInput::Ime(event.clone()))?;
                true
            }
            WindowEvent::Focused(focused) => {
                self.dispatch(UiInput::FocusChanged(*focused))?;
                true
            }
            _ => false,
        };
        if redraw {
            self.update_passes()?;
        }
        self.flush_clipboard()?;
        let requests = self
            .accessibility
            .as_mut()
            .map(|adapter| adapter.drain_requests())
            .unwrap_or_default();
        let accessibility_requested = !requests.is_empty();
        for request in requests {
            if let Some(action) = self
                .ui
                .perform_semantic_action(request.target, request.action)
                .map_err(HostError::from_display)?
            {
                self.actions.push(action);
            }
        }
        if accessibility_requested {
            self.update_passes()?;
        }
        Ok(HostUpdate {
            redraw: redraw || accessibility_requested,
            ..HostUpdate::default()
        })
    }

    /// Generates and presents one incremental UI frame.
    pub fn redraw(&mut self) -> Result<Option<RenderStats>, HostError> {
        self.redraw_composited(
            |_| ViewOptions::default(),
            |id, _, _| -> Result<(), HostError> {
                Err(HostError::new(format!(
                    "no scene callback was supplied for compositor view {}",
                    id.get()
                )))
            },
        )
        .map(|stats| stats.map(|stats| stats.paint))
    }

    /// Registers an application-owned texture sampled by retained paint.
    pub fn register_external_image(
        &mut self,
        image: &ExternalImage,
        view: astrelis_gpu::TextureView,
    ) -> Result<(), HostError> {
        self.ready_gpu()?
            .compositor
            .paint_mut()
            .register_external_image(image, view)
            .map_err(HostError::from_display)
    }

    /// Removes a previously registered retained-paint image.
    pub fn unregister_external_image(&mut self, image: &ExternalImage) -> bool {
        self.sync_initialization();
        self.gpu
            .as_mut()
            .is_some_and(|gpu| gpu.compositor.paint_mut().unregister_external_image(image))
    }

    /// Generates and presents a frame with application-rendered compositor views.
    pub fn redraw_composited<E>(
        &mut self,
        view_options: impl FnMut(CompositorViewId) -> ViewOptions,
        render_view: impl FnMut(
            CompositorViewId,
            &mut astrelis_gpu::CommandEncoder,
            ViewRenderTarget,
        ) -> Result<(), E>,
    ) -> Result<Option<CompositionStats>, HostError>
    where
        E: std::fmt::Display,
    {
        self.sync_initialization();
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        if self.gpu.as_ref().is_some_and(|gpu| gpu.device.is_lost()) {
            return Err(HostError::new(
                "the shared GPU device was lost; recreate the graphics context and window hosts",
            ));
        }
        if self.gpu.is_none() {
            return Ok(None);
        }
        self.update_passes()?;
        let list = self.ui.scene().flatten().map_err(HostError::from_display)?;
        let gpu = self.gpu.as_mut().expect("checked above");
        let frame = match gpu.surface.acquire().map_err(HostError::from_display)? {
            SurfaceFrameStatus::Ready(frame) | SurfaceFrameStatus::Suboptimal(frame) => frame,
            SurfaceFrameStatus::Outdated | SurfaceFrameStatus::Lost => {
                WindowHost::<()>::reconfigure_gpu(gpu)?;
                return Ok(None);
            }
            SurfaceFrameStatus::Timeout | SurfaceFrameStatus::Occluded => return Ok(None),
            _ => return Ok(None),
        };
        let view = frame.texture().create_view(TextureViewDescriptor {
            format: Some(gpu.render_format),
            ..TextureViewDescriptor::default()
        });
        let mut encoder = gpu.device.create_command_encoder(Default::default());
        let stats = gpu
            .compositor
            .render(
                &mut encoder,
                &list,
                RenderTarget {
                    view,
                    format: gpu.render_format,
                    size: Size::new(gpu.configuration.width, gpu.configuration.height),
                    scale_factor: self.window.scale_factor() as f32,
                    clear_color: self.clear_color,
                },
                view_options,
                render_view,
            )
            .map_err(HostError::from_display)?;
        gpu.queue
            .submit([encoder.finish().map_err(HostError::from_display)?])
            .map_err(HostError::from_display)?;
        frame.present().map_err(HostError::from_display)?;
        Ok(Some(stats))
    }

    fn dispatch(&mut self, input: UiInput) -> Result<(), HostError> {
        if let Some(action) = self.ui.dispatch(input).map_err(HostError::from_display)? {
            self.actions.push(action);
        }
        Ok(())
    }

    fn flush_clipboard(&mut self) -> Result<(), HostError> {
        for operation in self.ui.drain_clipboard() {
            match operation {
                ClipboardOperation::WriteText(text) if self.clipboard.capabilities().write_text => {
                    self.clipboard
                        .write_text(text)
                        .map_err(HostError::from_display)?;
                }
                ClipboardOperation::WriteText(_) => {}
            }
        }
        Ok(())
    }

    fn ready_gpu(&mut self) -> Result<&mut GpuState, HostError> {
        self.sync_initialization();
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        if self.gpu.as_ref().is_some_and(|gpu| gpu.device.is_lost()) {
            return Err(HostError::new(
                "the shared GPU device was lost; recreate the graphics context and window hosts",
            ));
        }
        self.gpu
            .as_mut()
            .ok_or_else(|| HostError::new("GPU initialization is still pending"))
    }

    fn update_passes(&mut self) -> Result<(), HostError> {
        let delta = self
            .ui
            .update_passes()
            .map(|update| update.accessibility.clone())
            .map_err(HostError::from_display)?;
        if let Some(accessibility) = &mut self.accessibility
            && (!delta.changed.is_empty() || !delta.removed.is_empty())
        {
            accessibility.update(&self.window, &self.ui.semantic_snapshot(), &delta)?;
        }
        Ok(())
    }

    fn logical_point(&self, x: f64, y: f64) -> LogicalPoint {
        let scale = self.window.scale_factor().max(f64::EPSILON);
        LogicalPoint::new((x / scale) as f32, (y / scale) as f32)
    }

    fn configure(&mut self, width: u32, height: u32) -> Result<(), HostError> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        if let Some(gpu) = &mut self.gpu {
            gpu.configuration.width = width;
            gpu.configuration.height = height;
            WindowHost::<()>::reconfigure_gpu(gpu)?;
        }
        self.sync_viewport();
        Ok(())
    }

    fn sync_viewport(&mut self) {
        let scale = (self.window.scale_factor() as f32).max(f32::EPSILON);
        let size = self.window.inner_size().ok();
        let (width, height) = self
            .gpu
            .as_ref()
            .map(|gpu| (gpu.configuration.width, gpu.configuration.height))
            .or_else(|| size.map(|size| (size.width.max(1), size.height.max(1))))
            .unwrap_or((1, 1));
        self.ui.set_viewport(LogicalSize::new(
            width as f32 / scale,
            height as f32 / scale,
        ));
    }

    fn sync_initialization(&mut self) {
        #[cfg(target_arch = "wasm32")]
        if self.gpu.is_none() && self.failed.is_none() {
            let result = self
                .pending
                .lock()
                .expect("host initialization state poisoned")
                .take();
            if let Some(result) = result {
                match result {
                    Ok(gpu) => self.gpu = Some(gpu),
                    Err(error) => self.failed = Some(error),
                }
                self.sync_viewport();
            }
        }
    }
}
