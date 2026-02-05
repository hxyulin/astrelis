//! Tier 3: BindlessBatchRenderer
//!
//! Uses `binding_array<texture_2d<f32>>` for all textures and a single
//! `multi_draw_indirect()` per pass (opaque + transparent).
//! Requires `INDIRECT_FIRST_INSTANCE` + `TEXTURE_BINDING_ARRAY` + `PARTIALLY_BOUND_BINDING_ARRAY`.

use std::sync::Arc;

use astrelis_core::profiling::profile_function;

use crate::context::GraphicsContext;
use crate::indirect::{DrawIndirect, IndirectBuffer};

use super::BINDLESS_MAX_TEXTURES;
use super::pipeline;
use super::texture_array::BindlessTextureArray;
use super::traits::BatchRenderer2D;
use super::types::{BatchRenderStats2D, DrawBatch2D, DrawType2D, RenderTier, UnifiedInstance2D};

pub struct BindlessBatchRenderer2D {
    context: Arc<GraphicsContext>,
    // Pipelines
    opaque_pipeline: wgpu::RenderPipeline,
    transparent_pipeline: wgpu::RenderPipeline,
    // Shared resources
    quad_vbo: wgpu::Buffer,
    projection_buffer: wgpu::Buffer,
    projection_bind_group: wgpu::BindGroup,
    // Texture management
    texture_array: BindlessTextureArray,
    // Instance buffer
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    // Indirect buffer (2 entries: opaque + transparent)
    indirect_buffer: IndirectBuffer<DrawIndirect>,
    // Prepared frame data
    opaque_instances: Vec<UnifiedInstance2D>,
    transparent_instances: Vec<UnifiedInstance2D>,
    // Stats
    stats: BatchRenderStats2D,
    // Depth buffer
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    depth_width: u32,
    depth_height: u32,
}

impl BindlessBatchRenderer2D {
    const INITIAL_INSTANCE_CAPACITY: usize = 4096;
    const MAX_TEXTURES: u32 = BINDLESS_MAX_TEXTURES;

    pub fn new(context: Arc<GraphicsContext>, surface_format: wgpu::TextureFormat) -> Self {
        profile_function!();
        let device = context.device();
        let queue = context.queue();

        let quad_vbo = pipeline::create_quad_vbo(device, queue);
        let projection_buffer = pipeline::create_projection_buffer(device);
        let texture_array = BindlessTextureArray::new(device, queue, Self::MAX_TEXTURES);

        let projection_layout = pipeline::create_projection_bind_group_layout(device);
        let projection_bind_group =
            pipeline::create_projection_bind_group(device, &projection_layout, &projection_buffer);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("batched_bindless_shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/batched_bindless.wgsl").into(),
            ),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("batched_bindless_pipeline_layout"),
            bind_group_layouts: &[texture_array.layout(), &projection_layout],
            push_constant_ranges: &[],
        });

        let opaque_pipeline = pipeline::create_batched_pipeline(
            device,
            &shader,
            &pipeline_layout,
            surface_format,
            true,
        );
        let transparent_pipeline = pipeline::create_batched_pipeline(
            device,
            &shader,
            &pipeline_layout,
            surface_format,
            false,
        );

        let instance_buffer =
            pipeline::create_instance_buffer(device, Self::INITIAL_INSTANCE_CAPACITY);

        // 2 indirect commands: one for opaque, one for transparent
        let indirect_buffer = IndirectBuffer::new(&context, Some("batched_bindless_indirect"), 2);

        let (depth_texture, depth_view) = pipeline::create_depth_texture(device, 1, 1);

        Self {
            context,
            opaque_pipeline,
            transparent_pipeline,
            quad_vbo,
            projection_buffer,
            projection_bind_group,
            texture_array,
            instance_buffer,
            instance_capacity: Self::INITIAL_INSTANCE_CAPACITY,
            indirect_buffer,
            opaque_instances: Vec::new(),
            transparent_instances: Vec::new(),
            stats: BatchRenderStats2D::default(),
            depth_texture,
            depth_view,
            depth_width: 1,
            depth_height: 1,
        }
    }

    fn ensure_depth_buffer(&mut self, width: u32, height: u32) {
        if self.depth_width != width || self.depth_height != height {
            let (tex, view) = pipeline::create_depth_texture(self.context.device(), width, height);
            self.depth_texture = tex;
            self.depth_view = view;
            self.depth_width = width;
            self.depth_height = height;
        }
    }

    fn ensure_instance_buffer(&mut self, required: usize) {
        if required > self.instance_capacity {
            let new_capacity = required.next_power_of_two();
            self.instance_buffer =
                pipeline::create_instance_buffer(self.context.device(), new_capacity);
            self.instance_capacity = new_capacity;
        }
    }
}

impl BatchRenderer2D for BindlessBatchRenderer2D {
    fn tier(&self) -> RenderTier {
        RenderTier::Bindless
    }

    fn prepare(&mut self, batch: &DrawBatch2D) {
        profile_function!();
        let mut stats = BatchRenderStats2D {
            instance_count: batch.instances.len() as u32,
            texture_count: batch.textures.len() as u32,
            ..Default::default()
        };

        // Update projection
        self.context.queue().write_buffer(
            &self.projection_buffer,
            0,
            bytemuck::cast_slice(&batch.projection),
        );

        // Update bindless texture array
        self.texture_array
            .update(self.context.device(), &batch.textures);

        // Separate opaque and transparent
        self.opaque_instances.clear();
        self.transparent_instances.clear();

        for inst in &batch.instances {
            let is_transparent = inst.color[3] < 1.0
                || inst.draw_type == DrawType2D::Text as u32
                || inst.border_radius > 0.0
                || inst.border_thickness > 0.0;

            if is_transparent {
                self.transparent_instances.push(*inst);
            } else {
                self.opaque_instances.push(*inst);
            }
        }

        // Sort opaque front-to-back
        self.opaque_instances.sort_by(|a, b| {
            b.z_depth
                .partial_cmp(&a.z_depth)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Sort transparent back-to-front
        self.transparent_instances.sort_by(|a, b| {
            a.z_depth
                .partial_cmp(&b.z_depth)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        stats.opaque_count = self.opaque_instances.len() as u32;
        stats.transparent_count = self.transparent_instances.len() as u32;

        // Upload instances: [opaque | transparent]
        let total = self.opaque_instances.len() + self.transparent_instances.len();
        self.ensure_instance_buffer(total);

        if !self.opaque_instances.is_empty() {
            self.context.queue().write_buffer(
                &self.instance_buffer,
                0,
                bytemuck::cast_slice(&self.opaque_instances),
            );
        }
        if !self.transparent_instances.is_empty() {
            let offset =
                (self.opaque_instances.len() * std::mem::size_of::<UnifiedInstance2D>()) as u64;
            self.context.queue().write_buffer(
                &self.instance_buffer,
                offset,
                bytemuck::cast_slice(&self.transparent_instances),
            );
        }

        // Build 2 indirect commands
        let opaque_cmd = DrawIndirect::new(6, self.opaque_instances.len() as u32, 0, 0);
        let transparent_cmd = DrawIndirect::new(
            6,
            self.transparent_instances.len() as u32,
            0,
            self.opaque_instances.len() as u32,
        );
        self.indirect_buffer
            .write(self.context.queue(), &[opaque_cmd, transparent_cmd]);

        // Bindless: minimal draw calls
        stats.draw_calls = if self.opaque_instances.is_empty() {
            0
        } else {
            1
        } + if self.transparent_instances.is_empty() {
            0
        } else {
            1
        };
        stats.bind_group_switches = 1; // single bindless bind group
        stats.pipeline_switches = 2;

        self.stats = stats;
    }

    fn render(&self, pass: &mut wgpu::RenderPass<'_>) {
        profile_function!();
        let Some(bindless_bg) = self.texture_array.bind_group() else {
            tracing::warn!("Bindless bind group not available, skipping render");
            return;
        };

        pass.push_debug_group("BindlessBatch::render");

        pass.set_vertex_buffer(0, self.quad_vbo.slice(..));
        pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        pass.set_bind_group(0, bindless_bg, &[]);
        pass.set_bind_group(1, &self.projection_bind_group, &[]);

        // --- Opaque pass ---
        if !self.opaque_instances.is_empty() {
            pass.push_debug_group("opaque");
            pass.set_pipeline(&self.opaque_pipeline);
            pass.multi_draw_indirect(self.indirect_buffer.buffer(), 0, 1);
            pass.pop_debug_group();
        }

        // --- Transparent pass ---
        if !self.transparent_instances.is_empty() {
            pass.push_debug_group("transparent");
            pass.set_pipeline(&self.transparent_pipeline);
            let offset = self.indirect_buffer.offset_of(1);
            pass.multi_draw_indirect(self.indirect_buffer.buffer(), offset, 1);
            pass.pop_debug_group();
        }

        pass.pop_debug_group();
    }

    fn stats(&self) -> BatchRenderStats2D {
        self.stats
    }
}

impl BindlessBatchRenderer2D {
    /// Get the depth texture view for creating render passes.
    pub fn depth_view(&self) -> &wgpu::TextureView {
        &self.depth_view
    }

    /// Ensure depth buffer is ready for the given viewport size.
    pub fn prepare_depth_buffer(&mut self, width: u32, height: u32) {
        self.ensure_depth_buffer(width, height);
    }
}
