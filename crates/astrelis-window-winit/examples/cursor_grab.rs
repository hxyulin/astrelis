//! Cursor grab and hide example.
//!
//! Demonstrates cursor visibility, grab modes (confined / locked), and
//! cursor icon switching — the setup used for first-person camera controls.
//!
//! Controls:
//! - `H` — toggle cursor visibility
//! - `L` — toggle cursor lock (hidden + only deltas reported)
//! - `C` — toggle cursor confinement (confined to window bounds)
//! - `1`–`3` — switch cursor icon (default, crosshair, pointer)
//! - `Escape` — exit
//!
//! Run with:
//! ```sh
//! cargo run -p astrelis-window-winit --example cursor_grab
//! ```

use astrelis_window::backend::{AppHandler, EventLoopContext, WindowBackend};
use astrelis_window::control_flow::ControlFlow;
use astrelis_window::cursor::{CursorGrabMode, CursorIcon};
use astrelis_window::event::{KeyEvent, WindowEvent};
use astrelis_window::keyboard::{Key, NamedKey};
use astrelis_window::lifecycle::AppLifecycle;
use astrelis_window::types::LogicalInnerSize;
use astrelis_window::window_id::WindowId;
use astrelis_window::WindowBuilder;
use astrelis_window_winit::WinitBackend;

struct App {
    window_id: Option<WindowId>,
    cursor_visible: bool,
    grab_mode: CursorGrabMode,
}

impl AppHandler for App {
    fn on_lifecycle(&mut self, ctx: &mut dyn EventLoopContext, state: AppLifecycle) {
        astrelis_profiling::profile_function!();
        if state == AppLifecycle::Resumed {
            let attrs = WindowBuilder::new()
                .with_title("Astrelis — Cursor Grab (H=hide, L=lock, C=confine)")
                .with_inner_size(LogicalInnerSize::new(800.0, 600.0))
                .build();
            self.window_id = Some(ctx.create_window(attrs).expect("failed to create window"));
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
            WindowEvent::CloseRequested => ctx.exit(),

            WindowEvent::KeyboardInput(KeyEvent { key, state, .. }) if state.is_pressed() => {
                let Some(win) = ctx.window(window_id) else {
                    return;
                };

                match &key {
                    Key::Character(c) => match c.as_str() {
                        "h" | "H" => {
                            self.cursor_visible = !self.cursor_visible;
                            win.set_cursor_visible(self.cursor_visible);
                            println!(
                                "Cursor {}",
                                if self.cursor_visible { "shown" } else { "hidden" }
                            );
                        }
                        "l" | "L" => {
                            self.grab_mode = match self.grab_mode {
                                CursorGrabMode::Locked => CursorGrabMode::None,
                                _ => CursorGrabMode::Locked,
                            };
                            match win.set_cursor_grab(self.grab_mode) {
                                Ok(()) => println!("Grab: {:?}", self.grab_mode),
                                Err(e) => println!("Grab failed: {e}"),
                            }
                        }
                        "c" | "C" => {
                            self.grab_mode = match self.grab_mode {
                                CursorGrabMode::Confined => CursorGrabMode::None,
                                _ => CursorGrabMode::Confined,
                            };
                            match win.set_cursor_grab(self.grab_mode) {
                                Ok(()) => println!("Grab: {:?}", self.grab_mode),
                                Err(e) => println!("Grab failed: {e}"),
                            }
                        }
                        "1" => {
                            win.set_cursor_icon(CursorIcon::Default);
                            println!("Cursor: Default");
                        }
                        "2" => {
                            win.set_cursor_icon(CursorIcon::Crosshair);
                            println!("Cursor: Crosshair");
                        }
                        "3" => {
                            win.set_cursor_icon(CursorIcon::Pointer);
                            println!("Cursor: Pointer");
                        }
                        _ => {}
                    },
                    Key::Named(NamedKey::Escape) => ctx.exit(),
                    _ => {}
                }
            }

            WindowEvent::CursorMoved(pos) => {
                if self.grab_mode == CursorGrabMode::Locked {
                    println!("Cursor position: ({:.1}, {:.1})", pos.x, pos.y);
                }
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
        window_id: None,
        cursor_visible: true,
        grab_mode: CursorGrabMode::None,
    };
    backend.run(&mut app).expect("event loop error");
}
