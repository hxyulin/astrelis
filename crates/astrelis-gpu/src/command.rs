//! Command recording traits and types.
//!
//! GPU work is recorded into command encoders and submitted to the queue.
//! Render and compute passes are scoped within an encoder.

use std::ops::Range;

use astrelis_core::color::Color;

use crate::bind_group::ShaderStages;
use crate::id::{
    BindGroupId, BufferId, ComputePipelineId, RenderPipelineId, TextureId, TextureViewId,
};
use crate::texture::Extent3d;
use crate::types::{IndexFormat, LoadOp, StoreOp};

/// A color attachment for a render pass.
#[derive(Clone, Debug)]
pub struct ColorAttachment {
    /// Texture view to render into.
    pub view: TextureViewId,
    /// Texture view to resolve multisample data into. `None` = no resolve.
    pub resolve_target: Option<TextureViewId>,
    /// Operation at the start of the pass.
    pub load_op: LoadOp<Color>,
    /// Operation at the end of the pass.
    pub store_op: StoreOp,
}

/// A depth-stencil attachment for a render pass.
#[derive(Clone, Debug)]
pub struct DepthStencilAttachment {
    /// Depth/stencil texture view.
    pub view: TextureViewId,
    /// Depth load operation.
    pub depth_load_op: LoadOp<f32>,
    /// Depth store operation.
    pub depth_store_op: StoreOp,
    /// If `true`, depth is read-only (no writes).
    pub depth_read_only: bool,
}

/// Describes a render pass.
#[derive(Clone, Debug)]
pub struct RenderPassDescriptor<'a> {
    /// Debug label.
    pub label: Option<&'a str>,
    /// Color attachments.
    pub color_attachments: &'a [ColorAttachment],
    /// Depth/stencil attachment. `None` = no depth/stencil.
    pub depth_stencil_attachment: Option<DepthStencilAttachment>,
}

/// Source/destination for buffer copies.
#[derive(Clone, Debug)]
pub struct BufferCopyView {
    /// Buffer handle.
    pub buffer: BufferId,
    /// Byte offset into the buffer.
    pub offset: u64,
    /// Bytes per row of image data. Required for texture copies.
    pub bytes_per_row: Option<u32>,
    /// Number of rows per image slice. Required for 3D/array copies.
    pub rows_per_image: Option<u32>,
}

/// Source/destination for texture copies.
#[derive(Clone, Debug)]
pub struct TextureCopyView {
    /// Texture handle.
    pub texture: TextureId,
    /// Mip level of the texture.
    pub mip_level: u32,
    /// Origin within the texture.
    pub origin: Origin3d,
}

/// 3D origin point for copy operations.
#[derive(Clone, Copy, Debug, Default)]
pub struct Origin3d {
    /// X offset.
    pub x: u32,
    /// Y offset.
    pub y: u32,
    /// Z offset (or array layer).
    pub z: u32,
}

/// Records render commands within a pass.
///
/// Obtained from [`CommandEncoder::begin_render_pass`]. The pass ends
/// when this value is dropped.
pub trait RenderPass {
    /// Sets the active render pipeline.
    fn set_pipeline(&mut self, pipeline: RenderPipelineId);

    /// Binds a bind group at the given index.
    fn set_bind_group(&mut self, index: u32, bind_group: BindGroupId, offsets: &[u32]);

    /// Binds a vertex buffer to the given slot.
    fn set_vertex_buffer(&mut self, slot: u32, buffer: BufferId, offset: u64, size: Option<u64>);

    /// Binds the index buffer.
    fn set_index_buffer(
        &mut self,
        buffer: BufferId,
        format: IndexFormat,
        offset: u64,
        size: Option<u64>,
    );

    /// Sets the viewport rectangle.
    fn set_viewport(&mut self, x: f32, y: f32, w: f32, h: f32, min_depth: f32, max_depth: f32);

    /// Sets the scissor rectangle.
    fn set_scissor_rect(&mut self, x: u32, y: u32, w: u32, h: u32);

    /// Sets the blend constant color.
    fn set_blend_constant(&mut self, color: Color);

    /// Sets the stencil reference value.
    fn set_stencil_reference(&mut self, reference: u32);

    /// Draws non-indexed primitives.
    fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>);

    /// Draws indexed primitives.
    fn draw_indexed(&mut self, indices: Range<u32>, base_vertex: i32, instances: Range<u32>);

    /// Sets push constant data for the given stages.
    fn set_push_constants(&mut self, stages: ShaderStages, offset: u32, data: &[u8]);
}

/// Records compute commands within a pass.
///
/// Obtained from [`CommandEncoder::begin_compute_pass`]. The pass ends
/// when this value is dropped.
pub trait ComputePass {
    /// Sets the active compute pipeline.
    fn set_pipeline(&mut self, pipeline: ComputePipelineId);

    /// Binds a bind group at the given index.
    fn set_bind_group(&mut self, index: u32, bind_group: BindGroupId, offsets: &[u32]);

    /// Dispatches compute work groups.
    fn dispatch(&mut self, x: u32, y: u32, z: u32);

    /// Dispatches compute work groups using an indirect buffer.
    fn dispatch_indirect(&mut self, buffer: BufferId, offset: u64);

    /// Sets push constant data.
    fn set_push_constants(&mut self, offset: u32, data: &[u8]);
}

/// Encodes GPU commands into a command buffer.
///
/// Created via [`GpuDevice::create_command_encoder`](crate::device::GpuDevice::create_command_encoder).
/// After recording, submit the encoder via [`GpuQueue::submit`](crate::queue::GpuQueue::submit).
pub trait CommandEncoder {
    /// The render pass type produced by this encoder.
    type RenderPass<'pass>: RenderPass
    where
        Self: 'pass;
    /// The compute pass type produced by this encoder.
    type ComputePass<'pass>: ComputePass
    where
        Self: 'pass;

    /// Begins a render pass.
    ///
    /// The returned pass borrows this encoder exclusively; drop the pass
    /// to end it before starting another pass or submitting.
    fn begin_render_pass<'pass>(
        &'pass mut self,
        desc: &RenderPassDescriptor<'_>,
    ) -> Self::RenderPass<'pass>;

    /// Begins a compute pass.
    fn begin_compute_pass<'pass>(
        &'pass mut self,
        label: Option<&str>,
    ) -> Self::ComputePass<'pass>;

    /// Copies data between buffers.
    fn copy_buffer_to_buffer(
        &mut self,
        src: BufferId,
        src_offset: u64,
        dst: BufferId,
        dst_offset: u64,
        size: u64,
    );

    /// Copies data from a buffer to a texture.
    fn copy_buffer_to_texture(
        &mut self,
        src: BufferCopyView,
        dst: TextureCopyView,
        size: Extent3d,
    );

    /// Copies data from a texture to a buffer.
    fn copy_texture_to_buffer(
        &mut self,
        src: TextureCopyView,
        dst: BufferCopyView,
        size: Extent3d,
    );

    /// Copies data between textures.
    fn copy_texture_to_texture(
        &mut self,
        src: TextureCopyView,
        dst: TextureCopyView,
        size: Extent3d,
    );
}
