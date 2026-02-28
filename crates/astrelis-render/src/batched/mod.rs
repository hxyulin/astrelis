//! Batched deferred indirect renderer.
//!
//! Provides three runtime-selected render backends behind a unified [`BatchRenderer2D`] trait:
//!
//! | Tier | Name | Draw Strategy |
//! |------|------|---------------|
//! | 1 | Direct | Per-texture `draw()` calls |
//! | 2 | Indirect | `multi_draw_indirect()` per texture group |
//! | 3 | Bindless | Single `multi_draw_indirect()` per frame |
//!
//! All tiers share a unified [`UnifiedInstance2D`] format and a single shader pipeline.

#[allow(dead_code)]
mod bindless;
pub mod capability;
#[allow(dead_code)]
mod direct;
#[allow(dead_code)]
mod indirect;
#[allow(dead_code)]
mod pipeline;
#[allow(dead_code)]
mod texture_array;
mod traits;
mod types;

pub use capability::{
    BestBatchCapability2D, BindlessBatchCapability2D, DirectBatchCapability2D,
    IndirectBatchCapability2D,
};
pub use traits::*;
pub use types::*;

use std::sync::Arc;

use crate::context::GraphicsContext;
use crate::features::GpuFeatures;

/// Maximum number of textures in the bindless binding array (Tier 3).
const BINDLESS_MAX_TEXTURES: u32 = 256;

/// Detect the highest supported render tier for the given context.
pub fn detect_render_tier(context: &GraphicsContext) -> RenderTier {
    let features = context.gpu_features();

    let has_indirect_first_instance = features.contains(GpuFeatures::INDIRECT_FIRST_INSTANCE);
    let has_texture_binding_array = features.contains(GpuFeatures::TEXTURE_BINDING_ARRAY);
    let has_partially_bound = features.contains(GpuFeatures::PARTIALLY_BOUND_BINDING_ARRAY);
    let has_non_uniform_indexing = features
        .contains(GpuFeatures::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING);

    if has_indirect_first_instance
        && has_texture_binding_array
        && has_partially_bound
        && has_non_uniform_indexing
    {
        RenderTier::Bindless
    } else if has_indirect_first_instance {
        RenderTier::Indirect
    } else {
        RenderTier::Direct
    }
}

/// Create a 2D batch renderer for the given context.
///
/// Automatically selects the highest supported tier unless `tier_override` is specified.
/// The `surface_format` is used for pipeline color target configuration.
pub fn create_batch_renderer_2d(
    context: Arc<GraphicsContext>,
    surface_format: wgpu::TextureFormat,
    tier_override: Option<RenderTier>,
) -> Box<dyn BatchRenderer2D> {
    let tier = tier_override.unwrap_or_else(|| detect_render_tier(&context));

    tracing::info!("Creating batch renderer 2D: {tier}");

    match tier {
        RenderTier::Direct => Box::new(direct::DirectBatchRenderer2D::new(context, surface_format)),
        RenderTier::Indirect => Box::new(indirect::IndirectBatchRenderer2D::new(
            context,
            surface_format,
        )),
        RenderTier::Bindless => Box::new(bindless::BindlessBatchRenderer2D::new(
            context,
            surface_format,
        )),
    }
}
