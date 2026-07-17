//! Shared-runtime integration tests for the retained UI core.

use std::any::Any;
use std::io;

use astrelis_app::{App, AppContext, Runtime, RuntimeConfig};
use astrelis_core::geometry::{LogicalSize, Point, Size};
use astrelis_platform::{
    DeviceId, ElementState, Key, KeyLocation, KeyboardInput, NamedKey, PhysicalKey, PointerButton,
    Window, WindowAttributes, WindowEvent, WindowId,
};
use astrelis_platform_test::{ScriptEvent, TestRunner};
use astrelis_text::FontDatabase;
use astrelis_ui_core::{
    Button, ElementHandle, EventContext, RoutedEvent, RoutedEventKind, Theme, Ui, UiEventKind,
    Widget,
};

struct TestUiApp {
    window: Option<Window>,
    ui: Ui,
    button: ElementHandle<Button>,
    redraws: usize,
    activations: usize,
}

impl TestUiApp {
    fn new() -> Self {
        let mut ui = Ui::new(FontDatabase::default(), Theme::default());
        let root = ui.root();
        let button = ui.add_button(root, "Save").unwrap();
        ui.set_viewport(Size::new(800.0, 600.0), 1.0);
        Self {
            window: None,
            ui,
            button,
            redraws: 0,
            activations: 0,
        }
    }
}

impl App for TestUiApp {
    type Error = io::Error;

    fn resumed(&mut self, context: &mut AppContext<'_, '_, Self>) -> Result<(), Self::Error> {
        self.window = Some(
            context
                .create_window(WindowAttributes::default())
                .map_err(io::Error::other)?,
        );
        Ok(())
    }

    fn window_event(
        &mut self,
        context: &mut AppContext<'_, '_, Self>,
        id: WindowId,
        event: WindowEvent,
    ) -> Result<(), Self::Error> {
        let window = self.window.as_ref().expect("window");
        let update = self
            .ui
            .handle_window_event(window, &context.clipboard(), &event)
            .map_err(io::Error::other)?;
        if update.redraw {
            context.invalidate_window(id);
        }
        for event in self.ui.drain_events() {
            if event.is_from(self.button) && event.kind == UiEventKind::ButtonActivated {
                self.activations += 1;
            }
        }
        Ok(())
    }

    fn redraw(
        &mut self,
        _context: &mut AppContext<'_, '_, Self>,
        _window: WindowId,
    ) -> Result<(), Self::Error> {
        self.ui.display_list().map_err(io::Error::other)?;
        self.redraws += 1;
        Ok(())
    }
}

/// Mimics the shape of `astrelis-ui-docking`'s `DockTab`: it takes over the
/// press so the built-in control defaults do not run, and relies on receiving
/// the matching release to report a click.
#[derive(Default)]
struct PressTakeoverWidget {
    pressed: Option<DeviceId>,
    releases: usize,
}

impl<Message: 'static> Widget<Message> for PressTakeoverWidget {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn intrinsic_size(&self, _theme: &Theme) -> LogicalSize {
        Size::new(120.0, 40.0)
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn event(&mut self, context: &mut EventContext<'_, Message>, event: &RoutedEvent) {
        match &event.kind {
            RoutedEventKind::PointerButton {
                device_id,
                button: PointerButton::Primary,
                state: ElementState::Pressed,
                ..
            } => {
                self.pressed = Some(*device_id);
                context.prevent_default();
            }
            RoutedEventKind::PointerButton {
                device_id,
                button: PointerButton::Primary,
                state: ElementState::Released,
                ..
            } if self.pressed == Some(*device_id) => {
                self.releases += 1;
                self.pressed = None;
            }
            _ => {}
        }
    }
}

struct TakeoverApp {
    window: Option<Window>,
    ui: Ui,
    widget: ElementHandle<PressTakeoverWidget>,
}

impl TakeoverApp {
    fn new() -> Self {
        let mut ui = Ui::new(FontDatabase::default(), Theme::default());
        let root = ui.root();
        let widget = ui.add_widget(root, PressTakeoverWidget::default()).unwrap();
        ui.set_viewport(Size::new(800.0, 600.0), 1.0);
        Self {
            window: None,
            ui,
            widget,
        }
    }

    fn releases(&self) -> usize {
        self.ui.widget(self.widget).unwrap().releases
    }
}

impl App for TakeoverApp {
    type Error = io::Error;

    fn resumed(&mut self, context: &mut AppContext<'_, '_, Self>) -> Result<(), Self::Error> {
        self.window = Some(
            context
                .create_window(WindowAttributes::default())
                .map_err(io::Error::other)?,
        );
        Ok(())
    }

    fn window_event(
        &mut self,
        context: &mut AppContext<'_, '_, Self>,
        _id: WindowId,
        event: WindowEvent,
    ) -> Result<(), Self::Error> {
        let window = self.window.as_ref().expect("window");
        self.ui
            .handle_window_event(window, &context.clipboard(), &event)
            .map_err(io::Error::other)?;
        Ok(())
    }

    fn redraw(
        &mut self,
        _context: &mut AppContext<'_, '_, Self>,
        _window: WindowId,
    ) -> Result<(), Self::Error> {
        self.ui.display_list().map_err(io::Error::other)?;
        Ok(())
    }
}

/// A widget that calls `prevent_default()` on press must still receive the
/// matching release.
///
/// Regression test: the press path used to return early whenever a listener
/// prevented the default, which skipped `capture.insert` entirely. Without the
/// capture there was nothing to deliver the release to, so docking tabs
/// activated on Space/Enter but never on click.
#[test]
fn preventing_the_press_default_still_delivers_the_release() {
    let mut runner = TestRunner::new();
    runner.push(ScriptEvent::Resumed);
    runner.push(ScriptEvent::Window(
        WindowId(1),
        WindowEvent::PointerMoved {
            device_id: DeviceId(2),
            position: Point::new(20.0, 10.0),
        },
    ));
    runner.push(ScriptEvent::Window(
        WindowId(1),
        WindowEvent::PointerButton {
            device_id: DeviceId(2),
            button: PointerButton::Primary,
            state: ElementState::Pressed,
        },
    ));
    runner.push(ScriptEvent::Window(
        WindowId(1),
        WindowEvent::PointerButton {
            device_id: DeviceId(2),
            button: PointerButton::Primary,
            state: ElementState::Released,
        },
    ));
    runner.push(ScriptEvent::Exit);

    let (runtime, _state) = runner
        .run_return(Runtime::new(TakeoverApp::new(), RuntimeConfig::default()))
        .unwrap();
    let app = runtime.into_result().unwrap();
    assert_eq!(
        app.releases(),
        1,
        "widget that prevented the press default never received its release"
    );
}

fn tab_event() -> WindowEvent {
    WindowEvent::KeyboardInput(KeyboardInput {
        device_id: DeviceId(1),
        physical_key: PhysicalKey::Unidentified,
        logical_key: Key::Named(NamedKey::Tab),
        text: None,
        location: KeyLocation::Standard,
        state: ElementState::Pressed,
        repeat: false,
        synthetic: false,
    })
}

#[test]
fn ui_invalidates_once_then_desktop_runtime_returns_to_wait() {
    let mut runner = TestRunner::new();
    runner.push(ScriptEvent::Resumed);
    runner.push(ScriptEvent::Window(
        WindowId(1),
        WindowEvent::RedrawRequested,
    ));
    runner.push(ScriptEvent::AboutToWait);
    runner.push(ScriptEvent::Window(WindowId(1), tab_event()));
    runner.push(ScriptEvent::Window(
        WindowId(1),
        WindowEvent::PointerMoved {
            device_id: DeviceId(2),
            position: Point::new(5.0, 5.0),
        },
    ));
    runner.push(ScriptEvent::Window(
        WindowId(1),
        WindowEvent::PointerButton {
            device_id: DeviceId(2),
            button: PointerButton::Primary,
            state: ElementState::Pressed,
        },
    ));
    runner.push(ScriptEvent::Window(
        WindowId(1),
        WindowEvent::PointerButton {
            device_id: DeviceId(2),
            button: PointerButton::Primary,
            state: ElementState::Released,
        },
    ));
    runner.push(ScriptEvent::AboutToWait);
    runner.push(ScriptEvent::Window(
        WindowId(1),
        WindowEvent::RedrawRequested,
    ));
    runner.push(ScriptEvent::AboutToWait);
    runner.push(ScriptEvent::Exit);

    let (runtime, state) = runner
        .run_return(Runtime::new(TestUiApp::new(), RuntimeConfig::default()))
        .unwrap();
    let app = runtime.into_result().unwrap();
    assert_eq!(app.redraws, 2);
    assert_eq!(app.activations, 1);
    assert_eq!(
        state.control_flows.last(),
        Some(&astrelis_platform::ControlFlow::Wait)
    );
    let redraw_requests = state.windows[0]
        .1
        .commands
        .iter()
        .filter(|command| matches!(command, astrelis_platform::WindowCommand::RequestRedraw))
        .count();
    assert_eq!(redraw_requests, 1);
}
