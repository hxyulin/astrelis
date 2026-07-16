//! Updates a window title once per second and otherwise sleeps.

use std::{convert::Infallible, time::Duration};

use astrelis_app::{App, AppContext, Runtime, RuntimeConfig};
use astrelis_platform::{Window, WindowAttributes, WindowEvent, WindowId};

#[derive(Default)]
struct Counter {
    window: Option<Window>,
    value: u64,
}

impl App for Counter {
    type Error = Infallible;

    fn resumed(&mut self, context: &mut AppContext<'_, '_, Self>) -> Result<(), Self::Error> {
        if self.window.is_some() {
            return Ok(());
        }
        let window = context
            .create_window(WindowAttributes {
                title: "Idle counter: 0".into(),
                ..Default::default()
            })
            .expect("create counter window");
        self.window = Some(window);
        context.set_interval(Duration::from_secs(1), |app, context| {
            app.value += 1;
            if let Some(window) = &app.window {
                window.set_title(format!("Idle counter: {}", app.value));
                context.invalidate_window(window.id());
            }
            Ok(())
        });
        Ok(())
    }

    fn window_event(
        &mut self,
        context: &mut AppContext<'_, '_, Self>,
        window: WindowId,
        event: WindowEvent,
    ) -> Result<(), Self::Error> {
        if matches!(event, WindowEvent::CloseRequested) {
            context.unregister_window(window);
            self.window = None;
            context.exit();
        }
        Ok(())
    }
}

fn main() -> Result<(), astrelis_app::RuntimeError<Infallible>> {
    Runtime::finish(astrelis_platform_winit::run_return(Runtime::new(
        Counter::default(),
        RuntimeConfig::default(),
    )))
    .map(|_| ())
}
