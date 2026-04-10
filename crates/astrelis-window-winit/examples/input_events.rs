//! Input events example.
//!
//! Logs all keyboard, mouse, and device-level input events to stdout so you
//! can see exactly what the engine receives. Useful for verifying key codes,
//! mouse buttons, scroll deltas, and raw device motion.
//!
//! There are two event streams demonstrated here:
//!
//! - **Window events** (`on_window_event`) — tied to a specific window.
//!   Cursor position, keyboard input with logical key values, mouse buttons,
//!   scroll wheel, touch, and focus changes. These are the events you use for
//!   UI interaction and most gameplay input.
//!
//! - **Device events** (`on_device_event`) — raw hardware events not tied to
//!   any window. The key one is `DeviceEvent::MouseMotion`, which reports
//!   raw mouse deltas even when the cursor is locked. This is how you
//!   implement first-person camera rotation — `CursorMoved` window events
//!   become unreliable once the cursor is locked in place.
//!
//! ## Controls
//!
//! - `L` — toggle cursor lock (enables `DeviceEvent::MouseMotion` delta logging)
//! - `Escape` — exit
//!
//! ## Run
//!
//! ```sh
//! cargo run -p astrelis-window-winit --example input_events
//! ```

use astrelis_window::backend::{AppHandler, EventLoopContext, WindowBackend};
use astrelis_window::control_flow::ControlFlow;
use astrelis_window::cursor::CursorGrabMode;
use astrelis_window::event::{DeviceEvent, KeyEvent, WindowEvent};
use astrelis_window::keyboard::{Key, NamedKey};
use astrelis_window::lifecycle::AppLifecycle;
use astrelis_window::types::LogicalInnerSize;
use astrelis_window::window_id::WindowId;
use astrelis_window::WindowBuilder;
use astrelis_window_winit::WinitBackend;

struct App {
    window_id: Option<WindowId>,
    cursor_locked: bool,
}

impl AppHandler for App {
    fn on_lifecycle(&mut self, ctx: &mut dyn EventLoopContext, state: AppLifecycle) {
        astrelis_profiling::profile_function!();
        if state == AppLifecycle::Resumed {
            let attrs = WindowBuilder::new()
                .with_title("Astrelis — Input Events (L=lock cursor, Esc=quit)")
                .with_inner_size(LogicalInnerSize::new(800.0, 600.0))
                .build();
            self.window_id = Some(ctx.create_window(attrs).expect("failed to create window"));
            ctx.set_control_flow(ControlFlow::Wait);
            println!("=== Input Events Demo ===");
            println!("Press keys, click, scroll, move the mouse.");
            println!("Press L to toggle cursor lock (raw mouse deltas).");
            println!("Press Escape to quit.");
            println!();
        }
    }

    /// Window-level events: keyboard input (with logical key values), mouse
    /// position, buttons, scroll, touch, and focus. These are tied to a
    /// specific window and use the window's coordinate space.
    fn on_window_event(
        &mut self,
        ctx: &mut dyn EventLoopContext,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        astrelis_profiling::profile_function!();
        match event {
            WindowEvent::CloseRequested => ctx.exit(),

            // --- Keyboard ---
            // KeyboardInput gives you both the physical scan code (key_code)
            // and the layout-dependent logical key (key). For gameplay bindings
            // use key_code (WASD stays WASD on AZERTY); for text input use key.
            WindowEvent::KeyboardInput(KeyEvent {
                key_code,
                key,
                state,
                location,
                repeat,
            }) => {
                let action = if state.is_pressed() { "pressed" } else { "released" };
                let rep = if repeat { " (repeat)" } else { "" };
                println!(
                    "[Key] {action}: code={key_code:?}, key={key:?}, loc={location:?}{rep}"
                );

                // Toggle cursor lock on L press.
                if state.is_pressed() {
                    if let Key::Character(ref c) = key
                        && (c == "l" || c == "L")
                    {
                        self.cursor_locked = !self.cursor_locked;
                        if let Some(win) = ctx.window(_window_id) {
                            let mode = if self.cursor_locked {
                                CursorGrabMode::Locked
                            } else {
                                CursorGrabMode::None
                            };
                            match win.set_cursor_grab(mode) {
                                Ok(()) => {
                                    win.set_cursor_visible(!self.cursor_locked);
                                    println!(
                                        "  -> Cursor {}",
                                        if self.cursor_locked {
                                            "LOCKED (watch DeviceEvent::MouseMotion)"
                                        } else {
                                            "released"
                                        }
                                    );
                                }
                                Err(e) => println!("  -> Lock failed: {e}"),
                            }
                        }
                    }
                    if matches!(key, Key::Named(NamedKey::Escape)) {
                        ctx.exit();
                    }
                }
            }

            WindowEvent::ModifiersChanged(mods) => {
                println!(
                    "[Modifiers] shift={}, ctrl={}, alt={}, meta={}",
                    mods.shift, mods.control, mods.alt, mods.meta
                );
            }

            // --- Mouse ---
            WindowEvent::CursorMoved(pos) => {
                println!("[CursorMoved] ({:.1}, {:.1})", pos.x, pos.y);
            }

            WindowEvent::CursorEntered => {
                println!("[CursorEntered]");
            }

            WindowEvent::CursorLeft => {
                println!("[CursorLeft]");
            }

            WindowEvent::MouseButtonInput { button, state } => {
                let action = if state.is_pressed() { "pressed" } else { "released" };
                println!("[MouseButton] {action}: {button:?}");
            }

            WindowEvent::MouseWheel(delta) => {
                println!("[MouseWheel] {delta:?}");
            }

            // --- Touch ---
            WindowEvent::Touch(touch) => {
                let p = touch.position;
                println!(
                    "[Touch] id={:?}, phase={:?}, pos=({:.1}, {:.1})",
                    touch.id, touch.phase, p.x, p.y
                );
            }

            // --- Focus ---
            WindowEvent::Focused(focused) => {
                println!(
                    "[Focus] {}",
                    if focused { "gained" } else { "lost" }
                );
            }

            _ => {}
        }
    }

    /// Device-level events: raw hardware input not tied to any window.
    ///
    /// `DeviceEvent::MouseMotion` is the critical one — it gives you raw
    /// dx/dy deltas from the mouse hardware. When the cursor is locked via
    /// `CursorGrabMode::Locked`, window-level `CursorMoved` events report a
    /// fixed position (the lock point), so this is the only way to get
    /// frame-to-frame mouse movement for camera rotation.
    fn on_device_event(
        &mut self,
        _ctx: &mut dyn EventLoopContext,
        event: DeviceEvent,
    ) {
        astrelis_profiling::profile_function!();
        match event {
            DeviceEvent::MouseMotion { delta_x, delta_y } => {
                // Only print when cursor is locked to avoid flooding stdout.
                if self.cursor_locked {
                    println!("[Device::MouseMotion] dx={delta_x:.2}, dy={delta_y:.2}");
                }
            }
            DeviceEvent::MouseWheel { delta_x, delta_y } => {
                println!("[Device::MouseWheel] dx={delta_x:.2}, dy={delta_y:.2}");
            }
            DeviceEvent::Button { button, state } => {
                let action = if state.is_pressed() { "pressed" } else { "released" };
                println!("[Device::Button] {action}: button={button}");
            }
            DeviceEvent::Key(key_event) => {
                let action = if key_event.state.is_pressed() {
                    "pressed"
                } else {
                    "released"
                };
                println!("[Device::Key] {action}: code={:?}", key_event.key_code);
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
        cursor_locked: false,
    };
    backend.run(&mut app).expect("event loop error");
}
