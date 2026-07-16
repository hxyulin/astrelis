//! Opens one idle window and exits when it is closed.

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
            self.window = Some(context.create_window(WindowAttributes::default()).unwrap());
        }
    }

    fn window_event(
        &mut self,
        context: &mut PlatformContext<'_, ()>,
        _id: WindowId,
        event: WindowEvent,
    ) {
        if matches!(event, WindowEvent::CloseRequested) {
            self.window = None;
            context.exit();
        }
    }
}

fn main() -> Result<(), astrelis_platform::PlatformError> {
    astrelis_platform_winit::run(App::default())
}
