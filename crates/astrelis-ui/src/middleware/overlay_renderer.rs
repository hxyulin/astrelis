//! GPU renderer for middleware overlay draw commands.
//!
//! Renders overlay commands (quads, text, lines) on top of the main UI.
//! Uses the same GPU pipelines as UiRenderer for efficient rendering.

use astrelis_render::wgpu::util::DeviceExt;
use astrelis_render::{GraphicsContext, Renderer, Viewport, wgpu};
use astrelis_text::{FontRenderer, FontSystem, TextPipeline, shape_text};
use std::sync::Arc;

use crate::glyph_atlas::glyphs_to_instances;
use crate::gpu_types::{QuadInstance, QuadVertex, TextInstance};
use crate::instance_buffer::InstanceBuffer;

use super::overlay_draw_list::{OverlayCommand, OverlayDrawList};

/// GPU renderer for overlay draw commands.
pub struct OverlayRenderer {
    /// Graphics context (kept alive for resource lifetime)
    #[allow(dead_code)]
    context: Arc<GraphicsContext>,
    renderer: Renderer,
    font_renderer: FontRenderer,
    text_pipeline: TextPipeline,

    // Pipelines
    quad_pipeline: wgpu::RenderPipeline,
    text_pipeline_gpu: wgpu::RenderPipeline,

    // Buffers
    unit_quad_vbo: wgpu::Buffer,
    quad_instances: InstanceBuffer<QuadInstance>,
    text_instances: InstanceBuffer<TextInstance>,

    // Bind groups
    projection_buffer: wgpu::Buffer,
    projection_bind_group: wgpu::BindGroup,
    text_atlas_bind_group: wgpu::BindGroup,
    text_projection_bind_group: wgpu::BindGroup,

    scale_factor: f64,
}

impl OverlayRenderer {
    /// Create a new overlay renderer.
    pub fn new(context: Arc<GraphicsContext>) -> Self {
        let renderer = Renderer::new(context.clone());

        // Create font renderer
        let font_system = FontSystem::with_system_fonts();
        let font_renderer = FontRenderer::new(context.clone(), font_system);

        // Create unit quad VBO
        let unit_quad_vertices = QuadVertex::unit_quad();
        let unit_quad_vbo = context
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Overlay Unit Quad VBO"),
                contents: bytemuck::cast_slice(&unit_quad_vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        // Load shaders
        let quad_shader = renderer.create_shader(
            Some("Overlay Quad Shader"),
            include_str!("../../shaders/quad_instanced.wgsl"),
        );
        let text_shader = renderer.create_shader(
            Some("Overlay Text Shader"),
            include_str!("../../shaders/text_instanced.wgsl"),
        );

        // Create projection buffer
        let projection_buffer = context.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("Overlay Projection Buffer"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layouts
        let projection_bind_group_layout = renderer.create_bind_group_layout(
            Some("Overlay Projection BGL"),
            &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        );

        let text_atlas_bind_group_layout = renderer.create_bind_group_layout(
            Some("Overlay Text Atlas BGL"),
            &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        );

        // Create bind groups
        let projection_bind_group = renderer.create_bind_group(
            Some("Overlay Projection BG"),
            &projection_bind_group_layout,
            &[wgpu::BindGroupEntry {
                binding: 0,
                resource: projection_buffer.as_entire_binding(),
            }],
        );

        let text_atlas_bind_group = renderer.create_bind_group(
            Some("Overlay Text Atlas BG"),
            &text_atlas_bind_group_layout,
            &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        font_renderer.atlas_texture_view(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(font_renderer.atlas_sampler()),
                },
            ],
        );

        let text_projection_bind_group = renderer.create_bind_group(
            Some("Overlay Text Projection BG"),
            &projection_bind_group_layout,
            &[wgpu::BindGroupEntry {
                binding: 0,
                resource: projection_buffer.as_entire_binding(),
            }],
        );

        // Create pipelines
        let quad_layout = renderer.create_pipeline_layout(
            Some("Overlay Quad Pipeline Layout"),
            &[&projection_bind_group_layout],
            &[],
        );

        let quad_pipeline = renderer.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Overlay Quad Pipeline"),
            layout: Some(&quad_layout),
            vertex: wgpu::VertexState {
                module: &quad_shader,
                entry_point: Some("vs_main"),
                buffers: &[QuadVertex::vertex_layout(), QuadInstance::vertex_layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &quad_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        let text_layout = renderer.create_pipeline_layout(
            Some("Overlay Text Pipeline Layout"),
            &[&text_atlas_bind_group_layout, &projection_bind_group_layout],
            &[],
        );

        let text_pipeline_gpu = renderer.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Overlay Text Pipeline"),
            layout: Some(&text_layout),
            vertex: wgpu::VertexState {
                module: &text_shader,
                entry_point: Some("vs_main"),
                buffers: &[QuadVertex::vertex_layout(), TextInstance::vertex_layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &text_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        // Create instance buffers
        let quad_instances =
            InstanceBuffer::new(context.device(), Some("Overlay Quad Instances"), 256);
        let text_instances =
            InstanceBuffer::new(context.device(), Some("Overlay Text Instances"), 1024);

        Self {
            context,
            renderer,
            font_renderer,
            text_pipeline: TextPipeline::new(),
            quad_pipeline,
            text_pipeline_gpu,
            unit_quad_vbo,
            quad_instances,
            text_instances,
            projection_buffer,
            projection_bind_group,
            text_atlas_bind_group,
            text_projection_bind_group,
            scale_factor: 1.0,
        }
    }

    /// Set the viewport (updates scale factor).
    pub fn set_viewport(&mut self, viewport: Viewport) {
        if (self.scale_factor - viewport.scale_factor.0).abs() > f64::EPSILON {
            self.text_pipeline.clear_cache();
        }
        self.scale_factor = viewport.scale_factor.0;
        self.font_renderer.set_viewport(viewport);
    }

    /// Render overlay commands.
    pub fn render(
        &mut self,
        draw_list: &OverlayDrawList,
        render_pass: &mut wgpu::RenderPass,
        viewport: Viewport,
    ) {
        if draw_list.is_empty() {
            return;
        }

        // Update projection matrix
        let logical = viewport.to_logical();
        let projection = orthographic_projection(logical.width, logical.height);
        self.renderer.queue().write_buffer(
            &self.projection_buffer,
            0,
            bytemuck::cast_slice(&projection),
        );

        // Build instance data
        let mut quad_instances = Vec::new();
        let mut text_instances = Vec::new();

        // Overlays render on top of all UI content, use maximum z_depth
        const OVERLAY_Z_DEPTH: f32 = 1.0;

        for cmd in draw_list.commands() {
            match cmd {
                OverlayCommand::Quad(q) => {
                    // Main fill quad
                    quad_instances.push(QuadInstance {
                        position: [q.position.x, q.position.y],
                        size: [q.size.x, q.size.y],
                        color: [
                            q.fill_color.r,
                            q.fill_color.g,
                            q.fill_color.b,
                            q.fill_color.a,
                        ],
                        border_radius: q.border_radius,
                        border_thickness: 0.0,
                        z_depth: OVERLAY_Z_DEPTH,
                        _padding: 0.0,
                    });

                    // Border quad (if present)
                    if let Some(border_color) = q.border_color
                        && q.border_width > 0.0
                    {
                        quad_instances.push(QuadInstance {
                            position: [q.position.x, q.position.y],
                            size: [q.size.x, q.size.y],
                            color: [
                                border_color.r,
                                border_color.g,
                                border_color.b,
                                border_color.a,
                            ],
                            border_radius: q.border_radius,
                            border_thickness: q.border_width,
                            z_depth: OVERLAY_Z_DEPTH,
                            _padding: 0.0,
                        });
                    }
                }
                OverlayCommand::Text(t) => {
                    // Shape text and generate glyph instances
                    let shaped = {
                        let font_system = self.font_renderer.font_system();
                        let mut font_sys = font_system.write().unwrap();
                        shape_text(
                            &mut font_sys,
                            &t.text,
                            t.size,
                            None,
                            self.scale_factor as f32,
                        )
                    };

                    let instances = glyphs_to_instances(
                        &mut self.font_renderer,
                        &shaped.glyphs,
                        t.position,
                        t.color,
                        OVERLAY_Z_DEPTH,
                    );
                    text_instances.extend(instances);
                }
                OverlayCommand::Line(l) => {
                    // Render line as a thin rotated quad
                    let delta = l.end - l.start;
                    let length = delta.length();
                    if length < 0.001 {
                        continue;
                    }

                    // Calculate center and rotation
                    let center = (l.start + l.end) * 0.5;
                    let _angle = delta.y.atan2(delta.x);

                    // For now, render as axis-aligned quad (simplified)
                    // A proper implementation would use a rotation in the shader
                    let min_x = l.start.x.min(l.end.x);
                    let min_y = l.start.y.min(l.end.y);
                    let max_x = l.start.x.max(l.end.x);
                    let max_y = l.start.y.max(l.end.y);

                    let width = (max_x - min_x).max(l.thickness);
                    let height = (max_y - min_y).max(l.thickness);

                    quad_instances.push(QuadInstance {
                        position: [center.x - width * 0.5, center.y - height * 0.5],
                        size: [width, height],
                        color: [l.color.r, l.color.g, l.color.b, l.color.a],
                        border_radius: 0.0,
                        border_thickness: 0.0,
                        z_depth: OVERLAY_Z_DEPTH,
                        _padding: 0.0,
                    });
                }
            }
        }

        // Upload instance data
        self.quad_instances
            .set_instances(self.renderer.device(), quad_instances);
        self.text_instances
            .set_instances(self.renderer.device(), text_instances);
        self.quad_instances.upload_dirty(self.renderer.queue());
        self.text_instances.upload_dirty(self.renderer.queue());
        self.font_renderer.upload_atlas_if_dirty();

        // Render quads
        if !self.quad_instances.is_empty() {
            render_pass.set_pipeline(&self.quad_pipeline);
            render_pass.set_bind_group(0, &self.projection_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.unit_quad_vbo.slice(..));
            render_pass.set_vertex_buffer(1, self.quad_instances.buffer().slice(..));
            render_pass.draw(0..6, 0..self.quad_instances.len() as u32);
        }

        // Render text
        if !self.text_instances.is_empty() {
            render_pass.set_pipeline(&self.text_pipeline_gpu);
            render_pass.set_bind_group(0, &self.text_atlas_bind_group, &[]);
            render_pass.set_bind_group(1, &self.text_projection_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.unit_quad_vbo.slice(..));
            render_pass.set_vertex_buffer(1, self.text_instances.buffer().slice(..));
            render_pass.draw(0..6, 0..self.text_instances.len() as u32);
        }
    }

    /// Get reference to font renderer for text measurement.
    pub fn font_renderer(&self) -> &FontRenderer {
        &self.font_renderer
    }
}

/// Create an orthographic projection matrix for 2D rendering.
fn orthographic_projection(width: f32, height: f32) -> [[f32; 4]; 4] {
    [
        [2.0 / width, 0.0, 0.0, 0.0],
        [0.0, -2.0 / height, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0, 1.0],
    ]
}
