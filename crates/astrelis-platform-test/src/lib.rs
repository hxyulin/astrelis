//! Deterministic scripted backend for platform-independent application tests.
//!
//! A script drives the same [`astrelis_platform::Application`] callbacks as a
//! native backend, while [`TestState`] records all observable platform actions.

#![warn(missing_docs)]

use std::{
    collections::VecDeque,
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use astrelis_core::geometry::{Point, Size};
use astrelis_platform::{
    Application, Clipboard, ControlFlow, DeviceEvent, DeviceId, EventLoopClosed, EventLoopProxy,
    Monitor, PlatformContext, PlatformError, StartCause, Window, WindowAttributes,
    WindowCapabilities, WindowCommand, WindowEvent, WindowId, WindowValue, backend,
};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};

/// One scripted application callback.
#[derive(Debug)]
#[non_exhaustive]
pub enum ScriptEvent<T> {
    /// Starts an event batch.
    NewEvents(StartCause),
    /// Resumes the application.
    Resumed,
    /// Suspends the application.
    Suspended,
    /// Delivers a window event.
    Window(WindowId, WindowEvent),
    /// Delivers a device event.
    Device(DeviceId, DeviceEvent),
    /// Delivers a user event directly.
    User(T),
    /// Invokes the pre-wait callback.
    AboutToWait,
    /// Terminates the loop.
    Exit,
}

/// Kind of callback delivered by the runner, in exact order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dispatch {
    /// A new event batch.
    NewEvents,
    /// Resume callback.
    Resumed,
    /// Suspend callback.
    Suspended,
    /// Window callback.
    Window(WindowId),
    /// Device callback.
    Device(DeviceId),
    /// User callback.
    User,
    /// Pre-wait callback.
    AboutToWait,
    /// Exit callback.
    Exiting,
}

/// A snapshot-friendly record of a window.
#[derive(Clone, Debug)]
pub struct RecordedWindow {
    /// Creation attributes.
    pub attributes: WindowAttributes,
    /// Commands issued after creation.
    pub commands: Vec<WindowCommand>,
}

/// State recorded by a scripted run.
#[derive(Clone, Debug, Default)]
pub struct TestState {
    /// Windows in creation order.
    pub windows: Vec<(WindowId, RecordedWindow)>,
    /// Exact callback order.
    pub dispatches: Vec<Dispatch>,
    /// Every selected control flow.
    pub control_flows: Vec<ControlFlow>,
    /// Number of proxy sends.
    pub proxy_sends: usize,
    /// Number of exit requests.
    pub exit_requests: usize,
    /// Windows whose final test handle was dropped.
    pub destroyed_windows: Vec<WindowId>,
    /// Current deterministic text clipboard contents.
    pub clipboard_text: Option<String>,
}

struct Shared<T> {
    state: Mutex<TestState>,
    queued: Mutex<VecDeque<T>>,
    destroyed: Mutex<VecDeque<WindowId>>,
    open: AtomicBool,
    next_id: AtomicU64,
}

/// Deterministic scripted event-loop runner.
pub struct TestRunner<T> {
    script: Vec<ScriptEvent<T>>,
    shared: Arc<Shared<T>>,
    monitors: Vec<Monitor>,
}

impl<T: Send + 'static> Default for TestRunner<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Send + 'static> TestRunner<T> {
    /// Creates an empty runner.
    pub fn new() -> Self {
        Self {
            script: Vec::new(),
            shared: Arc::new(Shared {
                state: Mutex::new(TestState::default()),
                queued: Mutex::new(VecDeque::new()),
                destroyed: Mutex::new(VecDeque::new()),
                open: AtomicBool::new(true),
                next_id: AtomicU64::new(1),
            }),
            monitors: Vec::new(),
        }
    }

    /// Appends a scripted callback.
    pub fn push(&mut self, event: ScriptEvent<T>) {
        self.script.push(event);
    }

    /// Sets monitors returned by the active context.
    pub fn set_monitors(&mut self, monitors: Vec<Monitor>) {
        self.monitors = monitors;
    }

    /// Returns a proxy that can be used before or during the run.
    pub fn proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy::from_backend(Arc::new(TestProxy {
            shared: self.shared.clone(),
        }))
    }

    /// Returns a snapshot of current state.
    pub fn state(&self) -> TestState {
        self.shared
            .state
            .lock()
            .expect("test state poisoned")
            .clone()
    }

    /// Runs a script to completion.
    pub fn run<A>(mut self, mut app: A) -> Result<TestState, PlatformError>
    where
        A: Application<UserEvent = T>,
    {
        self.run_return_inner(&mut app)?;
        drop(app);
        Ok(self.state())
    }

    /// Runs a script and returns the application after loop termination.
    pub fn run_return<A>(mut self, mut app: A) -> Result<(A, TestState), PlatformError>
    where
        A: Application<UserEvent = T>,
    {
        self.run_return_inner(&mut app)?;
        let state = self.state();
        Ok((app, state))
    }

    fn run_return_inner<A>(&mut self, app: &mut A) -> Result<(), PlatformError>
    where
        A: Application<UserEvent = T>,
    {
        let mut context = TestContext {
            shared: self.shared.clone(),
            monitors: self.monitors.clone(),
            control_flow: ControlFlow::Wait,
            exited: false,
        };

        for event in self.script.drain(..) {
            if context.exited {
                break;
            }
            match event {
                ScriptEvent::NewEvents(cause) => invoke(&mut context, Dispatch::NewEvents, |c| {
                    app.new_events(c, cause)
                }),
                ScriptEvent::Resumed => invoke(&mut context, Dispatch::Resumed, |c| app.resumed(c)),
                ScriptEvent::Suspended => {
                    invoke(&mut context, Dispatch::Suspended, |c| app.suspended(c))
                }
                ScriptEvent::Window(id, event) => invoke(&mut context, Dispatch::Window(id), |c| {
                    app.window_event(c, id, event)
                }),
                ScriptEvent::Device(id, event) => invoke(&mut context, Dispatch::Device(id), |c| {
                    app.device_event(c, id, event)
                }),
                ScriptEvent::User(event) => {
                    invoke(&mut context, Dispatch::User, |c| app.user_event(c, event))
                }
                ScriptEvent::AboutToWait => invoke(&mut context, Dispatch::AboutToWait, |c| {
                    app.about_to_wait(c)
                }),
                ScriptEvent::Exit => context.exited = true,
            }
            drain_proxy(&mut context, app);
            drain_destroyed(&mut context, app);
        }
        self.shared.open.store(false, Ordering::Release);
        invoke(&mut context, Dispatch::Exiting, |c| app.exiting(c));
        Ok(())
    }
}

fn invoke<T: Send + 'static>(
    context: &mut TestContext<T>,
    dispatch: Dispatch,
    callback: impl FnOnce(&mut PlatformContext<'_, T>),
) {
    context
        .shared
        .state
        .lock()
        .expect("test state poisoned")
        .dispatches
        .push(dispatch);
    callback(&mut PlatformContext::from_backend(context));
}

fn drain_proxy<A: Application>(context: &mut TestContext<A::UserEvent>, app: &mut A) {
    loop {
        let event = context
            .shared
            .queued
            .lock()
            .expect("proxy queue poisoned")
            .pop_front();
        let Some(event) = event else { break };
        invoke(context, Dispatch::User, |c| app.user_event(c, event));
    }
}

fn drain_destroyed<A: Application>(context: &mut TestContext<A::UserEvent>, app: &mut A) {
    loop {
        let id = context
            .shared
            .destroyed
            .lock()
            .expect("destroyed queue poisoned")
            .pop_front();
        let Some(id) = id else { break };
        invoke(context, Dispatch::Window(id), |c| {
            app.window_event(c, id, WindowEvent::Destroyed);
        });
    }
}

struct TestContext<T> {
    shared: Arc<Shared<T>>,
    monitors: Vec<Monitor>,
    control_flow: ControlFlow,
    exited: bool,
}

impl<T: Send + 'static> backend::ActiveContext<T> for TestContext<T> {
    fn create_window(&mut self, attributes: WindowAttributes) -> Result<Window, PlatformError> {
        let id = WindowId(self.shared.next_id.fetch_add(1, Ordering::Relaxed));
        self.shared
            .state
            .lock()
            .expect("test state poisoned")
            .windows
            .push((
                id,
                RecordedWindow {
                    attributes,
                    commands: Vec::new(),
                },
            ));
        Ok(Window::from_backend(Arc::new(TestWindow {
            id,
            shared: self.shared.clone(),
        })))
    }

    fn set_control_flow(&mut self, control_flow: ControlFlow) {
        self.control_flow = control_flow;
        self.shared
            .state
            .lock()
            .expect("test state poisoned")
            .control_flows
            .push(control_flow);
    }

    fn control_flow(&self) -> ControlFlow {
        self.control_flow
    }
    fn available_monitors(&self) -> Vec<Monitor> {
        self.monitors.clone()
    }
    fn primary_monitor(&self) -> Option<Monitor> {
        self.monitors.first().cloned()
    }
    fn event_loop_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy::from_backend(Arc::new(TestProxy {
            shared: self.shared.clone(),
        }))
    }
    fn clipboard(&self) -> Clipboard {
        Clipboard::from_backend(Arc::new(TestClipboard {
            shared: self.shared.clone(),
        }))
    }
    fn exit(&mut self) {
        self.exited = true;
        self.shared
            .state
            .lock()
            .expect("test state poisoned")
            .exit_requests += 1;
    }
}

struct TestClipboard<T> {
    shared: Arc<Shared<T>>,
}

impl<T> fmt::Debug for TestClipboard<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TestClipboard")
            .finish_non_exhaustive()
    }
}

impl<T: Send + 'static> backend::Clipboard for TestClipboard<T> {
    fn capabilities(&self) -> astrelis_platform::ClipboardCapabilities {
        astrelis_platform::ClipboardCapabilities {
            read_text: true,
            write_text: true,
        }
    }

    fn read_text(&self) -> Result<Option<String>, PlatformError> {
        Ok(self
            .shared
            .state
            .lock()
            .expect("test state poisoned")
            .clipboard_text
            .clone())
    }

    fn write_text(&self, text: String) -> Result<(), PlatformError> {
        self.shared
            .state
            .lock()
            .expect("test state poisoned")
            .clipboard_text = Some(text);
        Ok(())
    }
}

struct TestProxy<T> {
    shared: Arc<Shared<T>>,
}

impl<T> fmt::Debug for TestProxy<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TestProxy")
    }
}

impl<T: Send + 'static> backend::EventLoopProxy<T> for TestProxy<T> {
    fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        if !self.shared.open.load(Ordering::Acquire) {
            return Err(EventLoopClosed(event));
        }
        self.shared
            .queued
            .lock()
            .expect("proxy queue poisoned")
            .push_back(event);
        self.shared
            .state
            .lock()
            .expect("test state poisoned")
            .proxy_sends += 1;
        Ok(())
    }
}

struct TestWindow<T> {
    id: WindowId,
    shared: Arc<Shared<T>>,
}

impl<T> fmt::Debug for TestWindow<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TestWindow").field(&self.id).finish()
    }
}

impl<T> Drop for TestWindow<T> {
    fn drop(&mut self) {
        self.shared
            .state
            .lock()
            .expect("test state poisoned")
            .destroyed_windows
            .push(self.id);
        if self.shared.open.load(Ordering::Acquire) {
            self.shared
                .destroyed
                .lock()
                .expect("destroyed queue poisoned")
                .push_back(self.id);
        }
    }
}

impl<T> HasWindowHandle for TestWindow<T> {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Err(HandleError::Unavailable)
    }
}

impl<T> HasDisplayHandle for TestWindow<T> {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Err(HandleError::Unavailable)
    }
}

impl<T: Send + 'static> backend::Window for TestWindow<T> {
    fn id(&self) -> WindowId {
        self.id
    }
    fn capabilities(&self) -> WindowCapabilities {
        WindowCapabilities::default()
    }
    fn command(&self, command: WindowCommand) -> Result<Option<WindowValue>, PlatformError> {
        let mut state = self.shared.state.lock().expect("test state poisoned");
        let (_, window) = state
            .windows
            .iter_mut()
            .find(|(id, _)| *id == self.id)
            .ok_or_else(|| PlatformError::new("unknown test window"))?;
        window.commands.push(command.clone());
        let value = match command {
            WindowCommand::InnerSize => Some(WindowValue::PhysicalSize(Size::new(800, 600))),
            WindowCommand::OuterPosition => Some(WindowValue::PhysicalPosition(Point::new(0, 0))),
            WindowCommand::ScaleFactor => Some(WindowValue::Float(1.0)),
            WindowCommand::IsFocused => Some(WindowValue::Bool(false)),
            WindowCommand::Theme => Some(WindowValue::Theme(None)),
            WindowCommand::CurrentMonitor => Some(WindowValue::Monitor(None)),
            WindowCommand::SetCursorGrab(_)
            | WindowCommand::SetCursorPosition(_)
            | WindowCommand::DragWindow
            | WindowCommand::DragResizeWindow(_) => {
                return Err(PlatformError::new(
                    "operation unavailable in scripted backend",
                ));
            }
            _ => None,
        };
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use astrelis_platform::{Application, WindowAttributes};

    struct App {
        window: Option<Window>,
    }

    impl Application for App {
        type UserEvent = u32;
        fn resumed(&mut self, context: &mut PlatformContext<'_, u32>) {
            self.window = Some(context.create_window(WindowAttributes::default()).unwrap());
            context.event_loop_proxy().send_event(7).unwrap();
        }
        fn user_event(&mut self, context: &mut PlatformContext<'_, u32>, event: u32) {
            assert_eq!(event, 7);
            self.window.as_ref().unwrap().request_redraw();
            context.exit();
        }
    }

    #[test]
    fn records_proxy_redraw_and_final_handle_destruction() {
        let mut runner = TestRunner::new();
        runner.push(ScriptEvent::Resumed);
        let state = runner.run(App { window: None }).unwrap();
        assert_eq!(
            state.dispatches,
            [Dispatch::Resumed, Dispatch::User, Dispatch::Exiting]
        );
        assert_eq!(state.proxy_sends, 1);
        assert_eq!(state.exit_requests, 1);
        assert!(
            state.windows[0]
                .1
                .commands
                .contains(&WindowCommand::RequestRedraw)
        );
        assert_eq!(state.destroyed_windows, [WindowId(1)]);
    }

    #[test]
    fn raw_handles_are_unavailable() {
        fn accepts_handles<T: HasWindowHandle + HasDisplayHandle>(_value: &T) {}
        let mut runner = TestRunner::<()>::new();
        runner.push(ScriptEvent::Resumed);
        struct HandleApp;
        impl Application for HandleApp {
            type UserEvent = ();
            fn resumed(&mut self, context: &mut PlatformContext<'_, ()>) {
                let window = context.create_window(WindowAttributes::default()).unwrap();
                accepts_handles(&window);
                assert!(matches!(
                    window.window_handle(),
                    Err(HandleError::Unavailable)
                ));
            }
        }
        runner.run(HandleApp).unwrap();
    }

    #[test]
    fn clipboard_is_shared_and_recorded() {
        struct ClipboardApp;
        impl Application for ClipboardApp {
            type UserEvent = ();
            fn resumed(&mut self, context: &mut PlatformContext<'_, ()>) {
                let clipboard = context.clipboard();
                assert_eq!(
                    clipboard.capabilities(),
                    astrelis_platform::ClipboardCapabilities {
                        read_text: true,
                        write_text: true,
                    }
                );
                assert_eq!(clipboard.read_text().unwrap(), None);
                clipboard.write_text("Astrelis").unwrap();
                assert_eq!(clipboard.read_text().unwrap().as_deref(), Some("Astrelis"));
            }
        }
        let mut runner = TestRunner::<()>::new();
        runner.push(ScriptEvent::Resumed);
        let state = runner.run(ClipboardApp).unwrap();
        assert_eq!(state.clipboard_text.as_deref(), Some("Astrelis"));
    }
}
