//! GPU scope reporting bridge.
//!
//! GPU backend implementations (e.g., `astrelis-gpu-wgpu`) call
//! [`report_gpu_scopes`] after resolving timestamp queries each frame.
//! The active profiling backend displays these under a virtual "GPU" thread.

use crate::data::GpuScope;

/// Reports completed GPU profiling scopes from a prior frame.
///
/// GPU timestamp results typically arrive 1-3 frames behind the CPU due to
/// GPU buffering. The profiling backend handles this latency transparently.
///
/// No-op when no profiling backend is enabled.
///
/// # Example
///
/// ```ignore
/// // In a GPU backend implementation:
/// let scopes = convert_query_results(&raw_results);
/// astrelis_profiling::gpu::report_gpu_scopes(&scopes);
/// ```
pub fn report_gpu_scopes(scopes: &[GpuScope]) {
    crate::backend::report_gpu_scopes(scopes);
}
