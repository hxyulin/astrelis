//! Command recording types.
//!
//! GPU work is recorded into command encoders and submitted via
//! [`Gpu::submit`](crate::backend::Gpu::submit). Render and compute
//! passes are scoped within an encoder.

use std::ops::Range;

use astrelis_core::color::Color;

use crate::bind_group::ShaderStages;
use crate::convert::types as conv;
use crate::device::GpuDevice;
use crate::profiling::query_pool::TimestampPair;
use crate::profiling::GpuProfilingTier;
use crate::resources::*;
use crate::texture::Extent3d;
use crate::types::{IndexFormat, LoadOp, StoreOp};

/// A color attachment for a render pass.
pub struct ColorAttachment<'a> {
    /// Texture view to render into.
    pub view: &'a TextureView,
    /// Texture view to resolve multisample data into. `None` = no resolve.
    pub resolve_target: Option<&'a TextureView>,
    /// Operation at the start of the pass.
    pub load_op: LoadOp<Color>,
    /// Operation at the end of the pass.
    pub store_op: StoreOp,
}

/// A depth-stencil attachment for a render pass.
pub struct DepthStencilAttachment<'a> {
    /// Depth/stencil texture view.
    pub view: &'a TextureView,
    /// Depth load operation.
    pub depth_load_op: LoadOp<f32>,
    /// Depth store operation.
    pub depth_store_op: StoreOp,
    /// If `true`, depth is read-only (no writes).
    pub depth_read_only: bool,
}

/// Describes a render pass.
pub struct RenderPassDescriptor<'a> {
    /// Debug label.
    pub label: Option<&'a str>,
    /// Color attachments.
    pub color_attachments: &'a [ColorAttachment<'a>],
    /// Depth/stencil attachment. `None` = no depth/stencil.
    pub depth_stencil_attachment: Option<DepthStencilAttachment<'a>>,
}

/// Source/destination for buffer copies.
pub struct BufferCopyView<'a> {
    /// Buffer reference.
    pub buffer: &'a Buffer,
    /// Byte offset into the buffer.
    pub offset: u64,
    /// Bytes per row of image data. Required for texture copies.
    pub bytes_per_row: Option<u32>,
    /// Number of rows per image slice. Required for 3D/array copies.
    pub rows_per_image: Option<u32>,
}

/// Source/destination for texture copies.
pub struct TextureCopyView<'a> {
    /// Texture reference.
    pub texture: &'a Texture,
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

/// Records GPU commands into a command buffer.
///
/// Created via [`GpuDevice::create_command_encoder`]. After recording,
/// submit via [`Gpu::submit`](crate::backend::Gpu::submit).
pub struct CommandEncoder<'a> {
    pub(crate) encoder: Option<wgpu::CommandEncoder>,
    pub(crate) device: &'a GpuDevice,
    /// The most recent scope whose end timestamp hasn't been written yet.
    open_scope: Option<TimestampPair>,
}

impl<'a> CommandEncoder<'a> {
    pub(crate) fn new(encoder: wgpu::CommandEncoder, device: &'a GpuDevice) -> Self {
        Self {
            encoder: Some(encoder),
            device,
            open_scope: None,
        }
    }

    /// Writes the end timestamp for the currently open scope, if any.
    fn close_open_scope(&mut self) {
        if let Some(pair) = self.open_scope.take() {
            let profiler = self.device.gpu_profiler.lock().unwrap();
            if profiler.tier() >= GpuProfilingTier::Encoder
                && let Some(query_set) = profiler.active_query_set()
            {
                let encoder = self.encoder.as_mut().expect("encoder already consumed");
                encoder.write_timestamp(query_set, pair.end_index);
            }
        }
    }

    /// Consumes the encoder and returns the finished command buffer.
    pub(crate) fn finish(mut self) -> wgpu::CommandBuffer {
        astrelis_profiling::profile_function!();
        self.close_open_scope();

        let mut encoder = self.encoder.take().expect("encoder already consumed");

        // Resolve all queries in the active pool.
        {
            let mut profiler = self.device.gpu_profiler.lock().unwrap();
            profiler.resolve_frame(&mut encoder);
        }

        encoder.finish()
    }

    /// Begins a render pass.
    ///
    /// The returned pass borrows this encoder exclusively; drop the pass
    /// to end it before starting another pass or submitting.
    pub fn begin_render_pass<'pass>(
        &'pass mut self,
        desc: &RenderPassDescriptor<'_>,
    ) -> RenderPass<'pass> {
        astrelis_profiling::profile_function!();

        self.close_open_scope();

        let encoder = self.encoder.as_mut().expect("encoder already consumed");
        let label = desc.label.unwrap_or("render_pass");

        // GPU profiling timestamp insertion.
        let mut profiler = self.device.gpu_profiler.lock().unwrap();
        let tier = profiler.tier();
        if tier >= GpuProfilingTier::Encoder {
            if let Some(pair) = profiler.begin_scope(label) {
                if let Some(query_set) = profiler.active_query_set() {
                    encoder.write_timestamp(query_set, pair.start_index);
                }
                self.open_scope = Some(pair);
            }
        } else if tier == GpuProfilingTier::Basic {
            profiler.begin_scope(label);
        }
        drop(profiler);

        RenderPass::new(encoder, desc)
    }

    /// Begins a compute pass.
    pub fn begin_compute_pass<'pass>(
        &'pass mut self,
        label: Option<&str>,
    ) -> ComputePass<'pass> {
        astrelis_profiling::profile_function!();

        self.close_open_scope();

        let encoder = self.encoder.as_mut().expect("encoder already consumed");
        let scope_label = label.unwrap_or("compute_pass");

        let mut profiler = self.device.gpu_profiler.lock().unwrap();
        let tier = profiler.tier();
        if tier >= GpuProfilingTier::Encoder {
            if let Some(pair) = profiler.begin_scope(scope_label) {
                if let Some(query_set) = profiler.active_query_set() {
                    encoder.write_timestamp(query_set, pair.start_index);
                }
                self.open_scope = Some(pair);
            }
        } else if tier == GpuProfilingTier::Basic {
            profiler.begin_scope(scope_label);
        }
        drop(profiler);

        ComputePass::new(encoder, label)
    }

    /// Copies data between buffers.
    pub fn copy_buffer_to_buffer(
        &mut self,
        src: &Buffer,
        src_offset: u64,
        dst: &Buffer,
        dst_offset: u64,
        size: u64,
    ) {
        astrelis_profiling::profile_function!();
        self.encoder
            .as_mut()
            .expect("encoder already consumed")
            .copy_buffer_to_buffer(&src.0, src_offset, &dst.0, dst_offset, size);
    }

    /// Copies data from a buffer to a texture.
    pub fn copy_buffer_to_texture(
        &mut self,
        src: BufferCopyView<'_>,
        dst: TextureCopyView<'_>,
        size: Extent3d,
    ) {
        astrelis_profiling::profile_function!();
        self.encoder
            .as_mut()
            .expect("encoder already consumed")
            .copy_buffer_to_texture(
                wgpu::TexelCopyBufferInfo {
                    buffer: &src.buffer.0,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: src.offset,
                        bytes_per_row: src.bytes_per_row,
                        rows_per_image: src.rows_per_image,
                    },
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &dst.texture.0,
                    mip_level: dst.mip_level,
                    origin: wgpu::Origin3d {
                        x: dst.origin.x,
                        y: dst.origin.y,
                        z: dst.origin.z,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                crate::convert::texture::extent3d(size),
            );
    }

    /// Copies data from a texture to a buffer.
    pub fn copy_texture_to_buffer(
        &mut self,
        src: TextureCopyView<'_>,
        dst: BufferCopyView<'_>,
        size: Extent3d,
    ) {
        astrelis_profiling::profile_function!();
        self.encoder
            .as_mut()
            .expect("encoder already consumed")
            .copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &src.texture.0,
                    mip_level: src.mip_level,
                    origin: wgpu::Origin3d {
                        x: src.origin.x,
                        y: src.origin.y,
                        z: src.origin.z,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &dst.buffer.0,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: dst.offset,
                        bytes_per_row: dst.bytes_per_row,
                        rows_per_image: dst.rows_per_image,
                    },
                },
                crate::convert::texture::extent3d(size),
            );
    }

    /// Copies data between textures.
    pub fn copy_texture_to_texture(
        &mut self,
        src: TextureCopyView<'_>,
        dst: TextureCopyView<'_>,
        size: Extent3d,
    ) {
        astrelis_profiling::profile_function!();
        self.encoder
            .as_mut()
            .expect("encoder already consumed")
            .copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &src.texture.0,
                    mip_level: src.mip_level,
                    origin: wgpu::Origin3d {
                        x: src.origin.x,
                        y: src.origin.y,
                        z: src.origin.z,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &dst.texture.0,
                    mip_level: dst.mip_level,
                    origin: wgpu::Origin3d {
                        x: dst.origin.x,
                        y: dst.origin.y,
                        z: dst.origin.z,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                crate::convert::texture::extent3d(size),
            );
    }
}

/// Records render commands within a pass.
///
/// The pass ends when this value is dropped.
pub struct RenderPass<'a> {
    pass: Option<wgpu::RenderPass<'a>>,
}

impl<'a> RenderPass<'a> {
    pub(crate) fn new(
        encoder: &'a mut wgpu::CommandEncoder,
        desc: &RenderPassDescriptor<'_>,
    ) -> Self {
        astrelis_profiling::profile_function!();

        let color_attachments: Vec<Option<wgpu::RenderPassColorAttachment<'_>>> = desc
            .color_attachments
            .iter()
            .map(|att| {
                Some(wgpu::RenderPassColorAttachment {
                    view: &att.view.0,
                    resolve_target: att.resolve_target.map(|v| &v.0),
                    ops: wgpu::Operations {
                        load: match att.load_op {
                            LoadOp::Clear(c) => wgpu::LoadOp::Clear(wgpu::Color {
                                r: c.r as f64,
                                g: c.g as f64,
                                b: c.b as f64,
                                a: c.a as f64,
                            }),
                            LoadOp::Load => wgpu::LoadOp::Load,
                        },
                        store: conv::store_op(att.store_op),
                    },
                    depth_slice: None,
                })
            })
            .collect();

        let depth_stencil_attachment =
            desc.depth_stencil_attachment.as_ref().map(|att| {
                wgpu::RenderPassDepthStencilAttachment {
                    view: &att.view.0,
                    depth_ops: Some(wgpu::Operations {
                        load: match att.depth_load_op {
                            LoadOp::Clear(v) => wgpu::LoadOp::Clear(v),
                            LoadOp::Load => wgpu::LoadOp::Load,
                        },
                        store: conv::store_op(att.depth_store_op),
                    }),
                    stencil_ops: None,
                }
            });

        let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: desc.label,
            color_attachments: &color_attachments,
            depth_stencil_attachment,
            ..Default::default()
        });

        Self {
            pass: Some(pass),
        }
    }

    fn pass_mut(&mut self) -> &mut wgpu::RenderPass<'a> {
        self.pass.as_mut().expect("render pass already ended")
    }

    /// Sets the active render pipeline.
    pub fn set_pipeline(&mut self, pipeline: &RenderPipeline) {
        self.pass_mut().set_pipeline(&pipeline.0);
    }

    /// Binds a bind group at the given index.
    pub fn set_bind_group(&mut self, index: u32, bind_group: &BindGroup, offsets: &[u32]) {
        self.pass_mut().set_bind_group(index, Some(&bind_group.0), offsets);
    }

    /// Binds a vertex buffer to the given slot.
    pub fn set_vertex_buffer(&mut self, slot: u32, buffer: &Buffer, offset: u64, size: Option<u64>) {
        let slice = match size {
            Some(s) => buffer.0.slice(offset..offset + s),
            None => buffer.0.slice(offset..),
        };
        self.pass_mut().set_vertex_buffer(slot, slice);
    }

    /// Binds the index buffer.
    pub fn set_index_buffer(
        &mut self,
        buffer: &Buffer,
        format: IndexFormat,
        offset: u64,
        size: Option<u64>,
    ) {
        let slice = match size {
            Some(s) => buffer.0.slice(offset..offset + s),
            None => buffer.0.slice(offset..),
        };
        self.pass_mut()
            .set_index_buffer(slice, conv::index_format(format));
    }

    /// Sets the viewport rectangle.
    pub fn set_viewport(&mut self, x: f32, y: f32, w: f32, h: f32, min_depth: f32, max_depth: f32) {
        self.pass_mut().set_viewport(x, y, w, h, min_depth, max_depth);
    }

    /// Sets the scissor rectangle.
    pub fn set_scissor_rect(&mut self, x: u32, y: u32, w: u32, h: u32) {
        self.pass_mut().set_scissor_rect(x, y, w, h);
    }

    /// Sets the blend constant color.
    pub fn set_blend_constant(&mut self, color: Color) {
        self.pass_mut().set_blend_constant(wgpu::Color {
            r: color.r as f64,
            g: color.g as f64,
            b: color.b as f64,
            a: color.a as f64,
        });
    }

    /// Sets the stencil reference value.
    pub fn set_stencil_reference(&mut self, reference: u32) {
        self.pass_mut().set_stencil_reference(reference);
    }

    /// Draws non-indexed primitives.
    pub fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>) {
        astrelis_profiling::profile_function!();
        self.pass_mut().draw(vertices, instances);
    }

    /// Draws indexed primitives.
    pub fn draw_indexed(&mut self, indices: Range<u32>, base_vertex: i32, instances: Range<u32>) {
        astrelis_profiling::profile_function!();
        self.pass_mut()
            .draw_indexed(indices, base_vertex, instances);
    }

    /// Sets push constant data for the given stages.
    pub fn set_push_constants(&mut self, _stages: ShaderStages, _offset: u32, _data: &[u8]) {
        unimplemented!("push constants are not supported in the wgpu 29 backend");
    }
}

/// Records compute commands within a pass.
///
/// The pass ends when this value is dropped.
pub struct ComputePass<'a> {
    pass: Option<wgpu::ComputePass<'a>>,
}

impl<'a> ComputePass<'a> {
    pub(crate) fn new(
        encoder: &'a mut wgpu::CommandEncoder,
        label: Option<&str>,
    ) -> Self {
        astrelis_profiling::profile_function!();
        let pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label,
            ..Default::default()
        });
        Self {
            pass: Some(pass),
        }
    }

    fn pass_mut(&mut self) -> &mut wgpu::ComputePass<'a> {
        self.pass.as_mut().expect("compute pass already ended")
    }

    /// Sets the active compute pipeline.
    pub fn set_pipeline(&mut self, pipeline: &ComputePipeline) {
        self.pass_mut().set_pipeline(&pipeline.0);
    }

    /// Binds a bind group at the given index.
    pub fn set_bind_group(&mut self, index: u32, bind_group: &BindGroup, offsets: &[u32]) {
        self.pass_mut().set_bind_group(index, Some(&bind_group.0), offsets);
    }

    /// Dispatches compute work groups.
    pub fn dispatch(&mut self, x: u32, y: u32, z: u32) {
        astrelis_profiling::profile_function!();
        self.pass_mut().dispatch_workgroups(x, y, z);
    }

    /// Dispatches compute work groups using an indirect buffer.
    pub fn dispatch_indirect(&mut self, buffer: &Buffer, offset: u64) {
        self.pass_mut().dispatch_workgroups_indirect(&buffer.0, offset);
    }

    /// Sets push constant data.
    pub fn set_push_constants(&mut self, _offset: u32, _data: &[u8]) {
        unimplemented!("push constants are not supported in the wgpu 29 backend");
    }
}
