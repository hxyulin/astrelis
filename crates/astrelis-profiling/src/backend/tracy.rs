//! Tracy profiling backend.
//!
//! Enables high-performance profiling via the [Tracy](https://github.com/wolfpld/tracy)
//! profiler. Supports CPU spans, GPU timelines, plots, counters, and thread naming.
//!
//! Connect the Tracy profiler GUI to view live profiling data.

use std::sync::OnceLock;

use tracy_client::Client;

use crate::data::{CounterValue, GpuScope};

/// Global Tracy GPU context, created during [`init`].
///
/// Tracy GPU contexts represent a logical timeline in the profiler viewer.
/// We use a single context for all GPU work, matching the engine's single
/// graphics queue.
static GPU_CONTEXT: OnceLock<tracy_client::GpuContext> = OnceLock::new();

/// Initializes the Tracy profiler client.
///
/// Tracy is always-on when the feature is enabled — the client starts
/// automatically. This function creates the GPU context for timestamp
/// reporting.
pub fn init() {
    let client = Client::start();

    // Create a GPU context for reporting GPU timestamps.
    // period = 1.0 because our GpuScope timestamps are already in nanoseconds.
    // gpu_timestamp = 0 as the initial calibration point.
    match client.new_gpu_context(
        Some("GPU"),
        tracy_client::GpuContextType::Vulkan,
        0,
        1.0,
    ) {
        Ok(ctx) => {
            let _ = GPU_CONTEXT.set(ctx);
        }
        Err(e) => {
            eprintln!("Failed to create Tracy GPU context: {e:?}");
        }
    }

    eprintln!("Tracy profiler enabled — connect the Tracy GUI to capture data");
}

/// Signals a frame boundary to Tracy.
///
/// Call this once per frame (e.g., at the start of the main loop iteration)
/// so Tracy can separate per-frame data in the timeline view.
#[inline]
pub fn new_frame() {
    tracy_client::frame_mark();
}

/// Shuts down the Tracy profiler.
///
/// Tracy handles cleanup automatically when the client is dropped,
/// but this provides an explicit shutdown point for symmetry with [`init`].
pub fn finish() {
    // Tracy client lifetime is managed automatically.
}

/// Names the current thread for the Tracy profiler.
///
/// The thread name appears in the Tracy timeline view alongside its spans.
#[inline]
pub fn set_thread_name(name: &str) {
    if let Some(client) = Client::running() {
        client.set_thread_name(name);
    }
}

/// Reports GPU profiling scopes to Tracy under a dedicated GPU timeline.
///
/// GPU scopes appear as a separate GPU context in the Tracy viewer with
/// proper CPU-GPU correlation. The timing data is from a prior frame
/// (GPU results are inherently delayed by 1-3 frames).
pub fn report_gpu_scopes(scopes: &[GpuScope]) {
    if scopes.is_empty() {
        return;
    }

    let Some(gpu_ctx) = GPU_CONTEXT.get() else {
        return;
    };

    report_gpu_scopes_recursive(gpu_ctx, scopes);
}

/// Recursively reports nested GPU scopes to Tracy using native GPU zones.
fn report_gpu_scopes_recursive(gpu_ctx: &tracy_client::GpuContext, scopes: &[GpuScope]) {
    for scope in scopes {
        match gpu_ctx.span_alloc(&scope.label, "", file!(), line!()) {
            Ok(gpu_span) => {
                gpu_span.upload_timestamp_start(scope.start_ns);

                // Report nested scopes before uploading end timestamp,
                // as Tracy requires monotonically increasing timestamps.
                report_gpu_scopes_recursive(gpu_ctx, &scope.nested);

                gpu_span.upload_timestamp_end(scope.end_ns);
                // GpuSpan::drop handles end_zone automatically.
            }
            Err(_) => {
                // GPU span creation can fail if Tracy is not connected.
                // Silently skip — this is expected during startup/shutdown.
            }
        }
    }
}

/// Records a counter value as a Tracy plot.
///
/// Tracy does not distinguish counters from plots — both are displayed as
/// time-series graphs. The category is not used by Tracy; only the name
/// appears in the viewer.
///
/// Note: For compile-time plot names, prefer using the [`profile_counter!`]
/// macro which calls `tracy_client::plot!` directly with the literal name.
/// This function exists for programmatic callers with dynamic names and
/// falls back to Tracy's message API.
#[inline]
pub fn record_counter(category: &'static str, name: &'static str, value: CounterValue) {
    let plot_value: f64 = match value {
        CounterValue::U64(v) => v as f64,
        CounterValue::I64(v) => v as f64,
        CounterValue::F64(v) => v,
    };

    if let Some(client) = Client::running() {
        client.message(&format!("[counter] {category}/{name}: {plot_value}"), 0);
    }
}

/// Records a plot value in the Tracy timeline.
///
/// Note: For compile-time plot names, prefer using the [`profile_plot!`]
/// macro which calls `tracy_client::plot!` directly with the literal name.
/// This function exists for programmatic callers with dynamic names and
/// falls back to Tracy's message API.
#[inline]
pub fn record_plot(name: &'static str, value: f64) {
    if let Some(client) = Client::running() {
        client.message(&format!("[plot] {name}: {value}"), 0);
    }
}
