//! GPU profiling tier detection from adapter features.

use crate::profiling::GpuProfilingTier;

/// All wgpu timestamp query features, ordered from most to least capable.
const TIMESTAMP_FEATURES: &[(wgpu::Features, GpuProfilingTier)] = &[
    (
        wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES,
        GpuProfilingTier::Pass,
    ),
    (
        wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS,
        GpuProfilingTier::Encoder,
    ),
    (
        wgpu::Features::TIMESTAMP_QUERY,
        GpuProfilingTier::Basic,
    ),
];

/// Detects the highest GPU profiling tier supported by the adapter.
pub(crate) fn detect_tier(adapter: &wgpu::Adapter) -> GpuProfilingTier {
    let features = adapter.features();
    for &(feature, tier) in TIMESTAMP_FEATURES {
        if features.contains(feature) {
            return tier;
        }
    }
    GpuProfilingTier::None
}

/// Returns the wgpu features required for the given profiling tier.
pub(crate) fn required_features(tier: GpuProfilingTier) -> wgpu::Features {
    let mut features = wgpu::Features::empty();
    for &(feature, feature_tier) in TIMESTAMP_FEATURES {
        if feature_tier <= tier {
            features |= feature;
        }
    }
    features
}
