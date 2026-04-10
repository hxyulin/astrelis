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
//! | Feature   | Backend                                          |
//! |-----------|--------------------------------------------------|
//! | `puffin`  | [puffin](https://crates.io/crates/puffin) viewer |
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
// Feature: puffin ENABLED — re-export puffin's own macros directly
// ============================================================================
#[cfg(feature = "puffin")]
pub use puffin::{profile_function, profile_scope};

/// Re-export puffin so downstream crates can access it if needed.
#[cfg(feature = "puffin")]
#[doc(hidden)]
pub use puffin;

// ============================================================================
// Feature: puffin DISABLED — zero-cost no-op stubs
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
#[cfg(not(feature = "puffin"))]
#[macro_export]
macro_rules! profile_scope {
    ($name:expr) => {};
    ($name:expr, $data:expr) => {};
}

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
/// Counters track named integer/float values over time. The value is
/// reported to the active profiling backend under the given category and name.
///
/// No-op when no profiling backend is enabled.
///
/// # Example
///
/// ```rust
/// astrelis_profiling::profile_counter!("gpu_memory", "buffer_bytes", 1024u64);
/// astrelis_profiling::profile_counter!("cache", "hit_rate", 0.95f64);
/// ```
#[macro_export]
macro_rules! profile_counter {
    ($category:expr, $name:expr, $value:expr) => {
        $crate::counters::record_counter($category, $name, $value)
    };
}

/// Records a profiling plot value.
///
/// Plots display as continuous line graphs in the profiler viewer.
/// Useful for frame time, FPS, temperature, or any continuous metric.
///
/// No-op when no profiling backend is enabled.
///
/// # Example
///
/// ```rust
/// astrelis_profiling::profile_plot!("frame_time_ms", 16.3);
/// ```
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
