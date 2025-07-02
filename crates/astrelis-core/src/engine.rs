use winit::event_loop::ActiveEventLoop;

use crate::{Window, WindowOpts, graphics::GraphicsContextOpts};

pub struct Engine {}

pub struct EngineCtx<'a> {
    pub(crate) engine: &'a mut Engine,
    pub(crate) event_loop: &'a ActiveEventLoop,
}

impl Engine {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl<'a> EngineCtx<'a> {
    pub fn create_window(&self, opts: WindowOpts, graphics_opts: GraphicsContextOpts) -> Window {
        Window::new(self.event_loop, opts, graphics_opts)
    }

    pub fn request_shutdown(&self) {
        self.event_loop.exit();
    }
}
