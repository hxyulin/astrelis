//! Shared scheduling runtime for idle desktop and continuously updating applications.

#![warn(missing_docs)]

use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    error::Error,
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use astrelis_platform::{
    Application, Clipboard, ControlFlow, DeviceEvent, DeviceId, EventLoopClosed, EventLoopProxy,
    Instant, PlatformContext, PlatformError, StartCause, Window, WindowAttributes, WindowEvent,
    WindowId,
};

const DEFAULT_TASK_BATCH_LIMIT: usize = 1_024;
const DEFAULT_MAX_FIXED_STEPS: u32 = 8;

/// A monotonic source of runtime time.
pub trait Clock: fmt::Debug + Send + Sync + 'static {
    /// Returns the current monotonic instant.
    fn now(&self) -> Instant;
}

/// Clock backed by [`Instant::now`].
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// A manually advanced monotonic clock for deterministic tests.
#[derive(Clone, Debug)]
pub struct ManualClock {
    now: Arc<Mutex<Instant>>,
}

impl ManualClock {
    /// Creates a manual clock at the supplied instant.
    pub fn new(now: Instant) -> Self {
        Self {
            now: Arc::new(Mutex::new(now)),
        }
    }

    /// Advances the clock by a duration.
    pub fn advance(&self, duration: Duration) {
        let mut now = self.now.lock().expect("manual clock poisoned");
        *now += duration;
    }

    /// Sets the clock to an instant that is not earlier than its current value.
    pub fn set(&self, instant: Instant) {
        let mut now = self.now.lock().expect("manual clock poisoned");
        assert!(instant >= *now, "a monotonic clock cannot move backwards");
        *now = instant;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Instant {
        *self.now.lock().expect("manual clock poisoned")
    }
}

/// Configuration for fixed-step simulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedStep {
    /// Duration represented by one simulation step.
    pub step: Duration,
    /// Maximum steps executed during one frame.
    pub max_steps_per_frame: u32,
}

impl FixedStep {
    /// Creates fixed-step configuration with the default catch-up limit.
    pub fn new(step: Duration) -> Self {
        Self {
            step,
            max_steps_per_frame: DEFAULT_MAX_FIXED_STEPS,
        }
    }
}

/// Policy controlling when application updates occur.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RuntimePolicy {
    /// Update only in response to work and sleep while idle.
    #[default]
    Desktop,
    /// Update continuously, optionally paced and with fixed simulation steps.
    Continuous {
        /// Target time between frames. `None` selects polling.
        frame_interval: Option<Duration>,
        /// Optional fixed-step simulation configuration.
        fixed_step: Option<FixedStep>,
    },
}

impl RuntimePolicy {
    /// Creates an unpaced continuously updating policy.
    pub const fn continuous() -> Self {
        Self::Continuous {
            frame_interval: None,
            fixed_step: None,
        }
    }

    /// Creates a continuously updating policy with a target frame interval.
    pub const fn paced(frame_interval: Duration) -> Self {
        Self::Continuous {
            frame_interval: Some(frame_interval),
            fixed_step: None,
        }
    }
}

/// Configuration for an application runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeConfig {
    /// Initial scheduling policy.
    pub policy: RuntimePolicy,
    /// Maximum queued tasks executed for one wake event.
    pub task_batch_limit: usize,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            policy: RuntimePolicy::Desktop,
            task_batch_limit: DEFAULT_TASK_BATCH_LIMIT,
        }
    }
}

/// Timing information for one variable update.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UpdateInfo {
    /// Time since the previous variable update.
    pub delta: Duration,
    /// Time since the runtime was first resumed.
    pub elapsed: Duration,
    /// Fractional fixed-step accumulator in the range `0.0..1.0`.
    pub interpolation: f64,
}

/// Timing information for one fixed update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedUpdateInfo {
    /// Duration represented by this step.
    pub step: Duration,
    /// Simulated time after this step completes.
    pub elapsed: Duration,
}

/// Stable identifier for a scheduled timer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TimerId(u64);

/// Error returned after a runtime-driven application terminates.
#[derive(Debug)]
pub enum RuntimeError<E> {
    /// The platform event loop failed.
    Platform(PlatformError),
    /// An application callback failed.
    Application(E),
}

impl<E: fmt::Display> fmt::Display for RuntimeError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Platform(error) => write!(formatter, "platform error: {error}"),
            Self::Application(error) => write!(formatter, "application error: {error}"),
        }
    }
}

impl<E: Error + 'static> Error for RuntimeError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Platform(error) => Some(error),
            Self::Application(error) => Some(error),
        }
    }
}

/// Application callbacks driven by [`Runtime`].
pub trait App: Sized + 'static {
    /// Error produced by application callbacks.
    type Error: Error + Send + Sync + 'static;

    /// Called when native resources may be created.
    fn resumed(&mut self, _context: &mut AppContext<'_, '_, Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Called when native resources should be considered unavailable.
    fn suspended(&mut self, _context: &mut AppContext<'_, '_, Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Delivers an event for a registered or platform-known window.
    fn window_event(
        &mut self,
        _context: &mut AppContext<'_, '_, Self>,
        _window: WindowId,
        _event: WindowEvent,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Delivers a raw device event.
    fn device_event(
        &mut self,
        _context: &mut AppContext<'_, '_, Self>,
        _device: DeviceId,
        _event: DeviceEvent,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Runs one variable-rate application update.
    fn update(
        &mut self,
        _context: &mut AppContext<'_, '_, Self>,
        _info: UpdateInfo,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Runs one fixed simulation update.
    fn fixed_update(
        &mut self,
        _context: &mut AppContext<'_, '_, Self>,
        _info: FixedUpdateInfo,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Renders a window in response to a platform redraw event.
    fn redraw(
        &mut self,
        _context: &mut AppContext<'_, '_, Self>,
        _window: WindowId,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Called when the event loop is terminating.
    fn exiting(&mut self, _context: &mut AppContext<'_, '_, Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}

type Task<A> =
    Box<dyn FnOnce(&mut A, &mut AppContext<'_, '_, A>) -> Result<(), <A as App>::Error> + Send>;
type TimerCallback<A> =
    Box<dyn FnMut(&mut A, &mut AppContext<'_, '_, A>) -> Result<(), <A as App>::Error>>;

struct Shared<A: App> {
    tasks: Mutex<VecDeque<Task<A>>>,
    wake_pending: AtomicBool,
}

/// Thread-safe handle for scheduling work on the event-loop thread.
pub struct RuntimeProxy<A: App> {
    shared: Arc<Shared<A>>,
    platform: EventLoopProxy<RuntimeEvent<A>>,
}

impl<A: App> Clone for RuntimeProxy<A> {
    fn clone(&self) -> Self {
        Self {
            shared: self.shared.clone(),
            platform: self.platform.clone(),
        }
    }
}

impl<A: App> fmt::Debug for RuntimeProxy<A> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeProxy")
            .finish_non_exhaustive()
    }
}

impl<A: App> RuntimeProxy<A> {
    /// Queues work for exclusive execution on the event-loop thread.
    pub fn run_on_main_thread(
        &self,
        task: impl FnOnce(&mut A, &mut AppContext<'_, '_, A>) -> Result<(), A::Error> + Send + 'static,
    ) -> Result<(), EventLoopClosed<()>> {
        self.shared
            .tasks
            .lock()
            .expect("runtime task queue poisoned")
            .push_back(Box::new(task));
        self.request_wake()
    }

    fn request_wake(&self) -> Result<(), EventLoopClosed<()>> {
        if self.shared.wake_pending.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        self.platform
            .send_event(RuntimeEvent::Wake)
            .map_err(|error| {
                self.shared.wake_pending.store(false, Ordering::Release);
                match error.0 {
                    RuntimeEvent::Wake => (),
                    RuntimeEvent::_Marker(_) => (),
                }
                EventLoopClosed(())
            })
    }
}

/// Runtime-owned user event used by platform backends.
#[doc(hidden)]
pub enum RuntimeEvent<A: App> {
    /// Wakes the runtime to drain queued work.
    Wake,
    #[doc(hidden)]
    _Marker(std::marker::PhantomData<fn() -> A>),
}

impl<A: App> fmt::Debug for RuntimeEvent<A> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RuntimeEvent::Wake")
    }
}

struct WindowState {
    window: Window,
    dirty: bool,
    redraw_pending: bool,
    occluded: bool,
}

enum TimerKind {
    Once,
    Repeating(Duration),
}

struct Timer<A: App> {
    id: TimerId,
    kind: TimerKind,
    callback: TimerCallback<A>,
}

struct State<A: App> {
    clock: Arc<dyn Clock>,
    shared: Arc<Shared<A>>,
    policy: RuntimePolicy,
    task_batch_limit: usize,
    windows: HashMap<WindowId, WindowState>,
    timers: BTreeMap<(Instant, TimerId), Timer<A>>,
    timer_deadlines: HashMap<TimerId, Instant>,
    next_timer_id: u64,
    active_timer: Option<TimerId>,
    cancel_active_timer: bool,
    suspended: bool,
    work_pending: bool,
    started_at: Option<Instant>,
    last_update: Option<Instant>,
    last_frame: Option<Instant>,
    next_frame: Option<Instant>,
    fixed_accumulator: Duration,
    fixed_elapsed: Duration,
    application_error: Option<A::Error>,
}

impl<A: App> State<A> {
    fn new(clock: Arc<dyn Clock>, config: RuntimeConfig, shared: Arc<Shared<A>>) -> Self {
        Self {
            clock,
            shared,
            policy: config.policy,
            task_batch_limit: config.task_batch_limit.max(1),
            windows: HashMap::new(),
            timers: BTreeMap::new(),
            timer_deadlines: HashMap::new(),
            next_timer_id: 1,
            active_timer: None,
            cancel_active_timer: false,
            suspended: true,
            work_pending: false,
            started_at: None,
            last_update: None,
            last_frame: None,
            next_frame: None,
            fixed_accumulator: Duration::ZERO,
            fixed_elapsed: Duration::ZERO,
            application_error: None,
        }
    }
}

/// Operations available to application callbacks.
pub struct AppContext<'a, 'platform, A: App> {
    platform: &'a mut PlatformContext<'platform, RuntimeEvent<A>>,
    state: &'a mut State<A>,
}

impl<A: App> AppContext<'_, '_, A> {
    /// Creates and registers a native window.
    pub fn create_window(&mut self, attributes: WindowAttributes) -> Result<Window, PlatformError> {
        let window = self.platform.create_window(attributes)?;
        self.register_window(window.clone());
        Ok(window)
    }

    /// Registers a runtime-owned clone of a window.
    pub fn register_window(&mut self, window: Window) -> bool {
        let id = window.id();
        let inserted = self
            .state
            .windows
            .insert(
                id,
                WindowState {
                    window,
                    dirty: true,
                    redraw_pending: false,
                    occluded: false,
                },
            )
            .is_none();
        self.state.work_pending = true;
        inserted
    }

    /// Unregisters and releases the runtime-owned window clone.
    pub fn unregister_window(&mut self, window: WindowId) -> Option<Window> {
        self.state.windows.remove(&window).map(|entry| entry.window)
    }

    /// Marks one registered window as needing redraw.
    pub fn invalidate_window(&mut self, window: WindowId) -> bool {
        let Some(entry) = self.state.windows.get_mut(&window) else {
            return false;
        };
        entry.dirty = true;
        self.state.work_pending = true;
        true
    }

    /// Marks every registered window as needing redraw.
    pub fn invalidate_all(&mut self) {
        for entry in self.state.windows.values_mut() {
            entry.dirty = true;
        }
        if !self.state.windows.is_empty() {
            self.state.work_pending = true;
        }
    }

    /// Returns a cross-thread task and wakeup proxy.
    pub fn proxy(&self) -> RuntimeProxy<A> {
        RuntimeProxy {
            shared: self.state.shared.clone(),
            platform: self.platform.event_loop_proxy(),
        }
    }

    /// Returns a cloneable text clipboard handle.
    pub fn clipboard(&self) -> Clipboard {
        self.platform.clipboard()
    }

    /// Schedules a one-shot timer.
    pub fn set_timeout(
        &mut self,
        delay: Duration,
        callback: impl FnMut(&mut A, &mut AppContext<'_, '_, A>) -> Result<(), A::Error> + 'static,
    ) -> TimerId {
        self.insert_timer(delay, TimerKind::Once, Box::new(callback))
    }

    /// Schedules a repeating timer.
    ///
    /// Missed intervals are coalesced into one callback per event-loop turn.
    pub fn set_interval(
        &mut self,
        interval: Duration,
        callback: impl FnMut(&mut A, &mut AppContext<'_, '_, A>) -> Result<(), A::Error> + 'static,
    ) -> TimerId {
        assert!(!interval.is_zero(), "timer interval must be non-zero");
        self.insert_timer(interval, TimerKind::Repeating(interval), Box::new(callback))
    }

    fn insert_timer(
        &mut self,
        delay: Duration,
        kind: TimerKind,
        callback: TimerCallback<A>,
    ) -> TimerId {
        let id = TimerId(self.state.next_timer_id);
        self.state.next_timer_id += 1;
        let deadline = self.state.clock.now() + delay;
        self.state.timer_deadlines.insert(id, deadline);
        self.state
            .timers
            .insert((deadline, id), Timer { id, kind, callback });
        id
    }

    /// Cancels a timer, returning whether it was still scheduled.
    pub fn cancel_timer(&mut self, id: TimerId) -> bool {
        if self.state.active_timer == Some(id) {
            self.state.cancel_active_timer = true;
            return true;
        }
        let Some(deadline) = self.state.timer_deadlines.remove(&id) else {
            return false;
        };
        self.state.timers.remove(&(deadline, id)).is_some()
    }

    /// Returns the current monotonic runtime time.
    pub fn now(&self) -> Instant {
        self.state.clock.now()
    }

    /// Returns the current scheduling policy.
    pub fn policy(&self) -> RuntimePolicy {
        self.state.policy
    }

    /// Changes the scheduling policy.
    pub fn set_policy(&mut self, policy: RuntimePolicy) {
        validate_policy(policy);
        self.state.policy = policy;
        let now = self.state.clock.now();
        self.state.last_frame = Some(now);
        self.state.next_frame = initial_frame_deadline(policy, now);
        self.state.fixed_accumulator = Duration::ZERO;
        self.state.fixed_elapsed = Duration::ZERO;
        self.state.work_pending = true;
    }

    /// Requests orderly application termination.
    pub fn exit(&mut self) {
        self.platform.exit();
    }
}

/// Adapter that drives an [`App`] over Astrelis platform callbacks.
pub struct Runtime<A: App> {
    app: A,
    state: State<A>,
}

impl<A: App> Runtime<A> {
    /// Creates a runtime using the system clock.
    pub fn new(app: A, config: RuntimeConfig) -> Self {
        Self::with_clock(app, config, SystemClock)
    }

    /// Creates a runtime using an injected monotonic clock.
    pub fn with_clock(app: A, config: RuntimeConfig, clock: impl Clock) -> Self {
        validate_policy(config.policy);
        let shared = Arc::new(Shared {
            tasks: Mutex::new(VecDeque::new()),
            wake_pending: AtomicBool::new(false),
        });
        Self {
            app,
            state: State::new(Arc::new(clock), config, shared),
        }
    }

    /// Returns the application when no callback failed.
    pub fn into_result(self) -> Result<A, A::Error> {
        match self.state.application_error {
            Some(error) => Err(error),
            None => Ok(self.app),
        }
    }

    /// Combines a platform runner result with the runtime callback result.
    pub fn finish(result: Result<Self, PlatformError>) -> Result<A, RuntimeError<A::Error>> {
        let runtime = result.map_err(RuntimeError::Platform)?;
        runtime.into_result().map_err(RuntimeError::Application)
    }

    fn call(
        &mut self,
        platform: &mut PlatformContext<'_, RuntimeEvent<A>>,
        callback: impl FnOnce(&mut A, &mut AppContext<'_, '_, A>) -> Result<(), A::Error>,
    ) {
        if self.state.application_error.is_some() {
            return;
        }
        let result = {
            let app = &mut self.app;
            let state = &mut self.state;
            let mut context = AppContext { platform, state };
            callback(app, &mut context)
        };
        if let Err(error) = result {
            self.state.application_error = Some(error);
            platform.exit();
        }
    }

    fn process_tasks(&mut self, platform: &mut PlatformContext<'_, RuntimeEvent<A>>) {
        astrelis_profiling::profile_scope!("app.tasks");
        self.state
            .shared
            .wake_pending
            .store(false, Ordering::Release);
        let limit = self.state.task_batch_limit;
        for _ in 0..limit {
            let task = self
                .state
                .shared
                .tasks
                .lock()
                .expect("runtime task queue poisoned")
                .pop_front();
            let Some(task) = task else { break };
            self.state.work_pending = true;
            self.call(platform, task);
            if self.state.application_error.is_some() {
                return;
            }
        }
        let has_more = !self
            .state
            .shared
            .tasks
            .lock()
            .expect("runtime task queue poisoned")
            .is_empty();
        if has_more {
            let proxy = RuntimeProxy {
                shared: self.state.shared.clone(),
                platform: platform.event_loop_proxy(),
            };
            let _ = proxy.request_wake();
        }
    }

    fn process_timers(
        &mut self,
        platform: &mut PlatformContext<'_, RuntimeEvent<A>>,
        now: Instant,
    ) {
        astrelis_profiling::profile_scope!("app.timers");
        let due: Vec<_> = self
            .state
            .timers
            .range(..=(now, TimerId(u64::MAX)))
            .map(|(key, _)| *key)
            .collect();
        for key in due {
            let Some(mut timer) = self.state.timers.remove(&key) else {
                continue;
            };
            self.state.timer_deadlines.remove(&timer.id);
            self.state.active_timer = Some(timer.id);
            self.state.cancel_active_timer = false;
            self.state.work_pending = true;
            self.call(platform, |app, context| (timer.callback)(app, context));
            self.state.active_timer = None;
            let cancelled = std::mem::take(&mut self.state.cancel_active_timer);
            if self.state.application_error.is_some() {
                return;
            }
            if !cancelled && let TimerKind::Repeating(interval) = timer.kind {
                let mut deadline = key.0 + interval;
                while deadline <= now {
                    deadline += interval;
                }
                self.state.timer_deadlines.insert(timer.id, deadline);
                self.state.timers.insert((deadline, timer.id), timer);
            }
        }
    }

    fn run_updates(&mut self, platform: &mut PlatformContext<'_, RuntimeEvent<A>>, now: Instant) {
        astrelis_profiling::profile_scope!("app.update");
        if self.state.suspended || self.state.application_error.is_some() {
            return;
        }
        let continuous_due = match self.state.policy {
            RuntimePolicy::Desktop => false,
            RuntimePolicy::Continuous {
                frame_interval: None,
                ..
            } => true,
            RuntimePolicy::Continuous {
                frame_interval: Some(_),
                ..
            } => self.state.next_frame.is_none_or(|deadline| now >= deadline),
        };
        let should_update = match self.state.policy {
            RuntimePolicy::Desktop => self.state.work_pending,
            RuntimePolicy::Continuous { .. } => continuous_due,
        };
        if !should_update {
            return;
        }

        let previous = match self.state.policy {
            RuntimePolicy::Desktop => self.state.last_update,
            RuntimePolicy::Continuous { .. } => self.state.last_frame,
        };
        let delta = previous.map_or(Duration::ZERO, |previous| {
            now.saturating_duration_since(previous)
        });
        let started_at = *self.state.started_at.get_or_insert(now);
        let elapsed = now.saturating_duration_since(started_at);

        let mut interpolation = 0.0;
        if let RuntimePolicy::Continuous {
            fixed_step: Some(fixed),
            ..
        } = self.state.policy
        {
            assert!(!fixed.step.is_zero(), "fixed step must be non-zero");
            self.state.fixed_accumulator += delta;
            let mut steps = 0;
            while self.state.fixed_accumulator >= fixed.step
                && steps < fixed.max_steps_per_frame.max(1)
            {
                self.state.fixed_accumulator -= fixed.step;
                self.state.fixed_elapsed += fixed.step;
                let info = FixedUpdateInfo {
                    step: fixed.step,
                    elapsed: self.state.fixed_elapsed,
                };
                self.call(platform, |app, context| app.fixed_update(context, info));
                if self.state.application_error.is_some() {
                    return;
                }
                steps += 1;
            }
            while self.state.fixed_accumulator >= fixed.step {
                self.state.fixed_accumulator -= fixed.step;
            }
            interpolation = self.state.fixed_accumulator.as_secs_f64() / fixed.step.as_secs_f64();
        }

        self.call(platform, |app, context| {
            app.update(
                context,
                UpdateInfo {
                    delta,
                    elapsed,
                    interpolation,
                },
            )
        });
        if self.state.application_error.is_some() {
            return;
        }

        self.state.last_update = Some(now);
        if matches!(self.state.policy, RuntimePolicy::Continuous { .. }) {
            self.state.last_frame = Some(now);
            for entry in self.state.windows.values_mut() {
                if !entry.occluded {
                    entry.dirty = true;
                }
            }
            advance_frame_deadline(&mut self.state.next_frame, self.state.policy, now);
        }
        self.state.work_pending = false;
    }

    fn request_redraws(&mut self) {
        astrelis_profiling::profile_scope!("app.invalidate");
        if self.state.suspended {
            return;
        }
        for entry in self.state.windows.values_mut() {
            if entry.dirty && !entry.redraw_pending && !entry.occluded {
                entry.window.request_redraw();
                entry.redraw_pending = true;
            }
        }
    }

    fn select_control_flow(&mut self, platform: &mut PlatformContext<'_, RuntimeEvent<A>>) {
        if self.state.application_error.is_some() {
            return;
        }
        let timer = self.state.timers.first_key_value().map(|(key, _)| key.0);
        let frame = if self.state.suspended {
            None
        } else {
            match self.state.policy {
                RuntimePolicy::Desktop => None,
                RuntimePolicy::Continuous {
                    frame_interval: None,
                    ..
                } => {
                    platform.set_control_flow(ControlFlow::Poll);
                    return;
                }
                RuntimePolicy::Continuous {
                    frame_interval: Some(_),
                    ..
                } => self.state.next_frame,
            }
        };
        match timer.into_iter().chain(frame).min() {
            Some(deadline) => platform.set_control_flow(ControlFlow::WaitUntil(deadline)),
            None => platform.set_control_flow(ControlFlow::Wait),
        }
    }
}

impl<A: App> Application for Runtime<A> {
    type UserEvent = RuntimeEvent<A>;

    fn new_events(
        &mut self,
        _context: &mut PlatformContext<'_, Self::UserEvent>,
        _cause: StartCause,
    ) {
    }

    fn resumed(&mut self, platform: &mut PlatformContext<'_, Self::UserEvent>) {
        let now = self.state.clock.now();
        self.state.suspended = false;
        self.state.work_pending = true;
        self.state.started_at.get_or_insert(now);
        self.state.last_update = Some(now);
        self.state.last_frame = Some(now);
        self.state.next_frame = initial_frame_deadline(self.state.policy, now);
        self.state.fixed_accumulator = Duration::ZERO;
        self.call(platform, |app, context| app.resumed(context));
    }

    fn suspended(&mut self, platform: &mut PlatformContext<'_, Self::UserEvent>) {
        self.state.suspended = true;
        self.state.next_frame = None;
        self.call(platform, |app, context| app.suspended(context));
    }

    fn window_event(
        &mut self,
        platform: &mut PlatformContext<'_, Self::UserEvent>,
        window: WindowId,
        event: WindowEvent,
    ) {
        match &event {
            WindowEvent::RedrawRequested => {
                astrelis_profiling::profile_scope!("app.redraw");
                if let Some(entry) = self.state.windows.get_mut(&window) {
                    entry.redraw_pending = false;
                    entry.dirty = false;
                }
                self.call(platform, |app, context| app.redraw(context, window));
                return;
            }
            WindowEvent::Destroyed => {
                self.state.windows.remove(&window);
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(entry) = self.state.windows.get_mut(&window) {
                    entry.dirty = true;
                }
            }
            WindowEvent::Occluded(occluded) => {
                if let Some(entry) = self.state.windows.get_mut(&window) {
                    entry.occluded = *occluded;
                    if *occluded {
                        entry.redraw_pending = false;
                    }
                    if !occluded {
                        entry.dirty = true;
                    }
                }
            }
            _ => {}
        }
        self.state.work_pending = true;
        self.call(platform, |app, context| {
            app.window_event(context, window, event)
        });
    }

    fn device_event(
        &mut self,
        platform: &mut PlatformContext<'_, Self::UserEvent>,
        device: DeviceId,
        event: DeviceEvent,
    ) {
        self.state.work_pending = true;
        self.call(platform, |app, context| {
            app.device_event(context, device, event)
        });
    }

    fn user_event(
        &mut self,
        platform: &mut PlatformContext<'_, Self::UserEvent>,
        _event: Self::UserEvent,
    ) {
        self.process_tasks(platform);
    }

    fn about_to_wait(&mut self, platform: &mut PlatformContext<'_, Self::UserEvent>) {
        let now = self.state.clock.now();
        self.process_timers(platform, now);
        self.run_updates(platform, now);
        self.request_redraws();
        self.select_control_flow(platform);
    }

    fn exiting(&mut self, platform: &mut PlatformContext<'_, Self::UserEvent>) {
        self.call(platform, |app, context| app.exiting(context));
    }
}

fn initial_frame_deadline(policy: RuntimePolicy, now: Instant) -> Option<Instant> {
    match policy {
        RuntimePolicy::Continuous {
            frame_interval: Some(_),
            ..
        } => Some(now),
        _ => None,
    }
}

fn validate_policy(policy: RuntimePolicy) {
    if let RuntimePolicy::Continuous {
        frame_interval,
        fixed_step,
    } = policy
    {
        assert!(
            frame_interval.is_none_or(|interval| !interval.is_zero()),
            "frame interval must be non-zero"
        );
        if let Some(fixed) = fixed_step {
            assert!(!fixed.step.is_zero(), "fixed step must be non-zero");
            assert!(
                fixed.max_steps_per_frame > 0,
                "fixed-step catch-up limit must be non-zero"
            );
        }
    }
}

fn advance_frame_deadline(deadline: &mut Option<Instant>, policy: RuntimePolicy, now: Instant) {
    let RuntimePolicy::Continuous {
        frame_interval: Some(interval),
        ..
    } = policy
    else {
        *deadline = None;
        return;
    };
    assert!(!interval.is_zero(), "frame interval must be non-zero");
    let mut next = deadline.unwrap_or(now) + interval;
    while next <= now {
        next += interval;
    }
    *deadline = Some(next);
}
