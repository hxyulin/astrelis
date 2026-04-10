//! GPU profiling capabilities and tier detection.
//!
//! GPU profiling support varies widely across platforms and graphics backends.
//! This module defines a tier system that allows engine code to query what
//! level of GPU profiling is available and adapt accordingly.
//!
//! # Tier System
//!
//! | Tier | Capability | Platforms |
//! |------|-----------|-----------|
//! | [`None`](GpuProfilingTier::None) | No GPU timestamps | WebGL, GLES3 |
//! | [`Basic`](GpuProfilingTier::Basic) | Pass start/end timestamps | WebGPU, most mobile |
//! | [`Encoder`](GpuProfilingTier::Encoder) | Between-pass timestamps | Desktop Vulkan/DX12/Metal |
//! | [`Pass`](GpuProfilingTier::Pass) | In-pass timestamps | Vulkan/DX12, Metal (AMD/Intel) |

/// GPU profiling capability tier.
///
/// Detected at runtime based on the GPU adapter's feature support.
/// Higher tiers provide finer-grained timing data but are available
/// on fewer platforms.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum GpuProfilingTier {
    /// No GPU timing available.
    ///
    /// CPU-side `profile_scope!` markers are the only profiling option.
    /// Platforms: WebGL, OpenGL ES without timer query extensions.
    None = 0,

    /// Timestamps at pass start and end.
    ///
    /// Uses `RenderPassDescriptor::timestamp_writes` /
    /// `ComputePassDescriptor::timestamp_writes`. Requires the
    /// `TIMESTAMP_QUERY` feature.
    ///
    /// Platforms: Vulkan, DX12, Metal, desktop OpenGL, WebGPU (quantized to ~100us).
    Basic = 1,

    /// Timestamps between passes on command encoders.
    ///
    /// Allows `encoder.write_timestamp()` at arbitrary points between passes.
    /// Requires `TIMESTAMP_QUERY_INSIDE_ENCODERS`.
    ///
    /// Platforms: Vulkan, DX12, Metal (including Apple Silicon), desktop OpenGL.
    /// Not available on WebGPU.
    Encoder = 2,

    /// Timestamps inside render and compute passes.
    ///
    /// Allows `render_pass.write_timestamp()` around individual draw/dispatch
    /// calls. Requires `TIMESTAMP_QUERY_INSIDE_PASSES`.
    ///
    /// Platforms: Vulkan, DX12, desktop OpenGL, Metal (AMD/Intel only).
    /// Not available on Apple Silicon (tile-based GPU) or WebGPU.
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
