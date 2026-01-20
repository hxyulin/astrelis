use std::sync::Arc;

use astrelis_core::geometry::{LogicalSize, PhysicalSize, ScaleFactor};
pub use winit::dpi::PhysicalSize as WinitPhysicalSize;
pub use winit::window::Fullscreen;
pub use winit::window::Window as WinitWindow;
use winit::{error::OsError, event_loop::ActiveEventLoop};

pub struct WindowDescriptor {
    pub title: String,
    pub resizeable: bool,
    pub size: Option<winit::dpi::PhysicalSize<f32>>,
    pub visible: bool,
    pub fullscreen: Option<Fullscreen>,
}

impl Default for WindowDescriptor {
    fn default() -> Self {
        Self {
            title: "Astrelis Window".to_string(),
            resizeable: true,
            size: None,
            visible: true,
            fullscreen: None,
        }
    }
}

pub struct Window {
    pub window: Arc<winit::window::Window>,
}

impl Window {
    pub fn id(&self) -> winit::window::WindowId {
        self.window.id()
    }

    /// Get the logical size of the window (DPI-independent).
    pub fn logical_size(&self) -> LogicalSize<u32> {
        let physical_size = self.window.inner_size();
        let scale_factor = self.window.scale_factor();
        LogicalSize::new(
            (physical_size.width as f64 / scale_factor) as u32,
            (physical_size.height as f64 / scale_factor) as u32,
        )
    }

    /// Get the physical size of the window in pixels.
    pub fn physical_size(&self) -> PhysicalSize<u32> {
        self.window.inner_size().into()
    }

    /// Get the scale factor for this window.
    pub fn scale_factor(&self) -> ScaleFactor {
        ScaleFactor(self.window.scale_factor())
    }

    /// Get the raw scale factor as f64.
    pub fn scale_factor_f64(&self) -> f64 {
        self.window.scale_factor()
    }

    pub fn platform_dpi() -> f64 {
        #[cfg(target_os = "macos")]
        return 2.0;
        #[cfg(not(target_os = "macos"))]
        return 1.0;
    }

    pub(crate) fn new(
        event_loop: &ActiveEventLoop,
        descriptor: WindowDescriptor,
    ) -> Result<Self, OsError> {
        let mut attributes = WinitWindow::default_attributes()
            .with_title(descriptor.title)
            .with_resizable(descriptor.resizeable)
            .with_visible(descriptor.visible)
            .with_fullscreen(descriptor.fullscreen);

        if let Some(size) = descriptor.size {
            attributes = attributes.with_inner_size(size);
        }

        let window = Arc::new(event_loop.create_window(attributes)?);

        Ok(Window { window })
    }
}

pub trait WindowBackend {
    type FrameContext;

    fn begin_drawing(&mut self) -> Self::FrameContext;
}

pub trait WindowExt {
    /// Requests a redraw of the window.
    ///
    /// WindowBackend::begin_drawing should be preferred over this method where possible.
    /// This method only serves as a fallback when no drawing backend is available.
    fn request_redraw(&self);
}

impl WindowExt for Window {
    fn request_redraw(&self) {
        self.window.request_redraw();
    }
}
