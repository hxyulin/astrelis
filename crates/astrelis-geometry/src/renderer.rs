//! Geometry renderer for 2D shapes and paths.
//!
//! Provides GPU-accelerated rendering of tessellated geometry.

use crate::gpu_types::{FillInstance, ProjectionUniform, StrokeInstance};
use crate::instance_buffer::InstanceBuffer;
use crate::pipeline::{
    create_fill_pipeline, create_projection_bind_group_layout, create_stroke_pipeline,
};
use crate::vertex::{FillVertex, StrokeVertex, TessellatedMesh};
use crate::{FillRule, Path, Shape, Stroke, Style, Tessellator};
use astrelis_core::profiling::profile_scope;
use astrelis_render::wgpu::util::DeviceExt;
use astrelis_render::{Color, GraphicsContext, RenderWindow, Renderer, Viewport, wgpu};
use glam::Vec2;
use std::sync::Arc;

/// Configuration for creating a [`GeometryRenderer`].
///
/// # Example
///
/// ```rust,no_run
/// # use astrelis_geometry::GeometryRendererDescriptor;
/// # use astrelis_render::wgpu;
/// // Create descriptor from a window (recommended)
/// // let desc = GeometryRendererDescriptor::from_window(&window);
///
/// // Or configure manually
/// let desc = GeometryRendererDescriptor {
///     name: "Shapes".to_string(),
///     surface_format: wgpu::TextureFormat::Bgra8UnormSrgb,
///     depth_format: None,
/// };
/// ```
#[derive(Clone, Debug)]
pub struct GeometryRendererDescriptor {
    /// Name for the renderer (used in pipeline labels for debugging/profiling).
    pub name: String,
    /// Surface texture format. Must match the render target.
    pub surface_format: wgpu::TextureFormat,
    /// Depth format for z-ordering. `None` disables depth testing.
    pub depth_format: Option<wgpu::TextureFormat>,
}

impl Default for GeometryRendererDescriptor {
    fn default() -> Self {
        Self {
            name: "Geometry".to_string(),
            surface_format: wgpu::TextureFormat::Bgra8UnormSrgb,
            depth_format: None,
        }
    }
}

impl GeometryRendererDescriptor {
    /// Create descriptor from a [`RenderWindow`], inheriting its format configuration.
    ///
    /// This is the **recommended** way to create a descriptor as it ensures
    /// pipeline-renderpass format compatibility automatically.
    pub fn from_window(window: &RenderWindow) -> Self {
        Self {
            name: "Geometry".to_string(),
            surface_format: window.surface_format(),
            depth_format: window.depth_format(),
        }
    }

    /// Set the renderer name (used in pipeline labels).
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Enable depth testing with the specified format.
    pub fn with_depth(mut self, format: wgpu::TextureFormat) -> Self {
        self.depth_format = Some(format);
        self
    }

    /// Disable depth testing.
    pub fn without_depth(mut self) -> Self {
        self.depth_format = None;
        self
    }
}

/// A scissor rectangle for clipping.
#[derive(Debug, Clone, Copy)]
pub struct ScissorRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl ScissorRect {
    /// Create a new scissor rect.
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Create from floating point coordinates (will be rounded).
    pub fn from_f32(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x: x.max(0.0) as u32,
            y: y.max(0.0) as u32,
            width: width.max(0.0) as u32,
            height: height.max(0.0) as u32,
        }
    }
}

/// A draw command for the geometry renderer.
#[derive(Debug)]
enum DrawCommand {
    /// Fill a tessellated mesh.
    Fill {
        mesh: TessellatedMesh<FillVertex>,
        color: Color,
        offset: Vec2,
    },
    /// Stroke a tessellated mesh.
    Stroke {
        mesh: TessellatedMesh<StrokeVertex>,
        color: Color,
        width: f32,
        offset: Vec2,
    },
    /// Set scissor rect for clipping.
    SetScissor(ScissorRect),
    /// Reset scissor to full viewport.
    ResetScissor,
}

/// GPU-accelerated geometry renderer.
///
/// Renders 2D shapes and paths using tessellation and instanced rendering.
pub struct GeometryRenderer {
    context: Arc<GraphicsContext>,
    renderer: Renderer,

    /// Current configuration (stored for reconfigure and descriptor access).
    descriptor: GeometryRendererDescriptor,

    // Pipelines
    fill_pipeline: wgpu::RenderPipeline,
    stroke_pipeline: wgpu::RenderPipeline,

    // Bind groups
    projection_bind_group_layout: wgpu::BindGroupLayout,
    projection_bind_group: wgpu::BindGroup,
    projection_buffer: wgpu::Buffer,

    // Tessellator
    tessellator: Tessellator,

    // Draw commands for current frame
    draw_commands: Vec<DrawCommand>,

    // Buffers (rebuilt each frame for simplicity)
    fill_vertex_buffer: Option<wgpu::Buffer>,
    fill_index_buffer: Option<wgpu::Buffer>,
    fill_instances: InstanceBuffer<FillInstance>,

    stroke_vertex_buffer: Option<wgpu::Buffer>,
    stroke_index_buffer: Option<wgpu::Buffer>,
    stroke_instances: InstanceBuffer<StrokeInstance>,
}

impl GeometryRenderer {
    /// Create a new geometry renderer with default configuration.
    pub fn new(context: Arc<GraphicsContext>) -> Self {
        Self::with_descriptor(context, GeometryRendererDescriptor::default())
    }

    /// Create renderer from a [`RenderWindow`], matching its format configuration.
    ///
    /// This is the **recommended** constructor as it ensures the renderer's pipelines
    /// are compatible with the window's render pass configuration.
    pub fn from_window(context: Arc<GraphicsContext>, window: &RenderWindow) -> Self {
        Self::with_descriptor(context, GeometryRendererDescriptor::from_window(window))
    }

    /// Create renderer with explicit configuration.
    pub fn with_descriptor(
        context: Arc<GraphicsContext>,
        descriptor: GeometryRendererDescriptor,
    ) -> Self {
        let renderer = Renderer::new(context.clone());

        // Create projection buffer
        let projection_buffer = context.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{} Projection Buffer", descriptor.name)),
            size: std::mem::size_of::<ProjectionUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout and bind group
        let projection_bind_group_layout = create_projection_bind_group_layout(&renderer);
        let projection_bind_group = renderer.create_bind_group(
            Some(&format!("{} Projection Bind Group", descriptor.name)),
            &projection_bind_group_layout,
            &[wgpu::BindGroupEntry {
                binding: 0,
                resource: projection_buffer.as_entire_binding(),
            }],
        );

        // Create pipelines with descriptor configuration
        let fill_pipeline = create_fill_pipeline(
            &renderer,
            &projection_bind_group_layout,
            descriptor.surface_format,
            descriptor.depth_format,
            &descriptor.name,
        );
        let stroke_pipeline = create_stroke_pipeline(
            &renderer,
            &projection_bind_group_layout,
            descriptor.surface_format,
            descriptor.depth_format,
            &descriptor.name,
        );

        // Create instance buffers
        let fill_instances = InstanceBuffer::new(
            context.device(),
            Some(&format!("{} Fill Instances", descriptor.name)),
            256,
        );
        let stroke_instances = InstanceBuffer::new(
            context.device(),
            Some(&format!("{} Stroke Instances", descriptor.name)),
            256,
        );

        Self {
            context,
            renderer,
            descriptor,
            fill_pipeline,
            stroke_pipeline,
            projection_bind_group_layout,
            projection_bind_group,
            projection_buffer,
            tessellator: Tessellator::new(),
            draw_commands: Vec::new(),
            fill_vertex_buffer: None,
            fill_index_buffer: None,
            fill_instances,
            stroke_vertex_buffer: None,
            stroke_index_buffer: None,
            stroke_instances,
        }
    }

    /// Get the current renderer configuration.
    pub fn descriptor(&self) -> &GeometryRendererDescriptor {
        &self.descriptor
    }

    /// Reconfigure the renderer with new format settings.
    ///
    /// This recreates all pipelines with the new configuration.
    /// Buffers and non-format-dependent resources are preserved.
    ///
    /// # Use Case
    ///
    /// When a window is moved to a different monitor, the surface format
    /// may change. Call this method to update the renderer to match.
    pub fn reconfigure(&mut self, descriptor: GeometryRendererDescriptor) {
        // Skip if formats haven't changed (optimization)
        if self.descriptor.surface_format == descriptor.surface_format
            && self.descriptor.depth_format == descriptor.depth_format
        {
            // Only update name if that changed
            self.descriptor.name = descriptor.name;
            return;
        }

        self.descriptor = descriptor;

        // Recreate pipelines with new formats
        self.fill_pipeline = create_fill_pipeline(
            &self.renderer,
            &self.projection_bind_group_layout,
            self.descriptor.surface_format,
            self.descriptor.depth_format,
            &self.descriptor.name,
        );
        self.stroke_pipeline = create_stroke_pipeline(
            &self.renderer,
            &self.projection_bind_group_layout,
            self.descriptor.surface_format,
            self.descriptor.depth_format,
            &self.descriptor.name,
        );
    }

    /// Reconfigure from a window, inheriting its format configuration.
    ///
    /// Convenience method equivalent to:
    /// ```rust,ignore
    /// renderer.reconfigure(GeometryRendererDescriptor::from_window(window));
    /// ```
    pub fn reconfigure_from_window(&mut self, window: &RenderWindow) {
        self.reconfigure(
            GeometryRendererDescriptor::from_window(window).with_name(self.descriptor.name.clone()),
        );
    }

    /// Set the tessellation tolerance.
    pub fn set_tolerance(&mut self, tolerance: f32) {
        self.tessellator.tolerance = tolerance;
    }

    /// Clear all draw commands.
    pub fn clear(&mut self) {
        self.draw_commands.clear();
    }

    /// Draw a shape with the given style.
    pub fn draw_shape(&mut self, shape: &Shape, style: &Style) {
        let path = shape.to_path();
        self.draw_path(&path, style);
    }

    /// Draw a path with the given style.
    pub fn draw_path(&mut self, path: &Path, style: &Style) {
        // Handle fill
        if let Some(fill) = &style.fill
            && let Some(color) = fill.effective_color()
        {
            let mesh = self.tessellator.tessellate_fill(path, fill.rule);
            if !mesh.is_empty() {
                self.draw_commands.push(DrawCommand::Fill {
                    mesh,
                    color,
                    offset: style.transform.translation(),
                });
            }
        }

        // Handle stroke
        if let Some(stroke) = &style.stroke
            && stroke.is_visible()
            && let Some(color) = stroke.effective_color()
        {
            let mesh = self.tessellator.tessellate_stroke(path, stroke);
            if !mesh.is_empty() {
                self.draw_commands.push(DrawCommand::Stroke {
                    mesh,
                    color,
                    width: stroke.width,
                    offset: style.transform.translation(),
                });
            }
        }
    }

    /// Draw a filled rectangle.
    pub fn draw_rect(&mut self, position: Vec2, size: Vec2, color: Color) {
        let mesh = self.tessellator.tessellate_rect_fill(position, size);
        self.draw_commands.push(DrawCommand::Fill {
            mesh,
            color,
            offset: Vec2::ZERO,
        });
    }

    /// Draw a filled circle.
    pub fn draw_circle(&mut self, center: Vec2, radius: f32, color: Color) {
        profile_scope!("draw_circle");
        let shape = Shape::circle(center, radius);
        let style = Style::fill_color(color);
        self.draw_shape(&shape, &style);
    }

    /// Draw a line.
    pub fn draw_line(&mut self, start: Vec2, end: Vec2, width: f32, color: Color) {
        profile_scope!("draw_line");
        let mesh = self.tessellator.tessellate_line(start, end, width);
        self.draw_commands.push(DrawCommand::Fill {
            mesh,
            color,
            offset: Vec2::ZERO,
        });
    }

    /// Draw a stroked rectangle.
    pub fn draw_rect_stroke(&mut self, position: Vec2, size: Vec2, stroke: &Stroke) {
        let shape = Shape::rect(position, size);
        let style = Style::new().with_stroke(stroke.clone());
        self.draw_shape(&shape, &style);
    }

    /// Draw a stroked circle.
    pub fn draw_circle_stroke(&mut self, center: Vec2, radius: f32, stroke: &Stroke) {
        let shape = Shape::circle(center, radius);
        let style = Style::new().with_stroke(stroke.clone());
        self.draw_shape(&shape, &style);
    }

    /// Draw a filled shape directly.
    pub fn draw_shape_fill(&mut self, shape: &Shape, color: Color) {
        let style = Style::fill_color(color);
        self.draw_shape(shape, &style);
    }

    /// Draw a stroked shape directly.
    pub fn draw_shape_stroke(&mut self, shape: &Shape, stroke: &Stroke) {
        let style = Style::new().with_stroke(stroke.clone());
        self.draw_shape(shape, &style);
    }

    /// Draw a path with fill only.
    pub fn draw_path_fill(&mut self, path: &Path, color: Color, fill_rule: FillRule) {
        let mesh = self.tessellator.tessellate_fill(path, fill_rule);
        if !mesh.is_empty() {
            self.draw_commands.push(DrawCommand::Fill {
                mesh,
                color,
                offset: Vec2::ZERO,
            });
        }
    }

    /// Draw a path with stroke only.
    pub fn draw_path_stroke(&mut self, path: &Path, stroke: &Stroke) {
        profile_scope!("draw_path_stroke");
        if stroke.is_visible()
            && let Some(color) = stroke.effective_color()
        {
            let mesh = self.tessellator.tessellate_stroke(path, stroke);
            if !mesh.is_empty() {
                self.draw_commands.push(DrawCommand::Stroke {
                    mesh,
                    color,
                    width: stroke.width,
                    offset: Vec2::ZERO,
                });
            }
        }
    }

    /// Set a scissor rectangle to clip subsequent drawing.
    ///
    /// All geometry drawn after this call will be clipped to the specified rectangle.
    /// Call `reset_scissor()` to restore full viewport rendering.
    pub fn set_scissor(&mut self, scissor: ScissorRect) {
        self.draw_commands.push(DrawCommand::SetScissor(scissor));
    }

    /// Reset scissor to full viewport (no clipping).
    pub fn reset_scissor(&mut self) {
        self.draw_commands.push(DrawCommand::ResetScissor);
    }

    /// Render all queued geometry.
    pub fn render(&mut self, pass: &mut wgpu::RenderPass, viewport: Viewport) {
        profile_scope!("geometry_render_total");

        if self.draw_commands.is_empty() {
            return;
        }

        let num_commands = self.draw_commands.len();
        tracing::trace!("Rendering {} draw commands", num_commands);

        // Update projection
        let logical_size = viewport.to_logical();
        let physical_size = viewport.size; // Already physical
        let projection = ProjectionUniform::orthographic(logical_size.width, logical_size.height);
        self.renderer.queue().write_buffer(
            &self.projection_buffer,
            0,
            bytemuck::cast_slice(&[projection]),
        );

        // Scale factor for converting logical to physical coordinates
        let scale = viewport.scale_factor.0 as f32;

        // Collect all geometry data first
        let mut fill_vertices: Vec<FillVertex> = Vec::new();
        let mut fill_indices: Vec<u32> = Vec::new();
        let mut fill_instance_data: Vec<FillInstance> = Vec::new();

        let mut stroke_vertices: Vec<StrokeVertex> = Vec::new();
        let mut stroke_indices: Vec<u32> = Vec::new();
        let mut stroke_instance_data: Vec<StrokeInstance> = Vec::new();

        // Build a list of render operations with scissor state
        #[derive(Debug)]
        enum RenderOp {
            SetScissor(u32, u32, u32, u32), // x, y, w, h in physical pixels
            ResetScissor,
            DrawFill {
                index_start: u32,
                index_count: u32,
                instance_idx: u32,
            },
            DrawStroke {
                index_start: u32,
                index_count: u32,
                instance_idx: u32,
            },
        }

        let mut ops: Vec<RenderOp> = Vec::new();

        profile_scope!("collect_geometry");
        for cmd in &self.draw_commands {
            match cmd {
                DrawCommand::Fill {
                    mesh,
                    color,
                    offset,
                } => {
                    let vertex_offset = fill_vertices.len() as u32;
                    let index_start = fill_indices.len() as u32;

                    fill_vertices.extend_from_slice(&mesh.vertices);
                    fill_indices.extend(mesh.indices.iter().map(|i| i + vertex_offset));

                    let instance_idx = fill_instance_data.len() as u32;
                    fill_instance_data.push(FillInstance::new(
                        offset.x,
                        offset.y,
                        [color.r, color.g, color.b, color.a],
                    ));

                    ops.push(RenderOp::DrawFill {
                        index_start,
                        index_count: mesh.indices.len() as u32,
                        instance_idx,
                    });
                }
                DrawCommand::Stroke {
                    mesh,
                    color,
                    width,
                    offset,
                } => {
                    let vertex_offset = stroke_vertices.len() as u32;
                    let index_start = stroke_indices.len() as u32;

                    stroke_vertices.extend_from_slice(&mesh.vertices);
                    stroke_indices.extend(mesh.indices.iter().map(|i| i + vertex_offset));

                    let instance_idx = stroke_instance_data.len() as u32;
                    stroke_instance_data.push(StrokeInstance::new(
                        offset.x,
                        offset.y,
                        [color.r, color.g, color.b, color.a],
                        *width,
                    ));

                    ops.push(RenderOp::DrawStroke {
                        index_start,
                        index_count: mesh.indices.len() as u32,
                        instance_idx,
                    });
                }
                DrawCommand::SetScissor(scissor) => {
                    // Convert logical coordinates to physical pixels
                    let x = (scissor.x as f32 * scale) as u32;
                    let y = (scissor.y as f32 * scale) as u32;
                    let w = (scissor.width as f32 * scale) as u32;
                    let h = (scissor.height as f32 * scale) as u32;
                    ops.push(RenderOp::SetScissor(x, y, w, h));
                }
                DrawCommand::ResetScissor => {
                    ops.push(RenderOp::ResetScissor);
                }
            }
        }

        // Create/update fill buffers
        {
            profile_scope!("create_fill_buffers");
            if !fill_vertices.is_empty() {
                self.fill_vertex_buffer = Some(self.context.device().create_buffer_init(
                    &wgpu::util::BufferInitDescriptor {
                        label: Some("Fill Vertex Buffer"),
                        contents: bytemuck::cast_slice(&fill_vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    },
                ));
                self.fill_index_buffer = Some(self.context.device().create_buffer_init(
                    &wgpu::util::BufferInitDescriptor {
                        label: Some("Fill Index Buffer"),
                        contents: bytemuck::cast_slice(&fill_indices),
                        usage: wgpu::BufferUsages::INDEX,
                    },
                ));
                self.fill_instances
                    .set_instances(self.context.device(), fill_instance_data);
                self.fill_instances.upload_dirty(self.renderer.queue());
            }
        }

        // Create/update stroke buffers
        {
            profile_scope!("create_stroke_buffers");
            if !stroke_vertices.is_empty() {
                self.stroke_vertex_buffer = Some(self.context.device().create_buffer_init(
                    &wgpu::util::BufferInitDescriptor {
                        label: Some("Stroke Vertex Buffer"),
                        contents: bytemuck::cast_slice(&stroke_vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    },
                ));
                self.stroke_index_buffer = Some(self.context.device().create_buffer_init(
                    &wgpu::util::BufferInitDescriptor {
                        label: Some("Stroke Index Buffer"),
                        contents: bytemuck::cast_slice(&stroke_indices),
                        usage: wgpu::BufferUsages::INDEX,
                    },
                ));
                self.stroke_instances
                    .set_instances(self.context.device(), stroke_instance_data);
                self.stroke_instances.upload_dirty(self.renderer.queue());
            }
        }

        // Track current pipeline state
        let mut fill_pipeline_bound = false;
        let mut stroke_pipeline_bound = false;

        // Execute render operations in order
        profile_scope!("execute_draw_ops");
        for op in ops {
            match op {
                RenderOp::SetScissor(x, y, w, h) => {
                    pass.set_scissor_rect(x, y, w, h);
                }
                RenderOp::ResetScissor => {
                    pass.set_scissor_rect(
                        0,
                        0,
                        physical_size.width as u32,
                        physical_size.height as u32,
                    );
                }
                RenderOp::DrawFill {
                    index_start,
                    index_count,
                    instance_idx,
                } => {
                    if let (Some(vbo), Some(ibo)) =
                        (&self.fill_vertex_buffer, &self.fill_index_buffer)
                    {
                        if !fill_pipeline_bound {
                            pass.set_pipeline(&self.fill_pipeline);
                            pass.set_bind_group(0, &self.projection_bind_group, &[]);
                            pass.set_vertex_buffer(0, vbo.slice(..));
                            pass.set_vertex_buffer(1, self.fill_instances.buffer().slice(..));
                            pass.set_index_buffer(ibo.slice(..), wgpu::IndexFormat::Uint32);
                            fill_pipeline_bound = true;
                            stroke_pipeline_bound = false;
                        }
                        pass.draw_indexed(
                            index_start..(index_start + index_count),
                            0,
                            instance_idx..(instance_idx + 1),
                        );
                    }
                }
                RenderOp::DrawStroke {
                    index_start,
                    index_count,
                    instance_idx,
                } => {
                    if let (Some(vbo), Some(ibo)) =
                        (&self.stroke_vertex_buffer, &self.stroke_index_buffer)
                    {
                        if !stroke_pipeline_bound {
                            pass.set_pipeline(&self.stroke_pipeline);
                            pass.set_bind_group(0, &self.projection_bind_group, &[]);
                            pass.set_vertex_buffer(0, vbo.slice(..));
                            pass.set_vertex_buffer(1, self.stroke_instances.buffer().slice(..));
                            pass.set_index_buffer(ibo.slice(..), wgpu::IndexFormat::Uint32);
                            stroke_pipeline_bound = true;
                            fill_pipeline_bound = false;
                        }
                        pass.draw_indexed(
                            index_start..(index_start + index_count),
                            0,
                            instance_idx..(instance_idx + 1),
                        );
                    }
                }
            }
        }

        // Clear commands for next frame
        self.draw_commands.clear();
    }
}
