//! Prints UI-ready window and input events while sleeping when idle.

use astrelis_platform::{
    Application, PlatformContext, Window, WindowAttributes, WindowEvent, WindowId,
};

#[derive(Default)]
struct App {
    window: Option<Window>,
}

impl Application for App {
    type UserEvent = ();

    fn resumed(&mut self, context: &mut PlatformContext<'_, ()>) {
        if self.window.is_none() {
            let window = context
                .create_window(WindowAttributes {
                    title: "Astrelis event inspector".into(),
                    ..WindowAttributes::default()
                })
                .unwrap();
            window.set_ime_allowed(true);
            self.window = Some(window);
        }
    }

    fn window_event(
        &mut self,
        context: &mut PlatformContext<'_, ()>,
        id: WindowId,
        event: WindowEvent,
    ) {
        println!("{id:?}: {event:?}");
        if matches!(event, WindowEvent::CloseRequested) {
            self.window = None;
            context.exit();
        }
    }
}

fn main() -> Result<(), astrelis_platform::PlatformError> {
    astrelis_platform_winit::run(App::default())
}
