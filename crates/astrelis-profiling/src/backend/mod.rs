//! Profiling backend dispatch.
//!
//! The active backend is selected at compile time via feature flags.
//! When no backend feature is enabled, all functions are no-ops.

#[cfg(not(any(feature = "puffin", feature = "tracy")))]
mod noop;
#[cfg(feature = "puffin")]
mod puffin;
#[cfg(feature = "tracy")]
mod tracy;

// ============================================================================
// No backend enabled — zero-cost no-ops
// ============================================================================

/// Initializes the profiling backend.
///
/// With a backend feature enabled, this starts the profiler.
/// Without any backend feature, this is a no-op.
#[cfg(not(any(feature = "puffin", feature = "tracy")))]
pub use noop::init;
/// Signals a frame boundary to the profiler.
///
/// Call once per frame (typically at the start of the main loop) so the
/// profiler can separate per-frame data. No-op without a backend feature.
#[cfg(not(any(feature = "puffin", feature = "tracy")))]
pub use noop::new_frame;
/// Shuts down the profiling backend. No-op without a backend feature.
#[cfg(not(any(feature = "puffin", feature = "tracy")))]
pub use noop::finish;
/// Names the current thread for the profiler. No-op without a backend feature.
#[cfg(not(any(feature = "puffin", feature = "tracy")))]
pub use noop::set_thread_name;
/// Reports GPU scopes to the profiler. No-op without a backend feature.
#[cfg(not(any(feature = "puffin", feature = "tracy")))]
pub use noop::report_gpu_scopes;
/// Records a counter value. No-op without a backend feature.
#[cfg(not(any(feature = "puffin", feature = "tracy")))]
pub use noop::record_counter;
/// Records a plot value. No-op without a backend feature.
#[cfg(not(any(feature = "puffin", feature = "tracy")))]
pub use noop::record_plot;

// ============================================================================
// Puffin backend
// ============================================================================

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

// ============================================================================
// Tracy backend
// ============================================================================

/// Initializes the profiling backend.
///
/// With the `tracy` feature enabled, this starts the Tracy client and
/// creates a GPU context. Connect the Tracy GUI to view live data.
#[cfg(feature = "tracy")]
pub use self::tracy::init;
/// Signals a frame boundary to the profiler.
///
/// Call once per frame (typically at the start of the main loop) so the
/// profiler can separate per-frame data.
#[cfg(feature = "tracy")]
pub use self::tracy::new_frame;
/// Shuts down the profiling backend.
#[cfg(feature = "tracy")]
pub use self::tracy::finish;
/// Names the current thread for the Tracy profiler.
///
/// The thread name appears in the Tracy timeline view alongside its spans.
#[cfg(feature = "tracy")]
pub use self::tracy::set_thread_name;
/// Reports GPU scopes to Tracy under a dedicated GPU timeline.
///
/// GPU scopes appear as native GPU zones in the Tracy viewer with
/// proper CPU-GPU correlation.
#[cfg(feature = "tracy")]
pub use self::tracy::report_gpu_scopes;
/// Records a counter value as a Tracy plot.
#[cfg(feature = "tracy")]
pub use self::tracy::record_counter;
/// Records a plot value in the Tracy timeline.
#[cfg(feature = "tracy")]
pub use self::tracy::record_plot;
