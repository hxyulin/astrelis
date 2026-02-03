//! Fast instanced point renderer with GPU-based coordinate transformation.
//!
//! Renders thousands of points efficiently using GPU instancing.
//! Points are stored in data coordinates, and the GPU transforms
//! them to screen coordinates using a transformation matrix.
//!
//! This means pan/zoom only updates a small uniform buffer, not all point data.

use astrelis_core::profiling::profile_scope;
use crate::capability::{GpuRequirements, RenderCapability};
use crate::transform::{DataTransform, TransformUniform};
use crate::{Color, GraphicsContext, Viewport};
use bytemuck::{Pod, Zeroable};
use glam::Vec2;
use std::sync::Arc;
use wgpu::util::DeviceExt;

/// A point for batch rendering.
///
/// Coordinates can be in any space - use `render_with_data_transform()` to map
/// them to screen coordinates.
#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub position: Vec2,
    pub size: f32,
    pub color: Color,
}

impl Point {
    pub fn new(position: Vec2, size: f32, color: Color) -> Self {
        Self { position, size, color }
    }
}

/// GPU instance data for a point.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct PointInstance {
    position: [f32; 2],
    size: f32,
    color: [f32; 4],
    _padding: f32,
}

impl PointInstance {
    fn new(point: &Point) -> Self {
        Self {
            position: [point.position.x, point.position.y],
            size: point.size,
            color: [point.color.r, point.color.g, point.color.b, point.color.a],
            _padding: 0.0,
        }
    }
}

impl RenderCapability for PointRenderer {
    fn requirements() -> GpuRequirements {
        GpuRequirements::none()
    }

    fn name() -> &'static str {
        "PointRenderer"
    }
}

/// Fast batched point renderer using GPU instancing.
///
/// Optimized for scatter charts with large datasets. Key features:
/// - Points stored in data coordinates
/// - GPU transforms data → screen (pan/zoom is cheap)
/// - Only rebuild instance buffer when data actually changes
pub struct PointRenderer {
    context: Arc<GraphicsContext>,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    transform_buffer: wgpu::Buffer,
    transform_bind_group: wgpu::BindGroup,
    instance_buffer: Option<wgpu::Buffer>,
    instance_count: u32,
    /// Pending points
    pending_points: Vec<Point>,
    /// Whether points need to be re-uploaded
    data_dirty: bool,
}

impl PointRenderer {
    /// Create a new point renderer with the given target texture format.
    ///
    /// The `target_format` must match the render target this renderer will draw into.
    /// For window surfaces, use the format from `WindowContext::format()`.
    pub fn new(context: Arc<GraphicsContext>, target_format: wgpu::TextureFormat) -> Self {
        // Create transform uniform buffer
        let transform_buffer = context.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("Point Renderer Transform Buffer"),
            size: std::mem::size_of::<TransformUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Bind group layout
        let bind_group_layout =
            context
                .device()
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Point Renderer Bind Group Layout"),
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
            label: Some("Point Renderer Transform Bind Group"),
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
                label: Some("Point Renderer Shader"),
                source: wgpu::ShaderSource::Wgsl(POINT_SHADER.into()),
            });

        // Pipeline
        let pipeline_layout =
            context
                .device()
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Point Renderer Pipeline Layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });

        let pipeline = context
            .device()
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Point Renderer Pipeline"),
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
                        // Point instances
                        wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<PointInstance>() as u64,
                            step_mode: wgpu::VertexStepMode::Instance,
                            attributes: &[
                                wgpu::VertexAttribute {
                                    format: wgpu::VertexFormat::Float32x2,
                                    offset: 0,
                                    shader_location: 1,
                                },
                                wgpu::VertexAttribute {
                                    format: wgpu::VertexFormat::Float32,
                                    offset: 8,
                                    shader_location: 2,
                                },
                                wgpu::VertexAttribute {
                                    format: wgpu::VertexFormat::Float32x4,
                                    offset: 12,
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

        // Unit quad (for rendering circles as billboards)
        let quad_vertices: [[f32; 2]; 4] = [
            [-0.5, -0.5],
            [0.5, -0.5],
            [-0.5, 0.5],
            [0.5, 0.5],
        ];

        let vertex_buffer = context
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Point Renderer Vertex Buffer"),
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
            pending_points: Vec::with_capacity(1024),
            data_dirty: false,
        }
    }

    /// Clear all points. Call this when data changes.
    pub fn clear(&mut self) {
        self.pending_points.clear();
        self.data_dirty = true;
    }

    /// Add a point.
    #[inline]
    pub fn add_point(&mut self, position: Vec2, size: f32, color: Color) {
        self.pending_points.push(Point::new(position, size, color));
        self.data_dirty = true;
    }

    /// Add a point.
    #[inline]
    pub fn add(&mut self, point: Point) {
        self.pending_points.push(point);
        self.data_dirty = true;
    }

    /// Get the number of points.
    pub fn point_count(&self) -> usize {
        self.pending_points.len()
    }

    /// Prepare GPU buffers. Only uploads data if it changed.
    pub fn prepare(&mut self) {
        profile_scope!("point_renderer_prepare");

        if !self.data_dirty {
            return; // No data change, skip upload
        }

        if self.pending_points.is_empty() {
            self.instance_buffer = None;
            self.instance_count = 0;
            self.data_dirty = false;
            return;
        }

        tracing::trace!("Uploading {} points to GPU", self.pending_points.len());

        // Convert to GPU format
        let instances: Vec<PointInstance> = {
            profile_scope!("convert_instances");
            self.pending_points.iter().map(PointInstance::new).collect()
        };

        // Create buffer
        {
            profile_scope!("create_instance_buffer");
            self.instance_buffer = Some(
                self.context
                    .device()
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Point Renderer Instance Buffer"),
                        contents: bytemuck::cast_slice(&instances),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
            );
        }

        self.instance_count = self.pending_points.len() as u32;
        self.data_dirty = false;
    }

    /// Render points with identity transform (data coords = screen coords).
    pub fn render(&self, pass: &mut wgpu::RenderPass, viewport: Viewport) {
        let transform = DataTransform::identity(viewport);
        self.render_transformed(pass, &transform);
    }

    /// Render points with a [`DataTransform`].
    ///
    /// This is the preferred method for rendering with data-to-screen mapping.
    /// The transform is cheap to update (32 bytes), so pan/zoom only updates
    /// the transform, not the point data.
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
    /// point_renderer.render_transformed(pass, &transform);
    /// ```
    pub fn render_transformed(&self, pass: &mut wgpu::RenderPass, transform: &DataTransform) {
        self.render_with_uniform(pass, transform.uniform());
    }

    /// Render points with a data-to-screen transformation.
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
        profile_scope!("point_renderer_render");

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
        pass.push_debug_group("PointRenderer::render");
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.transform_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, instance_buffer.slice(..));
        pass.draw(0..4, 0..self.instance_count);
        pass.pop_debug_group();
    }
}

/// WGSL shader for points with circle rendering and data coordinate transformation.
const POINT_SHADER: &str = r#"
struct Transform {
    projection: mat4x4<f32>,
    scale: vec2<f32>,
    offset: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> transform: Transform;

struct VertexInput {
    @location(0) quad_pos: vec2<f32>,
    @location(1) point_position: vec2<f32>,
    @location(2) point_size: f32,
    @location(3) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    // Transform data coordinates to screen coordinates
    let screen_pos = input.point_position * transform.scale + transform.offset;

    // Offset quad by point size (in screen pixels)
    let world_pos = screen_pos + input.quad_pos * input.point_size;

    output.position = transform.projection * vec4<f32>(world_pos, 0.0, 1.0);
    output.color = input.color;
    output.uv = input.quad_pos + 0.5; // UV from 0 to 1

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Render as circle: distance from center
    let center = vec2<f32>(0.5, 0.5);
    let dist = distance(input.uv, center);

    // Smooth edge for anti-aliasing
    let alpha = 1.0 - smoothstep(0.4, 0.5, dist);

    if alpha < 0.01 {
        discard;
    }

    return vec4<f32>(input.color.rgb, input.color.a * alpha);
}
"#;
