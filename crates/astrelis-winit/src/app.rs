use std::collections::HashMap;
pub use winit::error::OsError;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::{
    event::{Event, EventBatch, EventQueue, HandleStatus},
    window::{Window, WindowDescriptor},
};

struct WindowResources {
    events: EventQueue,
    scale_factor: f64,
}

pub struct AppCtx<'a> {
    event_loop: &'a ActiveEventLoop,
    windows: &'a mut HashMap<WindowId, WindowResources>,
}

impl AppCtx<'_> {
    pub fn create_window(&mut self, descriptor: WindowDescriptor) -> Result<Window, OsError> {
        let window = Window::new(self.event_loop, descriptor)?;

        self.windows.insert(
            window.id(),
            WindowResources {
                events: EventQueue::new(),
                scale_factor: window.scale_factor(),
            },
        );

        Ok(window)
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
    app: Option<Box<dyn App>>,
    update_called_this_frame: bool,
    windows: HashMap<WindowId, WindowResources>,
}

impl winit::application::ApplicationHandler for AppProxy {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        if self.app.is_none() {
            let mut ctx = AppCtx {
                event_loop: _event_loop,
                windows: &mut self.windows,
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

        let mut ctx = AppCtx {
            event_loop,
            windows: &mut self.windows,
        };

        match event {
            WindowEvent::RedrawRequested => {
                let app = self.app.as_mut().unwrap();

                // Call update() once per frame on first redraw
                if !self.update_called_this_frame {
                    app.update(&mut ctx);
                    self.update_called_this_frame = true;
                }

                // Call render() for this window with its events
                let window = ctx.windows.get_mut(&window_id).unwrap();
                let mut events = window.events.drain();

                app.render(&mut ctx, window_id, &mut events);

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
            event => {
                let window = self.windows.get_mut(&window_id).unwrap();
                if let WindowEvent::ScaleFactorChanged { scale_factor, .. } = event {
                    window.scale_factor = scale_factor;
                }
                let astrelis_event = match Event::from_winit(event, window.scale_factor) {
                    Some(event) => event,
                    None => return,
                };

                window.events.push(astrelis_event);
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
        app: None,
        update_called_this_frame: false,
        windows: HashMap::new(),
    };
    event_loop
        .run_app(&mut app_proxy)
        .expect("failed to run app");
}
