//! Opens two windows that can be closed independently.

use astrelis_platform::{
    Application, PlatformContext, Window, WindowAttributes, WindowEvent, WindowId,
};

#[derive(Default)]
struct App {
    windows: Vec<Window>,
}

impl Application for App {
    type UserEvent = ();

    fn resumed(&mut self, context: &mut PlatformContext<'_, ()>) {
        if self.windows.is_empty() {
            for title in ["First window", "Second window"] {
                self.windows.push(
                    context
                        .create_window(WindowAttributes {
                            title: title.into(),
                            ..WindowAttributes::default()
                        })
                        .unwrap(),
                );
            }
        }
    }

    fn window_event(
        &mut self,
        context: &mut PlatformContext<'_, ()>,
        id: WindowId,
        event: WindowEvent,
    ) {
        if matches!(event, WindowEvent::CloseRequested) {
            self.windows.retain(|window| window.id() != id);
            if self.windows.is_empty() {
                context.exit();
            }
        }
    }
}

fn main() -> Result<(), astrelis_platform::PlatformError> {
    astrelis_platform_winit::run(App::default())
}
