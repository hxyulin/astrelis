pub use winit::{
    dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize, Position, Size},
    window::Fullscreen,
};
use winit::{event_loop::ActiveEventLoop, window::Window as WinitWindow};

#[derive(Debug)]
pub struct WindowOpts {
    size: Option<(f32, f32)>,
    title: String,
}

impl Default for WindowOpts {
    fn default() -> Self {
        Self {
            size: None,
            title: "Astrelis Window".to_string(),
        }
    }
}

impl WindowOpts {}

/// A user handle to a window
pub struct Window {
    window: WinitWindow,
}

impl Window {
    pub(crate) fn new(event_loop: &ActiveEventLoop, opts: WindowOpts) -> Self {
        let mut attributes = WinitWindow::default_attributes().with_title(opts.title);
        if let Some((width, height)) = opts.size {
            attributes.inner_size = Some(LogicalSize::new(width, height).into());
        }
        let window = event_loop
            .create_window(attributes)
            .expect("failed to create window");
        Self { window }
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }
}
