//! Converts raw GPU timestamp query results into [`GpuScope`] values
//! and reports them to the active profiling backend.

use astrelis_profiling::data::GpuScope;

use super::PendingScope;

/// Converts pending scopes and raw timestamps into engine-agnostic [`GpuScope`]
/// values and reports them to the profiling backend.
pub(crate) fn report_results(
    pending_scopes: &[PendingScope],
    timestamps: &[u64],
    timestamp_period_ns: f32,
) {
    if pending_scopes.is_empty() || timestamps.is_empty() {
        return;
    }

    let scopes: Vec<GpuScope> = pending_scopes
        .iter()
        .filter_map(|scope| {
            let start_idx = scope.pair.start_index as usize;
            let end_idx = scope.pair.end_index as usize;

            if start_idx >= timestamps.len() || end_idx >= timestamps.len() {
                return None;
            }

            let start_raw = timestamps[start_idx];
            let end_raw = timestamps[end_idx];

            // Skip scopes with zero/invalid timestamps.
            if start_raw == 0 && end_raw == 0 {
                return None;
            }

            let start_ns = (start_raw as f64 * timestamp_period_ns as f64) as i64;
            let end_ns = (end_raw as f64 * timestamp_period_ns as f64) as i64;

            Some(GpuScope {
                label: scope.label.clone(),
                start_ns,
                end_ns,
                nested: Vec::new(),
            })
        })
        .collect();

    astrelis_profiling::gpu::report_gpu_scopes(&scopes);
}
