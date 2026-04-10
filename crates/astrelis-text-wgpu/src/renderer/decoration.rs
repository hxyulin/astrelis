//! Decoration renderer for underlines, strikethrough, and backgrounds.

use astrelis_gpu::Gpu;
use astrelis_text::{DecorationQuad, TextBounds, TextDecoration, generate_decoration_quads};

use super::vertex::DecorationVertex;
use super::orthographic_projection;

/// Renderer for text decorations (underlines, strikethrough, backgrounds).
pub struct DecorationRenderer {
    pipeline: wgpu::RenderPipeline,
    uniform_bind_group_layout: wgpu::BindGroupLayout,
    background_vertices: Vec<DecorationVertex>,
    background_indices: Vec<u16>,
    line_vertices: Vec<DecorationVertex>,
    line_indices: Vec<u16>,
}

impl DecorationRenderer {
    /// Create a new decoration renderer.
    pub fn new(gpu: &Gpu, surface_format: wgpu::TextureFormat) -> Self {
        astrelis_profiling::profile_function!();
        let dev = gpu.raw_device();

        let uniform_bind_group_layout =
            dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("decoration_uniform_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let shader = dev.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("decoration_shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../shaders/decoration.wgsl").into(),
            ),
        });

        let pipeline_layout = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("decoration_pipeline_layout"),
            bind_group_layouts: &[Some(&uniform_bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("decoration_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[DecorationVertex::layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline,
            uniform_bind_group_layout,
            background_vertices: Vec::new(),
            background_indices: Vec::new(),
            line_vertices: Vec::new(),
            line_indices: Vec::new(),
        }
    }

    /// Queue a decoration quad for rendering.
    pub fn queue_quad(&mut self, quad: &DecorationQuad) {
        astrelis_profiling::profile_function!();
        let (x, y, w, h) = quad.bounds;
        let color = [quad.color.r, quad.color.g, quad.color.b, quad.color.a];

        let (vertices, indices) = if quad.is_background() {
            (&mut self.background_vertices, &mut self.background_indices)
        } else {
            (&mut self.line_vertices, &mut self.line_indices)
        };

        let base_idx = vertices.len() as u16;
        vertices.extend_from_slice(&[
            DecorationVertex { position: [x, y], color },
            DecorationVertex { position: [x + w, y], color },
            DecorationVertex { position: [x + w, y + h], color },
            DecorationVertex { position: [x, y + h], color },
        ]);
        indices.extend_from_slice(&[
            base_idx,
            base_idx + 1,
            base_idx + 2,
            base_idx,
            base_idx + 2,
            base_idx + 3,
        ]);
    }

    /// Queue decoration quads from text bounds and decoration config.
    pub fn queue_from_text(&mut self, bounds: &TextBounds, decoration: &TextDecoration) {
        astrelis_profiling::profile_function!();
        let quads = generate_decoration_quads(bounds, decoration);
        for quad in &quads {
            self.queue_quad(quad);
        }
    }

    /// Check if there are any queued decorations.
    pub fn has_queued(&self) -> bool {
        !self.background_vertices.is_empty() || !self.line_vertices.is_empty()
    }

    /// Render all queued decorations.
    pub fn render(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) {
        astrelis_profiling::profile_function!();
        if !self.has_queued() {
            return;
        }

        let dev = gpu.raw_device();

        // Projection uniform
        use wgpu::util::DeviceExt;
        let projection = orthographic_projection(width as f32, height as f32);
        let uniform_buffer = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("decoration_uniform_buffer"),
            contents: bytemuck::cast_slice(&projection),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let uniform_bind_group = dev.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("decoration_uniform_bind_group"),
            layout: &self.uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Helper to render a batch
        let render_batch = |encoder: &mut wgpu::CommandEncoder,
                           vertices: &[DecorationVertex],
                           indices: &[u16],
                           label: &str| {
            if vertices.is_empty() {
                return;
            }

            let vertex_buffer = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("decoration_{label}_vertex")),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

            let index_buffer = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("decoration_{label}_index")),
                contents: bytemuck::cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            let num_indices = indices.len() as u32;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(&format!("decoration_{label}_pass")),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &uniform_bind_group, &[]);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..num_indices, 0, 0..1);
        };

        // Render backgrounds first, then lines
        let bg_verts: Vec<_> = self.background_vertices.drain(..).collect();
        let bg_indices: Vec<_> = self.background_indices.drain(..).collect();
        let line_verts: Vec<_> = self.line_vertices.drain(..).collect();
        let line_indices: Vec<_> = self.line_indices.drain(..).collect();

        render_batch(encoder, &bg_verts, &bg_indices, "background");
        render_batch(encoder, &line_verts, &line_indices, "line");
    }

    /// Clear all queued decorations.
    pub fn clear(&mut self) {
        self.background_vertices.clear();
        self.background_indices.clear();
        self.line_vertices.clear();
        self.line_indices.clear();
    }
}
