//! The `BatchRenderer2D` trait defining the interface for all render tiers.

use super::types::{BatchRenderStats2D, DrawBatch2D, RenderTier};

/// Trait implemented by all three render tier backends.
///
/// The lifecycle is: `prepare()` uploads data to the GPU, then `render()` records
/// draw commands into a render pass. The caller manages render pass creation and
/// depth buffer clearing.
pub trait BatchRenderer2D: Send {
    /// Returns the render tier of this backend.
    fn tier(&self) -> RenderTier;

    /// Prepare GPU resources for the given batch.
    ///
    /// This sorts instances into opaque/transparent groups, uploads instance data,
    /// builds indirect buffers (Tier 2-3), and updates bind groups as needed.
    fn prepare(&mut self, batch: &DrawBatch2D);

    /// Record draw commands into the given render pass.
    ///
    /// The render pass must have been created with a depth-stencil attachment
    /// (Depth32Float, depth_compare: GreaterEqual) and appropriate color target.
    /// The caller is responsible for clearing the depth buffer before calling this.
    fn render(&self, pass: &mut wgpu::RenderPass<'_>);

    /// Returns statistics from the last `prepare()` + `render()` cycle.
    fn stats(&self) -> BatchRenderStats2D;
}
