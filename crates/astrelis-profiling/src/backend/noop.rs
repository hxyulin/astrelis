//! No-op profiling backend.
//!
//! All functions compile to nothing. This is the default when no
//! backend feature is enabled.

/// No-op initialization.
#[inline(always)]
pub fn init() {}

/// No-op frame boundary signal.
#[inline(always)]
pub fn new_frame() {}

/// No-op shutdown.
#[inline(always)]
pub fn finish() {}
