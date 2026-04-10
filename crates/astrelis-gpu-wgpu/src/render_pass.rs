//! wgpu render pass implementation.
//!
//! The render pass holds read guards to the device's resource maps so it can
//! resolve handles to `&wgpu::X` references for immediate wgpu API calls.

use std::ops::Range;
use std::sync::Arc;

use astrelis_core::color::Color;

use astrelis_gpu::bind_group::ShaderStages;
use astrelis_gpu::command::{RenderPass, RenderPassDescriptor};
use astrelis_gpu::id::{BindGroupId, BufferId, RenderPipelineId};
use astrelis_gpu::types::{IndexFormat, LoadOp};

use crate::convert::types as conv;
use crate::device::WgpuDevice;

/// wgpu render pass that executes commands immediately against a real
/// `wgpu::RenderPass`.
pub struct WgpuRenderPass<'a> {
    // The actual wgpu render pass, created in `new()`.
    // We use Option so we can take it in Drop to end the pass.
    pass: Option<wgpu::RenderPass<'a>>,
    device: &'a Arc<WgpuDevice>,
}

impl<'a> WgpuRenderPass<'a> {
    pub(crate) fn new(
        encoder: &'a mut wgpu::CommandEncoder,
        device: &'a Arc<WgpuDevice>,
        desc: &RenderPassDescriptor<'_>,
    ) -> Self {
        astrelis_profiling::profile_function!();
        // Resolve attachment texture view handles to wgpu references.
        // We hold the read guard for the duration of this scope to build
        // the wgpu descriptor, then the guard is dropped before we create
        // the render pass (since the pass borrows the encoder, not the views).
        let views_guard = device.texture_views.read_guard();

        let color_attachments: Vec<Option<wgpu::RenderPassColorAttachment<'_>>> = desc
            .color_attachments
            .iter()
            .map(|att| {
                let view = views_guard
                    .get(&att.view.raw())
                    .expect("invalid texture view handle in color attachment");
                let resolve_target = att.resolve_target.map(|id| {
                    views_guard
                        .get(&id.raw())
                        .expect("invalid resolve target handle")
                });
                Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target,
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
                let view = views_guard
                    .get(&att.view.raw())
                    .expect("invalid depth/stencil view handle");
                wgpu::RenderPassDepthStencilAttachment {
                    view,
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

        // Guard is dropped here — fine, wgpu has captured what it needs.
        drop(views_guard);

        Self {
            pass: Some(pass),
            device,
        }
    }

    fn pass_mut(&mut self) -> &mut wgpu::RenderPass<'a> {
        self.pass.as_mut().expect("render pass already ended")
    }
}

impl RenderPass for WgpuRenderPass<'_> {
    fn set_pipeline(&mut self, pipeline: RenderPipelineId) {
        let pipelines = self.device.render_pipelines.read_guard();
        let p = pipelines
            .get(&pipeline.raw())
            .expect("invalid render pipeline handle");
        self.pass_mut().set_pipeline(p);
    }

    fn set_bind_group(&mut self, index: u32, bind_group: BindGroupId, offsets: &[u32]) {
        let groups = self.device.bind_groups.read_guard();
        let bg = groups
            .get(&bind_group.raw())
            .expect("invalid bind group handle");
        self.pass_mut().set_bind_group(index, Some(bg), offsets);
    }

    fn set_vertex_buffer(&mut self, slot: u32, buffer: BufferId, offset: u64, size: Option<u64>) {
        let buffers = self.device.buffers.read_guard();
        let buf = buffers
            .get(&buffer.raw())
            .expect("invalid vertex buffer handle");
        let slice = match size {
            Some(s) => buf.slice(offset..offset + s),
            None => buf.slice(offset..),
        };
        self.pass_mut().set_vertex_buffer(slot, slice);
    }

    fn set_index_buffer(
        &mut self,
        buffer: BufferId,
        format: IndexFormat,
        offset: u64,
        size: Option<u64>,
    ) {
        let buffers = self.device.buffers.read_guard();
        let buf = buffers
            .get(&buffer.raw())
            .expect("invalid index buffer handle");
        let slice = match size {
            Some(s) => buf.slice(offset..offset + s),
            None => buf.slice(offset..),
        };
        self.pass_mut()
            .set_index_buffer(slice, conv::index_format(format));
    }

    fn set_viewport(&mut self, x: f32, y: f32, w: f32, h: f32, min_depth: f32, max_depth: f32) {
        self.pass_mut().set_viewport(x, y, w, h, min_depth, max_depth);
    }

    fn set_scissor_rect(&mut self, x: u32, y: u32, w: u32, h: u32) {
        self.pass_mut().set_scissor_rect(x, y, w, h);
    }

    fn set_blend_constant(&mut self, color: Color) {
        self.pass_mut().set_blend_constant(wgpu::Color {
            r: color.r as f64,
            g: color.g as f64,
            b: color.b as f64,
            a: color.a as f64,
        });
    }

    fn set_stencil_reference(&mut self, reference: u32) {
        self.pass_mut().set_stencil_reference(reference);
    }

    fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>) {
        astrelis_profiling::profile_function!();
        self.pass_mut().draw(vertices, instances);
    }

    fn draw_indexed(&mut self, indices: Range<u32>, base_vertex: i32, instances: Range<u32>) {
        astrelis_profiling::profile_function!();
        self.pass_mut()
            .draw_indexed(indices, base_vertex, instances);
    }

    fn set_push_constants(&mut self, _stages: ShaderStages, _offset: u32, _data: &[u8]) {
        // Push constants are not supported in wgpu 29+.
        // Use uniform buffers or immediate data instead.
        unimplemented!("push constants are not supported in the wgpu 29 backend");
    }
}

