//! Puffin profiling backend.
//!
//! Enables CPU profiling via the [puffin](https://crates.io/crates/puffin)
//! crate with an HTTP viewer server for live inspection. Also handles
//! GPU scope reporting under a virtual "GPU" thread and counter/plot data.

use std::sync::Mutex;

use puffin::{NanoSecond, ScopeDetails, StreamInfo, ThreadInfo};

use crate::data::{CounterValue, GpuScope};

/// Global handle to the puffin HTTP server, kept alive for the process lifetime.
static PUFFIN_SERVER: Mutex<Option<puffin_http::Server>> = Mutex::new(None);

/// Initializes the puffin profiler and starts the HTTP viewer server.
///
/// The viewer is accessible at `http://localhost:8585` by default.
pub fn init() {
    puffin::set_scopes_on(true);

    // Start the puffin HTTP server for the viewer.
    let server_addr = format!("0.0.0.0:{}", puffin_http::DEFAULT_PORT);
    match puffin_http::Server::new(&server_addr) {
        Ok(server) => {
            eprintln!("Puffin profiler listening on {server_addr}");
            *PUFFIN_SERVER.lock().unwrap() = Some(server);
        }
        Err(e) => {
            eprintln!("Failed to start puffin server on {server_addr}: {e}");
        }
    }
}

/// Signals a frame boundary to puffin.
///
/// Call this once per frame (e.g., at the start of the main loop iteration)
/// so puffin can separate per-frame profiling data.
#[inline]
pub fn new_frame() {
    puffin::GlobalProfiler::lock().new_frame();
}

/// Shuts down the puffin profiler and stops the HTTP server.
pub fn finish() {
    puffin::set_scopes_on(false);
    *PUFFIN_SERVER.lock().unwrap() = None;
}

/// Names the current thread for the puffin profiler.
///
/// Puffin 0.20 reads the thread name from [`std::thread::current().name()`],
/// so threads should be named via [`std::thread::Builder::name`] at spawn time.
/// This function is kept as a no-op for API consistency — use
/// [`crate::spawn_profiled`] to spawn threads with proper naming.
#[inline]
pub fn set_thread_name(_name: &str) {
    // Puffin 0.20 derives thread names from std::thread::current().name().
    // Thread naming must happen at spawn time via std::thread::Builder::name().
}

/// Reports GPU profiling scopes to puffin under a virtual "GPU" thread.
///
/// GPU scopes appear alongside CPU threads in the puffin viewer on port 8585.
/// The timing data is from a prior frame (GPU results are inherently delayed
/// by 1-3 frames).
pub fn report_gpu_scopes(scopes: &[GpuScope]) {
    if scopes.is_empty() {
        return;
    }

    let mut profiler = puffin::GlobalProfiler::lock();
    let mut stream_info = StreamInfo::default();
    collect_gpu_scopes_recursive(&mut profiler, &mut stream_info, scopes, 0);

    if stream_info.num_scopes > 0 {
        profiler.report_user_scopes(
            ThreadInfo {
                start_time_ns: None,
                name: "GPU".to_string(),
            },
            &stream_info.as_stream_into_ref(),
        );
    }
}

fn collect_gpu_scopes_recursive(
    profiler: &mut puffin::GlobalProfiler,
    stream_info: &mut StreamInfo,
    scopes: &[GpuScope],
    depth: usize,
) {
    let details: Vec<_> = scopes
        .iter()
        .map(|s| ScopeDetails::from_scope_name(s.label.clone()))
        .collect();
    let ids = profiler.register_user_scopes(&details);

    for (scope, id) in scopes.iter().zip(ids) {
        let start = scope.start_ns as NanoSecond;
        let end = scope.end_ns as NanoSecond;

        stream_info.depth = stream_info.depth.max(depth + 1);
        stream_info.num_scopes += 1;
        stream_info.range_ns.0 = stream_info.range_ns.0.min(start);
        stream_info.range_ns.1 = stream_info.range_ns.1.max(end);

        let (offset, _) = stream_info.stream.begin_scope(|| start, id, "");
        collect_gpu_scopes_recursive(profiler, stream_info, &scope.nested, depth + 1);
        stream_info.stream.end_scope(offset, end);
    }
}

/// Records a counter value. Puffin has limited counter support, so this
/// logs the value as a scope data string for now.
#[inline]
pub fn record_counter(_category: &'static str, _name: &'static str, _value: CounterValue) {
    // Puffin does not have native counter support.
    // Future: could report as a custom scope or use puffin's data field.
}

/// Records a plot value. Puffin has limited plot support.
#[inline]
pub fn record_plot(_name: &'static str, _value: f64) {
    // Puffin does not have native plot support.
    // Future: could integrate with a custom viewer.
}
