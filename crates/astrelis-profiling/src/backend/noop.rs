//! No-op profiling backend.
//!
//! All functions compile to nothing. This is the default when no
//! backend feature is enabled.

use crate::data::{CounterValue, GpuScope};

/// No-op initialization.
#[inline(always)]
pub fn init() {}

/// No-op frame boundary signal.
#[inline(always)]
pub fn new_frame() {}

/// No-op shutdown.
#[inline(always)]
pub fn finish() {}

/// No-op thread naming.
#[inline(always)]
pub fn set_thread_name(_name: &str) {}

/// No-op GPU scope reporting.
#[inline(always)]
pub fn report_gpu_scopes(_scopes: &[GpuScope]) {}

/// No-op counter recording.
#[inline(always)]
pub fn record_counter(_category: &'static str, _name: &'static str, _value: CounterValue) {}

/// No-op plot recording.
#[inline(always)]
pub fn record_plot(_name: &'static str, _value: f64) {}
