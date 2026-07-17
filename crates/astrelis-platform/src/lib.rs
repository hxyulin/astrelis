//! Backend-neutral desktop windowing, lifecycle, and input vocabulary.

#![warn(missing_docs)]

mod application;
mod clipboard;
mod error;
mod event;
mod input;
mod window;

pub use application::*;
pub use clipboard::*;
pub use error::*;
pub use event::*;
pub use input::*;
pub use window::*;

/// Monotonic instant used by platform scheduling on the current target.
#[cfg(not(target_arch = "wasm32"))]
pub use std::time::Instant;
/// Monotonic instant used by platform scheduling on the current target.
#[cfg(target_arch = "wasm32")]
pub use web_time::Instant;

/// Unstable contracts implemented by platform backends.
///
/// These traits may change between releases. Applications should use the
/// stable wrapper types at the crate root instead.
pub mod backend;
