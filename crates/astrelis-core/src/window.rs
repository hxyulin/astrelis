use std::sync::Arc;

pub use winit::{
    dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize, Position, Size},
    window::Fullscreen,
};
use winit::{event_loop::ActiveEventLoop, window::Window as WinitWindow};

use crate::{
    graphics::{GraphicsContext, GraphicsContextOpts},
    profiling::{profile_function, profile_scope},
};

#[derive(Debug)]
pub struct WindowOpts {
    pub size: Option<(f32, f32)>,
    pub title: String,
    pub fullscreen: Option<Fullscreen>,
}

impl Default for WindowOpts {
    fn default() -> Self {
        Self {
            size: None,
            title: "Astrelis Window".to_string(),
            fullscreen: None,
        }
    }
}

impl WindowOpts {}

/// A user handle to a window
pub struct Window {
    pub(crate) window: Arc<WinitWindow>,
    pub(crate) context: GraphicsContext,
}

impl Window {
    pub(crate) fn new(
        event_loop: &ActiveEventLoop,
        opts: WindowOpts,
        graphics_opts: GraphicsContextOpts,
    ) -> Self {
        let mut attributes = WinitWindow::default_attributes().with_title(opts.title);
        if let Some((width, height)) = opts.size {
            attributes.inner_size = Some(LogicalSize::new(width, height).into());
        }
        attributes.fullscreen = opts.fullscreen;
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .expect("failed to create window"),
        );
        let context = GraphicsContext::new(window.clone(), graphics_opts)
            .expect("failed to create GraphicsContext");
        Self { window, context }
    }

    pub fn begin_render(&mut self) -> RenderContext {
        profile_function!();
        RenderContext::new(self)
    }

    pub fn resized(&mut self, new_size: PhysicalSize<u32>) {
        self.context.resized(new_size);
    }
}

pub struct RenderContext<'window> {
    pub(crate) window: &'window mut Window,
}

impl<'window> RenderContext<'window> {
    fn new(window: &'window mut Window) -> Self {
        window.context.begin_render();
        Self { window }
    }
}

impl Drop for RenderContext<'_> {
    fn drop(&mut self) {
        profile_scope!("GraphicsContext::end_render");
        self.window.context.end_render();
        self.window.window.request_redraw();
    }
}

impl AsRef<Window> for Window {
    fn as_ref(&self) -> &Window {
        self
    }
}

impl AsRef<Window> for &RenderContext<'_> {
    fn as_ref(&self) -> &Window {
        &self.window
    }
}
