//! Cross-platform Astrelis UI, surface, painter, and compositor hosting.

#![warn(missing_docs)]

use std::{error::Error, fmt};

#[cfg(target_arch = "wasm32")]
use std::sync::{Arc, Mutex};

use astrelis_app::{App, AppContext};
use astrelis_compositor::{CompositionStats, Compositor, ViewOptions, ViewRenderTarget};
use astrelis_core::{color::Color, geometry::Size};
use astrelis_gpu::{
    CompositeAlphaMode, DeviceDescriptor, PresentMode, RequestAdapterOptions, SurfaceConfiguration,
    SurfaceFrameStatus, SurfaceTarget, TextureUsages, TextureViewDescriptor,
};
use astrelis_paint::CompositorViewId;
use astrelis_paint_gpu::{ExternalImage, RenderStats, RenderTarget, Renderer, RendererOptions};
use astrelis_platform::{Window, WindowAttributes, WindowEvent, WindowId};
use astrelis_ui_core::Ui;

/// Shared graphics entry point used to open Astrelis windows.
#[derive(Clone)]
pub struct GraphicsContext {
    instance: astrelis_gpu::Instance,
}

impl GraphicsContext {
    /// Creates graphics using Astrelis's default wgpu instance configuration.
    pub fn new() -> Self {
        Self {
            instance: astrelis_gpu_wgpu::create_instance(Default::default()),
        }
    }

    /// Wraps an application-configured backend-neutral instance.
    pub const fn from_instance(instance: astrelis_gpu::Instance) -> Self {
        Self { instance }
    }

    /// Returns the underlying backend-neutral instance.
    pub const fn instance(&self) -> &astrelis_gpu::Instance {
        &self.instance
    }
}

impl Default for GraphicsContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Creation and rendering policy for one UI window.
#[derive(Clone, Debug)]
pub struct WindowHostOptions {
    /// Platform window attributes.
    pub window: WindowAttributes,
    /// Color used to clear pixels behind the UI display list.
    pub clear_color: Color,
    /// Painter renderer configuration.
    pub renderer: RendererOptions,
}

impl Default for WindowHostOptions {
    fn default() -> Self {
        Self {
            window: WindowAttributes::default(),
            clear_color: Color::BLACK,
            renderer: RendererOptions::default(),
        }
    }
}

struct GpuState {
    surface: astrelis_gpu::Surface,
    device: astrelis_gpu::Device,
    queue: astrelis_gpu::Queue,
    configuration: SurfaceConfiguration,
    render_format: astrelis_gpu::TextureFormat,
    compositor: Compositor,
}

/// Observable GPU initialization state for a hosted window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostStatus {
    /// Browser WebGPU adapter/device acquisition is still running.
    Initializing,
    /// The surface, device, queue, painter, and compositor are ready.
    Ready,
    /// Initialization failed; rendering returns the same stored error.
    Failed,
}

/// Result of routing one platform event through a window host.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostUpdate {
    /// The platform close button was requested.
    pub close_requested: bool,
    /// The window should be invalidated in the Astrelis runtime.
    pub redraw: bool,
    /// UI input changed cursor, IME, or another platform window property.
    pub platform_state_changed: bool,
}

/// One retained UI tree connected to a platform window and GPU surface.
pub struct WindowHost<Message = ()> {
    window: Window,
    gpu: Option<GpuState>,
    #[cfg(target_arch = "wasm32")]
    pending: Arc<Mutex<Option<Result<GpuState, HostError>>>>,
    failed: Option<HostError>,
    ui: Ui<Message>,
    clear_color: Color,
}

impl<Message: 'static> WindowHost<Message> {
    /// Creates and registers a window.
    ///
    /// Native GPU initialization completes before this returns. Browser GPU
    /// initialization is spawned locally and completes asynchronously.
    pub fn open<A: App>(
        context: &mut AppContext<'_, '_, A>,
        graphics: &GraphicsContext,
        ui: Ui<Message>,
        options: WindowHostOptions,
    ) -> Result<Self, HostError> {
        let window = context
            .create_window(options.window)
            .map_err(HostError::from_display)?;

        #[cfg(not(target_arch = "wasm32"))]
        {
            let result = pollster::block_on(initialize_gpu(
                graphics.instance.clone(),
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
            };
            host.sync_viewport();
            Ok(host)
        }

        #[cfg(target_arch = "wasm32")]
        {
            let pending = Arc::new(Mutex::new(None));
            let completion = pending.clone();
            let instance = graphics.instance.clone();
            let initialization_window = window.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let result =
                    initialize_gpu(instance, initialization_window.clone(), options.renderer).await;
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
            };
            host.sync_viewport();
            Ok(host)
        }
    }

    /// Returns the current initialization state.
    pub fn status(&mut self) -> HostStatus {
        self.sync_initialization();
        if self.gpu.is_some() {
            HostStatus::Ready
        } else if self.failed.is_some() {
            HostStatus::Failed
        } else {
            HostStatus::Initializing
        }
    }

    /// Returns the stable initialization failure, when initialization failed.
    pub fn initialization_error(&mut self) -> Option<&HostError> {
        self.sync_initialization();
        self.failed.as_ref()
    }

    /// Returns the platform window identifier.
    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    /// Returns the platform window.
    pub const fn window(&self) -> &Window {
        &self.window
    }

    /// Returns the retained UI tree.
    pub const fn ui(&self) -> &Ui<Message> {
        &self.ui
    }

    /// Returns the retained UI tree for application updates.
    pub const fn ui_mut(&mut self) -> &mut Ui<Message> {
        &mut self.ui
    }

    /// Returns the GPU device, or `None` while initialization is pending or failed.
    pub fn device(&mut self) -> Option<&astrelis_gpu::Device> {
        self.sync_initialization();
        self.gpu.as_ref().map(|gpu| &gpu.device)
    }

    /// Returns the GPU queue, or `None` while initialization is pending or failed.
    pub fn queue(&mut self) -> Option<&astrelis_gpu::Queue> {
        self.sync_initialization();
        self.gpu.as_ref().map(|gpu| &gpu.queue)
    }

    /// Returns the configured presentation format once GPU initialization completes.
    pub fn surface_format(&mut self) -> Option<astrelis_gpu::TextureFormat> {
        self.sync_initialization();
        self.gpu.as_ref().map(|gpu| gpu.configuration.format)
    }

    /// Registers or replaces an application-owned texture sampled by a render view.
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

    /// Removes a previously registered render-view image.
    pub fn unregister_external_image(&mut self, image: &ExternalImage) -> bool {
        self.sync_initialization();
        self.gpu
            .as_mut()
            .is_some_and(|gpu| gpu.compositor.paint_mut().unregister_external_image(image))
    }

    /// Drains typed messages emitted by UI listeners.
    pub fn drain_messages(&mut self) -> impl Iterator<Item = Message> + '_ {
        self.ui.drain_messages()
    }

    /// Routes one platform event, updates the surface, and reports scheduling work.
    pub fn handle_event(
        &mut self,
        clipboard: &astrelis_platform::Clipboard,
        event: &WindowEvent,
    ) -> Result<HostUpdate, HostError> {
        self.sync_initialization();
        if matches!(event, WindowEvent::CloseRequested) {
            return Ok(HostUpdate {
                close_requested: true,
                ..Default::default()
            });
        }
        match event {
            WindowEvent::Resized(size) => self.configure(size.width, size.height)?,
            WindowEvent::ScaleFactorChanged { inner_size, .. } => {
                self.configure(inner_size.width, inner_size.height)?;
            }
            _ => {}
        }
        let update = self
            .ui
            .handle_window_event(&self.window, clipboard, event)
            .map_err(HostError::from_display)?;
        Ok(HostUpdate {
            close_requested: false,
            redraw: update.redraw || self.ui.needs_redraw(),
            platform_state_changed: update.platform_state_changed,
        })
    }

    /// Generates and presents a UI-only frame.
    ///
    /// `None` means initialization is pending or the surface is temporarily
    /// unavailable. A failed initialization always returns the stored error.
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

    /// Generates and presents a UI frame with compositor-backed scene views.
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
        E: fmt::Display,
    {
        self.sync_initialization();
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        if self.gpu.is_none() {
            return Ok(None);
        }
        let list = self.ui.display_list().map_err(HostError::from_display)?;
        let gpu = self.gpu.as_mut().expect("checked above");
        let frame = match gpu.surface.acquire().map_err(HostError::from_display)? {
            SurfaceFrameStatus::Ready(frame) | SurfaceFrameStatus::Suboptimal(frame) => frame,
            SurfaceFrameStatus::Outdated | SurfaceFrameStatus::Lost => {
                Self::reconfigure_gpu(gpu)?;
                return Ok(None);
            }
            SurfaceFrameStatus::Timeout | SurfaceFrameStatus::Occluded => return Ok(None),
            _ => return Ok(None),
        };
        let view = frame.texture().create_view(TextureViewDescriptor {
            format: Some(gpu.render_format),
            ..Default::default()
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

    fn ready_gpu(&mut self) -> Result<&mut GpuState, HostError> {
        self.sync_initialization();
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        self.gpu
            .as_mut()
            .ok_or_else(|| HostError::new("GPU initialization is still pending"))
    }

    fn configure(&mut self, width: u32, height: u32) -> Result<(), HostError> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        if let Some(gpu) = &mut self.gpu {
            gpu.configuration.width = width;
            gpu.configuration.height = height;
            Self::reconfigure_gpu(gpu)?;
        }
        self.sync_viewport();
        Ok(())
    }

    fn reconfigure_gpu(gpu: &GpuState) -> Result<(), HostError> {
        gpu.surface
            .configure(&gpu.device, gpu.configuration.clone())
            .map_err(HostError::from_display)
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
        self.ui.set_viewport(
            Size::new(width as f32 / scale, height as f32 / scale),
            scale,
        );
    }
}

async fn initialize_gpu(
    instance: astrelis_gpu::Instance,
    window: Window,
    renderer_options: RendererOptions,
) -> Result<GpuState, HostError> {
    let surface = instance
        .create_surface(SurfaceTarget::new(window.clone()))
        .map_err(HostError::from_display)?;
    let adapter = instance
        .request_adapter(RequestAdapterOptions {
            compatible_surface: Some(surface.clone()),
            ..Default::default()
        })
        .await
        .map_err(HostError::from_display)?;
    let (device, queue) = adapter
        .request_device(DeviceDescriptor::default())
        .await
        .map_err(HostError::from_display)?;
    let capabilities = surface
        .capabilities(&adapter)
        .map_err(HostError::from_display)?;
    let format = capabilities
        .formats
        .first()
        .copied()
        .ok_or_else(|| HostError::new("surface reported no supported formats"))?;
    let size = window.inner_size().map_err(HostError::from_display)?;
    let render_format = srgb_view_format(format);
    let configuration = SurfaceConfiguration {
        usage: TextureUsages::RENDER_ATTACHMENT,
        format,
        view_formats: (render_format != format)
            .then_some(render_format)
            .into_iter()
            .collect(),
        width: size.width.max(1),
        height: size.height.max(1),
        present_mode: PresentMode::Fifo,
        alpha_mode: capabilities
            .alpha_modes
            .first()
            .copied()
            .unwrap_or(CompositeAlphaMode::Opaque),
        desired_maximum_frame_latency: 2,
    };
    surface
        .configure(&device, configuration.clone())
        .map_err(HostError::from_display)?;
    let painter = Renderer::new(device.clone(), queue.clone(), renderer_options)
        .map_err(HostError::from_display)?;
    let compositor = Compositor::new(device.clone(), painter);
    Ok(GpuState {
        surface,
        device,
        queue,
        configuration,
        render_format,
        compositor,
    })
}

/// Failure while creating, updating, or rendering a hosted window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostError(String);

impl HostError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }

    fn from_display(error: impl fmt::Display) -> Self {
        Self(error.to_string())
    }
}

impl fmt::Display for HostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for HostError {}

fn srgb_view_format(format: astrelis_gpu::TextureFormat) -> astrelis_gpu::TextureFormat {
    match format {
        astrelis_gpu::TextureFormat::Bgra8Unorm => astrelis_gpu::TextureFormat::Bgra8UnormSrgb,
        astrelis_gpu::TextureFormat::Rgba8Unorm => astrelis_gpu::TextureFormat::Rgba8UnormSrgb,
        _ => format,
    }
}

#[cfg(test)]
mod tests {
    use astrelis_gpu::TextureFormat;

    use super::srgb_view_format;

    #[test]
    fn linear_surface_formats_use_srgb_frame_views() {
        assert_eq!(
            srgb_view_format(TextureFormat::Bgra8Unorm),
            TextureFormat::Bgra8UnormSrgb
        );
        assert_eq!(
            srgb_view_format(TextureFormat::Rgba8Unorm),
            TextureFormat::Rgba8UnormSrgb
        );
        assert_eq!(
            srgb_view_format(TextureFormat::Rgba16Float),
            TextureFormat::Rgba16Float
        );
    }
}
