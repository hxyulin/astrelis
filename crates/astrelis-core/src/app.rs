use std::marker::PhantomData;

use crate::{
    EngineCtx,
    engine::Engine,
    event::{Event, HandleStatus},
};

pub trait App {
    fn init(ctx: EngineCtx) -> Box<dyn AppHandler>;
}

pub trait AppHandler {
    fn update(&mut self, ctx: EngineCtx);
    fn on_event(&mut self, ctx: EngineCtx, event: &Event) -> HandleStatus;
}

pub fn run_app<T: App>() {
    use winit::event_loop::{ControlFlow, EventLoop};
    env_logger::init();

    let event_loop = EventLoop::new().expect("failed to create event loop");
    // TODO: Make configurable
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = AppHandlerProxy::<T>::new();
    log::debug!("starting application...");
    event_loop.run_app(&mut app).expect("failed to run app");
}

struct AppHandlerProxy<T: App> {
    app: Box<dyn AppHandler>,
    engine: Engine,
    _marker: PhantomData<T>,
}

struct NullApp;
impl AppHandler for NullApp {
    fn update(&mut self, _ctx: EngineCtx) {
        unimplemented!()
    }
    fn on_event(&mut self, _ctx: EngineCtx, _event: &Event) -> HandleStatus {
        unimplemented!()
    }
}

impl<T> AppHandlerProxy<T>
where
    T: App,
{
    fn new() -> Self {
        Self {
            engine: Engine::new(),
            app: Box::new(NullApp {}),
            _marker: PhantomData,
        }
    }
}

impl<T> winit::application::ApplicationHandler for AppHandlerProxy<T>
where
    T: App,
{
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        log::debug!("intializing app...");
        // We initialize the app during the resumed event
        let ctx = EngineCtx {
            engine: &mut self.engine,
            event_loop: event_loop,
        };
        self.app = T::init(ctx);
    }

    fn exiting(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {}

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        let ctx = EngineCtx {
            engine: &mut self.engine,
            event_loop: event_loop,
        };

        if let winit::event::WindowEvent::RedrawRequested = event {
            self.app.update(ctx);
        } else if let Some(event) = Event::from_winit(event) {
            let HandleStatus { handled, consumed } = self.app.on_event(ctx, &event);
            match event {
                Event::CloseRequested if !handled => event_loop.exit(),
                _ => {}
            }
        }
    }
}
