//! Resizable window with size constraints.
//!
//! Demonstrates minimum/maximum size constraints, logging resize events,
//! and toggling maximize with the `M` key. Uses app mode (reactive, Wait).
//!
//! Controls:
//! - `M` — toggle maximize
//! - `Escape` — exit
//!
//! Run with:
//! ```sh
//! cargo run -p astrelis-window-winit --example resizable_window
//! ```

use astrelis_window::backend::{AppHandler, EventLoopContext, WindowBackend};
use astrelis_window::control_flow::ControlFlow;
use astrelis_window::event::{KeyEvent, WindowEvent};
use astrelis_window::keyboard::{Key, NamedKey};
use astrelis_window::lifecycle::AppLifecycle;
use astrelis_window::types::LogicalInnerSize;
use astrelis_window::window_id::WindowId;
use astrelis_window::WindowBuilder;
use astrelis_window_winit::WinitBackend;

struct App {
    window_id: Option<WindowId>,
}

impl AppHandler for App {
    fn on_lifecycle(&mut self, ctx: &mut dyn EventLoopContext, state: AppLifecycle) {
        if state == AppLifecycle::Resumed {
            let attrs = WindowBuilder::new()
                .with_title("Astrelis — Resizable Window (min 400x300, max 1920x1080)")
                .with_inner_size(LogicalInnerSize::new(800.0, 600.0))
                .with_min_inner_size(LogicalInnerSize::new(400.0, 300.0))
                .with_max_inner_size(LogicalInnerSize::new(1920.0, 1080.0))
                .with_resizable(true)
                .build();

            self.window_id = Some(ctx.create_window(attrs).expect("failed to create window"));

            // App mode: sleep until an event arrives (low CPU usage).
            ctx.set_control_flow(ControlFlow::Wait);
        }
    }

    fn on_window_event(
        &mut self,
        ctx: &mut dyn EventLoopContext,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => ctx.exit(),

            WindowEvent::Resized(inner_size) => {
                let s = inner_size.physical();
                println!("Resized to {:.0}x{:.0} physical pixels", s.width, s.height);
            }

            WindowEvent::KeyboardInput(KeyEvent { key, state, .. }) if state.is_pressed() => {
                if let Some(win) = ctx.window(window_id) {
                    match &key {
                        Key::Character(c) if c == "m" || c == "M" => {
                            let maximized = win.is_maximized();
                            win.set_maximized(!maximized);
                            println!(
                                "{}",
                                if maximized { "Restoring" } else { "Maximizing" }
                            );
                        }
                        Key::Named(NamedKey::Escape) => ctx.exit(),
                        _ => {}
                    }
                }
            }

            _ => {}
        }
    }

    fn on_events_cleared(&mut self, _ctx: &mut dyn EventLoopContext) {}
}

fn main() {
    let backend = WinitBackend::new().expect("failed to create backend");
    let mut app = App { window_id: None };
    backend.run(&mut app).expect("event loop error");
}
