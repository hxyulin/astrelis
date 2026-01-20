use std::collections::HashMap;
pub use winit::error::OsError;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::{
    event::{Event, EventBatch, EventQueue, HandleStatus},
    time::{FrameTime, TimeTracker},
    window::{Window, WindowDescriptor},
};

// Re-export FrameTime for convenience
pub use crate::time::FrameTime as Time;

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
                scale_factor: window.scale_factor_f64(),
            },
        );

        Ok(window)
    }

    pub fn exit(&self) {
        self.event_loop.exit();
    }
}

pub trait App {
    /// Called once when the app starts, before the first update.
    ///
    /// Use this for initialization that requires the event loop to be running.
    #[allow(unused_variables)]
    fn on_start(&mut self, ctx: &mut AppCtx) {}

    /// Called at the beginning of each frame, before update().
    ///
    /// Use this for pre-frame setup like profiling markers.
    #[allow(unused_variables)]
    fn begin_frame(&mut self, ctx: &mut AppCtx, time: &FrameTime) {}

    /// Called once per frame for global logic (game state, physics, etc.)
    ///
    /// This is the main game logic update. Frame-independent movement should
    /// use `time.delta_seconds()` for consistent behavior across frame rates.
    #[allow(unused_variables)]
    fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {}

    /// Called at a fixed rate for physics simulation.
    ///
    /// Unlike `update()`, this is called zero or more times per frame to maintain
    /// a consistent physics timestep. The `fixed_dt` parameter is the fixed timestep
    /// duration in seconds.
    ///
    /// Note: The fixed timestep is controlled by the app, not the framework.
    /// Apps that need fixed timestep should track their own accumulator.
    #[allow(unused_variables)]
    fn fixed_update(&mut self, ctx: &mut AppCtx, fixed_dt: f32) {}

    /// Called at the end of each frame, after all rendering.
    ///
    /// Use this for post-frame cleanup or telemetry.
    #[allow(unused_variables)]
    fn end_frame(&mut self, ctx: &mut AppCtx, time: &FrameTime) {}

    /// Called once per window that needs rendering, with window-specific input.
    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch);

    /// Called when the app is about to exit.
    ///
    /// Use this for cleanup tasks like saving state, flushing logs, etc.
    #[allow(unused_variables)]
    fn on_exit(&mut self, ctx: &mut AppCtx) {}
}

pub type AppFactory = fn(ctx: &mut AppCtx) -> Box<dyn App>;

struct AppProxy {
    factory: AppFactory,
    app: Option<Box<dyn App>>,
    update_called_this_frame: bool,
    windows: HashMap<WindowId, WindowResources>,
    time_tracker: TimeTracker,
    started: bool,
    current_frame_time: Option<FrameTime>,
}

impl winit::application::ApplicationHandler for AppProxy {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        if self.app.is_none() {
            let mut ctx = AppCtx {
                event_loop: _event_loop,
                windows: &mut self.windows,
            };
            let mut app = (self.factory)(&mut ctx);

            // Call on_start lifecycle hook
            app.on_start(&mut ctx);
            self.started = true;

            self.app = Some(app);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Call end_frame after all events have been processed
        if let (Some(app), Some(frame_time)) = (&mut self.app, &self.current_frame_time) {
            let mut ctx = AppCtx {
                event_loop,
                windows: &mut self.windows,
            };
            app.end_frame(&mut ctx, frame_time);
        }

        // Mark that we need to call update() on next redraw
        self.update_called_this_frame = false;
        self.current_frame_time = None;
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
                    // Update time tracker
                    let frame_time = self.time_tracker.tick();

                    // Call lifecycle hooks in order
                    app.begin_frame(&mut ctx, &frame_time);
                    app.update(&mut ctx, &frame_time);
                    // Note: fixed_update is called by the app itself if needed

                    // Save frame time for end_frame call in about_to_wait
                    self.current_frame_time = Some(frame_time);
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

                        // Call on_exit before exiting
                        if let Some(app) = self.app.as_mut() {
                            app.on_exit(&mut ctx);
                        }

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
        time_tracker: TimeTracker::new(),
        started: false,
        current_frame_time: None,
    };
    event_loop
        .run_app(&mut app_proxy)
        .expect("failed to run app");
}
