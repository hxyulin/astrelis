//! Basic Window Example - Minimal Astrelis Application
//!
//! Demonstrates the simplest possible Astrelis application using the App trait:
//! - Window creation and management
//! - Basic app lifecycle (update/render)
//! - Event loop integration
//! - Frame counting
//!
//! ## Features Showcased
//! - `App` trait implementation
//! - Window creation with `WindowDescriptor`
//! - Basic event handling
//! - Frame-by-frame updates
//!
//! ## Usage
//! ```bash
//! cargo run -p astrelis-winit --example basic_window
//! ```
//!
//! This is the starting point for understanding Astrelis applications.

use astrelis_winit::{
    FrameTime, WindowId,
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
    fn update(&mut self, _ctx: &mut AppCtx, _time: &FrameTime) {
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
