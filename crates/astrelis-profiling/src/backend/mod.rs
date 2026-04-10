//! Profiling backend dispatch.
//!
//! The active backend is selected at compile time via feature flags.
//! When no backend feature is enabled, all functions are no-ops.

#[cfg(not(feature = "puffin"))]
mod noop;
#[cfg(feature = "puffin")]
mod puffin;

// --- Lifecycle functions ---

/// Initializes the profiling backend.
///
/// With the `puffin` feature enabled, this starts the puffin HTTP viewer
/// server. Without any backend feature, this is a no-op.
#[cfg(not(feature = "puffin"))]
pub use noop::init;
/// Signals a frame boundary to the profiler.
///
/// Call once per frame (typically at the start of the main loop) so the
/// profiler can separate per-frame data. No-op without a backend feature.
#[cfg(not(feature = "puffin"))]
pub use noop::new_frame;
/// Shuts down the profiling backend. No-op without a backend feature.
#[cfg(not(feature = "puffin"))]
pub use noop::finish;
/// Names the current thread for the profiler. No-op without a backend feature.
#[cfg(not(feature = "puffin"))]
pub use noop::set_thread_name;
/// Reports GPU scopes to the profiler. No-op without a backend feature.
#[cfg(not(feature = "puffin"))]
pub use noop::report_gpu_scopes;
/// Records a counter value. No-op without a backend feature.
#[cfg(not(feature = "puffin"))]
pub use noop::record_counter;
/// Records a plot value. No-op without a backend feature.
#[cfg(not(feature = "puffin"))]
pub use noop::record_plot;

/// Initializes the profiling backend.
///
/// With the `puffin` feature enabled, this starts the puffin HTTP viewer
/// server. Without any backend feature, this is a no-op.
#[cfg(feature = "puffin")]
pub use self::puffin::init;
/// Signals a frame boundary to the profiler.
///
/// Call once per frame (typically at the start of the main loop) so the
/// profiler can separate per-frame data. No-op without a backend feature.
#[cfg(feature = "puffin")]
pub use self::puffin::new_frame;
/// Shuts down the profiling backend. No-op without a backend feature.
#[cfg(feature = "puffin")]
pub use self::puffin::finish;
/// Names the current thread for the profiler.
///
/// With the `puffin` feature enabled, this sets the thread name in the
/// puffin viewer. Without any backend feature, this is a no-op.
#[cfg(feature = "puffin")]
pub use self::puffin::set_thread_name;
/// Reports GPU scopes to the profiler.
///
/// With the `puffin` feature enabled, GPU scopes appear under a virtual
/// "GPU" thread in the puffin viewer.
#[cfg(feature = "puffin")]
pub use self::puffin::report_gpu_scopes;
/// Records a counter value.
#[cfg(feature = "puffin")]
pub use self::puffin::record_counter;
/// Records a plot value.
#[cfg(feature = "puffin")]
pub use self::puffin::record_plot;
