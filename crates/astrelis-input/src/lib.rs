//! Polling-style input state tracking for the Astrelis engine.
//!
//! This crate provides [`InputState`], a stateful tracker that accumulates
//! [`WindowEvent`](astrelis_window::event::WindowEvent) and
//! [`DeviceEvent`](astrelis_window::event::DeviceEvent) data each frame.
//! Game code queries the current state rather than subscribing to events.
//!
//! # Example
//!
//! ```
//! use astrelis_input::InputState;
//! use astrelis_window::keyboard::KeyCode;
//!
//! let mut input = InputState::new();
//!
//! // Each frame:
//! input.begin_frame();
//! // ... feed window events via input.handle_event(&event) ...
//!
//! if input.is_key_pressed(KeyCode::KeyW) {
//!     // move forward
//! }
//! ```

#![warn(missing_docs)]

mod input;
mod state;

pub use input::InputState;
