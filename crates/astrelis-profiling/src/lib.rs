//! Backend-agnostic profiling for the Astrelis engine.
//!
//! This crate provides profiling macros that compile to zero-cost no-ops when
//! no backend feature is enabled. Enable a backend feature (e.g., `tracy`) to
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
//! # GPU Profiling
//!
//! GPU backend implementations report timing data via [`gpu::report_gpu_scopes`].
//! The active profiling backend displays these under a virtual "GPU" thread.
//!
//! # Counters & Plots
//!
//! Track custom metrics with [`profile_counter!`] and [`profile_plot!`]:
//!
//! ```rust
//! astrelis_profiling::profile_counter!("gpu_memory", "buffer_bytes", 1024u64);
//! astrelis_profiling::profile_plot!("frame_time_ms", 16.3);
//! ```
//!
//! # Backends
//!
//! | Feature  | Backend                                              |
//! |----------|------------------------------------------------------|
//! | `puffin` | [puffin](https://crates.io/crates/puffin) viewer     |
//! | `tracy`  | [Tracy](https://github.com/wolfpld/tracy) profiler   |
//!
//! When no backend feature is enabled, all macros and functions are zero-cost no-ops.

pub(crate) mod backend;
pub mod counters;
pub mod data;
pub mod gpu;
pub mod thread;

pub use backend::{finish, init, new_frame, set_thread_name};
pub use thread::spawn_profiled;

// ============================================================================
// Feature: tracy ENABLED — use tracy_client::span!() for CPU zones
// ============================================================================

/// Profiles the enclosing function.
///
/// When the `tracy` backend is enabled, this records a Tracy zone spanning
/// the entire function. When no backend is enabled, this is a no-op.
#[cfg(feature = "tracy")]
#[macro_export]
macro_rules! profile_function {
    () => {
        let _tracy_span = ::tracy_client::span!();
    };
    ($data:expr) => {
        let _tracy_span = ::tracy_client::span!($data);
    };
}

/// Profiles a named scope within a function.
///
/// When the `tracy` backend is enabled, this records a named Tracy zone.
#[cfg(feature = "tracy")]
#[macro_export]
macro_rules! profile_scope {
    ($name:expr) => {
        let _tracy_span = ::tracy_client::span!($name);
    };
    ($name:expr, $data:expr) => {
        // Tracy span! only takes name + optional callstack depth.
        // Attach extra data as a message on the span.
        let _tracy_span = ::tracy_client::span!($name);
    };
}

/// Re-export tracy_client so downstream crates can access it if needed.
#[cfg(feature = "tracy")]
#[doc(hidden)]
pub use tracy_client;

// ============================================================================
// Feature: puffin ENABLED — re-export puffin's own macros directly
// ============================================================================

/// Profiles the enclosing function.
///
/// When the `puffin` backend is enabled, this records a puffin scope
/// spanning the entire function.
#[cfg(feature = "puffin")]
pub use puffin::{profile_function, profile_scope};

/// Re-export puffin so downstream crates can access it if needed.
#[cfg(feature = "puffin")]
#[doc(hidden)]
pub use puffin;

// ============================================================================
// No backend — zero-cost no-op stubs
// ============================================================================

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
#[cfg(not(any(feature = "puffin", feature = "tracy")))]
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
#[cfg(not(any(feature = "puffin", feature = "tracy")))]
#[macro_export]
macro_rules! profile_scope {
    ($name:expr) => {};
    ($name:expr, $data:expr) => {};
}

// ============================================================================
// Macros that are the same regardless of backend
// ============================================================================

/// Names the current thread for profiling.
///
/// Call at the start of thread entry points (e.g., thread pool workers,
/// rayon callbacks) to give the thread a human-readable name in the
/// profiler viewer.
///
/// # Example
///
/// ```rust
/// // Inside a thread pool worker:
/// astrelis_profiling::profile_thread!("worker_0");
/// ```
#[macro_export]
macro_rules! profile_thread {
    ($name:expr) => {
        $crate::set_thread_name($name);
    };
}

/// Records a profiling counter value.
///
/// When the `tracy` backend is enabled, counters are displayed as native
/// Tracy plots. With other backends, they route through the backend's
/// counter API.
///
/// No-op when no profiling backend is enabled.
///
/// # Example
///
/// ```rust
/// astrelis_profiling::profile_counter!("gpu_memory", "buffer_bytes", 1024u64);
/// astrelis_profiling::profile_counter!("cache", "hit_rate", 0.95f64);
/// ```
#[cfg(feature = "tracy")]
#[macro_export]
macro_rules! profile_counter {
    ($category:expr, $name:expr, $value:expr) => {
        ::tracy_client::plot!($name, $crate::data::counter_to_f64($value))
    };
}

/// Records a profiling counter value.
///
/// No-op when no profiling backend is enabled.
#[cfg(not(feature = "tracy"))]
#[macro_export]
macro_rules! profile_counter {
    ($category:expr, $name:expr, $value:expr) => {
        $crate::counters::record_counter($category, $name, $value)
    };
}

/// Records a profiling plot value.
///
/// When the `tracy` backend is enabled, plots are displayed as native
/// Tracy time-series graphs. With other backends, they route through
/// the backend's plot API.
///
/// No-op when no profiling backend is enabled.
///
/// # Example
///
/// ```rust
/// astrelis_profiling::profile_plot!("frame_time_ms", 16.3);
/// ```
#[cfg(feature = "tracy")]
#[macro_export]
macro_rules! profile_plot {
    ($name:expr, $value:expr) => {
        ::tracy_client::plot!($name, $value as f64)
    };
}

/// Records a profiling plot value.
///
/// No-op when no profiling backend is enabled.
#[cfg(not(feature = "tracy"))]
#[macro_export]
macro_rules! profile_plot {
    ($name:expr, $value:expr) => {
        $crate::counters::record_plot($name, $value)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_function_compiles() {
        profile_function!();
    }

    #[test]
    fn profile_function_with_data_compiles() {
        profile_function!("some data");
    }

    #[test]
    fn profile_scope_compiles() {
        profile_scope!("test_scope");
    }

    #[test]
    fn profile_scope_with_data_compiles() {
        profile_scope!("test_scope", "extra data");
    }

    #[test]
    fn lifecycle_functions_are_callable() {
        init();
        new_frame();
        finish();
    }

    #[test]
    fn counter_macro_compiles() {
        profile_counter!("test", "counter", 42u64);
        profile_counter!("test", "float_counter", 3.14f64);
    }

    #[test]
    fn plot_macro_compiles() {
        profile_plot!("test_metric", 1.0);
    }

    #[test]
    fn gpu_reporting_compiles() {
        gpu::report_gpu_scopes(&[]);
    }
}
