//! Tier 2: IndirectBatchRenderer
//!
//! Uses `multi_draw_indirect()` per texture group.
//! Requires `INDIRECT_FIRST_INSTANCE` feature.
//! Uses shader-based clipping only (no hardware scissor).

use std::sync::Arc;

use astrelis_core::profiling::profile_function;

use crate::context::GraphicsContext;
use crate::indirect::{DrawIndirect, IndirectBuffer};

use super::pipeline;
use super::texture_array::TextureArray;
use super::traits::BatchRenderer2D;
use super::types::{BatchRenderStats2D, DrawBatch2D, DrawType2D, RenderTier, UnifiedInstance2D};

/// A group of indirect draw commands for a single texture.
struct TextureGroup {
    texture_id: u64,
    /// Offset into the indirect buffer (in command index).
    indirect_offset: usize,
    /// Number of indirect draw commands.
    indirect_count: u32,
}

pub struct IndirectBatchRenderer2D {
    context: Arc<GraphicsContext>,
    // Pipelines
    opaque_pipeline: wgpu::RenderPipeline,
    transparent_pipeline: wgpu::RenderPipeline,
    // Shared resources
    quad_vbo: wgpu::Buffer,
    projection_buffer: wgpu::Buffer,
    projection_bind_group: wgpu::BindGroup,
    // Texture management
    texture_array: TextureArray,
    // Instance buffer
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    // Indirect buffer
    indirect_buffer: IndirectBuffer<DrawIndirect>,
    indirect_capacity: usize,
    // Prepared frame data
    opaque_texture_groups: Vec<TextureGroup>,
    transparent_texture_groups: Vec<TextureGroup>,
    opaque_instances: Vec<UnifiedInstance2D>,
    transparent_instances: Vec<UnifiedInstance2D>,
    indirect_commands: Vec<DrawIndirect>,
    // Stats
    stats: BatchRenderStats2D,
    // Depth buffer
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    depth_width: u32,
    depth_height: u32,
}

impl IndirectBatchRenderer2D {
    const INITIAL_INSTANCE_CAPACITY: usize = 4096;
    const INITIAL_INDIRECT_CAPACITY: usize = 256;

    pub fn new(context: Arc<GraphicsContext>, surface_format: wgpu::TextureFormat) -> Self {
        profile_function!();
        let device = context.device();
        let queue = context.queue();

        let quad_vbo = pipeline::create_quad_vbo(device, queue);
        let projection_buffer = pipeline::create_projection_buffer(device);
        let texture_array = TextureArray::new(device, queue);

        let projection_layout = pipeline::create_projection_bind_group_layout(device);
        let projection_bind_group =
            pipeline::create_projection_bind_group(device, &projection_layout, &projection_buffer);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("batched_standard_shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/batched_standard.wgsl").into(),
            ),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("batched_indirect_pipeline_layout"),
            bind_group_layouts: &[texture_array.standard_layout(), &projection_layout],
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
        let indirect_buffer = IndirectBuffer::new(
            &context,
            Some("batched_indirect_buffer"),
            Self::INITIAL_INDIRECT_CAPACITY,
        );

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
            indirect_capacity: Self::INITIAL_INDIRECT_CAPACITY,
            opaque_texture_groups: Vec::new(),
            transparent_texture_groups: Vec::new(),
            opaque_instances: Vec::new(),
            transparent_instances: Vec::new(),
            indirect_commands: Vec::new(),
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

    fn ensure_indirect_buffer(&mut self, required: usize) {
        if required > self.indirect_capacity {
            let new_capacity = required.next_power_of_two();
            self.indirect_buffer = IndirectBuffer::new(
                &self.context,
                Some("batched_indirect_buffer"),
                new_capacity,
            );
            self.indirect_capacity = new_capacity;
        }
    }

    /// Sort instances and build indirect commands per texture group.
    fn sort_and_build_indirect(
        instances: &[UnifiedInstance2D],
        opaque_instances: &mut Vec<UnifiedInstance2D>,
        transparent_instances: &mut Vec<UnifiedInstance2D>,
        opaque_groups: &mut Vec<TextureGroup>,
        transparent_groups: &mut Vec<TextureGroup>,
        indirect_commands: &mut Vec<DrawIndirect>,
    ) {
        opaque_instances.clear();
        transparent_instances.clear();
        opaque_groups.clear();
        transparent_groups.clear();
        indirect_commands.clear();

        // Separate opaque and transparent
        for inst in instances {
            let is_transparent = inst.color[3] < 1.0
                || inst.draw_type == DrawType2D::Text as u32
                || inst.border_radius > 0.0
                || inst.border_thickness > 0.0;

            if is_transparent {
                transparent_instances.push(*inst);
            } else {
                opaque_instances.push(*inst);
            }
        }

        // Sort opaque front-to-back, then by texture
        opaque_instances.sort_by(|a, b| {
            b.z_depth
                .partial_cmp(&a.z_depth)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.texture_index.cmp(&b.texture_index))
        });

        // Sort transparent back-to-front, then by texture
        transparent_instances.sort_by(|a, b| {
            a.z_depth
                .partial_cmp(&b.z_depth)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.texture_index.cmp(&b.texture_index))
        });

        // Build opaque indirect commands
        Self::build_texture_groups(opaque_instances, opaque_groups, indirect_commands, 0);

        // Build transparent indirect commands
        let opaque_offset = opaque_instances.len() as u32;
        Self::build_texture_groups(
            transparent_instances,
            transparent_groups,
            indirect_commands,
            opaque_offset,
        );
    }

    fn build_texture_groups(
        instances: &[UnifiedInstance2D],
        groups: &mut Vec<TextureGroup>,
        commands: &mut Vec<DrawIndirect>,
        first_instance_offset: u32,
    ) {
        if instances.is_empty() {
            return;
        }

        let mut current_tex = instances[0].texture_index;
        let mut current_type = instances[0].draw_type;
        let mut group_start = 0u32;

        for (i, inst) in instances.iter().enumerate() {
            if inst.texture_index != current_tex || inst.draw_type != current_type {
                // Emit a draw command for the completed group
                let indirect_offset = commands.len();
                commands.push(DrawIndirect::new(
                    6,
                    i as u32 - group_start,
                    0,
                    first_instance_offset + group_start,
                ));

                let texture_id = if current_type == DrawType2D::Quad as u32 {
                    0
                } else {
                    current_tex as u64
                };
                groups.push(TextureGroup {
                    texture_id,
                    indirect_offset,
                    indirect_count: 1,
                });

                current_tex = inst.texture_index;
                current_type = inst.draw_type;
                group_start = i as u32;
            }
        }

        // Final group
        let indirect_offset = commands.len();
        commands.push(DrawIndirect::new(
            6,
            instances.len() as u32 - group_start,
            0,
            first_instance_offset + group_start,
        ));
        let texture_id = if current_type == DrawType2D::Quad as u32 {
            0
        } else {
            current_tex as u64
        };
        groups.push(TextureGroup {
            texture_id,
            indirect_offset,
            indirect_count: 1,
        });
    }
}

impl BatchRenderer2D for IndirectBatchRenderer2D {
    fn tier(&self) -> RenderTier {
        RenderTier::Indirect
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

        // Update textures
        self.texture_array
            .update_standard(self.context.device(), &batch.textures);

        // Sort and build indirect commands
        Self::sort_and_build_indirect(
            &batch.instances,
            &mut self.opaque_instances,
            &mut self.transparent_instances,
            &mut self.opaque_texture_groups,
            &mut self.transparent_texture_groups,
            &mut self.indirect_commands,
        );

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

        // Upload indirect commands
        self.ensure_indirect_buffer(self.indirect_commands.len());
        if !self.indirect_commands.is_empty() {
            self.indirect_buffer
                .write(self.context.queue(), &self.indirect_commands);
        }

        // Count draw calls (one multi_draw_indirect per texture group)
        stats.draw_calls =
            (self.opaque_texture_groups.len() + self.transparent_texture_groups.len()) as u32;
        stats.bind_group_switches = stats.draw_calls;
        stats.pipeline_switches = 2;

        self.stats = stats;
    }

    fn render(&self, pass: &mut wgpu::RenderPass<'_>) {
        profile_function!();
        pass.push_debug_group("IndirectBatch::render");

        pass.set_vertex_buffer(0, self.quad_vbo.slice(..));
        pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        pass.set_bind_group(1, &self.projection_bind_group, &[]);

        // --- Opaque pass ---
        if !self.opaque_texture_groups.is_empty() {
            pass.push_debug_group("opaque");
            pass.set_pipeline(&self.opaque_pipeline);
            for group in &self.opaque_texture_groups {
                if group.texture_id == 0 {
                    pass.set_bind_group(0, self.texture_array.fallback_bind_group(), &[]);
                } else if let Some(bg) = self
                    .texture_array
                    .get_standard_bind_group(group.texture_id)
                {
                    pass.set_bind_group(0, bg, &[]);
                } else {
                    pass.set_bind_group(0, self.texture_array.fallback_bind_group(), &[]);
                }

                let offset = self.indirect_buffer.offset_of(group.indirect_offset);
                pass.multi_draw_indirect(
                    self.indirect_buffer.buffer(),
                    offset,
                    group.indirect_count,
                );
            }
            pass.pop_debug_group();
        }

        // --- Transparent pass ---
        if !self.transparent_texture_groups.is_empty() {
            pass.push_debug_group("transparent");
            pass.set_pipeline(&self.transparent_pipeline);
            for group in &self.transparent_texture_groups {
                if group.texture_id == 0 {
                    pass.set_bind_group(0, self.texture_array.fallback_bind_group(), &[]);
                } else if let Some(bg) = self
                    .texture_array
                    .get_standard_bind_group(group.texture_id)
                {
                    pass.set_bind_group(0, bg, &[]);
                } else {
                    pass.set_bind_group(0, self.texture_array.fallback_bind_group(), &[]);
                }

                let offset = self.indirect_buffer.offset_of(group.indirect_offset);
                pass.multi_draw_indirect(
                    self.indirect_buffer.buffer(),
                    offset,
                    group.indirect_count,
                );
            }
            pass.pop_debug_group();
        }

        pass.pop_debug_group();
    }

    fn stats(&self) -> BatchRenderStats2D {
        self.stats
    }
}

impl IndirectBatchRenderer2D {
    /// Get the depth texture view for creating render passes.
    pub fn depth_view(&self) -> &wgpu::TextureView {
        &self.depth_view
    }

    /// Ensure depth buffer is ready for the given viewport size.
    pub fn prepare_depth_buffer(&mut self, width: u32, height: u32) {
        self.ensure_depth_buffer(width, height);
    }
}
