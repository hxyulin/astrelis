//! Backend-neutral desktop windowing, lifecycle, and input vocabulary.

#![warn(missing_docs)]

mod application;
mod error;
mod event;
mod input;
mod window;

pub use application::*;
pub use error::*;
pub use event::*;
pub use input::*;
pub use window::*;

/// Unstable contracts implemented by platform backends.
///
/// These traits may change between releases. Applications should use the
/// stable wrapper types at the crate root instead.
pub mod backend;
