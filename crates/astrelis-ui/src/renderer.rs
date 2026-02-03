//! UI renderer for drawing widgets with WGPU.

use crate::clip::{ClipRect, should_clip};
#[cfg(feature = "docking")]
use crate::widgets::docking::plugin::CrossContainerPreview;
#[cfg(feature = "docking")]
use crate::widgets::docking::{DEFAULT_TAB_PADDING, DockAnimationState};
use crate::draw_list::{DrawCommand, DrawList};
use crate::glyph_atlas::glyphs_to_instances_into;
use crate::gpu_types::{ImageInstance, QuadInstance, QuadVertex, TextInstance};
use crate::instance_buffer::InstanceBuffer;
use crate::plugin::registry::{TraversalBehavior, WidgetTypeRegistry, WidgetRenderContext};
use crate::theme::ColorPalette;
use crate::tree::{NodeId, UiTree};
use crate::widgets::{Button, ImageTexture, Text};
use astrelis_core::alloc::HashMap;
use astrelis_core::math::Vec2;
use astrelis_core::profiling::{profile_function, profile_scope};
use astrelis_render::wgpu::util::DeviceExt;
use astrelis_render::{Color, GraphicsContext, Renderer, Viewport, wgpu};
use astrelis_text::{FontRenderer, FontSystem, TextPipeline};
use std::sync::Arc;

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

/// Batched image rendering data for a specific texture.
#[allow(dead_code)]
struct ImageBatch {
    /// The texture being rendered (kept for potential future use/debugging)
    texture: ImageTexture,
    /// Bind group for this texture
    bind_group: wgpu::BindGroup,
    /// Start index in the global instance buffer
    start_index: u32,
    /// Number of instances
    count: u32,
}

/// Batched rendering data for a specific clip rect.
#[derive(Clone)]
struct ClipBatch {
    /// The clip rect for this batch
    clip_rect: ClipRect,
    /// Quad instance range (start, count)
    quad_range: (u32, u32),
    /// Text instance range (start, count)
    text_range: (u32, u32),
}

/// Composite key for image bind group caching.
///
/// Combines texture pointer and sampling mode to uniquely identify
/// bind groups that need different samplers for the same texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ImageBindGroupKey {
    texture_ptr: usize,
    sampling: astrelis_render::ImageSampling,
}

/// UI renderer for rendering all widgets.
pub struct UiRenderer {
    renderer: Renderer,
    font_renderer: FontRenderer,
    context: Arc<GraphicsContext>,

    quad_instanced_pipeline: wgpu::RenderPipeline,
    text_instanced_pipeline: wgpu::RenderPipeline,
    image_instanced_pipeline: wgpu::RenderPipeline,

    unit_quad_vbo: wgpu::Buffer,

    projection_bind_group: wgpu::BindGroup,
    projection_buffer: wgpu::Buffer,
    text_atlas_bind_group: wgpu::BindGroup,
    text_projection_bind_group: wgpu::BindGroup,

    /// Bind group layout for image textures (reused for each texture)
    image_texture_bind_group_layout: wgpu::BindGroupLayout,
    /// Sampler cache for different sampling modes
    sampler_cache: astrelis_render::SamplerCache,
    /// Cache of bind groups for image textures (keyed by texture pointer + sampling mode)
    image_bind_group_cache: HashMap<ImageBindGroupKey, wgpu::BindGroup>,

    text_pipeline: TextPipeline,
    draw_list: DrawList,
    quad_instances: InstanceBuffer<QuadInstance>,
    text_instances: InstanceBuffer<TextInstance>,
    image_instances: InstanceBuffer<ImageInstance>,
    /// Current frame's image batches (grouped by texture)
    image_batches: Vec<ImageBatch>,
    /// Current frame's clip batches (for scissor rect rendering)
    clip_batches: Vec<ClipBatch>,
    /// Whether any non-infinite clip rects exist (enables scissor rendering)
    has_clipping: bool,
    scale_factor: f64,

    // Persistent allocations for encode_instances() - reused each frame
    /// Reusable quad instance buffer
    frame_quad_instances: Vec<QuadInstance>,
    /// Reusable text instance buffer
    frame_text_instances: Vec<TextInstance>,
    /// Reusable image instance buffer
    frame_image_instances: Vec<ImageInstance>,
    /// Reusable image groups map
    frame_image_groups: HashMap<ImageBindGroupKey, (ImageTexture, Vec<ImageInstance>)>,

    /// Current theme colors for resolving widget defaults
    theme_colors: ColorPalette,
}

impl UiRenderer {
    /// Create a new UI renderer.
    pub fn new(context: Arc<GraphicsContext>) -> Self {
        let renderer = Renderer::new(context.clone());

        // Create font renderer for text
        let font_system = FontSystem::with_system_fonts();
        let font_renderer = FontRenderer::new(context.clone(), font_system);

        // 1. Create unit quad VBO for instanced rendering
        let unit_quad_vertices = QuadVertex::unit_quad();
        let unit_quad_vbo = context
            .device()
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
        let image_instanced_shader = renderer.create_shader(
            Some("Image Instanced Shader"),
            include_str!("../shaders/image_instanced.wgsl"),
        );

        // 3. Create projection uniform buffer
        let projection_buffer = context.device().create_buffer(&wgpu::BufferDescriptor {
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

        // Image texture bind group layout (group 0 for image shader)
        let image_texture_bind_group_layout = renderer.create_bind_group_layout(
            Some("Image Texture Bind Group Layout"),
            &[
                // Image texture
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
                // Image sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        );

        // Create sampler cache for different sampling modes
        let sampler_cache = astrelis_render::SamplerCache::new();

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

        // Image instanced pipeline
        let image_instanced_layout = renderer.create_pipeline_layout(
            Some("Image Instanced Pipeline Layout"),
            &[
                &image_texture_bind_group_layout,
                &text_projection_bind_group_layout, // Reuse projection layout
            ],
            &[],
        );

        let image_instanced_pipeline =
            renderer.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Image Instanced Pipeline"),
                layout: Some(&image_instanced_layout),
                vertex: wgpu::VertexState {
                    module: &image_instanced_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[QuadVertex::vertex_layout(), ImageInstance::vertex_layout()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &image_instanced_shader,
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
        let quad_instances = InstanceBuffer::new(context.device(), Some("Quad Instances"), 1024);
        let text_instances = InstanceBuffer::new(context.device(), Some("Text Instances"), 4096);
        let image_instances = InstanceBuffer::new(context.device(), Some("Image Instances"), 256);

        Self {
            renderer,
            font_renderer,
            context,
            quad_instanced_pipeline,
            text_instanced_pipeline,
            image_instanced_pipeline,
            unit_quad_vbo,
            projection_bind_group,
            projection_buffer,
            text_atlas_bind_group,
            text_projection_bind_group,
            image_texture_bind_group_layout,
            sampler_cache,
            image_bind_group_cache: HashMap::new(),
            text_pipeline,
            draw_list,
            quad_instances,
            text_instances,
            image_instances,
            image_batches: Vec::new(),
            clip_batches: Vec::new(),
            has_clipping: false,
            scale_factor: 1.0,
            // Pre-allocate persistent frame buffers
            frame_quad_instances: Vec::with_capacity(1024),
            frame_text_instances: Vec::with_capacity(4096),
            frame_image_instances: Vec::with_capacity(256),
            frame_image_groups: HashMap::new(),
            theme_colors: ColorPalette::dark(),
        }
    }

    /// Update the theme colors used for resolving widget defaults.
    pub fn set_theme_colors(&mut self, colors: ColorPalette) {
        self.theme_colors = colors;
    }

    pub fn set_viewport(&mut self, viewport: Viewport) {
        // Clear caches if scale factor changed
        // (shaped text positions and glyph cache keys are scale-dependent)
        if (self.scale_factor - viewport.scale_factor.0).abs() > f64::EPSILON {
            self.text_pipeline.clear_cache();
            self.draw_list.clear(); // Force re-render of all nodes
        }
        self.scale_factor = viewport.scale_factor.0;
        self.font_renderer.set_viewport(viewport);
    }

    /// Get reference to the font renderer for text measurement.
    pub fn font_renderer(&self) -> &FontRenderer {
        &self.font_renderer
    }

    /// Clear the draw list.
    ///
    /// This should be called when the UI tree is rebuilt to ensure
    /// stale draw commands are removed.
    pub fn clear_draw_list(&mut self) {
        self.draw_list.clear();
    }

    /// Remove draw commands for nodes that have been removed from the tree.
    ///
    /// Called with the list of removed node IDs so the renderer stops
    /// drawing stale content (ghost tabs, collapsed containers, etc.).
    pub fn remove_stale_nodes(&mut self, removed_nodes: &[NodeId]) {
        for &node_id in removed_nodes {
            self.draw_list.remove_node(node_id);
        }
    }

    /// Update retained rendering state from the UI tree.
    ///
    /// This processes text shaping, updates the draw list from dirty nodes,
    /// encodes instances, and uploads to GPU buffers.
    pub fn update(&mut self, tree: &UiTree, widget_registry: &WidgetTypeRegistry) {
        profile_function!();

        // 1. Process text shaping
        self.process_text_shaping();

        // 2. Update DrawList from dirty nodes
        self.update_draw_list(tree, widget_registry);

        // 3. Update cross-container drop preview overlay (docking only)
        #[cfg(feature = "docking")]
        self.update_preview_overlay(None);

        // 4. Encode to instances
        self.encode_instances();

        // 5. Upload to GPU
        self.upload_instances();
    }

    /// Update retained rendering state with optional cross-container preview and animations.
    ///
    /// When a tab is being dragged over another container, the preview shows
    /// a semi-transparent overlay indicating where the tab will be dropped.
    /// Ghost overlays from the animation state follow the cursor during drag.
    #[cfg(feature = "docking")]
    pub fn update_with_preview(
        &mut self,
        tree: &UiTree,
        preview: Option<&CrossContainerPreview>,
        animations: Option<&DockAnimationState>,
        widget_registry: &WidgetTypeRegistry,
    ) {
        profile_function!();

        // 1. Process text shaping
        self.process_text_shaping();

        // 2. Update DrawList from dirty nodes
        self.update_draw_list(tree, widget_registry);

        // 3. Update cross-container drop preview overlay
        self.update_preview_overlay(preview);

        // 3b. Update ghost overlays from animation state
        self.update_ghost_overlays(animations);

        // 4. Encode to instances
        self.encode_instances();

        // 5. Upload to GPU
        self.upload_instances();
    }

    /// Process pending text shaping requests.
    fn process_text_shaping(&mut self) {
        profile_function!();
        let font_system = self.font_renderer.font_system();

        let shape_fn = |text: &str, font_size: f32, wrap_width: Option<f32>| {
            let mut font_sys = font_system.write().unwrap();
            astrelis_text::shape_text(
                &mut font_sys,
                text,
                font_size,
                wrap_width,
                self.scale_factor as f32,
            )
        };

        self.text_pipeline.process_pending(shape_fn);
    }

    /// Update draw list from dirty nodes in the tree.
    ///
    /// Two paths:
    /// 1. Full rebuild: When draw list is empty, rebuild all nodes from tree root
    /// 2. Incremental update: When draw list has content, only update dirty nodes
    fn update_draw_list(&mut self, tree: &UiTree, widget_registry: &WidgetTypeRegistry) {
        profile_function!();

        // If draw list is empty, do a full rebuild from tree root
        // This handles both initial render AND full rebuilds (after ui.build())
        if self.draw_list.is_empty() {
            profile_scope!("full_rebuild");
            // Request shaping for all text first
            if let Some(root) = tree.root() {
                self.request_text_shaping_recursive(tree, root, widget_registry);
            }

            // Process all pending text shaping
            self.process_text_shaping();

            // Build all nodes with shaped text available
            if let Some(root) = tree.root() {
                self.build_all_nodes_recursive(tree, root, widget_registry);
            }

            self.draw_list.sort_if_needed();
            return;
        }

        // Incremental update path - only process dirty nodes
        let dirty_roots = tree.dirty_roots();
        let has_dirty = !dirty_roots.is_empty() || !tree.dirty_nodes().is_empty();

        if !has_dirty {
            // Nothing to update
            self.draw_list.sort_if_needed();
            return;
        }

        profile_scope!("update_dirty_nodes");

        // Use dirty_roots if available, otherwise fall back to root
        let roots_to_process: Vec<NodeId> = if dirty_roots.is_empty() {
            tree.root().into_iter().collect()
        } else {
            dirty_roots.iter().copied().collect()
        };

        // Collect dirty nodes starting from dirty roots only (skip clean subtrees)
        let mut dirty_nodes_with_clips: Vec<(NodeId, ClipRect)> = Vec::new();

        for &root_id in &roots_to_process {
            // Compute inherited clip once per dirty root
            let root_clip = self.compute_inherited_clip(tree, root_id, widget_registry);

            // Collect dirty nodes from this subtree with their inherited clips
            self.collect_dirty_nodes_with_clips(
                tree,
                root_id,
                root_clip,
                &mut dirty_nodes_with_clips,
                widget_registry,
            );
        }

        // Request text shaping for all dirty nodes
        for &(node_id, _) in &dirty_nodes_with_clips {
            self.request_text_for_node(tree, node_id);
        }

        // Process pending shaping
        self.process_text_shaping();

        // Rebuild dirty nodes with pre-computed clips
        for (node_id, clip) in dirty_nodes_with_clips {
            self.update_single_node_with_clip(tree, node_id, clip, widget_registry);
        }

        self.draw_list.sort_if_needed();
    }

    /// Request text shaping for all nodes recursively (first pass).
    fn request_text_shaping_recursive(&mut self, tree: &UiTree, node_id: NodeId, widget_registry: &WidgetTypeRegistry) {
        self.request_text_for_node(tree, node_id);

        // Recurse to children using registry traversal behavior
        if let Some(widget) = tree.get_widget(node_id) {
            let traversal = widget_registry
                .get(widget.as_any().type_id())
                .and_then(|desc| desc.traversal)
                .map(|f| f(widget.as_any()))
                .unwrap_or(TraversalBehavior::Normal);

            match traversal {
                TraversalBehavior::Normal => {
                    for &child_id in widget.children() {
                        self.request_text_shaping_recursive(tree, child_id, widget_registry);
                    }
                }
                TraversalBehavior::OnlyChild(index) => {
                    if let Some(&child_id) = widget.children().get(index) {
                        self.request_text_shaping_recursive(tree, child_id, widget_registry);
                    }
                }
                TraversalBehavior::Skip => {}
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
            let font_id = text.font_id;
            self.text_pipeline
                .request_shape(text.content.clone(), font_id, text.font_size, None);
        } else if let Some(button) = widget.as_any().downcast_ref::<Button>() {
            let font_id = button.font_id;
            self.text_pipeline
                .request_shape(button.label.clone(), font_id, button.font_size, None);
        }
    }

    /// Build all nodes recursively (for initial render).
    fn build_all_nodes_recursive(&mut self, tree: &UiTree, node_id: NodeId, widget_registry: &WidgetTypeRegistry) {
        self.build_all_nodes_recursive_with_clip(tree, node_id, ClipRect::infinite(), widget_registry);
    }

    /// Build all nodes recursively with inherited clip rect.
    fn build_all_nodes_recursive_with_clip(
        &mut self,
        tree: &UiTree,
        node_id: NodeId,
        inherited_clip: ClipRect,
        widget_registry: &WidgetTypeRegistry,
    ) {
        // Compute this node's clip rect (may modify inherited_clip for children)
        let (node_clip, child_clip) = self.compute_node_clip(tree, node_id, inherited_clip, widget_registry);

        // Build this node with its clip rect
        self.update_single_node_with_clip(tree, node_id, node_clip, widget_registry);

        // Recurse to children using registry traversal behavior
        if let Some(widget) = tree.get_widget(node_id) {
            let traversal = widget_registry
                .get(widget.as_any().type_id())
                .and_then(|desc| desc.traversal)
                .map(|f| f(widget.as_any()))
                .unwrap_or(TraversalBehavior::Normal);

            match traversal {
                TraversalBehavior::Normal => {
                    for &child_id in widget.children() {
                        self.build_all_nodes_recursive_with_clip(tree, child_id, child_clip, widget_registry);
                    }
                }
                TraversalBehavior::OnlyChild(index) => {
                    // Clear draw commands for inactive children
                    for (i, &child_id) in widget.children().iter().enumerate() {
                        if i != index {
                            self.clear_node_recursive(tree, child_id);
                        }
                    }
                    // Only recurse into the active child
                    if let Some(&child_id) = widget.children().get(index) {
                        self.build_all_nodes_recursive_with_clip(tree, child_id, child_clip, widget_registry);
                    }
                }
                TraversalBehavior::Skip => {}
            }
        }
    }

    /// Compute the clip rect for a node based on its overflow settings.
    ///
    /// Returns (clip for this node's content, clip to pass to children).
    fn compute_node_clip(
        &self,
        tree: &UiTree,
        node_id: NodeId,
        inherited_clip: ClipRect,
        widget_registry: &WidgetTypeRegistry,
    ) -> (ClipRect, ClipRect) {
        let Some(widget) = tree.get_widget(node_id) else {
            return (inherited_clip, inherited_clip);
        };

        let Some(layout) = tree.get_layout(node_id) else {
            return (inherited_clip, inherited_clip);
        };

        // Check if this node has overflow clipping via registry dispatch
        let type_id = widget.as_any().type_id();
        let (overflow_x, overflow_y) = if let Some(desc) = widget_registry.get(type_id) {
            if let Some(overflow_fn) = desc.overflow {
                let o = overflow_fn(widget.as_any());
                (o.overflow_x, o.overflow_y)
            } else {
                // Default: check style directly for containers, visible for others
                (widget.style().overflow_x, widget.style().overflow_y)
            }
        } else {
            (widget.style().overflow_x, widget.style().overflow_y)
        };

        // If this node clips its children, compute the new clip rect
        if should_clip(overflow_x, overflow_y) {
            // Calculate absolute position
            let mut abs_x = layout.x;
            let mut abs_y = layout.y;
            let mut current_parent = tree.get_node(node_id).and_then(|n| n.parent);

            while let Some(parent_id) = current_parent {
                if let Some(parent_layout) = tree.get_layout(parent_id) {
                    abs_x += parent_layout.x;
                    abs_y += parent_layout.y;
                }
                // Subtract scroll offset if parent has a scroll_offset handler
                if let Some(parent_widget) = tree.get_widget(parent_id) {
                    let parent_type_id = parent_widget.as_any().type_id();
                    if let Some(desc) = widget_registry.get(parent_type_id)
                        && let Some(scroll_offset_fn) = desc.scroll_offset {
                            let offset = scroll_offset_fn(parent_widget.as_any());
                            abs_x -= offset.x;
                            abs_y -= offset.y;
                        }
                }
                current_parent = tree.get_node(parent_id).and_then(|n| n.parent);
            }

            // Create clip rect from node bounds
            let node_bounds = ClipRect::from_bounds(abs_x, abs_y, layout.width, layout.height);

            // Intersect with inherited clip (for nested clipping)
            let child_clip = inherited_clip.intersect(&node_bounds);

            // Container clips both its own content and children
            (child_clip, child_clip)
        } else {
            // No clipping, pass through inherited
            (inherited_clip, inherited_clip)
        }
    }

    /// Compute the inherited clip rect for a node by walking up to ancestors.
    ///
    /// This is used by the dirty update path to determine the correct clip rect
    /// for a node without rebuilding the entire tree.
    fn compute_inherited_clip(&self, tree: &UiTree, node_id: NodeId, widget_registry: &WidgetTypeRegistry) -> ClipRect {
        let mut clip = ClipRect::infinite();

        // Walk up the tree collecting ancestors
        let mut current = tree.get_node(node_id).and_then(|n| n.parent);
        let mut ancestors = Vec::new();

        while let Some(parent_id) = current {
            ancestors.push(parent_id);
            current = tree.get_node(parent_id).and_then(|n| n.parent);
        }

        // Process from root down to build proper nested clips
        for &ancestor_id in ancestors.iter().rev() {
            let (_, child_clip) = self.compute_node_clip(tree, ancestor_id, clip, widget_registry);
            clip = child_clip;
        }

        clip
    }

    /// Collect dirty nodes from a subtree with pre-computed inherited clips.
    ///
    /// This optimized version:
    /// - Starts from a dirty root instead of tree root
    /// - Computes child clips incrementally as it traverses down
    /// - Avoids redundant walks up the tree for each dirty node
    fn collect_dirty_nodes_with_clips(
        &self,
        tree: &UiTree,
        node_id: NodeId,
        inherited_clip: ClipRect,
        dirty_nodes: &mut Vec<(NodeId, ClipRect)>,
        widget_registry: &WidgetTypeRegistry,
    ) {
        let Some(_node) = tree.get_node(node_id) else {
            return;
        };

        // Collect ALL nodes in dirty subtrees, not just dirty ones.
        // When a subtree root is dirty, all descendants need draw command updates
        // because their absolute positions depend on ancestor layouts.
        dirty_nodes.push((node_id, inherited_clip));

        // Compute clip for children (this node may affect child clips)
        let (_, child_clip) = self.compute_node_clip(tree, node_id, inherited_clip, widget_registry);

        // Recurse to children using registry traversal behavior
        if let Some(widget) = tree.get_widget(node_id) {
            let traversal = widget_registry
                .get(widget.as_any().type_id())
                .and_then(|desc| desc.traversal)
                .map(|f| f(widget.as_any()))
                .unwrap_or(TraversalBehavior::Normal);

            match traversal {
                TraversalBehavior::Normal => {
                    for &child_id in widget.children() {
                        self.collect_dirty_nodes_with_clips(tree, child_id, child_clip, dirty_nodes, widget_registry);
                    }
                }
                TraversalBehavior::OnlyChild(index) => {
                    if let Some(&child_id) = widget.children().get(index) {
                        self.collect_dirty_nodes_with_clips(tree, child_id, child_clip, dirty_nodes, widget_registry);
                    }
                }
                TraversalBehavior::Skip => {}
            }
        }
    }

    /// Update commands for a single node with a specific clip rect.
    fn update_single_node_with_clip(
        &mut self,
        tree: &UiTree,
        node_id: NodeId,
        clip_rect: ClipRect,
        widget_registry: &WidgetTypeRegistry,
    ) {
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
            // Subtract scroll offset if parent has a scroll_offset handler
            if let Some(parent_widget) = tree.get_widget(parent_id) {
                let parent_type_id = parent_widget.as_any().type_id();
                if let Some(desc) = widget_registry.get(parent_type_id)
                    && let Some(scroll_offset_fn) = desc.scroll_offset {
                        abs_offset -= scroll_offset_fn(parent_widget.as_any());
                    }
            }
            current_parent = tree.get_node(parent_id).and_then(|n| n.parent);
        }

        let abs_x = abs_offset.x;
        let abs_y = abs_offset.y;

        // Generate commands via registry-based dispatch
        let mut commands = Vec::new();

        {
            let type_id = widget.as_any().type_id();
            if let Some(descriptor) = widget_registry.get(type_id)
                && let Some(render_fn) = descriptor.render {
                    let mut render_ctx = WidgetRenderContext {
                        abs_position: Vec2::new(abs_x, abs_y),
                        layout_size: Vec2::new(layout.width, layout.height),
                        clip_rect,
                        theme_colors: &self.theme_colors,
                        text_pipeline: &mut self.text_pipeline,
                    };
                    commands = render_fn(widget.as_any(), &mut render_ctx);
                }
        }

        // Update commands for this node in the draw list
        self.draw_list.update_node(node_id, commands);
    }

    /// Sentinel node ID for the cross-container drop preview overlay.
    /// Uses a high ID unlikely to conflict with real tree nodes.
    #[cfg(feature = "docking")]
    const PREVIEW_OVERLAY_NODE: NodeId = NodeId(usize::MAX - 1);

    /// Sentinel node ID for the ghost tab overlay (single tab drag).
    #[cfg(feature = "docking")]
    const GHOST_TAB_OVERLAY_NODE: NodeId = NodeId(usize::MAX - 2);

    /// Sentinel node ID for the ghost group overlay (tab group drag).
    #[cfg(feature = "docking")]
    const GHOST_GROUP_OVERLAY_NODE: NodeId = NodeId(usize::MAX - 3);

    /// Update the cross-container drop preview overlay.
    ///
    /// Renders a semi-transparent rectangle showing where a tab will be dropped
    /// when dragging between different DockTabs containers.
    #[cfg(feature = "docking")]
    fn update_preview_overlay(&mut self, preview: Option<&CrossContainerPreview>) {
        match preview {
            Some(preview) => {
                let bounds = preview.preview_bounds;
                let fill_color = Color::from_rgba_u8(100, 150, 255, 60);
                let border_color = Color::from_rgba_u8(100, 150, 255, 180);

                let commands = vec![
                    // Semi-transparent fill
                    DrawCommand::Quad(
                        crate::draw_list::QuadCommand::rounded(
                            Vec2::new(bounds.x, bounds.y),
                            Vec2::new(bounds.width, bounds.height),
                            fill_color,
                            4.0,
                            10, // High Z to render on top of everything
                        )
                        .with_clip(ClipRect::infinite()),
                    ),
                    // Border outline
                    DrawCommand::Quad(
                        crate::draw_list::QuadCommand::bordered(
                            Vec2::new(bounds.x, bounds.y),
                            Vec2::new(bounds.width, bounds.height),
                            border_color,
                            2.0,
                            4.0,
                            11, // Even higher Z for border
                        )
                        .with_clip(ClipRect::infinite()),
                    ),
                ];

                self.draw_list
                    .update_node(Self::PREVIEW_OVERLAY_NODE, commands);
            }
            None => {
                // Remove preview if not active
                self.draw_list.remove_node(Self::PREVIEW_OVERLAY_NODE);
            }
        }
    }

    /// Update ghost overlay draw commands from the dock animation state.
    ///
    /// Renders semi-transparent floating ghost elements that follow the cursor
    /// during tab or tab-group drag operations.
    #[cfg(feature = "docking")]
    fn update_ghost_overlays(&mut self, animations: Option<&DockAnimationState>) {
        // Ghost group animation (entire tab group drag)
        match animations.and_then(|a| a.ghost_group.as_ref()) {
            Some(ghost) if !ghost.is_done() => {
                let mut commands = Vec::new();
                let alpha = (ghost.opacity * 255.0) as u8;

                // Background quad for the ghost group
                commands.push(DrawCommand::Quad(
                    crate::draw_list::QuadCommand::rounded(
                        ghost.position,
                        ghost.size,
                        Color::from_rgba_u8(60, 80, 120, alpha),
                        4.0,
                        12, // High Z-index above everything
                    )
                    .with_clip(ClipRect::infinite()),
                ));

                // Render each tab label in the ghost group
                let tab_height = ghost.size.y;
                let tab_font_size = 13.0_f32;
                let tab_padding = DEFAULT_TAB_PADDING;
                let mut x_offset = 0.0_f32;

                for label in &ghost.labels {
                    let request_id =
                        self.text_pipeline
                            .request_shape(label.clone(), 0, tab_font_size, None);

                    if let Some(shaped) = self.text_pipeline.get_completed(request_id) {
                        let text_height = shaped.bounds().1;
                        let text_x = ghost.position.x + x_offset + tab_padding;
                        let text_y = ghost.position.y + (tab_height - text_height) * 0.5;

                        let text_alpha = (ghost.opacity * 200.0) as u8;
                        commands.push(DrawCommand::Text(
                            crate::draw_list::TextCommand::new(
                                Vec2::new(text_x, text_y),
                                shaped.clone(),
                                Color::from_rgba_u8(220, 220, 220, text_alpha),
                                13,
                            )
                            .with_clip(ClipRect::infinite()),
                        ));

                        // Advance x for next tab label (text width + padding + separator gap)
                        let text_width = shaped.bounds().0;
                        x_offset += text_width + tab_padding * 2.0 + 2.0;
                    }
                }

                self.draw_list
                    .update_node(Self::GHOST_GROUP_OVERLAY_NODE, commands);
            }
            _ => {
                self.draw_list.remove_node(Self::GHOST_GROUP_OVERLAY_NODE);
            }
        }

        // Ghost tab animation (single tab cross-container drag)
        match animations.and_then(|a| a.ghost_tab.as_ref()) {
            Some(ghost) if !ghost.is_done() => {
                let mut commands = Vec::new();
                let alpha = (ghost.opacity * 255.0) as u8;

                // Background quad for the ghost tab
                commands.push(DrawCommand::Quad(
                    crate::draw_list::QuadCommand::rounded(
                        ghost.position,
                        ghost.size,
                        Color::from_rgba_u8(60, 80, 120, alpha),
                        4.0,
                        12,
                    )
                    .with_clip(ClipRect::infinite()),
                ));

                // Tab label text
                let request_id =
                    self.text_pipeline
                        .request_shape(ghost.label.clone(), 0, 13.0, None);

                if let Some(shaped) = self.text_pipeline.get_completed(request_id) {
                    let text_height = shaped.bounds().1;
                    let text_x = ghost.position.x + DEFAULT_TAB_PADDING;
                    let text_y = ghost.position.y + (ghost.size.y - text_height) * 0.5;

                    let text_alpha = (ghost.opacity * 200.0) as u8;
                    commands.push(DrawCommand::Text(
                        crate::draw_list::TextCommand::new(
                            Vec2::new(text_x, text_y),
                            shaped,
                            Color::from_rgba_u8(220, 220, 220, text_alpha),
                            13,
                        )
                        .with_clip(ClipRect::infinite()),
                    ));
                }

                self.draw_list
                    .update_node(Self::GHOST_TAB_OVERLAY_NODE, commands);
            }
            _ => {
                self.draw_list.remove_node(Self::GHOST_TAB_OVERLAY_NODE);
            }
        }
    }

    /// Recursively clear draw commands for a node and all its children.
    fn clear_node_recursive(&mut self, tree: &UiTree, node_id: NodeId) {
        // Remove this node's draw commands
        self.draw_list.remove_node(node_id);

        // Recursively clear children
        if let Some(widget) = tree.get_widget(node_id) {
            for &child_id in widget.children() {
                self.clear_node_recursive(tree, child_id);
            }
        }
    }

    /// Encode draw list commands into GPU instance buffers.
    ///
    /// Groups instances by clip rect to ensure contiguous ranges for scissor batching.
    /// Uses a two-phase approach:
    /// 1. Collect unique clip rects
    /// 2. Process commands grouped by clip rect (ensures contiguous instance ranges)
    fn encode_instances(&mut self) {
        profile_function!();

        // Clear and reuse persistent allocations
        self.frame_quad_instances.clear();
        self.frame_text_instances.clear();
        self.frame_image_instances.clear();
        // Clear values but keep keys/capacity for image groups
        for (_, (_, instances)) in self.frame_image_groups.iter_mut() {
            instances.clear();
        }

        self.has_clipping = false;
        self.clip_batches.clear();

        // Phase 1: Collect unique clip rects in order of first appearance
        let mut clip_rects: Vec<ClipRect> = Vec::new();
        for cmd in self.draw_list.commands() {
            let clip = *cmd.clip_rect();
            if !clip.is_infinite() {
                self.has_clipping = true;
            }
            if !clip_rects.contains(&clip) {
                clip_rects.push(clip);
            }
        }

        // Ensure infinite clip is processed first (for proper draw order)
        if let Some(pos) = clip_rects.iter().position(|c| c.is_infinite()) {
            if pos != 0 {
                clip_rects.swap(0, pos);
            }
        } else if !clip_rects.is_empty() {
            // No infinite clip rect exists, insert at beginning
            clip_rects.insert(0, ClipRect::infinite());
        }

        // Phase 2: Process commands grouped by clip rect
        // This ensures instances for each clip rect are contiguous in the buffers
        for clip_rect in &clip_rects {
            let quad_start = self.frame_quad_instances.len() as u32;
            let text_start = self.frame_text_instances.len() as u32;

            for cmd in self.draw_list.commands() {
                if cmd.clip_rect() != clip_rect {
                    continue;
                }

                // Skip overlay sentinel commands — they are rendered in a final
                // pass after all clip batches so they always appear on top.
                #[cfg(feature = "docking")]
                {
                    let node_id = cmd.node_id();
                    if node_id == Self::PREVIEW_OVERLAY_NODE
                        || node_id == Self::GHOST_TAB_OVERLAY_NODE
                        || node_id == Self::GHOST_GROUP_OVERLAY_NODE
                    {
                        continue;
                    }
                }

                match cmd {
                    DrawCommand::Quad(q) => {
                        self.frame_quad_instances.push(QuadInstance {
                            position: [q.position.x, q.position.y],
                            size: [q.size.x, q.size.y],
                            color: [q.color.r, q.color.g, q.color.b, q.color.a],
                            border_radius: q.border_radius,
                            border_thickness: q.border_thickness,
                            _padding: [0.0; 2],
                        });
                    }
                    DrawCommand::Text(t) => {
                        glyphs_to_instances_into(
                            &mut self.font_renderer,
                            &t.shaped_text.inner.glyphs,
                            t.position,
                            t.color,
                            &mut self.frame_text_instances,
                        );
                    }
                    DrawCommand::Image(i) => {
                        // Use texture pointer + sampling mode as composite key for grouping
                        let bind_group_key = ImageBindGroupKey {
                            texture_ptr: std::sync::Arc::as_ptr(&i.texture) as usize,
                            sampling: i.sampling,
                        };

                        let instance = ImageInstance {
                            position: [i.position.x, i.position.y],
                            size: [i.size.x, i.size.y],
                            uv_min: [i.uv.u_min, i.uv.v_min],
                            uv_max: [i.uv.u_max, i.uv.v_max],
                            tint: [i.tint.r, i.tint.g, i.tint.b, i.tint.a],
                            border_radius: i.border_radius,
                            texture_index: 0, // Not used for now
                            _padding: [0.0; 2],
                        };

                        self.frame_image_groups
                            .entry(bind_group_key)
                            .or_insert_with(|| (i.texture.clone(), Vec::new()))
                            .1
                            .push(instance);
                    }
                }
            }

            let quad_count = self.frame_quad_instances.len() as u32 - quad_start;
            let text_count = self.frame_text_instances.len() as u32 - text_start;

            if quad_count > 0 || text_count > 0 {
                self.clip_batches.push(ClipBatch {
                    clip_rect: *clip_rect,
                    quad_range: (quad_start, quad_count),
                    text_range: (text_start, text_count),
                });
            }
        }

        // Encode overlay commands as a final clip batch so they render on top
        // of all regular content, regardless of which clip batch the content belongs to.
        #[cfg(feature = "docking")]
        {
            let overlay_quad_start = self.frame_quad_instances.len() as u32;
            let overlay_text_start = self.frame_text_instances.len() as u32;

            for cmd in self.draw_list.commands() {
                let node_id = cmd.node_id();
                if node_id != Self::PREVIEW_OVERLAY_NODE
                    && node_id != Self::GHOST_TAB_OVERLAY_NODE
                    && node_id != Self::GHOST_GROUP_OVERLAY_NODE
                {
                    continue;
                }

                match cmd {
                    DrawCommand::Quad(q) => {
                        self.frame_quad_instances.push(QuadInstance {
                            position: [q.position.x, q.position.y],
                            size: [q.size.x, q.size.y],
                            color: [q.color.r, q.color.g, q.color.b, q.color.a],
                            border_radius: q.border_radius,
                            border_thickness: q.border_thickness,
                            _padding: [0.0; 2],
                        });
                    }
                    DrawCommand::Text(t) => {
                        glyphs_to_instances_into(
                            &mut self.font_renderer,
                            &t.shaped_text.inner.glyphs,
                            t.position,
                            t.color,
                            &mut self.frame_text_instances,
                        );
                    }
                    DrawCommand::Image(_) => {} // Overlays don't use images
                }
            }

            let overlay_quad_count = self.frame_quad_instances.len() as u32 - overlay_quad_start;
            let overlay_text_count = self.frame_text_instances.len() as u32 - overlay_text_start;

            if overlay_quad_count > 0 || overlay_text_count > 0 {
                self.clip_batches.push(ClipBatch {
                    clip_rect: ClipRect::infinite(),
                    quad_range: (overlay_quad_start, overlay_quad_count),
                    text_range: (overlay_text_start, overlay_text_count),
                });
            }
        }

        // Build image batches with bind groups (images don't support clipping yet)
        // First, collect the data we need to avoid borrow conflicts
        let image_group_data: Vec<(ImageBindGroupKey, ImageTexture, Vec<ImageInstance>)> = self
            .frame_image_groups
            .iter()
            .filter(|(_, (_, instances))| !instances.is_empty())
            .map(|(key, (texture, instances))| (*key, texture.clone(), instances.clone()))
            .collect();

        self.image_batches.clear();
        self.frame_image_instances.clear();

        for (bind_group_key, texture, instances) in image_group_data {
            let start_index = self.frame_image_instances.len() as u32;
            let count = instances.len() as u32;

            // Get or create bind group for this texture + sampling mode combination
            let bind_group = self.get_or_create_image_bind_group(bind_group_key, &texture);

            self.image_batches.push(ImageBatch {
                texture,
                bind_group,
                start_index,
                count,
            });

            self.frame_image_instances.extend(instances);
        }

        // Upload to GPU instance buffers
        self.quad_instances.set_instances(
            self.renderer.device(),
            std::mem::take(&mut self.frame_quad_instances),
        );
        self.text_instances.set_instances(
            self.renderer.device(),
            std::mem::take(&mut self.frame_text_instances),
        );
        self.image_instances.set_instances(
            self.renderer.device(),
            std::mem::take(&mut self.frame_image_instances),
        );
    }

    /// Get or create a bind group for an image texture with a specific sampling mode.
    fn get_or_create_image_bind_group(
        &mut self,
        key: ImageBindGroupKey,
        texture: &ImageTexture,
    ) -> wgpu::BindGroup {
        if let Some(bind_group) = self.image_bind_group_cache.get(&key) {
            return bind_group.clone();
        }

        // Get the appropriate sampler for this sampling mode
        let sampler = self
            .sampler_cache
            .from_sampling(self.context.device(), key.sampling);

        let bind_group = self
            .context
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Image Texture Bind Group"),
                layout: &self.image_texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(texture),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });

        self.image_bind_group_cache.insert(key, bind_group.clone());
        bind_group
    }

    /// Upload dirty instance ranges to GPU.
    fn upload_instances(&mut self) {
        profile_function!();

        self.quad_instances.upload_dirty(self.renderer.queue());
        self.text_instances.upload_dirty(self.renderer.queue());
        self.image_instances.upload_dirty(self.renderer.queue());

        self.font_renderer.upload_atlas_if_dirty();
    }

    /// Render using retained mode instanced rendering.
    pub fn render_instanced(
        &mut self,
        tree: &UiTree,
        render_pass: &mut wgpu::RenderPass,
        viewport: Viewport,
        widget_registry: &WidgetTypeRegistry,
    ) {
        profile_function!();

        // Update state
        self.update(tree, widget_registry);

        // physical size -> logical size -> NDC
        let logical_size = viewport.to_logical();
        let projection = orthographic_projection(logical_size.width, logical_size.height);
        self.renderer.queue().write_buffer(
            &self.projection_buffer,
            0,
            bytemuck::cast_slice(&projection),
        );

        let viewport_width = viewport.size.width as u32;
        let viewport_height = viewport.size.height as u32;

        // Render with clip batches (handles scissor rects)
        for batch in &self.clip_batches {
            // Set scissor rect for this batch
            if batch.clip_rect.is_infinite() {
                render_pass.set_scissor_rect(0, 0, viewport_width, viewport_height);
            } else {
                let physical = batch.clip_rect.to_physical(viewport.scale_factor.0);
                let clamped = physical.clamp_to_viewport(viewport_width, viewport_height);
                if clamped.width == 0 || clamped.height == 0 {
                    continue; // Skip empty clip rects
                }
                render_pass.set_scissor_rect(clamped.x, clamped.y, clamped.width, clamped.height);
            }

            // Render quads for this clip batch
            if batch.quad_range.1 > 0 {
                render_pass.set_pipeline(&self.quad_instanced_pipeline);
                render_pass.set_bind_group(0, &self.projection_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.unit_quad_vbo.slice(..));
                render_pass.set_vertex_buffer(1, self.quad_instances.buffer().slice(..));
                render_pass.draw(
                    0..6,
                    batch.quad_range.0..(batch.quad_range.0 + batch.quad_range.1),
                );
            }

            // Render text for this clip batch
            if batch.text_range.1 > 0 {
                render_pass.set_pipeline(&self.text_instanced_pipeline);
                render_pass.set_bind_group(0, &self.text_atlas_bind_group, &[]);
                render_pass.set_bind_group(1, &self.text_projection_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.unit_quad_vbo.slice(..));
                render_pass.set_vertex_buffer(1, self.text_instances.buffer().slice(..));
                render_pass.draw(
                    0..6,
                    batch.text_range.0..(batch.text_range.0 + batch.text_range.1),
                );
            }
        }

        // Reset scissor rect for images (they don't support clipping yet)
        render_pass.set_scissor_rect(0, 0, viewport_width, viewport_height);

        // Render images (batched by texture)
        if !self.image_batches.is_empty() {
            render_pass.set_pipeline(&self.image_instanced_pipeline);
            render_pass.set_bind_group(1, &self.text_projection_bind_group, &[]); // Reuse projection
            render_pass.set_vertex_buffer(0, self.unit_quad_vbo.slice(..));
            render_pass.set_vertex_buffer(1, self.image_instances.buffer().slice(..));

            for batch in &self.image_batches {
                render_pass.set_bind_group(0, &batch.bind_group, &[]);
                render_pass.draw(0..6, batch.start_index..(batch.start_index + batch.count));
            }
        }
    }

    /// Render using retained mode instanced rendering with optional cross-container preview.
    #[cfg(feature = "docking")]
    pub fn render_instanced_with_preview(
        &mut self,
        tree: &UiTree,
        render_pass: &mut wgpu::RenderPass,
        viewport: Viewport,
        preview: Option<&CrossContainerPreview>,
        animations: Option<&DockAnimationState>,
        widget_registry: &WidgetTypeRegistry,
    ) {
        profile_function!();

        // Update state
        self.update_with_preview(tree, preview, animations, widget_registry);

        // physical size -> logical size -> NDC
        let logical_size = viewport.to_logical();
        let projection = orthographic_projection(logical_size.width, logical_size.height);
        self.renderer.queue().write_buffer(
            &self.projection_buffer,
            0,
            bytemuck::cast_slice(&projection),
        );

        let viewport_width = viewport.size.width as u32;
        let viewport_height = viewport.size.height as u32;

        // Render with clip batches (handles scissor rects)
        for batch in &self.clip_batches {
            // Set scissor rect for this batch
            if batch.clip_rect.is_infinite() {
                render_pass.set_scissor_rect(0, 0, viewport_width, viewport_height);
            } else {
                let physical = batch.clip_rect.to_physical(viewport.scale_factor.0);
                let clamped = physical.clamp_to_viewport(viewport_width, viewport_height);
                if clamped.width == 0 || clamped.height == 0 {
                    continue; // Skip empty clip rects
                }
                render_pass.set_scissor_rect(clamped.x, clamped.y, clamped.width, clamped.height);
            }

            // Render quads for this clip batch
            if batch.quad_range.1 > 0 {
                render_pass.set_pipeline(&self.quad_instanced_pipeline);
                render_pass.set_bind_group(0, &self.projection_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.unit_quad_vbo.slice(..));
                render_pass.set_vertex_buffer(1, self.quad_instances.buffer().slice(..));
                render_pass.draw(
                    0..6,
                    batch.quad_range.0..(batch.quad_range.0 + batch.quad_range.1),
                );
            }

            // Render text for this clip batch
            if batch.text_range.1 > 0 {
                render_pass.set_pipeline(&self.text_instanced_pipeline);
                render_pass.set_bind_group(0, &self.text_atlas_bind_group, &[]);
                render_pass.set_bind_group(1, &self.text_projection_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.unit_quad_vbo.slice(..));
                render_pass.set_vertex_buffer(1, self.text_instances.buffer().slice(..));
                render_pass.draw(
                    0..6,
                    batch.text_range.0..(batch.text_range.0 + batch.text_range.1),
                );
            }
        }

        // Reset scissor rect for images (they don't support clipping yet)
        render_pass.set_scissor_rect(0, 0, viewport_width, viewport_height);

        // Render images (batched by texture)
        if !self.image_batches.is_empty() {
            render_pass.set_pipeline(&self.image_instanced_pipeline);
            render_pass.set_bind_group(1, &self.text_projection_bind_group, &[]); // Reuse projection
            render_pass.set_vertex_buffer(0, self.unit_quad_vbo.slice(..));
            render_pass.set_vertex_buffer(1, self.image_instances.buffer().slice(..));

            for batch in &self.image_batches {
                render_pass.set_bind_group(0, &batch.bind_group, &[]);
                render_pass.draw(0..6, batch.start_index..(batch.start_index + batch.count));
            }
        }
    }

    /// Get text cache statistics for performance monitoring.
    ///
    /// NOTE: Phase 3 implementation caches measurements but not full text shaping.
    /// Hit rate shows measurement cache effectiveness. Full shaping cache requires
    /// Phase 5 (retained rendering).
    pub fn text_cache_stats(&self) -> String {
        // self.text_cache.stats_string()
        format!(
            "Text Cache Stats: {} entries, {:.1}% hit rate",
            self.text_pipeline.cache_stats().2,
            self.text_pipeline.cache_hit_rate()
        )
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

    /// Apply a scissor rect to the render pass.
    ///
    /// Converts a logical ClipRect to physical pixel coordinates and sets
    /// the scissor rect on the render pass. Handles clamping to viewport bounds.
    ///
    /// # Arguments
    /// * `render_pass` - The render pass to set the scissor on
    /// * `clip_rect` - The logical clip rectangle
    /// * `viewport` - The current viewport for scale factor and bounds
    ///
    /// # Returns
    /// `true` if the scissor rect has positive area (rendering should proceed),
    /// `false` if the clip rect is zero/negative area (skip rendering).
    pub fn apply_scissor_rect(
        render_pass: &mut wgpu::RenderPass,
        clip_rect: &ClipRect,
        viewport: &Viewport,
    ) -> bool {
        let viewport_width = viewport.size.width as u32;
        let viewport_height = viewport.size.height as u32;

        // Skip if infinite (no clipping needed)
        if clip_rect.is_infinite() {
            // Reset to full viewport
            render_pass.set_scissor_rect(0, 0, viewport_width, viewport_height);
            return true;
        }

        // Convert to physical coordinates
        let physical = clip_rect.to_physical(viewport.scale_factor.0);

        // Clamp to viewport bounds
        let clamped = physical.clamp_to_viewport(viewport_width, viewport_height);

        // Skip if no area to render
        if clamped.width == 0 || clamped.height == 0 {
            return false;
        }

        render_pass.set_scissor_rect(clamped.x, clamped.y, clamped.width, clamped.height);
        true
    }

    /// Reset the scissor rect to the full viewport.
    ///
    /// Call this after clipped rendering to restore normal rendering.
    pub fn reset_scissor_rect(render_pass: &mut wgpu::RenderPass, viewport: &Viewport) {
        let viewport_width = viewport.size.width as u32;
        let viewport_height = viewport.size.height as u32;
        render_pass.set_scissor_rect(0, 0, viewport_width, viewport_height);
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
