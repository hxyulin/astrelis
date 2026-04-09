//! Backend-agnostic profiling for the Astrelis engine.
//!
//! This crate provides profiling macros that compile to zero-cost no-ops when
//! no backend feature is enabled. Enable a backend feature (e.g., `puffin`) to
//! activate profiling.
//!
//! # Usage
//!
//! ```rust
//! fn update_physics() {
//!     astrelis_profiling::profile_function!();
//!
//!     {
//!         astrelis_profiling::profile_scope!("broad_phase");
//!         // ... broad phase collision detection ...
//!     }
//! }
//! ```
//!
//! # Backends
//!
//! | Feature   | Backend                                          |
//! |-----------|--------------------------------------------------|
//! | `puffin`  | [puffin](https://crates.io/crates/puffin) viewer |
//!
//! When no backend feature is enabled, all macros expand to nothing.

mod backend;

pub use backend::{finish, init, new_frame};

/// Trait for GPU profiling integration.
///
/// Implement this on your GPU context to enable GPU scope tracking.
/// The profiling crate does not depend on any GPU crate — implementors
/// pull in this trait and provide the implementation.
pub trait GpuProfiler {
    /// Begins a named GPU profiling scope.
    fn begin_scope(&mut self, label: &str);

    /// Ends the most recently opened GPU profiling scope.
    fn end_scope(&mut self);
}

/// Profiles the enclosing function.
///
/// When a profiling backend is enabled, this records a scope spanning the
/// entire function. When no backend is enabled, this is a no-op.
///
/// # Example
///
/// ```rust
/// fn my_function() {
///     astrelis_profiling::profile_function!();
///     // ...
/// }
/// ```
#[cfg(feature = "puffin")]
#[macro_export]
macro_rules! profile_function {
    () => {
        ::puffin::profile_function!();
    };
    ($data:expr) => {
        ::puffin::profile_function!($data);
    };
}

/// Profiles the enclosing function (no-op variant).
#[cfg(not(feature = "puffin"))]
#[macro_export]
macro_rules! profile_function {
    () => {};
    ($data:expr) => {};
}

/// Profiles a named scope within a function.
///
/// # Example
///
/// ```rust
/// fn process() {
///     {
///         astrelis_profiling::profile_scope!("step_1");
///         // ...
///     }
///     {
///         astrelis_profiling::profile_scope!("step_2", "extra data");
///         // ...
///     }
/// }
/// ```
#[cfg(feature = "puffin")]
#[macro_export]
macro_rules! profile_scope {
    ($name:expr) => {
        ::puffin::profile_scope!($name);
    };
    ($name:expr, $data:expr) => {
        ::puffin::profile_scope!($name, $data);
    };
}

/// Profiles a named scope (no-op variant).
#[cfg(not(feature = "puffin"))]
#[macro_export]
macro_rules! profile_scope {
    ($name:expr) => {};
    ($name:expr, $data:expr) => {};
}
