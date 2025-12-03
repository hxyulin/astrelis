use std::collections::HashMap;
pub use winit::error::OsError;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::{
    event::{Event, EventBatch, EventQueue, HandleStatus},
    window::{Window, WindowDescriptor},
};

pub struct AppCtx<'event_loop> {
    event_loop: &'event_loop ActiveEventLoop,
}

impl AppCtx<'_> {
    pub fn create_window(&mut self, descriptor: WindowDescriptor) -> Result<Window, OsError> {
        Window::new(self.event_loop, descriptor)
    }

    pub fn exit(&self) {
        self.event_loop.exit();
    }
}

pub trait App {
    /// Called once per frame for global logic (game state, physics, etc.)
    /// No window-specific input here
    #[allow(unused_variables)]
    fn update(&mut self, ctx: &mut AppCtx) {}

    /// Called once per window that needs rendering, with window-specific input
    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch);
}

pub type AppFactory = fn(ctx: &mut AppCtx) -> Box<dyn App>;

struct AppProxy {
    factory: AppFactory,
    events: HashMap<WindowId, EventQueue>,
    app: Option<Box<dyn App>>,
    update_called_this_frame: bool,
}

impl AppProxy {
    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId) {
        let app = self.app.as_mut().unwrap();

        // Call update() once per frame on first redraw
        if !self.update_called_this_frame {
            app.update(ctx);
            self.update_called_this_frame = true;
        }

        // Call render() for this window with its events
        let event_queue = self.events.entry(window_id).or_insert_with(EventQueue::new);
        let mut events = event_queue.drain();

        app.render(ctx, window_id, &mut events);

        // Default event handling
        events.dispatch(|event| match event {
            Event::CloseRequested => {
                tracing::info!("Close requested for window {:?}", window_id);
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

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Mark that we need to call update() on next redraw
        self.update_called_this_frame = false;
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        use winit::event::WindowEvent;

        let _app = match &mut self.app {
            Some(app) => app,
            None => return,
        };

        let mut ctx = AppCtx { event_loop };

        match event {
            WindowEvent::RedrawRequested => {
                self.render(&mut ctx, window_id);
            }
            event => {
                let event_copy = event.clone();
                let astrelis_event = match Event::from_winit(event) {
                    Some(event) => event,
                    None => {
                        tracing::trace!("Ignoring unsupported window event: {:?}", event_copy);
                        return;
                    }
                };

                let event_queue = self.events.entry(window_id).or_insert_with(EventQueue::new);
                event_queue.push(astrelis_event);
            }
        }
    }
}

/// Run the application with the given factory function.
pub fn run_app(factory: AppFactory) {
    use winit::event_loop::{ControlFlow, EventLoop};
    let event_loop = EventLoop::new().expect("failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app_proxy = AppProxy {
        factory,
        events: HashMap::new(),
        app: None,
        update_called_this_frame: false,
    };
    event_loop
        .run_app(&mut app_proxy)
        .expect("failed to run app");
}
