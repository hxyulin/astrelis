//! Fast instanced line renderer with GPU-based coordinate transformation.
//!
//! Renders thousands of line segments efficiently using GPU instancing.
//! Line segments are stored in data coordinates, and the GPU transforms
//! them to screen coordinates using a transformation matrix.
//!
//! This means pan/zoom only updates a small uniform buffer, not all line data.

use astrelis_core::profiling::profile_scope;
use crate::capability::{GpuRequirements, RenderCapability};
use crate::transform::{DataTransform, TransformUniform};
use crate::{Color, GraphicsContext, Viewport};
use bytemuck::{Pod, Zeroable};
use glam::Vec2;
use std::sync::Arc;
use wgpu::util::DeviceExt;

/// A line segment for batch rendering.
///
/// Coordinates can be in any space - use `set_data_transform()` to map
/// them to screen coordinates.
#[derive(Debug, Clone, Copy)]
pub struct LineSegment {
    pub start: Vec2,
    pub end: Vec2,
    pub width: f32,
    pub color: Color,
}

impl LineSegment {
    pub fn new(start: Vec2, end: Vec2, width: f32, color: Color) -> Self {
        Self { start, end, width, color }
    }
}

/// GPU instance data for a line segment.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct LineInstance {
    start: [f32; 2],
    end: [f32; 2],
    width: f32,
    color: [f32; 4],
    _padding: [f32; 1],
}

impl LineInstance {
    fn new(segment: &LineSegment) -> Self {
        Self {
            start: [segment.start.x, segment.start.y],
            end: [segment.end.x, segment.end.y],
            width: segment.width,
            color: [segment.color.r, segment.color.g, segment.color.b, segment.color.a],
            _padding: [0.0],
        }
    }
}

impl RenderCapability for LineRenderer {
    fn requirements() -> GpuRequirements {
        GpuRequirements::none()
    }

    fn name() -> &'static str {
        "LineRenderer"
    }
}

/// Fast batched line renderer using GPU instancing.
///
/// Optimized for charts with large datasets. Key features:
/// - Line segments stored in data coordinates
/// - GPU transforms data → screen (pan/zoom is cheap)
/// - Only rebuild instance buffer when data actually changes
pub struct LineRenderer {
    context: Arc<GraphicsContext>,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    transform_buffer: wgpu::Buffer,
    transform_bind_group: wgpu::BindGroup,
    instance_buffer: Option<wgpu::Buffer>,
    instance_count: u32,
    /// Pending line segments
    pending_segments: Vec<LineSegment>,
    /// Whether segments need to be re-uploaded
    data_dirty: bool,
}

impl LineRenderer {
    /// Create a new line renderer with the given target texture format.
    ///
    /// The `target_format` must match the render target this renderer will draw into.
    /// For window surfaces, use the format from `WindowContext::format()`.
    pub fn new(context: Arc<GraphicsContext>, target_format: wgpu::TextureFormat) -> Self {
        // Create transform uniform buffer
        let transform_buffer = context.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("Line Renderer Transform Buffer"),
            size: std::mem::size_of::<TransformUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Bind group layout
        let bind_group_layout =
            context
                .device()
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Line Renderer Bind Group Layout"),
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
            label: Some("Line Renderer Transform Bind Group"),
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
                label: Some("Line Renderer Shader"),
                source: wgpu::ShaderSource::Wgsl(LINE_SHADER.into()),
            });

        // Pipeline
        let pipeline_layout =
            context
                .device()
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Line Renderer Pipeline Layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });

        let pipeline = context
            .device()
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Line Renderer Pipeline"),
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
                        // Line instances
                        wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<LineInstance>() as u64,
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
                                    format: wgpu::VertexFormat::Float32,
                                    offset: 16,
                                    shader_location: 3,
                                },
                                wgpu::VertexAttribute {
                                    format: wgpu::VertexFormat::Float32x4,
                                    offset: 20,
                                    shader_location: 4,
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

        // Unit quad
        let quad_vertices: [[f32; 2]; 4] = [
            [-0.5, -0.5],
            [0.5, -0.5],
            [-0.5, 0.5],
            [0.5, 0.5],
        ];

        let vertex_buffer = context
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Line Renderer Vertex Buffer"),
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
            pending_segments: Vec::with_capacity(1024),
            data_dirty: false,
        }
    }

    /// Clear all line segments. Call this when data changes.
    pub fn clear(&mut self) {
        self.pending_segments.clear();
        self.data_dirty = true;
    }

    /// Add a line segment.
    #[inline]
    pub fn add_line(&mut self, start: Vec2, end: Vec2, width: f32, color: Color) {
        self.pending_segments.push(LineSegment::new(start, end, width, color));
        self.data_dirty = true;
    }

    /// Add a line segment.
    #[inline]
    pub fn add_segment(&mut self, segment: LineSegment) {
        self.pending_segments.push(segment);
        self.data_dirty = true;
    }

    /// Get the number of line segments.
    pub fn segment_count(&self) -> usize {
        self.pending_segments.len()
    }

    /// Prepare GPU buffers. Only uploads data if it changed.
    pub fn prepare(&mut self) {
        profile_scope!("line_renderer_prepare");

        if !self.data_dirty {
            return; // No data change, skip upload
        }

        if self.pending_segments.is_empty() {
            self.instance_buffer = None;
            self.instance_count = 0;
            self.data_dirty = false;
            return;
        }

        tracing::trace!("Uploading {} line segments to GPU", self.pending_segments.len());

        // Convert to GPU format
        let instances: Vec<LineInstance> = {
            profile_scope!("convert_instances");
            self.pending_segments.iter().map(LineInstance::new).collect()
        };

        // Create buffer
        {
            profile_scope!("create_instance_buffer");
            self.instance_buffer = Some(
                self.context
                    .device()
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Line Renderer Instance Buffer"),
                        contents: bytemuck::cast_slice(&instances),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
            );
        }

        self.instance_count = self.pending_segments.len() as u32;
        self.data_dirty = false;
    }

    /// Render lines with identity transform (data coords = screen coords).
    pub fn render(&self, pass: &mut wgpu::RenderPass, viewport: Viewport) {
        let transform = DataTransform::identity(viewport);
        self.render_transformed(pass, &transform);
    }

    /// Render lines with a [`DataTransform`].
    ///
    /// This is the preferred method for rendering with data-to-screen mapping.
    /// The transform is cheap to update (32 bytes), so pan/zoom only updates
    /// the transform, not the line data.
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
    /// line_renderer.render_transformed(pass, &transform);
    /// ```
    pub fn render_transformed(&self, pass: &mut wgpu::RenderPass, transform: &DataTransform) {
        self.render_with_uniform(pass, transform.uniform());
    }

    /// Render lines with a data-to-screen transformation.
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
        profile_scope!("line_renderer_render");

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
        pass.push_debug_group("LineRenderer::render");
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.transform_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, instance_buffer.slice(..));
        pass.draw(0..4, 0..self.instance_count);
        pass.pop_debug_group();
    }
}

/// WGSL shader with data coordinate transformation.
const LINE_SHADER: &str = r#"
struct Transform {
    projection: mat4x4<f32>,
    scale: vec2<f32>,
    offset: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> transform: Transform;

struct VertexInput {
    @location(0) quad_pos: vec2<f32>,
    @location(1) line_start: vec2<f32>,
    @location(2) line_end: vec2<f32>,
    @location(3) line_width: f32,
    @location(4) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    // Transform data coordinates to screen coordinates
    let screen_start = input.line_start * transform.scale + transform.offset;
    let screen_end = input.line_end * transform.scale + transform.offset;

    // Compute line direction and perpendicular
    let delta = screen_end - screen_start;
    let length = length(delta);

    var dir: vec2<f32>;
    var perp: vec2<f32>;
    if length < 0.0001 {
        dir = vec2<f32>(1.0, 0.0);
        perp = vec2<f32>(0.0, 1.0);
    } else {
        dir = delta / length;
        perp = vec2<f32>(-dir.y, dir.x);
    }

    // Transform quad to line segment
    let center = (screen_start + screen_end) * 0.5;
    let local_x = input.quad_pos.x * length;
    let local_y = input.quad_pos.y * input.line_width;
    let world_pos = center + dir * local_x + perp * local_y;

    output.position = transform.projection * vec4<f32>(world_pos, 0.0, 1.0);
    output.color = input.color;

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
"#;
