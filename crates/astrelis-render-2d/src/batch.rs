//! Batch sorting, grouping, and render tier abstraction.

use crate::instance::Instance2D;

/// Render tier describing GPU feature availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderTier {
    /// Tier 1: Per-texture-group `draw()` calls. Works on all hardware.
    Direct,
    /// Tier 2: `multi_draw_indirect()` per texture group.
    Indirect,
    /// Tier 3: Single `multi_draw_indirect()` per frame via bindless textures.
    Bindless,
}

impl std::fmt::Display for RenderTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderTier::Direct => write!(f, "Direct (Tier 1)"),
            RenderTier::Indirect => write!(f, "Indirect (Tier 2)"),
            RenderTier::Bindless => write!(f, "Bindless (Tier 3)"),
        }
    }
}

/// A complete draw batch submitted to the renderer each frame.
pub struct DrawBatch2D {
    /// Instances sorted by (z_depth, draw_type, texture_index).
    pub instances: Vec<Instance2D>,
    /// Texture views for this frame. Index 0 is the white pixel texture.
    pub textures: Vec<BatchTexture>,
    /// View-projection matrix from the camera.
    pub view_projection: [[f32; 4]; 4],
}

/// A texture bound for rendering in a batch.
pub struct BatchTexture {
    /// The texture view to bind.
    pub view: astrelis_gpu::TextureView,
    /// The sampler to use.
    pub sampler: astrelis_gpu::Sampler,
}

/// Statistics from the last render cycle.
#[derive(Debug, Clone, Copy, Default)]
pub struct BatchRenderStats {
    /// Total number of instances rendered.
    pub instance_count: u32,
    /// Number of GPU draw calls issued.
    pub draw_calls: u32,
    /// Number of texture bind switches.
    pub texture_switches: u32,
}

/// Trait implemented by all render tier backends.
///
/// The lifecycle is: `prepare()` uploads data to the GPU, then
/// `render()` records draw commands into a render pass.
pub trait BatchRenderer2D: Send {
    /// Returns the render tier of this backend.
    fn tier(&self) -> RenderTier;

    /// Prepare GPU resources for the given batch.
    fn prepare(&mut self, gpu: &astrelis_gpu::Gpu, batch: &DrawBatch2D);

    /// Record draw commands into the given render pass.
    fn render<'a>(&'a self, pass: &mut astrelis_gpu::RenderPass<'a>);

    /// Returns statistics from the last `prepare()` + `render()` cycle.
    fn stats(&self) -> BatchRenderStats;
}
