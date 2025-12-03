use astrelis_core::{geometry::{Size}, profiling::profile_function};
use astrelis_winit::{
    WindowId, event::PhysicalSize, window::{Window, WindowBackend}
};

use crate::{
    context::GraphicsContext,
    frame::{FrameContext, FrameStats, Surface},
};

/// Viewport definition for rendering.
#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub scale_factor: f64,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
            // it needs to be 1.0 to avoid division by zero and other issues
            scale_factor: 1.0,
        }
    }
}

impl Viewport {
    pub fn is_valid(&self) -> bool {
        self.width > 0.0 && self.height > 0.0 && self.scale_factor > 0.0
    }

    /// Get the size in logical pixels.
    pub fn to_logical(&self) -> Size<f32> {
        Size {
            width: self.width as f32 / self.scale_factor as f32,
            height: self.height as f32 / self.scale_factor as f32,
        }
    }
}

/// Descriptor for configuring a window's rendering context.
#[derive(Default)]
pub struct WindowContextDescriptor {
    /// The surface texture format. If None, uses the default format for the surface.
    pub format: Option<wgpu::TextureFormat>,
    /// Present mode for the surface.
    pub present_mode: Option<wgpu::PresentMode>,
    /// Alpha mode for the surface.
    pub alpha_mode: Option<wgpu::CompositeAlphaMode>,
}

pub struct PendingReconfigure {
    pub resize: Option<PhysicalSize<u32>>,
}

impl PendingReconfigure {
    const fn new() -> Self {
        Self { resize: None }
    }
}

/// Window rendering context that manages a surface and its configuration.
pub struct WindowContext {
    pub(crate) window: Window,
    pub(crate) context: &'static GraphicsContext,
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) reconfigure: PendingReconfigure,
}

impl WindowContext {
    pub fn new(
        window: Window,
        context: &'static GraphicsContext,
        descriptor: WindowContextDescriptor,
    ) -> Self {
        let scale_factor = window.window.scale_factor();
        let Size { width, height } = window.size();
        let (width, height) = (
            (width as f64 * scale_factor) as u32,
            (height as f64 * scale_factor) as u32,
        );

        let surface = context
            .instance
            .create_surface(window.window.clone())
            .expect("Failed to create surface");

        let mut config = surface
            .get_default_config(&context.adapter, width, height)
            .expect("Failed to get default surface configuration");

        if let Some(format) = descriptor.format {
            config.format = format;
        }
        if let Some(present_mode) = descriptor.present_mode {
            config.present_mode = present_mode;
        }
        if let Some(alpha_mode) = descriptor.alpha_mode {
            config.alpha_mode = alpha_mode;
        }

        surface.configure(&context.device, &config);

        Self {
            window,
            surface,
            config,
            reconfigure: PendingReconfigure::new(),
            context,
        }
    }

    /// Handle window resize event
    pub fn resized(&mut self, new_size: Size<u32>) {
        let scale_factor = self.window.window.scale_factor();
        self.reconfigure.resize = Some(PhysicalSize {
            width: (new_size.width as f64 * scale_factor) as u32,
            height: (new_size.height as f64 * scale_factor) as u32,
        });
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn graphics_context(&self) -> &GraphicsContext {
        self.context
    }

    pub fn surface(&self) -> &wgpu::Surface<'static> {
        &self.surface
    }

    pub fn surface_config(&self) -> &wgpu::SurfaceConfiguration {
        &self.config
    }

    /// Get the size of the window.
    pub fn size(&self) -> Size<u32> {
        self.window.size()
    }

    pub fn size_f32(&self) -> Size<f32> {
        let size = self.size();
        Size {
            width: size.width as f32,
            height: size.height as f32,
        }
    }

    /// Get the inner size of the window.
    pub fn inner_size(&self) -> PhysicalSize<u32> {
        self.window.inner_size()
    }

    /// Reconfigure the surface with a new configuration.
    pub fn reconfigure_surface(&mut self, config: wgpu::SurfaceConfiguration) {
        self.config = config;
        self.surface.configure(&self.context.device, &self.config);
    }
}

impl WindowBackend for WindowContext {
    type FrameContext = FrameContext;

    fn begin_drawing(&mut self) -> Self::FrameContext {
        profile_function!();

        let mut configure_needed = false;
        if let Some(new_size) = self.reconfigure.resize.take() {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            configure_needed = true;
        }

        if configure_needed {
            self.surface.configure(&self.context.device, &self.config);
        }

        let frame = self.surface.get_current_texture().unwrap();
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let encoder = self
            .context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Frame Encoder"),
            });

        FrameContext {
            surface: Some(Surface {
                texture: frame,
                view,
            }),
            encoder: Some(encoder),
            context: self.context,
            stats: FrameStats::new(),
            window: self.window.window.clone(),
        }
    }
}

/// A renderable window that combines a window with a rendering context.
pub struct RenderableWindow {
    pub(crate) context: WindowContext,
}

impl RenderableWindow {
    pub fn new(window: Window, context: &'static GraphicsContext) -> Self {
        Self::new_with_descriptor(window, context, WindowContextDescriptor::default())
    }

    pub fn new_with_descriptor(
        window: Window,
        context: &'static GraphicsContext,
        descriptor: WindowContextDescriptor,
    ) -> Self {
        let context = WindowContext::new(window, context, descriptor);
        Self { context }
    }

    pub fn id(&self) -> WindowId {
        self.context.window.id()
    }

    pub fn window(&self) -> &Window {
        &self.context.window
    }

    pub fn context(&self) -> &WindowContext {
        &self.context
    }

    pub fn context_mut(&mut self) -> &mut WindowContext {
        &mut self.context
    }

    /// Handle window resize event
    pub fn resized(&mut self, new_size: Size<u32>) {
        self.context.resized(new_size);
    }

    /// Get the inner size of the window.
    pub fn inner_size(&self) -> PhysicalSize<u32> {
        self.context.inner_size()
    }

    pub fn scale_factor(&self) -> f64 {
        self.window.window.scale_factor()
    }

    pub fn viewport(&self) -> Viewport {
        let PhysicalSize { width, height } = self.inner_size();
        let scale_factor = self.scale_factor();

        Viewport {
            x: 0.0,
            y: 0.0,
            width: width as f32,
            height: height as f32,
            scale_factor,
        }
    }
}

impl std::ops::Deref for RenderableWindow {
    type Target = WindowContext;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}

impl std::ops::DerefMut for RenderableWindow {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.context
    }
}

impl WindowBackend for RenderableWindow {
    type FrameContext = FrameContext;

    fn begin_drawing(&mut self) -> Self::FrameContext {
        self.context.begin_drawing()
    }
}
