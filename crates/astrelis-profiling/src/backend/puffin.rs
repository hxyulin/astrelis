//! Puffin profiling backend.
//!
//! Enables CPU profiling via the [puffin](https://crates.io/crates/puffin)
//! crate with an HTTP viewer server for live inspection.

/// Initializes the puffin profiler and starts the HTTP viewer server.
///
/// The viewer is accessible at `http://localhost:8585` by default.
pub fn init() {
    puffin::set_scopes_on(true);

    // Start the puffin HTTP server for the viewer.
    let server_addr = format!("0.0.0.0:{}", puffin_http::DEFAULT_PORT);
    let _server = puffin_http::Server::new(&server_addr).ok();
}

/// Signals a frame boundary to puffin.
///
/// Call this once per frame (e.g., at the start of the main loop iteration)
/// so puffin can separate per-frame profiling data.
#[inline]
pub fn new_frame() {
    puffin::GlobalProfiler::lock().new_frame();
}

/// Shuts down the puffin profiler.
pub fn finish() {
    puffin::set_scopes_on(false);
}
