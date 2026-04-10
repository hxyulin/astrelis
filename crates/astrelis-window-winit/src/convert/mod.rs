//! Conversions between winit types and their `astrelis-window` equivalents.
//!
//! Each submodule handles a specific category (keyboard codes, mouse buttons,
//! cursor icons, window events, monitor info).

pub(crate) mod cursor;
pub(crate) mod event;
pub(crate) mod keyboard;
pub(crate) mod monitor;
pub(crate) mod mouse;
