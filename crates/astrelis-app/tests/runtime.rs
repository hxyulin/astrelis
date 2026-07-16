//! Deterministic scheduling tests for the shared application runtime.

use std::{
    cell::Cell,
    error::Error,
    fmt,
    rc::Rc,
    time::{Duration, Instant},
};

use astrelis_app::{
    App, AppContext, FixedStep, FixedUpdateInfo, ManualClock, Runtime, RuntimeConfig,
    RuntimePolicy, UpdateInfo,
};
use astrelis_platform::{
    ControlFlow, Window, WindowAttributes, WindowCommand, WindowEvent, WindowId,
};
use astrelis_platform_test::{ScriptEvent, TestRunner};

#[derive(Debug)]
struct TestError;

impl fmt::Display for TestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("test error")
    }
}

impl Error for TestError {}

#[derive(Default)]
struct DesktopApp {
    windows: Vec<Window>,
    updates: usize,
    redraws: Vec<WindowId>,
}

impl App for DesktopApp {
    type Error = TestError;

    fn resumed(&mut self, context: &mut AppContext<'_, '_, Self>) -> Result<(), Self::Error> {
        let first = context.create_window(WindowAttributes::default()).unwrap();
        let second = context.create_window(WindowAttributes::default()).unwrap();
        context.invalidate_window(first.id());
        context.invalidate_window(first.id());
        self.windows = vec![first, second];
        Ok(())
    }

    fn update(
        &mut self,
        _context: &mut AppContext<'_, '_, Self>,
        _info: UpdateInfo,
    ) -> Result<(), Self::Error> {
        self.updates += 1;
        Ok(())
    }

    fn redraw(
        &mut self,
        _context: &mut AppContext<'_, '_, Self>,
        window: WindowId,
    ) -> Result<(), Self::Error> {
        self.redraws.push(window);
        Ok(())
    }
}

#[test]
fn desktop_sleeps_and_coalesces_redraw_requests_per_window() {
    let mut runner = TestRunner::new();
    runner.push(ScriptEvent::Resumed);
    runner.push(ScriptEvent::AboutToWait);
    runner.push(ScriptEvent::AboutToWait);
    let runtime = Runtime::new(DesktopApp::default(), RuntimeConfig::default());
    let (runtime, state) = runner.run_return(runtime).unwrap();
    let app = runtime.into_result().unwrap();

    assert_eq!(app.updates, 1);
    assert_eq!(state.control_flows, [ControlFlow::Wait, ControlFlow::Wait]);
    for (_, window) in &state.windows {
        assert_eq!(
            window
                .commands
                .iter()
                .filter(|command| **command == WindowCommand::RequestRedraw)
                .count(),
            1
        );
    }
}

#[test]
fn redraw_events_are_delivered_even_without_runtime_invalidation() {
    let mut runner = TestRunner::new();
    runner.push(ScriptEvent::Resumed);
    runner.push(ScriptEvent::AboutToWait);
    runner.push(ScriptEvent::Window(
        WindowId(1),
        WindowEvent::RedrawRequested,
    ));
    runner.push(ScriptEvent::Window(
        WindowId(1),
        WindowEvent::RedrawRequested,
    ));
    let runtime = Runtime::new(DesktopApp::default(), RuntimeConfig::default());
    let (runtime, _) = runner.run_return(runtime).unwrap();
    let app = runtime.into_result().unwrap();
    assert_eq!(app.redraws, [WindowId(1), WindowId(1)]);
}

struct TimerApp {
    clock: ManualClock,
    window: Option<Window>,
    fires: usize,
}

impl App for TimerApp {
    type Error = TestError;

    fn resumed(&mut self, context: &mut AppContext<'_, '_, Self>) -> Result<(), Self::Error> {
        self.window = Some(context.create_window(WindowAttributes::default()).unwrap());
        context.set_interval(Duration::from_millis(10), |app, context| {
            app.fires += 1;
            context.invalidate_window(app.window.as_ref().unwrap().id());
            Ok(())
        });
        Ok(())
    }

    fn window_event(
        &mut self,
        _context: &mut AppContext<'_, '_, Self>,
        _window: WindowId,
        event: WindowEvent,
    ) -> Result<(), Self::Error> {
        if matches!(event, WindowEvent::Focused(_)) {
            self.clock.advance(Duration::from_millis(35));
        }
        Ok(())
    }
}

#[test]
fn repeating_timers_coalesce_missed_intervals_and_keep_the_original_cadence() {
    let start = Instant::now();
    let clock = ManualClock::new(start);
    let app = TimerApp {
        clock: clock.clone(),
        window: None,
        fires: 0,
    };
    let mut runner = TestRunner::new();
    runner.push(ScriptEvent::Resumed);
    runner.push(ScriptEvent::AboutToWait);
    runner.push(ScriptEvent::Window(WindowId(1), WindowEvent::Focused(true)));
    runner.push(ScriptEvent::AboutToWait);
    let runtime = Runtime::with_clock(app, RuntimeConfig::default(), clock);
    let (runtime, state) = runner.run_return(runtime).unwrap();
    let app = runtime.into_result().unwrap();

    assert_eq!(app.fires, 1);
    assert_eq!(
        state.control_flows[0],
        ControlFlow::WaitUntil(start + Duration::from_millis(10))
    );
    assert_eq!(
        state.control_flows[1],
        ControlFlow::WaitUntil(start + Duration::from_millis(40))
    );
}

struct CancelTimerApp {
    fired: bool,
}

impl App for CancelTimerApp {
    type Error = TestError;

    fn resumed(&mut self, context: &mut AppContext<'_, '_, Self>) -> Result<(), Self::Error> {
        let timer = context.set_timeout(Duration::from_secs(1), |app, _| {
            app.fired = true;
            Ok(())
        });
        assert!(context.cancel_timer(timer));
        assert!(!context.cancel_timer(timer));
        Ok(())
    }
}

#[test]
fn cancelled_timer_leaves_desktop_in_wait_mode() {
    let mut runner = TestRunner::new();
    runner.push(ScriptEvent::Resumed);
    runner.push(ScriptEvent::AboutToWait);
    let runtime = Runtime::new(CancelTimerApp { fired: false }, RuntimeConfig::default());
    let (runtime, state) = runner.run_return(runtime).unwrap();
    assert!(!runtime.into_result().unwrap().fired);
    assert_eq!(state.control_flows, [ControlFlow::Wait]);
}

struct SelfCancellingTimerApp {
    clock: ManualClock,
    fires: usize,
}

impl App for SelfCancellingTimerApp {
    type Error = TestError;

    fn resumed(&mut self, context: &mut AppContext<'_, '_, Self>) -> Result<(), Self::Error> {
        let id = Rc::new(Cell::new(None));
        let callback_id = id.clone();
        let timer = context.set_interval(Duration::from_millis(10), move |app, context| {
            app.fires += 1;
            assert!(context.cancel_timer(callback_id.get().unwrap()));
            Ok(())
        });
        id.set(Some(timer));
        Ok(())
    }

    fn window_event(
        &mut self,
        _context: &mut AppContext<'_, '_, Self>,
        _window: WindowId,
        event: WindowEvent,
    ) -> Result<(), Self::Error> {
        if matches!(event, WindowEvent::Focused(_)) {
            self.clock.advance(Duration::from_millis(10));
        }
        Ok(())
    }
}

#[test]
fn repeating_timer_can_cancel_itself_from_its_callback() {
    let clock = ManualClock::new(Instant::now());
    let app = SelfCancellingTimerApp {
        clock: clock.clone(),
        fires: 0,
    };
    let mut runner = TestRunner::new();
    runner.push(ScriptEvent::Resumed);
    runner.push(ScriptEvent::Window(
        WindowId(99),
        WindowEvent::Focused(true),
    ));
    runner.push(ScriptEvent::AboutToWait);
    runner.push(ScriptEvent::AboutToWait);
    let runtime = Runtime::with_clock(app, RuntimeConfig::default(), clock);
    let (runtime, state) = runner.run_return(runtime).unwrap();
    assert_eq!(runtime.into_result().unwrap().fires, 1);
    assert_eq!(state.control_flows, [ControlFlow::Wait, ControlFlow::Wait]);
}

struct TaskApp {
    values: Vec<u32>,
}

impl App for TaskApp {
    type Error = TestError;

    fn resumed(&mut self, context: &mut AppContext<'_, '_, Self>) -> Result<(), Self::Error> {
        let proxy = context.proxy();
        for value in 1..=3 {
            proxy
                .run_on_main_thread(move |app, _| {
                    app.values.push(value);
                    Ok(())
                })
                .unwrap();
        }
        Ok(())
    }
}

#[test]
fn tasks_are_fifo_coalesced_and_redriven_after_the_batch_limit() {
    let mut runner = TestRunner::new();
    runner.push(ScriptEvent::Resumed);
    let runtime = Runtime::new(
        TaskApp { values: Vec::new() },
        RuntimeConfig {
            task_batch_limit: 2,
            ..Default::default()
        },
    );
    let (runtime, state) = runner.run_return(runtime).unwrap();
    assert_eq!(runtime.into_result().unwrap().values, [1, 2, 3]);
    assert_eq!(state.proxy_sends, 2);
}

struct ContinuousApp {
    clock: ManualClock,
    updates: Vec<UpdateInfo>,
    fixed: Vec<FixedUpdateInfo>,
}

impl App for ContinuousApp {
    type Error = TestError;

    fn resumed(&mut self, context: &mut AppContext<'_, '_, Self>) -> Result<(), Self::Error> {
        context.create_window(WindowAttributes::default()).unwrap();
        Ok(())
    }

    fn window_event(
        &mut self,
        _context: &mut AppContext<'_, '_, Self>,
        _window: WindowId,
        event: WindowEvent,
    ) -> Result<(), Self::Error> {
        if matches!(event, WindowEvent::Focused(_)) {
            self.clock.advance(Duration::from_millis(35));
        }
        Ok(())
    }

    fn update(
        &mut self,
        _context: &mut AppContext<'_, '_, Self>,
        info: UpdateInfo,
    ) -> Result<(), Self::Error> {
        self.updates.push(info);
        Ok(())
    }

    fn fixed_update(
        &mut self,
        _context: &mut AppContext<'_, '_, Self>,
        info: FixedUpdateInfo,
    ) -> Result<(), Self::Error> {
        self.fixed.push(info);
        Ok(())
    }
}

#[test]
fn fixed_updates_catch_up_and_report_interpolation() {
    let start = Instant::now();
    let clock = ManualClock::new(start);
    let app = ContinuousApp {
        clock: clock.clone(),
        updates: Vec::new(),
        fixed: Vec::new(),
    };
    let policy = RuntimePolicy::Continuous {
        frame_interval: None,
        fixed_step: Some(FixedStep {
            step: Duration::from_millis(10),
            max_steps_per_frame: 8,
        }),
    };
    let mut runner = TestRunner::new();
    runner.push(ScriptEvent::Resumed);
    runner.push(ScriptEvent::AboutToWait);
    runner.push(ScriptEvent::Window(WindowId(1), WindowEvent::Focused(true)));
    runner.push(ScriptEvent::AboutToWait);
    let runtime = Runtime::with_clock(
        app,
        RuntimeConfig {
            policy,
            ..Default::default()
        },
        clock,
    );
    let (runtime, state) = runner.run_return(runtime).unwrap();
    let app = runtime.into_result().unwrap();

    assert_eq!(app.fixed.len(), 3);
    assert_eq!(app.updates.len(), 2);
    assert_eq!(app.updates[1].delta, Duration::from_millis(35));
    assert!((app.updates[1].interpolation - 0.5).abs() < f64::EPSILON);
    assert_eq!(state.control_flows, [ControlFlow::Poll, ControlFlow::Poll]);
}

#[test]
fn fixed_update_limit_drops_whole_excess_steps_but_keeps_the_remainder() {
    let start = Instant::now();
    let clock = ManualClock::new(start);
    let app = ContinuousApp {
        clock: clock.clone(),
        updates: Vec::new(),
        fixed: Vec::new(),
    };
    let mut runner = TestRunner::new();
    runner.push(ScriptEvent::Resumed);
    runner.push(ScriptEvent::AboutToWait);
    runner.push(ScriptEvent::Window(WindowId(1), WindowEvent::Focused(true)));
    runner.push(ScriptEvent::AboutToWait);
    let runtime = Runtime::with_clock(
        app,
        RuntimeConfig {
            policy: RuntimePolicy::Continuous {
                frame_interval: None,
                fixed_step: Some(FixedStep {
                    step: Duration::from_millis(10),
                    max_steps_per_frame: 2,
                }),
            },
            ..Default::default()
        },
        clock,
    );
    let (runtime, _) = runner.run_return(runtime).unwrap();
    let app = runtime.into_result().unwrap();
    assert_eq!(app.fixed.len(), 2);
    assert!((app.updates[1].interpolation - 0.5).abs() < f64::EPSILON);
}

#[test]
fn paced_frames_advance_from_the_target_deadline_without_drift() {
    let start = Instant::now();
    let clock = ManualClock::new(start);
    let app = ContinuousApp {
        clock: clock.clone(),
        updates: Vec::new(),
        fixed: Vec::new(),
    };
    let mut runner = TestRunner::new();
    runner.push(ScriptEvent::Resumed);
    runner.push(ScriptEvent::AboutToWait);
    runner.push(ScriptEvent::Window(WindowId(1), WindowEvent::Focused(true)));
    runner.push(ScriptEvent::AboutToWait);
    let runtime = Runtime::with_clock(
        app,
        RuntimeConfig {
            policy: RuntimePolicy::paced(Duration::from_millis(10)),
            ..Default::default()
        },
        clock,
    );
    let (_, state) = runner.run_return(runtime).unwrap();

    assert_eq!(
        state.control_flows,
        [
            ControlFlow::WaitUntil(start + Duration::from_millis(10)),
            ControlFlow::WaitUntil(start + Duration::from_millis(40)),
        ]
    );
}

struct FailingApp;

impl App for FailingApp {
    type Error = TestError;

    fn resumed(&mut self, _context: &mut AppContext<'_, '_, Self>) -> Result<(), Self::Error> {
        Err(TestError)
    }
}

#[test]
fn callback_errors_request_orderly_exit_and_are_recoverable() {
    let mut runner = TestRunner::new();
    runner.push(ScriptEvent::Resumed);
    runner.push(ScriptEvent::AboutToWait);
    let runtime = Runtime::new(FailingApp, RuntimeConfig::default());
    let (runtime, state) = runner.run_return(runtime).unwrap();
    assert!(runtime.into_result().is_err());
    assert_eq!(state.exit_requests, 1);
}

#[test]
fn occluded_continuous_windows_are_not_repeatedly_invalidated() {
    let clock = ManualClock::new(Instant::now());
    let app = ContinuousApp {
        clock: clock.clone(),
        updates: Vec::new(),
        fixed: Vec::new(),
    };
    let mut runner = TestRunner::new();
    runner.push(ScriptEvent::Resumed);
    runner.push(ScriptEvent::Window(
        WindowId(1),
        WindowEvent::Occluded(true),
    ));
    runner.push(ScriptEvent::AboutToWait);
    runner.push(ScriptEvent::AboutToWait);
    let runtime = Runtime::with_clock(
        app,
        RuntimeConfig {
            policy: RuntimePolicy::continuous(),
            ..Default::default()
        },
        clock,
    );
    let (_, state) = runner.run_return(runtime).unwrap();
    assert!(
        state.windows[0]
            .1
            .commands
            .iter()
            .all(|command| *command != WindowCommand::RequestRedraw)
    );
}
