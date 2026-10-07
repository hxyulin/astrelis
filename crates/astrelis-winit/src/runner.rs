use crate::{
    Callback, CloseResponse, Handler, PrepareAction, RedrawMode, RunError, SurfaceSettings,
    UnknownWindow, WindowContext, WindowError, scheduler::Schedule,
};
use astrelis::{FrameError, GraphicsContext};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    window::{WindowAttributes, WindowId},
};

/// Native scheduling/recovery policy, validated before run starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunnerOptions {
    /// Exit when the last registered window closes, after at least one existed.
    pub exit_on_last_window: bool,
    /// Delay between retryable acquisitions; must be positive and representable.
    pub retry_delay: Duration,
    /// Allowed consecutive surface recreations before a successful presentation.
    /// Must be nonzero; device loss remains application policy.
    pub maximum_consecutive_surface_recoveries: u32,
}
impl Default for RunnerOptions {
    fn default() -> Self {
        Self {
            exit_on_last_window: true,
            retry_delay: Duration::from_millis(16),
            maximum_consecutive_surface_recoveries: 3,
        }
    }
}
impl RunnerOptions {
    /// Checks that retry pacing and recovery limits cannot cause an immediate loop.
    pub fn validate(&self) -> Result<(), RunError> {
        if self.retry_delay.is_zero()
            || Instant::now().checked_add(self.retry_delay).is_none()
            || self.maximum_consecutive_surface_recoveries == 0
        {
            Err(RunError::InvalidOptions)
        } else {
            Ok(())
        }
    }
}

struct ManagedWindow {
    context: WindowContext,
    schedule: Schedule,
    closing: bool,
    was_eligible: bool,
}
enum Operation {
    Created(WindowId),
    Close(WindowId),
}
struct State {
    windows: HashMap<WindowId, ManagedWindow>,
    operations: VecDeque<Operation>,
    graphics: Option<GraphicsContext>,
    active: bool,
    had_window: bool,
    options: RunnerOptions,
}
impl State {
    fn register(&mut self, context: WindowContext) -> Result<WindowId, WindowError> {
        let id = context.window().id();
        if self.windows.contains_key(&id) {
            return Err(WindowError::AlreadyManaged(id));
        }
        self.windows.insert(
            id,
            ManagedWindow {
                was_eligible: context.can_present(),
                context,
                schedule: Schedule::default(),
                closing: false,
            },
        );
        self.had_window = true;
        self.operations.push_back(Operation::Created(id));
        Ok(id)
    }
    fn get_mut(&mut self, id: WindowId) -> Result<&mut ManagedWindow, UnknownWindow> {
        self.windows
            .get_mut(&id)
            .filter(|w| !w.closing)
            .ok_or(UnknownWindow(id))
    }
}

/// Mutable application operations valid between recording callbacks.
///
/// Window creation and close notifications are deferred until the current
/// callback returns. No operation re-enters the handler. Queued closes immediately
/// disappear from lookups and cannot acquire another frame.
pub struct AppContext<'a, Message: 'static> {
    event_loop: &'a ActiveEventLoop,
    proxy: &'a EventLoopProxy<Message>,
    state: &'a mut State,
}
impl<Message: 'static> AppContext<'_, Message> {
    /// Creates a hidden native window, initializes graphics, then queues its hook.
    /// The first window synchronously waits for one-time GPU initialization on this
    /// desktop convenience path. Later windows share that device. No per-frame GPU
    /// completion wait is introduced. Use async WindowContext::new and adoption for
    /// caller-managed initialization.
    pub fn create_window(
        &mut self,
        mut attributes: WindowAttributes,
        settings: SurfaceSettings,
    ) -> Result<WindowId, WindowError> {
        if !self.state.active || self.event_loop.exiting() {
            return Err(WindowError::Inactive);
        }
        let visible = attributes.visible;
        attributes.visible = false;
        let window = Arc::new(self.event_loop.create_window(attributes)?);
        let mut context = if let Some(graphics) = &self.state.graphics {
            WindowContext::with_graphics(graphics, window, settings)?
        } else {
            let context = pollster::block_on(WindowContext::new(window, settings))?;
            self.state.graphics = Some(context.graphics().clone());
            context
        };
        context.defer_show = true;
        context.set_visible(visible);
        self.state.register(context)
    }
    /// Registers a caller-created context, preserving its native visibility/device.
    /// The caller is responsible for native adapters that require pre-show setup.
    pub fn adopt_window(&mut self, mut window: WindowContext) -> Result<WindowId, WindowError> {
        if !self.state.active || self.event_loop.exiting() {
            return Err(WindowError::Inactive);
        }
        if self.state.windows.contains_key(&window.window().id()) {
            return Err(WindowError::AlreadyManaged(window.window().id()));
        }
        window.resume()?;
        let graphics = window.graphics().clone();
        let id = self.state.register(window)?;
        if self.state.graphics.is_none() {
            self.state.graphics = Some(graphics);
        }
        Ok(id)
    }
    /// Looks up a registered window not queued for close.
    pub fn window(&self, id: WindowId) -> Option<&WindowContext> {
        self.state
            .windows
            .get(&id)
            .filter(|w| !w.closing)
            .map(|w| &w.context)
    }
    /// Changes managed settings/visibility/lifecycle between frames.
    pub fn window_mut(&mut self, id: WindowId) -> Option<&mut WindowContext> {
        self.state
            .windows
            .get_mut(&id)
            .filter(|w| !w.closing)
            .map(|w| &mut w.context)
    }
    /// Invalidates this window, also probing acquisition-level suspension.
    pub fn request_redraw(&mut self, id: WindowId) -> Result<(), UnknownWindow> {
        self.state.get_mut(id)?.schedule.invalidate();
        Ok(())
    }
    /// Schedules one redraw hint; earliest pending deadline wins.
    /// An immediate redraw does not cancel it. This is not a general timer service.
    pub fn request_redraw_at(&mut self, id: WindowId, at: Instant) -> Result<(), UnknownWindow> {
        self.state.get_mut(id)?.schedule.at(at);
        Ok(())
    }
    /// Removes this window's pending one-shot redraw deadline.
    pub fn cancel_redraw_at(&mut self, id: WindowId) -> Result<(), UnknownWindow> {
        self.state.get_mut(id)?.schedule.cancel_deadline();
        Ok(())
    }
    /// Selects per-window on-demand or continuous presentation.
    pub fn set_redraw_mode(&mut self, id: WindowId, mode: RedrawMode) -> Result<(), UnknownWindow> {
        self.state.get_mut(id)?.schedule.mode(mode);
        Ok(())
    }
    /// Queues an already-decided close; bypasses close_requested.
    /// Externally retained `Arc<Window>` handles may keep a native window alive.
    pub fn close_window(&mut self, id: WindowId) -> Result<(), UnknownWindow> {
        self.state.get_mut(id)?.closing = true;
        self.state.operations.push_back(Operation::Close(id));
        Ok(())
    }
    /// Requests orderly loop shutdown, including close and exiting hooks.
    pub fn exit(&mut self) {
        self.event_loop.exit();
    }
    /// Native loop access for platform-specific integrations.
    pub fn event_loop(&self) -> &ActiveEventLoop {
        self.event_loop
    }
    /// Original typed native proxy, compatible with external native adapters.
    pub fn proxy(&self) -> EventLoopProxy<Message> {
        self.proxy.clone()
    }
}

/// Optional native desktop event-loop driver over WindowContext.
///
/// Message is the loop's real user-event type. The handler is borrowed, and no
/// renderer, UI tree, render pass, or executor is supplied by this driver.
/// Default OnDemand windows sleep while idle. Platforms supported by run are
/// macOS, Windows, and Linux; low-level WindowContext is available separately.
pub struct Runner<Message: 'static = ()> {
    event_loop: EventLoop<Message>,
    graphics: Option<GraphicsContext>,
    options: RunnerOptions,
}
impl<Message: 'static> Runner<Message> {
    /// Builds a native loop with this exact user-event type.
    pub fn new() -> Result<Self, winit::error::EventLoopError> {
        EventLoop::<Message>::with_user_event()
            .build()
            .map(Self::from_event_loop)
    }
    /// Preserves caller-selected platform builder settings and native event type.
    pub fn from_event_loop(event_loop: EventLoop<Message>) -> Self {
        Self {
            event_loop,
            graphics: None,
            options: RunnerOptions::default(),
        }
    }
    /// Selects the device for subsequently created managed windows.
    #[must_use]
    pub fn with_graphics(mut self, graphics: GraphicsContext) -> Self {
        self.graphics = Some(graphics);
        self
    }
    /// Selects scheduling/recovery policy; invalid values are rejected by run.
    #[must_use]
    pub fn with_options(mut self, options: RunnerOptions) -> Self {
        self.options = options;
        self
    }
    /// Native typed proxy; usable before running, including by worker threads.
    pub fn proxy(&self) -> EventLoopProxy<Message> {
        self.event_loop.create_proxy()
    }
    /// Borrows the handler until native shutdown and returns the first failure.
    /// A render error abandons its frame. Shutdown errors do not replace an earlier
    /// failure, and a submitted-hook error cannot undo a successful submission.
    pub fn run<H: Handler<Message = Message>>(self, handler: &mut H) -> Result<(), RunError> {
        self.options.validate()?;
        let mut driver = Driver {
            handler,
            proxy: self.event_loop.create_proxy(),
            state: State {
                windows: HashMap::new(),
                operations: VecDeque::new(),
                graphics: self.graphics,
                active: false,
                had_window: false,
                options: self.options,
            },
            failure: None,
            shut_down: false,
        };
        let result = self.event_loop.run_app(&mut driver);
        if let Some(error) = driver.failure {
            Err(error)
        } else {
            result.map_err(RunError::EventLoop)
        }
    }
}

struct Driver<'a, H: Handler> {
    handler: &'a mut H,
    proxy: EventLoopProxy<H::Message>,
    state: State,
    failure: Option<RunError>,
    shut_down: bool,
}
impl<H: Handler> Driver<'_, H> {
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: RunError) {
        if self.failure.is_none() {
            self.failure = Some(error);
        }
        event_loop.exit();
    }
    fn call<T>(
        &mut self,
        event_loop: &ActiveEventLoop,
        callback: Callback,
        window: Option<WindowId>,
        f: impl FnOnce(&mut H, &mut AppContext<'_, H::Message>) -> Result<T, H::Error>,
    ) -> Option<T> {
        let mut cx = AppContext {
            event_loop,
            proxy: &self.proxy,
            state: &mut self.state,
        };
        match f(self.handler, &mut cx) {
            Ok(value) => Some(value),
            Err(source) => {
                self.fail(
                    event_loop,
                    RunError::Handler {
                        callback,
                        window,
                        source: source.into(),
                    },
                );
                None
            }
        }
    }
    fn drain(&mut self, event_loop: &ActiveEventLoop) {
        while self.failure.is_none() && !event_loop.exiting() {
            let Some(operation) = self.state.operations.pop_front() else {
                break;
            };
            match operation {
                Operation::Created(id) => {
                    if self.state.windows.get(&id).is_some_and(|w| !w.closing) {
                        self.call(event_loop, Callback::WindowCreated, Some(id), |h, cx| {
                            h.window_created(cx, id)
                        });
                        if self.failure.is_none()
                            && !event_loop.exiting()
                            && let Some(w) = self.state.windows.get_mut(&id).filter(|w| !w.closing)
                        {
                            w.context.finish_creation();
                        }
                    }
                }
                Operation::Close(id) => {
                    if self.state.windows.remove(&id).is_some() {
                        self.call(event_loop, Callback::WindowClosed, Some(id), |h, cx| {
                            h.window_closed(cx, id)
                        });
                    }
                }
            }
        }
        if self.state.options.exit_on_last_window
            && self.state.had_window
            && self.state.windows.is_empty()
        {
            event_loop.exit();
        }
    }
    fn ready(&self, id: WindowId, event_loop: &ActiveEventLoop) -> bool {
        self.failure.is_none()
            && !event_loop.exiting()
            && self.state.active
            && self
                .state
                .windows
                .get(&id)
                .is_some_and(|w| !w.closing && w.context.can_present())
    }
    fn redraw(&mut self, event_loop: &ActiveEventLoop, id: WindowId) {
        if !self.ready(id, event_loop) {
            if let Some(w) = self.state.windows.get_mut(&id) {
                w.schedule.unavailable();
            }
            return;
        }
        let action = {
            profiling::scope!("astrelis_winit::prepare");
            self.call(event_loop, Callback::Prepare, Some(id), |h, cx| {
                h.prepare(cx, id)
            })
        };
        self.drain(event_loop);
        if action != Some(PrepareAction::Render) {
            return;
        }
        if !self.ready(id, event_loop) {
            if let Some(w) = self.state.windows.get_mut(&id) {
                w.schedule.unavailable();
            }
            return;
        }
        // These disjoint field borrows avoid cloning Arc/window-format metadata.
        let w = self.state.windows.get_mut(&id).expect("ready window");
        let acquired = w.context.info_and_frame();
        let outcome = match acquired {
            Ok((info, mut frame)) => {
                let rendered = {
                    profiling::scope!("astrelis_winit::render");
                    self.handler.render(info, &mut frame)
                };
                match rendered {
                    Ok(()) => {
                        info.window().pre_present_notify();
                        frame.finish().map_err(|source| match source {
                            // The surface was never cleared: a contract violation by
                            // render, not a GPU submission failure. Name the callback.
                            astrelis::Error::UninitializedFrame => RunError::Handler {
                                callback: Callback::Render,
                                window: Some(id),
                                source: source.into(),
                            },
                            source => RunError::Submission { window: id, source },
                        })
                    }
                    Err(source) => Err(RunError::Handler {
                        callback: Callback::Render,
                        window: Some(id),
                        source: source.into(),
                    }),
                }
            }
            Err(source) => Err(RunError::Acquisition { window: id, source }),
        };
        match outcome {
            Ok(submission) => {
                profiling::finish_frame!();
                self.state
                    .windows
                    .get_mut(&id)
                    .expect("recorded window")
                    .schedule
                    .submitted();
                self.call(event_loop, Callback::Submitted, Some(id), |h, cx| {
                    h.submitted(cx, id, submission)
                });
                self.drain(event_loop);
            }
            Err(RunError::Acquisition { source, .. }) => {
                let w = self.state.windows.get_mut(&id).expect("acquisition window");
                match source {
                    FrameError::Retry => w
                        .schedule
                        .retry(Instant::now(), self.state.options.retry_delay),
                    FrameError::Suspended => w.schedule.suspended(),
                    FrameError::SurfaceLost => {
                        if !w
                            .schedule
                            .lost(self.state.options.maximum_consecutive_surface_recoveries)
                        {
                            self.fail(event_loop, RunError::RecoveryLimit { window: id });
                        } else if let Err(source) = w.context.recreate() {
                            self.fail(event_loop, RunError::Window { window: id, source });
                        } else {
                            // Recreation's invalidation must not bypass retry pacing.
                            w.context.invalidated = false;
                            w.context.availability_invalidated = false;
                            w.schedule
                                .retry(Instant::now(), self.state.options.retry_delay);
                        }
                    }
                    // Validation and future terminal outcomes stop the runner.
                    _ => self.fail(event_loop, RunError::Acquisition { window: id, source }),
                }
            }
            Err(error) => self.fail(event_loop, error),
        }
    }
}
impl<H: Handler> ApplicationHandler<H::Message> for Driver<'_, H> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.failure.is_some() || event_loop.exiting() {
            return;
        }
        self.state.active = true;
        for (id, w) in &mut self.state.windows {
            if let Err(source) = w.context.resume() {
                let error = RunError::Window {
                    window: *id,
                    source,
                };
                self.fail(event_loop, error);
                return;
            }
            w.schedule.availability_changed();
        }
        self.call(event_loop, Callback::Resumed, None, |h, cx| h.resumed(cx));
        self.drain(event_loop);
    }
    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        if self.failure.is_some() || event_loop.exiting() {
            return;
        }
        self.state.active = false;
        for w in self.state.windows.values_mut() {
            w.context.suspend();
            w.schedule.availability_changed();
        }
        self.call(event_loop, Callback::Suspended, None, |h, cx| {
            h.suspended(cx)
        });
        self.drain(event_loop);
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self.failure.is_some() || event_loop.exiting() {
            return;
        }
        let redraw = matches!(event, WindowEvent::RedrawRequested);
        let close = matches!(event, WindowEvent::CloseRequested);
        let destroyed = matches!(event, WindowEvent::Destroyed);
        let removed = if destroyed {
            self.state.windows.remove(&id).is_some()
        } else {
            false
        };
        if let Some(w) = self.state.windows.get_mut(&id).filter(|w| !w.closing) {
            if redraw {
                w.schedule.begin_redraw();
            }
            if let Err(source) = w.context.process_event(&event) {
                self.fail(event_loop, RunError::Window { window: id, source });
                return;
            }
        }
        self.call(event_loop, Callback::WindowEvent, Some(id), |h, cx| {
            h.window_event(cx, id, event)
        });
        // A forced destruction notification is delivered even if raw-event handling fails.
        if removed {
            self.call(event_loop, Callback::WindowClosed, Some(id), |h, cx| {
                h.window_closed(cx, id)
            });
        }
        self.drain(event_loop);
        if self.failure.is_some() || event_loop.exiting() {
            return;
        }
        if close && self.state.windows.get(&id).is_some_and(|w| !w.closing) {
            let decision = self.call(event_loop, Callback::CloseRequested, Some(id), |h, cx| {
                h.close_requested(cx, id)
            });
            if decision == Some(CloseResponse::Close)
                && self.state.windows.get(&id).is_some_and(|w| !w.closing)
            {
                self.state
                    .windows
                    .get_mut(&id)
                    .expect("close window")
                    .closing = true;
                self.state.operations.push_back(Operation::Close(id));
            }
            self.drain(event_loop);
        }
        if redraw {
            self.redraw(event_loop, id);
        }
    }
    fn user_event(&mut self, event_loop: &ActiveEventLoop, message: H::Message) {
        if self.failure.is_some() || event_loop.exiting() {
            return;
        }
        self.call(event_loop, Callback::UserEvent, None, |h, cx| {
            h.user_event(cx, message)
        });
        self.drain(event_loop);
    }
    fn device_event(&mut self, event_loop: &ActiveEventLoop, id: DeviceId, event: DeviceEvent) {
        if self.failure.is_some() || event_loop.exiting() {
            return;
        }
        self.call(event_loop, Callback::DeviceEvent, None, |h, cx| {
            h.device_event(cx, id, event)
        });
        self.drain(event_loop);
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.failure.is_some() || event_loop.exiting() {
            return;
        }
        self.drain(event_loop);
        if event_loop.exiting() {
            return;
        }
        let now = Instant::now();
        let mut wake = None;
        for w in self.state.windows.values_mut().filter(|w| !w.closing) {
            let eligible = self.state.active && w.context.can_present();
            if std::mem::take(&mut w.context.availability_invalidated) || w.was_eligible != eligible
            {
                w.was_eligible = eligible;
                w.schedule.availability_changed();
            }
            if std::mem::take(&mut w.context.invalidated) {
                w.schedule.invalidate();
            }
            w.schedule.advance(now);
            if w.schedule.request_native(now, eligible) {
                w.context.window().request_redraw();
            }
            if let Some(at) = w.schedule.wake(eligible) {
                wake = Some(wake.map_or(at, |old: Instant| old.min(at)));
            }
        }
        event_loop.set_control_flow(wake.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }
    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        if self.shut_down {
            return;
        }
        self.shut_down = true;
        self.state.active = false;
        self.state.operations.clear();
        // Remove before notification, including windows queued during a failed hook.
        while let Some(id) = self.state.windows.keys().next().copied() {
            self.state.windows.remove(&id);
            self.call(event_loop, Callback::WindowClosed, Some(id), |h, cx| {
                h.window_closed(cx, id)
            });
        }
        self.call(event_loop, Callback::Exiting, None, |h, cx| h.exiting(cx));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reject_unpaced_retries_and_unbounded_recovery_configuration() {
        assert!(RunnerOptions::default().validate().is_ok());
        assert!(
            RunnerOptions {
                retry_delay: Duration::ZERO,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            RunnerOptions {
                retry_delay: Duration::MAX,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            RunnerOptions {
                maximum_consecutive_surface_recoveries: 0,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
}
