//! Fast instanced quad renderer with GPU-based coordinate transformation.
//!
//! Renders thousands of quads (rectangles) efficiently using GPU instancing.
//! Quads are stored in data coordinates, and the GPU transforms
//! them to screen coordinates using a transformation matrix.
//!
//! This is primarily used for bar charts but can be used for any axis-aligned
//! rectangle rendering where data-to-screen transformation is needed.

use astrelis_core::profiling::profile_scope;
use crate::capability::{GpuRequirements, RenderCapability};
use crate::transform::{DataTransform, TransformUniform};
use crate::{Color, GraphicsContext, Viewport};
use bytemuck::{Pod, Zeroable};
use glam::Vec2;
use std::sync::Arc;
use wgpu::util::DeviceExt;

/// A quad (axis-aligned rectangle) for batch rendering.
///
/// The quad is defined by two data coordinates (min and max) which define
/// the corners. For bar charts, x_min/x_max define the bar width and
/// y_min/y_max define the bar height (typically y_min = baseline).
#[derive(Debug, Clone, Copy)]
pub struct Quad {
    /// Minimum corner (typically bottom-left in data coords)
    pub min: Vec2,
    /// Maximum corner (typically top-right in data coords)
    pub max: Vec2,
    /// Fill color
    pub color: Color,
}

impl Quad {
    pub fn new(min: Vec2, max: Vec2, color: Color) -> Self {
        Self { min, max, color }
    }

    /// Create a quad from center, width, and height.
    pub fn from_center(center: Vec2, width: f32, height: f32, color: Color) -> Self {
        let half = Vec2::new(width * 0.5, height * 0.5);
        Self {
            min: center - half,
            max: center + half,
            color,
        }
    }

    /// Create a bar from x center, width, y_bottom, and y_top.
    pub fn bar(x_center: f32, width: f32, y_bottom: f32, y_top: f32, color: Color) -> Self {
        Self {
            min: Vec2::new(x_center - width * 0.5, y_bottom),
            max: Vec2::new(x_center + width * 0.5, y_top),
            color,
        }
    }
}

/// GPU instance data for a quad.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct QuadInstance {
    min: [f32; 2],
    max: [f32; 2],
    color: [f32; 4],
}

impl QuadInstance {
    fn new(quad: &Quad) -> Self {
        Self {
            min: [quad.min.x, quad.min.y],
            max: [quad.max.x, quad.max.y],
            color: [quad.color.r, quad.color.g, quad.color.b, quad.color.a],
        }
    }
}

impl RenderCapability for QuadRenderer {
    fn requirements() -> GpuRequirements {
        GpuRequirements::none()
    }

    fn name() -> &'static str {
        "QuadRenderer"
    }
}

/// Fast batched quad renderer using GPU instancing.
///
/// Optimized for bar charts with large datasets. Key features:
/// - Quads stored in data coordinates
/// - GPU transforms data → screen (pan/zoom is cheap)
/// - Only rebuild instance buffer when data actually changes
pub struct QuadRenderer {
    context: Arc<GraphicsContext>,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    transform_buffer: wgpu::Buffer,
    transform_bind_group: wgpu::BindGroup,
    instance_buffer: Option<wgpu::Buffer>,
    instance_count: u32,
    /// Pending quads
    pending_quads: Vec<Quad>,
    /// Whether quads need to be re-uploaded
    data_dirty: bool,
}

impl QuadRenderer {
    /// Create a new quad renderer with the given target texture format.
    ///
    /// The `target_format` must match the render target this renderer will draw into.
    /// For window surfaces, use the format from `WindowContext::format()`.
    pub fn new(context: Arc<GraphicsContext>, target_format: wgpu::TextureFormat) -> Self {
        // Create transform uniform buffer
        let transform_buffer = context.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("Quad Renderer Transform Buffer"),
            size: std::mem::size_of::<TransformUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Bind group layout
        let bind_group_layout =
            context
                .device()
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Quad Renderer Bind Group Layout"),
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

        let transform_bind_group = context.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Quad Renderer Transform Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: transform_buffer.as_entire_binding(),
            }],
        });

        // Shader
        let shader = context
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Quad Renderer Shader"),
                source: wgpu::ShaderSource::Wgsl(QUAD_SHADER.into()),
            });

        // Pipeline
        let pipeline_layout =
            context
                .device()
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Quad Renderer Pipeline Layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });

        let pipeline = context
            .device()
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Quad Renderer Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[
                        // Unit quad vertices
                        wgpu::VertexBufferLayout {
                            array_stride: 8,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &[wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: 0,
                                shader_location: 0,
                            }],
                        },
                        // Quad instances
                        wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<QuadInstance>() as u64,
                            step_mode: wgpu::VertexStepMode::Instance,
                            attributes: &[
                                wgpu::VertexAttribute {
                                    format: wgpu::VertexFormat::Float32x2,
                                    offset: 0,
                                    shader_location: 1,
                                },
                                wgpu::VertexAttribute {
                                    format: wgpu::VertexFormat::Float32x2,
                                    offset: 8,
                                    shader_location: 2,
                                },
                                wgpu::VertexAttribute {
                                    format: wgpu::VertexFormat::Float32x4,
                                    offset: 16,
                                    shader_location: 3,
                                },
                            ],
                        },
                    ],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        // Unit quad (0,0 to 1,1)
        let quad_vertices: [[f32; 2]; 4] = [
            [0.0, 0.0],
            [1.0, 0.0],
            [0.0, 1.0],
            [1.0, 1.0],
        ];

        let vertex_buffer = context
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Quad Renderer Vertex Buffer"),
                contents: bytemuck::cast_slice(&quad_vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        Self {
            context,
            pipeline,
            vertex_buffer,
            transform_buffer,
            transform_bind_group,
            instance_buffer: None,
            instance_count: 0,
            pending_quads: Vec::with_capacity(1024),
            data_dirty: false,
        }
    }

    /// Clear all quads. Call this when data changes.
    pub fn clear(&mut self) {
        self.pending_quads.clear();
        self.data_dirty = true;
    }

    /// Add a quad.
    #[inline]
    pub fn add_quad(&mut self, min: Vec2, max: Vec2, color: Color) {
        self.pending_quads.push(Quad::new(min, max, color));
        self.data_dirty = true;
    }

    /// Add a bar from center x, width, y range.
    #[inline]
    pub fn add_bar(&mut self, x_center: f32, width: f32, y_bottom: f32, y_top: f32, color: Color) {
        self.pending_quads.push(Quad::bar(x_center, width, y_bottom, y_top, color));
        self.data_dirty = true;
    }

    /// Add a quad.
    #[inline]
    pub fn add(&mut self, quad: Quad) {
        self.pending_quads.push(quad);
        self.data_dirty = true;
    }

    /// Get the number of quads.
    pub fn quad_count(&self) -> usize {
        self.pending_quads.len()
    }

    /// Prepare GPU buffers. Only uploads data if it changed.
    pub fn prepare(&mut self) {
        profile_scope!("quad_renderer_prepare");

        if !self.data_dirty {
            return; // No data change, skip upload
        }

        if self.pending_quads.is_empty() {
            self.instance_buffer = None;
            self.instance_count = 0;
            self.data_dirty = false;
            return;
        }

        tracing::trace!("Uploading {} quads to GPU", self.pending_quads.len());

        // Convert to GPU format
        let instances: Vec<QuadInstance> = {
            profile_scope!("convert_instances");
            self.pending_quads.iter().map(QuadInstance::new).collect()
        };

        // Create buffer
        {
            profile_scope!("create_instance_buffer");
            self.instance_buffer = Some(
                self.context
                    .device()
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Quad Renderer Instance Buffer"),
                        contents: bytemuck::cast_slice(&instances),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
            );
        }

        self.instance_count = self.pending_quads.len() as u32;
        self.data_dirty = false;
    }

    /// Render quads with identity transform (data coords = screen coords).
    pub fn render(&self, pass: &mut wgpu::RenderPass, viewport: Viewport) {
        let transform = DataTransform::identity(viewport);
        self.render_transformed(pass, &transform);
    }

    /// Render quads with a [`DataTransform`].
    ///
    /// This is the preferred method for rendering with data-to-screen mapping.
    /// The transform is cheap to update (32 bytes), so pan/zoom only updates
    /// the transform, not the quad data.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let transform = DataTransform::from_data_range(viewport, DataRangeParams {
    ///     plot_x: 80.0, plot_y: 20.0,
    ///     plot_width: 600.0, plot_height: 400.0,
    ///     data_x_min: 0.0, data_x_max: 100.0,
    ///     data_y_min: 0.0, data_y_max: 50.0,
    /// });
    /// quad_renderer.render_transformed(pass, &transform);
    /// ```
    pub fn render_transformed(&self, pass: &mut wgpu::RenderPass, transform: &DataTransform) {
        self.render_with_uniform(pass, transform.uniform());
    }

    /// Render quads with a data-to-screen transformation.
    ///
    /// **Deprecated:** Use [`render_transformed`](Self::render_transformed) with a
    /// [`DataTransform`] instead for a cleaner API.
    ///
    /// This is the fast path for charts: data doesn't change on pan/zoom,
    /// only the transform does.
    pub fn render_with_data_transform(
        &self,
        pass: &mut wgpu::RenderPass,
        viewport: Viewport,
        plot_x: f32,
        plot_y: f32,
        plot_width: f32,
        plot_height: f32,
        data_x_min: f64,
        data_x_max: f64,
        data_y_min: f64,
        data_y_max: f64,
    ) {
        let transform = DataTransform::from_data_range(
            viewport,
            crate::transform::DataRangeParams::new(
                plot_x,
                plot_y,
                plot_width,
                plot_height,
                data_x_min,
                data_x_max,
                data_y_min,
                data_y_max,
            ),
        );
        self.render_transformed(pass, &transform);
    }

    /// Render with a specific transform uniform.
    fn render_with_uniform(&self, pass: &mut wgpu::RenderPass, transform: &TransformUniform) {
        profile_scope!("quad_renderer_render");

        if self.instance_count == 0 {
            return;
        }

        let Some(instance_buffer) = &self.instance_buffer else {
            return;
        };

        // Upload transform
        self.context.queue().write_buffer(
            &self.transform_buffer,
            0,
            bytemuck::cast_slice(&[*transform]),
        );

        // Draw
        pass.push_debug_group("QuadRenderer::render");
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.transform_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, instance_buffer.slice(..));
        pass.draw(0..4, 0..self.instance_count);
        pass.pop_debug_group();
    }
}

/// WGSL shader for quads with data coordinate transformation.
const QUAD_SHADER: &str = r#"
struct Transform {
    projection: mat4x4<f32>,
    scale: vec2<f32>,
    offset: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> transform: Transform;

struct VertexInput {
    @location(0) quad_pos: vec2<f32>,  // 0-1 range unit quad
    @location(1) rect_min: vec2<f32>,  // data coords
    @location(2) rect_max: vec2<f32>,  // data coords
    @location(3) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    // Interpolate between min and max based on quad position (0-1)
    let data_pos = mix(input.rect_min, input.rect_max, input.quad_pos);

    // Transform data coordinates to screen coordinates
    let screen_pos = data_pos * transform.scale + transform.offset;

    output.position = transform.projection * vec4<f32>(screen_pos, 0.0, 1.0);
    output.color = input.color;

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
"#;
