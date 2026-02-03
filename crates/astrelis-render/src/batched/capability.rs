//! Render capability types for the batched renderer tiers.
//!
//! Each struct represents a specific tier's GPU requirements.
//! Use [`BestBatchCapability2D`] for auto-detection with graceful degradation.

use crate::capability::{GpuRequirements, RenderCapability};
use crate::features::GpuFeatures;

use super::BINDLESS_MAX_TEXTURES;

/// Capability for the Direct (Tier 1) batch renderer.
///
/// No special GPU features required — works on all hardware.
pub struct DirectBatchCapability2D;

impl RenderCapability for DirectBatchCapability2D {
    fn requirements() -> GpuRequirements {
        GpuRequirements::none()
    }

    fn name() -> &'static str {
        "DirectBatchCapability2D (Tier 1)"
    }
}

/// Capability for the Indirect (Tier 2) batch renderer.
///
/// Requires `INDIRECT_FIRST_INSTANCE` for `multi_draw_indirect()` with per-instance offsets.
pub struct IndirectBatchCapability2D;

impl RenderCapability for IndirectBatchCapability2D {
    fn requirements() -> GpuRequirements {
        GpuRequirements::new()
            .require_features(GpuFeatures::INDIRECT_FIRST_INSTANCE)
    }

    fn name() -> &'static str {
        "IndirectBatchCapability2D (Tier 2)"
    }
}

/// Capability for the Bindless (Tier 3) batch renderer.
///
/// Requires indirect draw, texture binding arrays, partial binding, and
/// non-uniform indexing. Also requires elevated `max_binding_array_elements_per_shader_stage`.
pub struct BindlessBatchCapability2D;

impl RenderCapability for BindlessBatchCapability2D {
    fn requirements() -> GpuRequirements {
        GpuRequirements::new()
            .require_features(
                GpuFeatures::INDIRECT_FIRST_INSTANCE
                    | GpuFeatures::TEXTURE_BINDING_ARRAY
                    | GpuFeatures::PARTIALLY_BOUND_BINDING_ARRAY
                    | GpuFeatures::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING,
            )
            .with_min_limits(|l| {
                l.max_binding_array_elements_per_shader_stage = BINDLESS_MAX_TEXTURES;
            })
    }

    fn name() -> &'static str {
        "BindlessBatchCapability2D (Tier 3)"
    }
}

/// Capability for auto-detecting the best batch renderer tier.
///
/// All features are **requested** (not required), so device creation succeeds
/// on any hardware. At runtime, [`super::detect_render_tier`] picks the
/// highest supported tier.
///
/// This is the recommended capability for most applications.
pub struct BestBatchCapability2D;

impl RenderCapability for BestBatchCapability2D {
    fn requirements() -> GpuRequirements {
        GpuRequirements::new()
            .request_features(
                GpuFeatures::INDIRECT_FIRST_INSTANCE
                    | GpuFeatures::TEXTURE_BINDING_ARRAY
                    | GpuFeatures::PARTIALLY_BOUND_BINDING_ARRAY
                    | GpuFeatures::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING,
            )
            .with_min_limits(|l| {
                l.max_binding_array_elements_per_shader_stage = BINDLESS_MAX_TEXTURES;
            })
    }

    fn name() -> &'static str {
        "BestBatchCapability2D (auto-detect)"
    }
}
