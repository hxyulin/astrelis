//! Profiling utilities based on the `puffin` crate.
//!
//! When the `profiling` feature is enabled (default), this module re-exports
//! puffin macros and provides HTTP-based profiling. When disabled, all macros
//! and functions become no-ops with zero overhead.

// ============================================================================
// Feature: profiling ENABLED
// ============================================================================
#[cfg(feature = "profiling")]
mod enabled {
    use std::sync::OnceLock;

    pub use puffin::{GlobalProfiler, profile_function, profile_scope};

    /// Profiling backend options.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ProfilingBackend {
        /// Send profiling data to puffin_viewer via HTTP.
        PuffinHttp,
    }

    /// Global profiling server instance.
    static PROFILING_SERVER: OnceLock<puffin_http::Server> = OnceLock::new();

    /// Initialize profiling with the specified backend.
    ///
    /// # Example
    /// ```no_run
    /// use astrelis_core::profiling::{init_profiling, ProfilingBackend};
    ///
    /// init_profiling(ProfilingBackend::PuffinHttp);
    /// ```
    pub fn init_profiling(backend: ProfilingBackend) {
        match backend {
            ProfilingBackend::PuffinHttp => {
                // Enable puffin profiling
                puffin::set_scopes_on(true);

                // Start the puffin server on the default port (8585)
                match puffin_http::Server::new("0.0.0.0:8585") {
                    Ok(server) => {
                        tracing::info!("Puffin profiler server started on http://0.0.0.0:8585");
                        tracing::info!(
                            "Connect puffin_viewer or open browser to view profiling data"
                        );

                        // Store the server in a static to keep it alive
                        let _ = PROFILING_SERVER.set(server);
                    }
                    Err(e) => {
                        tracing::error!("Failed to start puffin server: {}", e);
                    }
                }
            }
        }
    }

    /// Mark the start of a new frame for profiling.
    ///
    /// Call this once per frame in your main loop to organize profiling data by frame.
    ///
    /// # Example
    /// ```no_run
    /// use astrelis_core::profiling::new_frame;
    ///
    /// loop {
    ///     new_frame();
    ///     // ... your frame code ...
    /// #   break;
    /// }
    /// ```
    #[inline]
    pub fn new_frame() {
        puffin::GlobalProfiler::lock().new_frame();
    }

    /// Finish profiling for the current scope and optionally upload data.
    ///
    /// This is useful when you want to ensure profiling data is sent to the viewer
    /// at specific points (e.g., end of frame).
    #[inline]
    pub fn finish_frame() {
        // The puffin_http server automatically handles data transmission
        // This is just a marker for semantic clarity
        puffin::GlobalProfiler::lock().new_frame();
    }
}

#[cfg(feature = "profiling")]
pub use enabled::*;

// ============================================================================
// Feature: profiling DISABLED (no-op stubs)
// ============================================================================
#[cfg(not(feature = "profiling"))]
mod disabled {
    /// No-op replacement for `puffin::profile_function!()`.
    #[macro_export]
    macro_rules! profile_function {
        () => {};
        ($data:expr) => {};
    }

    /// No-op replacement for `puffin::profile_scope!()`.
    #[macro_export]
    macro_rules! profile_scope {
        ($name:expr) => {};
        ($name:expr, $data:expr) => {};
    }

    /// Profiling backend options (no-op when profiling is disabled).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ProfilingBackend {
        /// Send profiling data to puffin_viewer via HTTP.
        PuffinHttp,
    }

    /// No-op: Initialize profiling with the specified backend.
    #[inline]
    pub fn init_profiling(_backend: ProfilingBackend) {}

    /// No-op: Mark the start of a new frame for profiling.
    #[inline]
    pub fn new_frame() {}

    /// No-op: Finish profiling for the current scope.
    #[inline]
    pub fn finish_frame() {}

    // Re-export the macros so they can be used via `use astrelis_core::profiling::profile_function;`
    pub use profile_function;
    pub use profile_scope;
}

#[cfg(not(feature = "profiling"))]
pub use disabled::*;
