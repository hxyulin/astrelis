//! UI renderer for drawing widgets with WGPU.

use crate::draw_list::{DrawCommand, DrawList};
use crate::glyph_atlas::glyphs_to_instances;
use crate::gpu_types::{QuadInstance, QuadVertex, TextInstance};
use crate::instance_buffer::InstanceBuffer;
use crate::tree::{NodeId, UiTree};
use crate::widgets::{Button, Container, Text};
use astrelis_core::math::Vec2;
use astrelis_core::profiling::{profile_function, profile_scope};
use astrelis_render::wgpu::util::DeviceExt;
use astrelis_render::{Color, GraphicsContext, Renderer, Viewport, wgpu};
use astrelis_text::{FontRenderer, FontSystem, TextPipeline};

/// Vertex data for immediate mode quad rendering (legacy).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ImmediateModeQuadVertex {
    position: [f32; 2],
    color: [f32; 4],
    // UV coords for position within the quad (0-1 range)
    uv: [f32; 2],
    // Border radius and rect size for rounded corners
    border_radius: f32,
    rect_size: [f32; 2],
    // Border thickness (0 for filled, > 0 for border outline)
    border_thickness: f32,
}

/// UI renderer for rendering all widgets.
pub struct UiRenderer {
    renderer: Renderer,
    font_renderer: FontRenderer,

    quad_instanced_pipeline: wgpu::RenderPipeline,
    text_instanced_pipeline: wgpu::RenderPipeline,

    unit_quad_vbo: wgpu::Buffer,

    projection_bind_group: wgpu::BindGroup,
    projection_buffer: wgpu::Buffer,
    text_atlas_bind_group: wgpu::BindGroup,
    text_projection_bind_group: wgpu::BindGroup,

    text_pipeline: TextPipeline,
    draw_list: DrawList,
    quad_instances: InstanceBuffer<QuadInstance>,
    text_instances: InstanceBuffer<TextInstance>,
    scale_factor: f64,
}

impl UiRenderer {
    /// Create a new UI renderer.
    pub fn new(context: &'static GraphicsContext) -> Self {
        let renderer = Renderer::new(context);

        // Create font renderer for text
        let font_system = FontSystem::with_system_fonts();
        let font_renderer = FontRenderer::new(context, font_system);

        // 1. Create unit quad VBO for instanced rendering
        let unit_quad_vertices = QuadVertex::unit_quad();
        let unit_quad_vbo = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Unit Quad VBO"),
                contents: bytemuck::cast_slice(&unit_quad_vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        // 2. Load instanced shaders
        let quad_instanced_shader = renderer.create_shader(
            Some("Quad Instanced Shader"),
            include_str!("../shaders/quad_instanced.wgsl"),
        );
        let text_instanced_shader = renderer.create_shader(
            Some("Text Instanced Shader"),
            include_str!("../shaders/text_instanced.wgsl"),
        );

        // 3. Create projection uniform buffer
        let projection_buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Projection Uniform"),
            size: 64, // mat4x4<f32>
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 4. Create bind group layouts
        let projection_bind_group_layout = renderer.create_bind_group_layout(
            Some("Projection Bind Group Layout"),
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

        // Bind group layout for atlas texture and sampler (group 0)
        let text_atlas_bind_group_layout = renderer.create_bind_group_layout(
            Some("Text Atlas Bind Group Layout"),
            &[
                // Atlas texture
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
                // Atlas sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        );

        // Bind group layout for projection matrix (group 1, shared with quads)
        let text_projection_bind_group_layout = renderer.create_bind_group_layout(
            Some("Text Projection Bind Group Layout"),
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

        // 5. Create bind groups
        let projection_bind_group = renderer.create_bind_group(
            Some("Projection Bind Group"),
            &projection_bind_group_layout,
            &[wgpu::BindGroupEntry {
                binding: 0,
                resource: projection_buffer.as_entire_binding(),
            }],
        );

        // Atlas bind group (group 0 for text shader)
        let text_atlas_bind_group = renderer.create_bind_group(
            Some("Text Atlas Bind Group"),
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

        // Projection bind group for text (group 1, same as quads)
        let text_projection_bind_group = renderer.create_bind_group(
            Some("Text Projection Bind Group"),
            &text_projection_bind_group_layout,
            &[wgpu::BindGroupEntry {
                binding: 0,
                resource: projection_buffer.as_entire_binding(),
            }],
        );

        // 6. Create instanced pipelines
        let quad_instanced_layout = renderer.create_pipeline_layout(
            Some("Quad Instanced Pipeline Layout"),
            &[&projection_bind_group_layout],
            &[],
        );

        let quad_instanced_pipeline =
            renderer.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Quad Instanced Pipeline"),
                layout: Some(&quad_instanced_layout),
                vertex: wgpu::VertexState {
                    module: &quad_instanced_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[QuadVertex::vertex_layout(), QuadInstance::vertex_layout()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &quad_instanced_shader,
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

        let text_instanced_layout = renderer.create_pipeline_layout(
            Some("Text Instanced Pipeline Layout"),
            &[
                &text_atlas_bind_group_layout,
                &text_projection_bind_group_layout,
            ],
            &[],
        );

        let text_instanced_pipeline =
            renderer.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Text Instanced Pipeline"),
                layout: Some(&text_instanced_layout),
                vertex: wgpu::VertexState {
                    module: &text_instanced_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[QuadVertex::vertex_layout(), TextInstance::vertex_layout()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &text_instanced_shader,
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

        // 7. Initialize retained components
        let text_pipeline = TextPipeline::new();
        let draw_list = DrawList::new();
        let quad_instances = InstanceBuffer::new(&context.device, Some("Quad Instances"), 1024);
        let text_instances = InstanceBuffer::new(&context.device, Some("Text Instances"), 4096);

        Self {
            renderer,
            font_renderer,
            quad_instanced_pipeline,
            text_instanced_pipeline,
            unit_quad_vbo,
            projection_bind_group,
            projection_buffer,
            text_atlas_bind_group,
            text_projection_bind_group,
            text_pipeline,
            draw_list,
            quad_instances,
            text_instances,
            scale_factor: 1.0,
        }
    }

    pub fn set_viewport(&mut self, viewport: Viewport) {
        self.scale_factor = viewport.scale_factor;
        self.font_renderer.set_viewport(viewport);
    }

    /// Get reference to the font renderer for text measurement.
    pub fn font_renderer(&self) -> &FontRenderer {
        &self.font_renderer
    }

    /// Update retained rendering state from the UI tree.
    ///
    /// This processes text shaping, updates the draw list from dirty nodes,
    /// encodes instances, and uploads to GPU buffers.
    pub fn update(&mut self, tree: &UiTree) {
        profile_function!();

        // 1. Process text shaping
        self.process_text_shaping();

        // 2. Update DrawList from dirty nodes
        self.update_draw_list(tree);

        // 3. Encode to instances
        self.encode_instances();

        // 4. Upload to GPU
        self.upload_instances();
    }

    /// Process pending text shaping requests.
    fn process_text_shaping(&mut self) {
        profile_function!();
        let font_system = self.font_renderer.font_system();

        let shape_fn = |text: &str, font_size: f32, wrap_width: Option<f32>| {
            let mut font_sys = font_system.write().unwrap();
            astrelis_text::shape_text(&mut font_sys, text, font_size, wrap_width, self.scale_factor as f32)
        };

        self.text_pipeline.process_pending(shape_fn);
    }

    /// Update draw list from dirty nodes in the tree.
    fn update_draw_list(&mut self, tree: &UiTree) {
        profile_function!();

        // Collect dirty nodes (nodes with non-empty dirty flags)
        let mut dirty_nodes = Vec::new();
        if let Some(root) = tree.root() {
            profile_scope!("collect_dirty_nodes");
            self.collect_dirty_nodes_recursive(tree, root, &mut dirty_nodes);
        }

        // If no dirty nodes but draw list is empty, this is initial render
        // Build everything
        if dirty_nodes.is_empty() {
            if self.draw_list.is_empty() {
                profile_scope!("initial_render_build");
                // Initial render - request shaping for all text first
                if let Some(root) = tree.root() {
                    self.request_text_shaping_recursive(tree, root);
                }

                // Process all pending text shaping
                self.process_text_shaping();

                // Now build all nodes with shaped text available
                if let Some(root) = tree.root() {
                    self.build_all_nodes_recursive(tree, root);
                }
            }
            // Otherwise nothing to update
        } else {
            profile_scope!("update_dirty_nodes");

            // For dirty nodes: request shaping first
            for &node_id in &dirty_nodes {
                self.request_text_for_node(tree, node_id);
            }

            // Process pending shaping
            self.process_text_shaping();

            // Now rebuild with shaped text available
            for &node_id in &dirty_nodes {
                self.update_single_node(tree, node_id);
            }
        }

        self.draw_list.sort_if_needed();
    }

    /// Request text shaping for all nodes recursively (first pass).
    fn request_text_shaping_recursive(&mut self, tree: &UiTree, node_id: NodeId) {
        self.request_text_for_node(tree, node_id);

        // Recurse to children
        if let Some(widget) = tree.get_widget(node_id) {
            for &child_id in widget.children() {
                self.request_text_shaping_recursive(tree, child_id);
            }
        }
    }

    /// Request text shaping for a single node.
    fn request_text_for_node(&mut self, tree: &UiTree, node_id: NodeId) {
        profile_function!();

        let Some(widget) = tree.get_widget(node_id) else {
            return;
        };

        // Request shaping for text widgets
        if let Some(text) = widget.as_any().downcast_ref::<Text>() {
            let font_id = 0;
            self.text_pipeline.request_shape(
                text.content.get().to_string(),
                font_id,
                text.font_size,
                None,
            );
        } else if let Some(button) = widget.as_any().downcast_ref::<Button>() {
            let font_id = 0;
            self.text_pipeline.request_shape(
                button.label.get().to_string(),
                font_id,
                button.font_size,
                None,
            );
        }
    }

    /// Build all nodes recursively (for initial render).
    fn build_all_nodes_recursive(&mut self, tree: &UiTree, node_id: NodeId) {
        // Build this node
        self.update_single_node(tree, node_id);

        // Recurse to children
        if let Some(widget) = tree.get_widget(node_id) {
            for &child_id in widget.children() {
                self.build_all_nodes_recursive(tree, child_id);
            }
        }
    }

    /// Recursively collect dirty nodes from the tree.
    fn collect_dirty_nodes_recursive(
        &self,
        tree: &UiTree,
        node_id: NodeId,
        dirty_nodes: &mut Vec<NodeId>,
    ) {
        if let Some(node) = tree.get_node(node_id) {
            if !node.dirty_flags.is_empty() {
                dirty_nodes.push(node_id);
            }

            // Recurse to children
            if let Some(widget) = tree.get_widget(node_id) {
                for &child_id in widget.children() {
                    self.collect_dirty_nodes_recursive(tree, child_id, dirty_nodes);
                }
            }
        }
    }

    /// Update commands for a single node.
    fn update_single_node(&mut self, tree: &UiTree, node_id: NodeId) {
        profile_function!();

        let Some(widget) = tree.get_widget(node_id) else {
            return;
        };

        let Some(layout) = tree.get_layout(node_id) else {
            return;
        };

        // Calculate absolute position by walking up the tree
        let mut abs_offset = Vec2::new(layout.x, layout.y);
        let mut current_parent = tree.get_node(node_id).and_then(|n| n.parent);

        while let Some(parent_id) = current_parent {
            if let Some(parent_layout) = tree.get_layout(parent_id) {
                abs_offset.x += parent_layout.x;
                abs_offset.y += parent_layout.y;
            }
            current_parent = tree.get_node(parent_id).and_then(|n| n.parent);
        }

        let abs_x = abs_offset.x;
        let abs_y = abs_offset.y;

        // Generate commands based on widget type
        let mut commands = Vec::new();

        if let Some(container) = widget.as_any().downcast_ref::<Container>() {
            // Background quad
            if let Some(bg_color) = container.style.background_color {
                commands.push(DrawCommand::Quad(crate::draw_list::QuadCommand::rounded(
                    Vec2::new(abs_x, abs_y),
                    Vec2::new(layout.width, layout.height),
                    bg_color,
                    container.style.border_radius,
                    0,
                )));
            }

            // Border quad
            if container.style.border_width > 0.0 {
                if let Some(border_color) = container.style.border_color {
                    commands.push(DrawCommand::Quad(crate::draw_list::QuadCommand::bordered(
                        Vec2::new(abs_x, abs_y),
                        Vec2::new(layout.width, layout.height),
                        border_color,
                        container.style.border_width,
                        container.style.border_radius,
                        0,
                    )));
                }
            }
        } else if let Some(text) = widget.as_any().downcast_ref::<Text>() {
            // Request text shaping
            let font_id = 0; // TODO: Get actual font ID
            let request_id = self.text_pipeline.request_shape(
                text.content.get().to_string(),
                font_id,
                text.font_size,
                None,
            );

            // If shaping is complete, add text command
            if let Some(shaped) = self.text_pipeline.get_completed(request_id) {
                commands.push(DrawCommand::Text(crate::draw_list::TextCommand::new(
                    Vec2::new(abs_x, abs_y),
                    shaped,
                    text.color,
                    0,
                )));
            }
        } else if let Some(button) = widget.as_any().downcast_ref::<Button>() {
            // Use current background color based on state
            let bg_color = button.current_bg_color();

            // Background
            commands.push(DrawCommand::Quad(crate::draw_list::QuadCommand::rounded(
                Vec2::new(abs_x, abs_y),
                Vec2::new(layout.width, layout.height),
                bg_color,
                4.0,
                0,
            )));

            // Text label
            let font_id = 0;
            let request_id = self.text_pipeline.request_shape(
                button.label.get().to_string(),
                font_id,
                16.0,
                None,
            );

            if let Some(shaped) = self.text_pipeline.get_completed(request_id) {
                let text_x = abs_x + (layout.width - shaped.bounds().0) * 0.5;
                let text_y = abs_y + (layout.height - shaped.bounds().1) * 0.5;

                commands.push(DrawCommand::Text(crate::draw_list::TextCommand::new(
                    Vec2::new(text_x, text_y),
                    shaped,
                    Color::WHITE,
                    1,
                )));
            }
        }

        // Update commands for this node in the draw list
        self.draw_list.update_node(node_id, commands);
    }

    /// Encode draw list commands into GPU instance buffers.
    fn encode_instances(&mut self) {
        profile_function!();

        let mut quad_instances = Vec::new();
        let mut text_instances = Vec::new();

        for cmd in self.draw_list.commands() {
            match cmd {
                DrawCommand::Quad(q) => {
                    quad_instances.push(QuadInstance {
                        position: [q.position.x, q.position.y],
                        size: [q.size.x, q.size.y],
                        color: [q.color.r, q.color.g, q.color.b, q.color.a],
                        border_radius: q.border_radius,
                        border_thickness: q.border_thickness,
                        _padding: [0.0; 2],
                    });
                }
                DrawCommand::Text(t) => {
                    let instances = glyphs_to_instances(
                        &mut self.font_renderer,
                        &t.shaped_text.inner.glyphs,
                        t.position,
                        t.color,
                    );
                    text_instances.extend(instances);
                }
            }
        }

        self.quad_instances
            .set_instances(&self.renderer.device(), quad_instances);
        self.text_instances
            .set_instances(&self.renderer.device(), text_instances);
    }

    /// Upload dirty instance ranges to GPU.
    fn upload_instances(&mut self) {
        profile_function!();

        self.quad_instances.upload_dirty(self.renderer.queue());
        self.text_instances.upload_dirty(self.renderer.queue());

        self.font_renderer.upload_atlas_if_dirty();
    }

    /// Render using retained mode instanced rendering.
    pub fn render_instanced(
        &mut self,
        tree: &UiTree,
        render_pass: &mut wgpu::RenderPass,
        viewport: Viewport,
    ) {
        profile_function!();

        // Update state
        self.update(tree);

        // physical size -> logical size -> NDC
        let projection = orthographic_projection(viewport.width / viewport.scale_factor as f32,
                                                 viewport.height / viewport.scale_factor as f32);
        self.renderer.queue().write_buffer(
            &self.projection_buffer,
            0,
            bytemuck::cast_slice(&projection),
        );

        // Render quads
        if self.quad_instances.len() > 0 {
            render_pass.set_pipeline(&self.quad_instanced_pipeline);
            render_pass.set_bind_group(0, &self.projection_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.unit_quad_vbo.slice(..));
            render_pass.set_vertex_buffer(1, self.quad_instances.buffer().slice(..));
            render_pass.draw(0..6, 0..self.quad_instances.len() as u32);
        }

        // Render text
        if self.text_instances.len() > 0 {
            render_pass.set_pipeline(&self.text_instanced_pipeline);
            render_pass.set_bind_group(0, &self.text_atlas_bind_group, &[]);
            render_pass.set_bind_group(1, &self.text_projection_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.unit_quad_vbo.slice(..));
            render_pass.set_vertex_buffer(1, self.text_instances.buffer().slice(..));
            render_pass.draw(0..6, 0..self.text_instances.len() as u32);
        }
    }

    /// Get text cache statistics for performance monitoring.
    ///
    /// NOTE: Phase 3 implementation caches measurements but not full text shaping.
    /// Hit rate shows measurement cache effectiveness. Full shaping cache requires
    /// Phase 5 (retained rendering).
    pub fn text_cache_stats(&self) -> String {
        // self.text_cache.stats_string()
        format!("Text Cache Stats: {} entries, {:.1}% hit rate", self.text_pipeline.cache_stats().2, self.text_pipeline.cache_hit_rate())
    }

    /// Get text cache hit rate (0.0 to 1.0).
    pub fn text_cache_hit_rate(&self) -> f32 {
        self.text_pipeline.cache_hit_rate() / 100.0
    }

    /// Get average renders per cached entry (effectiveness metric).
    pub fn text_cache_effectiveness(&self) -> f32 {
        // self.text_cache.avg_renders_per_entry()
        0.0
    }

    /// Clear the text cache (useful when fonts are reloaded).
    pub fn clear_text_cache(&mut self) {
        self.text_pipeline.clear_cache();
    }

    /// Print text cache statistics to console.
    pub fn log_text_cache_stats(&self) {
        tracing::info!(
            "Text Cache Stats: {} entries, {:.1}% hit rate",
            self.text_pipeline.cache_stats().2,
            self.text_pipeline.cache_hit_rate()
        );
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
