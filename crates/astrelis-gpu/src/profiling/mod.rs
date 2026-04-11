//! GPU profiling support.
//!
//! Uses `wgpu-profiler` for GPU timestamp queries and forwards
//! processed results to the global `astrelis-profiling` timeline.
//! GPU profiling is always on: the adapter's supported tier is
//! detected at device creation (see [`tier`]) and features are
//! requested accordingly; adapters with no timestamp support
//! produce no GPU data but still work normally.

pub(crate) mod tier;

/// GPU profiling capability tier.
///
/// Detected at runtime based on the GPU adapter's feature support.
/// Higher tiers provide finer-grained timing data but are available
/// on fewer platforms.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum GpuProfilingTier {
    /// No GPU timing available.
    None = 0,
    /// Timestamps at pass start and end.
    Basic = 1,
    /// Timestamps between passes on command encoders.
    Encoder = 2,
    /// Timestamps inside render and compute passes.
    Pass = 3,
}

/// GPU profiling capabilities detected at runtime.
#[derive(Clone, Debug)]
pub struct GpuProfilingCapabilities {
    /// The highest supported profiling tier.
    pub tier: GpuProfilingTier,
    /// Timestamp period in nanoseconds per tick.
    ///
    /// Multiply raw GPU timestamp values by this to get nanoseconds.
    /// Zero when `tier` is [`GpuProfilingTier::None`].
    pub timestamp_period_ns: f32,
}
