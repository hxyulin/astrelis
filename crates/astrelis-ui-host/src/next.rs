//! Native window hosting for the incremental UI runtime.

#[cfg(target_arch = "wasm32")]
use std::sync::{Arc, Mutex};
use std::{any::Any, collections::HashMap};

use astrelis_app::{App, AppContext};
use astrelis_core::{
    color::Color,
    geometry::{LogicalPoint, LogicalSize, Size},
};
use astrelis_gpu::{SurfaceFrameStatus, TextureViewDescriptor};
use astrelis_paint_gpu::{RenderStats, RenderTarget};
use astrelis_platform::{DeviceId, ElementState, Modifiers, PointerButton, Window, WindowEvent};
use astrelis_ui_next::{UiInput, UiRoot};

use super::{
    GpuState, GraphicsContext, HostError, HostStatus, HostUpdate, WindowHost, WindowHostOptions,
    initialize_gpu,
};

/// Incremental retained UI tree connected to a platform window and GPU surface.
pub struct NextWindowHost {
    window: Window,
    gpu: Option<GpuState>,
    #[cfg(target_arch = "wasm32")]
    pending: Arc<Mutex<Option<Result<GpuState, HostError>>>>,
    failed: Option<HostError>,
    ui: UiRoot,
    clear_color: Color,
    modifiers: Modifiers,
    pointer_positions: HashMap<DeviceId, LogicalPoint>,
    actions: Vec<Box<dyn Any>>,
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
                gpu: Some(gpu),
                failed: None,
                ui,
                clear_color: options.clear_color,
                modifiers: Modifiers::default(),
                pointer_positions: HashMap::new(),
                actions: Vec::new(),
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
                gpu: None,
                pending,
                failed: None,
                ui,
                clear_color: options.clear_color,
                modifiers: Modifiers::default(),
                pointer_positions: HashMap::new(),
                actions: Vec::new(),
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

    /// Routes a platform event and updates invalidated retained passes.
    pub fn handle_event(&mut self, event: &WindowEvent) -> Result<HostUpdate, HostError> {
        self.sync_initialization();
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
            WindowEvent::KeyboardInput(input) => {
                self.dispatch(UiInput::Keyboard {
                    input: input.clone(),
                    modifiers: self.modifiers,
                })?;
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
        Ok(HostUpdate {
            redraw,
            ..HostUpdate::default()
        })
    }

    /// Generates and presents one incremental UI frame.
    pub fn redraw(&mut self) -> Result<Option<RenderStats>, HostError> {
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
                |_| Default::default(),
                |id, _, _| -> Result<(), HostError> {
                    Err(HostError::new(format!(
                        "no scene callback was supplied for compositor view {}",
                        id.get()
                    )))
                },
            )
            .map_err(HostError::from_display)?;
        gpu.queue
            .submit([encoder.finish().map_err(HostError::from_display)?])
            .map_err(HostError::from_display)?;
        frame.present().map_err(HostError::from_display)?;
        Ok(Some(stats.paint))
    }

    fn dispatch(&mut self, input: UiInput) -> Result<(), HostError> {
        if let Some(action) = self.ui.dispatch(input).map_err(HostError::from_display)? {
            self.actions.push(action);
        }
        Ok(())
    }

    fn update_passes(&mut self) -> Result<(), HostError> {
        self.ui
            .update_passes()
            .map(|_| ())
            .map_err(HostError::from_display)
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
