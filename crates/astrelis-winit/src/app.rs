pub use winit::error::OsError;
use winit::event_loop::ActiveEventLoop;

use crate::{event::{Event, EventBatch, EventQueue, HandleStatus}, window::{Window, WindowDescriptor}};

pub struct AppCtx<'event_loop> {
    event_loop: &'event_loop ActiveEventLoop,
}

impl AppCtx<'_> {
    pub fn create_window(&mut self, descriptor: WindowDescriptor) -> Result<Window, OsError> {
        Window::new(self.event_loop, descriptor)
    }
}

pub trait App {
    #[allow(unused_variables)]
    fn update(&mut self, ctx: &mut AppCtx, events: &mut EventBatch) {
    }
}

pub type AppFactory = fn(ctx: &mut AppCtx) -> Box<dyn App>;

struct AppProxy {
    factory: AppFactory,
    events: EventQueue,
    app: Option<Box<dyn App>>,
}

impl AppProxy {
    fn update(&mut self, ctx: &mut AppCtx) {
        let app = self.app.as_mut().unwrap();
        let mut events = self.events.drain();

        app.update(ctx, &mut events);

        events.dispatch(|event| match event {
            Event::CloseRequested => {
                ctx.event_loop.exit();
                HandleStatus::consumed()
            }
            _ => HandleStatus::ignored(),
        });
    }
}

impl winit::application::ApplicationHandler for AppProxy {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        if self.app.is_none() {
            let mut ctx = AppCtx {
                event_loop: _event_loop,
            };
            self.app = Some((self.factory)(&mut ctx));
        } 
    }

    fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            _window_id: winit::window::WindowId,
            event: winit::event::WindowEvent,
        ) {
        use winit::event::WindowEvent;

        let _app = match &mut self.app {
            Some(app) => app,
            None => return,
        };

        let mut ctx = AppCtx {
            event_loop,
        };

        match event {
            WindowEvent::RedrawRequested => self.update(&mut ctx),
            event => {
                let event = match Event::from_winit(event.clone()) {
                    Some(event) => event,
                    None => {
                        tracing::trace!("Ignoring unsupported window event: {:?}", event);
                        return;
                    }
                };
                self.events.push(event);
            }
        }
    }
}

/// Run the application with the given factory function.
pub fn run_app(factory: AppFactory) {
    use winit::event_loop::{EventLoop, ControlFlow};
    let event_loop = EventLoop::new()
        .expect("failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app_proxy = AppProxy {
        factory,
        events: EventQueue::new(),
        app: None,
    };
    event_loop.run_app(&mut app_proxy)
        .expect("failed to run app");
}
