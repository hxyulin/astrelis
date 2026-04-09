//! Mock backend with scripted event injection.

use std::collections::HashMap;

use astrelis_core::geometry::Physical;
use astrelis_core::geometry::Size;
use astrelis_window::backend::AppHandler;
use astrelis_window::builder::WindowAttributes;
use astrelis_window::control_flow::ControlFlow;
use astrelis_window::event::{DeviceEvent, WindowEvent};
use astrelis_window::lifecycle::AppLifecycle;
use astrelis_window::window_id::WindowId;

use crate::context::MockEventLoopContext;
use crate::event_queue::ScriptedEvent;
use crate::window::MockWindow;

/// The result of running a mock test, containing all observable state.
#[derive(Debug)]
pub struct RunResult {
    /// Whether `exit()` was called during the run.
    pub exit_requested: bool,
    /// The final control flow mode.
    pub control_flow: ControlFlow,
    /// All windows that existed at the end of the run, keyed by ID.
    pub windows: HashMap<WindowId, MockWindow>,
    /// Window IDs created during the run (by the handler calling `create_window`).
    pub created_window_ids: Vec<WindowId>,
    /// Window IDs destroyed during the run.
    pub destroyed_window_ids: Vec<WindowId>,
}

/// A mock windowing backend for headless testing.
///
/// Script a sequence of events, then call [`run_test`](MockBackend::run_test)
/// with an [`AppHandler`] to execute them synchronously. Inspect the returned
/// [`RunResult`] to assert on handler behavior.
///
/// # Example
///
/// ```
/// use astrelis_window::backend::AppHandler;
/// use astrelis_window::lifecycle::AppLifecycle;
/// use astrelis_window::event::WindowEvent;
/// use astrelis_window_test::MockBackend;
///
/// # struct MyApp;
/// # impl AppHandler for MyApp {
/// #     fn on_lifecycle(&mut self, _: &mut dyn astrelis_window::backend::EventLoopContext, _: AppLifecycle) {}
/// #     fn on_window_event(&mut self, ctx: &mut dyn astrelis_window::backend::EventLoopContext, _: astrelis_window::window_id::WindowId, event: WindowEvent) {
/// #         if matches!(event, WindowEvent::CloseRequested) { ctx.exit(); }
/// #     }
/// #     fn on_events_cleared(&mut self, _: &mut dyn astrelis_window::backend::EventLoopContext) {}
/// # }
/// let mut backend = MockBackend::new();
/// let id = backend.add_window(Default::default());
/// backend.push_lifecycle(AppLifecycle::Resumed);
/// backend.push_window_event(id, WindowEvent::CloseRequested);
///
/// let result = backend.run_test(&mut MyApp);
/// assert!(result.exit_requested);
/// ```
pub struct MockBackend {
    windows: HashMap<WindowId, MockWindow>,
    events: Vec<ScriptedEvent>,
    next_id: u64,
}

impl MockBackend {
    /// Creates a new mock backend with no windows and no events.
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
            events: Vec::new(),
            next_id: 1,
        }
    }

    /// Pre-creates a window that will be available when the handler runs.
    ///
    /// Returns the assigned [`WindowId`]. Use this to script events targeting
    /// a specific window.
    pub fn add_window(&mut self, attrs: WindowAttributes) -> WindowId {
        let id = WindowId::new(self.next_id);
        self.next_id += 1;

        let logical = attrs.inner_size.logical();
        let window = MockWindow::new(
            id,
            attrs.title,
            Size::<Physical>::new(logical.width, logical.height),
        );
        self.windows.insert(id, window);
        id
    }

    /// Queues a lifecycle event.
    pub fn push_lifecycle(&mut self, state: AppLifecycle) {
        self.events.push(ScriptedEvent::Lifecycle(state));
    }

    /// Queues a window event for the given window.
    pub fn push_window_event(&mut self, id: WindowId, event: WindowEvent) {
        self.events.push(ScriptedEvent::Window { id, event });
    }

    /// Queues a device event.
    pub fn push_device_event(&mut self, event: DeviceEvent) {
        self.events.push(ScriptedEvent::Device(event));
    }

    /// Queues an explicit `EventsCleared` marker.
    ///
    /// If you don't call this, one is automatically appended at the end of the
    /// event sequence.
    pub fn push_events_cleared(&mut self) {
        self.events.push(ScriptedEvent::EventsCleared);
    }

    /// Runs the scripted events against the given handler and returns the
    /// final state.
    ///
    /// Events are dispatched synchronously in order. An `EventsCleared`
    /// callback is automatically appended if the sequence doesn't end with one.
    pub fn run_test(mut self, handler: &mut dyn AppHandler) -> RunResult {
        // Ensure the sequence ends with EventsCleared.
        if !matches!(self.events.last(), Some(ScriptedEvent::EventsCleared)) {
            self.events.push(ScriptedEvent::EventsCleared);
        }

        let mut ctx = MockEventLoopContext::new(
            std::mem::take(&mut self.windows),
            self.next_id,
        );

        for event in &self.events {
            if ctx.exit_requested {
                break;
            }

            match event.clone() {
                ScriptedEvent::Lifecycle(state) => {
                    handler.on_lifecycle(&mut ctx, state);
                }
                ScriptedEvent::Window { id, event } => {
                    handler.on_window_event(&mut ctx, id, event);
                }
                ScriptedEvent::Device(event) => {
                    handler.on_device_event(&mut ctx, event);
                }
                ScriptedEvent::EventsCleared => {
                    handler.on_events_cleared(&mut ctx);
                }
            }
        }

        RunResult {
            exit_requested: ctx.exit_requested,
            control_flow: ctx.control_flow,
            windows: ctx.windows,
            created_window_ids: ctx.created_ids,
            destroyed_window_ids: ctx.destroyed_ids,
        }
    }
}

impl Default for MockBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use astrelis_window::backend::EventLoopContext;
    use astrelis_window::types::LogicalInnerSize;
    use astrelis_window::WindowBuilder;

    /// A minimal app that creates a window on resume and exits on close.
    struct TestApp;

    impl AppHandler for TestApp {
        fn on_lifecycle(&mut self, ctx: &mut dyn EventLoopContext, state: AppLifecycle) {
            if state == AppLifecycle::Resumed {
                let attrs = WindowBuilder::new()
                    .with_title("Test Window")
                    .with_inner_size(LogicalInnerSize::new(800.0, 600.0))
                    .build();
                ctx.create_window(attrs).unwrap();
                ctx.set_control_flow(ControlFlow::Poll);
            }
        }

        fn on_window_event(
            &mut self,
            ctx: &mut dyn EventLoopContext,
            _id: WindowId,
            event: WindowEvent,
        ) {
            if matches!(event, WindowEvent::CloseRequested) {
                ctx.exit();
            }
        }

        fn on_events_cleared(&mut self, _ctx: &mut dyn EventLoopContext) {}
    }

    #[test]
    fn resumed_creates_window() {
        let mut backend = MockBackend::new();
        backend.push_lifecycle(AppLifecycle::Resumed);

        let result = backend.run_test(&mut TestApp);

        assert_eq!(result.created_window_ids.len(), 1);
        assert_eq!(result.control_flow, ControlFlow::Poll);
        assert!(!result.exit_requested);
    }

    #[test]
    fn close_requested_exits() {
        let mut backend = MockBackend::new();
        let id = backend.add_window(Default::default());
        backend.push_lifecycle(AppLifecycle::Resumed);
        backend.push_window_event(id, WindowEvent::CloseRequested);

        let result = backend.run_test(&mut TestApp);

        assert!(result.exit_requested);
    }

    #[test]
    fn events_stop_after_exit() {
        use std::sync::atomic::{AtomicU32, Ordering};

        static COUNT: AtomicU32 = AtomicU32::new(0);

        struct CountingApp;
        impl AppHandler for CountingApp {
            fn on_lifecycle(&mut self, _: &mut dyn EventLoopContext, _: AppLifecycle) {}
            fn on_window_event(
                &mut self,
                ctx: &mut dyn EventLoopContext,
                _: WindowId,
                _: WindowEvent,
            ) {
                COUNT.fetch_add(1, Ordering::Relaxed);
                ctx.exit();
            }
            fn on_events_cleared(&mut self, _: &mut dyn EventLoopContext) {}
        }

        COUNT.store(0, Ordering::Relaxed);

        let mut backend = MockBackend::new();
        let id = backend.add_window(Default::default());
        // Two events, but exit is called on the first — second should not fire.
        backend.push_window_event(id, WindowEvent::CloseRequested);
        backend.push_window_event(id, WindowEvent::CloseRequested);

        let result = backend.run_test(&mut CountingApp);

        assert!(result.exit_requested);
        assert_eq!(COUNT.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn destroy_window_removes_it() {
        struct DestroyApp {
            target: Option<WindowId>,
        }

        impl AppHandler for DestroyApp {
            fn on_lifecycle(&mut self, _: &mut dyn EventLoopContext, _: AppLifecycle) {}
            fn on_window_event(
                &mut self,
                ctx: &mut dyn EventLoopContext,
                id: WindowId,
                event: WindowEvent,
            ) {
                if matches!(event, WindowEvent::CloseRequested) {
                    ctx.destroy_window(id).unwrap();
                    self.target = Some(id);
                }
            }
            fn on_events_cleared(&mut self, _: &mut dyn EventLoopContext) {}
        }

        let mut backend = MockBackend::new();
        let id = backend.add_window(Default::default());
        backend.push_window_event(id, WindowEvent::CloseRequested);

        let mut app = DestroyApp { target: None };
        let result = backend.run_test(&mut app);

        assert_eq!(app.target, Some(id));
        assert!(result.destroyed_window_ids.contains(&id));
        assert!(!result.windows.contains_key(&id));
    }

    #[test]
    fn device_events_dispatched() {
        use std::sync::atomic::{AtomicBool, Ordering};

        static RECEIVED: AtomicBool = AtomicBool::new(false);

        struct DeviceApp;
        impl AppHandler for DeviceApp {
            fn on_lifecycle(&mut self, _: &mut dyn EventLoopContext, _: AppLifecycle) {}
            fn on_window_event(
                &mut self,
                _: &mut dyn EventLoopContext,
                _: WindowId,
                _: WindowEvent,
            ) {
            }
            fn on_device_event(
                &mut self,
                _: &mut dyn EventLoopContext,
                event: DeviceEvent,
            ) {
                if matches!(event, DeviceEvent::MouseMotion { .. }) {
                    RECEIVED.store(true, Ordering::Relaxed);
                }
            }
            fn on_events_cleared(&mut self, _: &mut dyn EventLoopContext) {}
        }

        RECEIVED.store(false, Ordering::Relaxed);

        let mut backend = MockBackend::new();
        backend.push_device_event(DeviceEvent::MouseMotion {
            delta_x: 5.0,
            delta_y: -3.0,
        });

        backend.run_test(&mut DeviceApp);

        assert!(RECEIVED.load(Ordering::Relaxed));
    }
}
