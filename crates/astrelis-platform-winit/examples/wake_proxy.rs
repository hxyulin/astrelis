//! Wakes a waiting event loop with a typed event from a worker thread.

use std::{thread, time::Duration};

use astrelis_platform::{Application, PlatformContext, Window, WindowAttributes};

#[derive(Default)]
struct App {
    window: Option<Window>,
}

impl Application for App {
    type UserEvent = &'static str;

    fn resumed(&mut self, context: &mut PlatformContext<'_, Self::UserEvent>) {
        if self.window.is_none() {
            self.window = Some(context.create_window(WindowAttributes::default()).unwrap());
            let proxy = context.event_loop_proxy();
            thread::spawn(move || {
                thread::sleep(Duration::from_secs(1));
                let _ = proxy.send_event("worker finished");
            });
        }
    }

    fn user_event(
        &mut self,
        context: &mut PlatformContext<'_, Self::UserEvent>,
        event: Self::UserEvent,
    ) {
        println!("{event}");
        context.exit();
    }
}

fn main() -> Result<(), astrelis_platform::PlatformError> {
    astrelis_platform_winit::run(App::default())
}
