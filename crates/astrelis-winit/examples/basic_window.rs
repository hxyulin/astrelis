use astrelis_winit::{app::{App, AppCtx, run_app}, event::EventBatch, window::{Window, WindowDescriptor}};

struct BasicApp {
    window: Window,
    counter: i32,
}

impl App for BasicApp {
    fn update(&mut self, _ctx: &mut AppCtx, _events: &mut EventBatch) {
        use astrelis_winit::window::WindowExt;

        self.counter += 1;
        if counter % 1000 == 0 {
            println!("Counter: {}", self.counter);
        }

        self.window.request_redraw();
    }
}

fn main() {
    run_app(|ctx| {
        let window = ctx.create_window(WindowDescriptor::default())
            .expect("Failed to create window");
        Box::new(BasicApp { window, counter: 0 })
    });
}
