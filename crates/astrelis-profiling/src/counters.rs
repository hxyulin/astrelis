//! Counter and plot recording API.
//!
//! Counters track named integer/float values over time. Plots record
//! named floating-point time series. Both are reported to the active
//! profiling backend for display.

use crate::data::CounterValue;

/// Records a counter value under the given category and name.
///
/// Counters are useful for tracking GPU memory usage, object counts,
/// cache statistics, and other discrete metrics.
///
/// No-op when no profiling backend is enabled.
///
/// # Example
///
/// ```ignore
/// astrelis_profiling::counters::record_counter("gpu_memory", "buffer_bytes", 1024u64);
/// ```
#[inline(always)]
pub fn record_counter(category: &'static str, name: &'static str, value: impl Into<CounterValue>) {
    crate::backend::record_counter(category, name, value.into());
}

/// Records a plot value for the given named time series.
///
/// Plots display as continuous line graphs in the profiler viewer.
/// Useful for frame time, FPS, temperature, or any continuous metric.
///
/// No-op when no profiling backend is enabled.
///
/// # Example
///
/// ```ignore
/// astrelis_profiling::counters::record_plot("frame_time_ms", 16.3);
/// ```
#[inline(always)]
pub fn record_plot(name: &'static str, value: f64) {
    crate::backend::record_plot(name, value);
}
