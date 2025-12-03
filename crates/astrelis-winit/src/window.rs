use std::{any::Any, sync::Arc};

pub use winit::dpi::PhysicalSize;
pub use winit::window::Fullscreen;
pub use winit::window::Window as WinitWindow;
use winit::{error::OsError, event_loop::ActiveEventLoop, window::WindowAttributes};

pub struct WindowDescriptor {
    pub title: String,
    pub resizeable: bool,
    pub size: Option<PhysicalSize<f32>>,
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

    pub(crate) fn new(
        event_loop: &ActiveEventLoop,
        descriptor: WindowDescriptor,
    ) -> Result<Self, OsError> {
        let mut attributes = WindowAttributes::new()
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
