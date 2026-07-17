//! Shared-runtime integration tests for the retained UI core.

use std::io;

use astrelis_app::{App, AppContext, Runtime, RuntimeConfig};
use astrelis_core::geometry::{Point, Size};
use astrelis_platform::{
    DeviceId, ElementState, Key, KeyLocation, KeyboardInput, NamedKey, PhysicalKey, PointerButton,
    Window, WindowAttributes, WindowEvent, WindowId,
};
use astrelis_platform_test::{ScriptEvent, TestRunner};
use astrelis_text::FontDatabase;
use astrelis_ui_core::{Button, ElementHandle, Theme, Ui, UiEventKind};

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
