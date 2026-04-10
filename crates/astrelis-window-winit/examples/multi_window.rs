//! Multi-window example.
//!
//! Opens two windows side by side. Each window tracks its own state.
//! Closing one window does not close the other — the application exits
//! only when all windows are closed.
//!
//! Controls:
//! - `N` — open a new window
//! - `Escape` or close button — close the focused window
//!
//! Run with:
//! ```sh
//! cargo run -p astrelis-window-winit --example multi_window
//! ```

use std::collections::HashSet;

use astrelis_window::backend::{AppHandler, EventLoopContext, WindowBackend};
use astrelis_window::control_flow::ControlFlow;
use astrelis_window::event::{KeyEvent, WindowEvent};
use astrelis_window::keyboard::{Key, NamedKey};
use astrelis_window::lifecycle::AppLifecycle;
use astrelis_window::types::{LogicalInnerSize, LogicalOuterPosition};
use astrelis_window::window_id::WindowId;
use astrelis_window::WindowBuilder;
use astrelis_window_winit::WinitBackend;

struct App {
    windows: HashSet<WindowId>,
    next_number: u32,
}

impl App {
    fn open_window(
        &mut self,
        ctx: &mut dyn EventLoopContext,
        x: f32,
        y: f32,
    ) {
        self.next_number += 1;
        let attrs = WindowBuilder::new()
            .with_title(format!("Astrelis — Window #{}", self.next_number))
            .with_inner_size(LogicalInnerSize::new(600.0, 400.0))
            .with_position(LogicalOuterPosition::new(x, y))
            .build();

        match ctx.create_window(attrs) {
            Ok(id) => {
                self.windows.insert(id);
                println!("Opened window #{} (id: {id})", self.next_number);
            }
            Err(e) => eprintln!("Failed to open window: {e}"),
        }
    }
}

impl AppHandler for App {
    fn on_lifecycle(&mut self, ctx: &mut dyn EventLoopContext, state: AppLifecycle) {
        astrelis_profiling::profile_function!();
        if state == AppLifecycle::Resumed {
            // Open two windows at different positions.
            self.open_window(ctx, 100.0, 100.0);
            self.open_window(ctx, 720.0, 100.0);

            // App mode: only wake when events arrive.
            ctx.set_control_flow(ControlFlow::Wait);
        }
    }

    fn on_window_event(
        &mut self,
        ctx: &mut dyn EventLoopContext,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        astrelis_profiling::profile_function!();
        match event {
            WindowEvent::CloseRequested => {
                println!("Closing window {window_id}");
                let _ = ctx.destroy_window(window_id);
                self.windows.remove(&window_id);
                if self.windows.is_empty() {
                    println!("All windows closed — exiting");
                    ctx.exit();
                }
            }

            WindowEvent::KeyboardInput(KeyEvent { key, state, .. }) if state.is_pressed() => {
                match &key {
                    Key::Named(NamedKey::Escape) => {
                        println!("Closing window {window_id} via Escape");
                        let _ = ctx.destroy_window(window_id);
                        self.windows.remove(&window_id);
                        if self.windows.is_empty() {
                            ctx.exit();
                        }
                    }
                    Key::Character(c) if c == "n" || c == "N" => {
                        // Open a new window offset from the origin.
                        let offset = self.next_number as f32 * 30.0;
                        self.open_window(ctx, 100.0 + offset, 100.0 + offset);
                    }
                    _ => {}
                }
            }

            WindowEvent::Focused(focused) => {
                println!(
                    "Window {window_id} {}",
                    if focused { "focused" } else { "unfocused" }
                );
            }

            _ => {}
        }
    }

    fn on_events_cleared(&mut self, _ctx: &mut dyn EventLoopContext) {
        astrelis_profiling::profile_function!();
    }
}

fn main() {
    astrelis_profiling::init();
    let backend = WinitBackend::new().expect("failed to create backend");
    let mut app = App {
        windows: HashSet::new(),
        next_number: 0,
    };
    backend.run(&mut app).expect("event loop error");
}
