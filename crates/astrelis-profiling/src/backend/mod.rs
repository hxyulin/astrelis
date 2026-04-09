//! Profiling backend dispatch.
//!
//! The active backend is selected at compile time via feature flags.
//! When no backend feature is enabled, all functions are no-ops.

#[cfg(not(feature = "puffin"))]
mod noop;
#[cfg(feature = "puffin")]
mod puffin;

#[cfg(not(feature = "puffin"))]
pub use noop::{finish, init, new_frame};
#[cfg(feature = "puffin")]
pub use self::puffin::{finish, init, new_frame};
