use astrelis_winit::{
    WindowId,
    app::{App, AppCtx, run_app},
    event::EventBatch,
    window::{Window, WindowDescriptor},
};

struct BasicApp {
    window: Window,
    window_id: WindowId,
    counter: i32,
}

impl App for BasicApp {
    fn update(&mut self, _ctx: &mut AppCtx) {
        // Global logic - called once per frame
        self.counter += 1;
        if self.counter % 1000 == 0 {
            println!("Counter: {}", self.counter);
        }
    }

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, _events: &mut EventBatch) {
        use astrelis_winit::window::WindowExt;

        if window_id == self.window_id {
            // Request next frame
            self.window.request_redraw();
        }
    }
}

fn main() {
    run_app(|ctx| {
        let window = ctx
            .create_window(WindowDescriptor::default())
            .expect("Failed to create window");
        let window_id = window.id();
        Box::new(BasicApp {
            window,
            window_id,
            counter: 0,
        })
    });
}
