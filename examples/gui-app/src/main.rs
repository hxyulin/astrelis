use astrelis_framework::{
    App, AppHandler, EngineCtx, Window, WindowOpts,
    event::{Event, HandleStatus},
    run_app,
};

fn main() {
    run_app::<GuiApp>();
}

struct GuiApp {
    window: Window,
}

impl App for GuiApp {
    fn init(ctx: EngineCtx) -> Box<dyn AppHandler> {
        let window = ctx.create_window(WindowOpts::default());
        Box::new(Self { window })
    }
}

impl GuiApp {
    pub fn shutdown(&mut self, ctx: EngineCtx) {
        log::info!("shutting down GUI Application");
        // Save user changes, and request to shutdown
        ctx.request_shutdown();
    }
}

impl AppHandler for GuiApp {
    fn on_event(&mut self, ctx: EngineCtx, event: &Event) -> HandleStatus {
        log::info!("received event from engine: {:?}", event);
        match event {
            Event::CloseRequested => self.shutdown(ctx),
            _ => {}
        }
        HandleStatus::ignored()
    }

    fn update(&mut self, ctx: EngineCtx) {
        self.window.request_redraw();
    }
}
