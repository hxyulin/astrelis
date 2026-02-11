//! UI renderer for drawing widgets with WGPU.

use crate::clip::{ClipRect, should_clip};
use crate::draw_list::{DrawCommand, DrawList, RenderLayer};
use crate::glyph_atlas::glyphs_to_instances_into;
use crate::gpu_types::{ImageInstance, QuadInstance, QuadVertex, TextInstance};
use crate::instance_buffer::InstanceBuffer;
use crate::plugin::registry::{TraversalBehavior, WidgetRenderContext, WidgetTypeRegistry};
use crate::theme::ColorPalette;
use crate::tree::{NodeId, UiTree};
#[cfg(feature = "docking")]
use crate::widgets::docking::plugin::CrossContainerPreview;
#[cfg(feature = "docking")]
use crate::widgets::docking::{DEFAULT_TAB_PADDING, DockAnimationState};
use crate::widgets::{Button, ImageTexture, Text};
use astrelis_core::alloc::{HashMap, HashSet};
use astrelis_core::math::Vec2;
use astrelis_core::profiling::{profile_function, profile_scope};
use astrelis_render::RenderWindow;
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
    /// Opaque quad instance range (start, count) - rendered with depth write ON
    opaque_quad_range: (u32, u32),
    /// Transparent quad instance range (start, count) - rendered with depth write OFF
    transparent_quad_range: (u32, u32),
    /// Text instance range (start, count) - always transparent
    text_range: (u32, u32),
    /// Image clip groups for this batch (grouped by texture)
    image_groups: Vec<ImageClipGroup>,
}

/// Image rendering data for a texture within a clip batch.
#[derive(Clone)]
struct ImageClipGroup {
    /// The bind group key for this texture
    bind_group_key: ImageBindGroupKey,
    /// The texture being rendered
    texture: ImageTexture,
    /// Opaque image instance range (start, count)
    opaque_range: (u32, u32),
    /// Transparent image instance range (start, count)
    transparent_range: (u32, u32),
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

/// Depth format used for UI depth testing.
///
/// **Deprecated:** Use `UiRendererDescriptor::depth_format` instead for explicit configuration.
/// This constant is kept for backwards compatibility but new code should use
/// `UiRenderer::from_window()` or `UiRendererBuilder` to configure depth format.
pub const UI_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Configuration for creating a [`UiRenderer`].
///
/// Use [`UiRenderer::builder()`] for a fluent API or create directly.
///
/// # Example
///
/// ```rust,no_run
/// # use astrelis_ui::renderer::UiRendererDescriptor;
/// # use astrelis_render::wgpu;
/// // Create descriptor from a window (recommended)
/// // let desc = UiRendererDescriptor::from_window(&window);
///
/// // Or configure manually
/// let desc = UiRendererDescriptor {
///     name: "Game HUD".to_string(),
///     surface_format: wgpu::TextureFormat::Bgra8UnormSrgb,
///     depth_format: Some(wgpu::TextureFormat::Depth32Float),
/// };
/// ```
#[derive(Clone, Debug)]
pub struct UiRendererDescriptor {
    /// Name for the renderer (used in pipeline labels for debugging/profiling).
    ///
    /// This name appears in GPU debuggers and profilers as a prefix for
    /// pipeline labels (e.g., "Game HUD Quad Pipeline").
    ///
    /// Default: `"UI"`
    pub name: String,

    /// Surface texture format. Must match the render target.
    ///
    /// Default: `Bgra8UnormSrgb`
    pub surface_format: wgpu::TextureFormat,

    /// Depth format for z-ordering. `None` disables depth testing.
    ///
    /// When `Some`, pipelines are created with depth testing enabled using
    /// reverse-Z (higher z_index = closer to camera). When `None`, pipelines
    /// have no depth attachment and z-ordering relies on draw order.
    ///
    /// **Important:** This must match the render pass depth attachment:
    /// - If the render pass has a depth attachment, this must be `Some` with the same format
    /// - If the render pass has no depth attachment, this must be `None`
    ///
    /// Default: `None` (no depth testing)
    pub depth_format: Option<wgpu::TextureFormat>,
}

impl Default for UiRendererDescriptor {
    fn default() -> Self {
        Self {
            name: "UI".to_string(),
            surface_format: wgpu::TextureFormat::Bgra8UnormSrgb,
            depth_format: Some(wgpu::TextureFormat::Depth32Float),
        }
    }
}

impl UiRendererDescriptor {
    /// Create descriptor from a [`RenderWindow`], inheriting its format configuration.
    ///
    /// This is the **recommended** way to create a descriptor as it ensures
    /// pipeline-renderpass format compatibility automatically.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use astrelis_ui::renderer::UiRendererDescriptor;
    /// # use astrelis_render::RenderWindow;
    /// # fn example(window: &RenderWindow) {
    /// let desc = UiRendererDescriptor::from_window(window);
    /// // desc.surface_format matches window.surface_format()
    /// // desc.depth_format matches window.depth_format()
    /// # }
    /// ```
    pub fn from_window(window: &RenderWindow) -> Self {
        Self {
            name: "UI".to_string(),
            surface_format: window.surface_format(),
            depth_format: window.depth_format(),
        }
    }

    /// Set the renderer name (used in pipeline labels).
    ///
    /// The name appears in GPU debuggers and profilers.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Enable depth testing with the specified format.
    pub fn with_depth(mut self, format: wgpu::TextureFormat) -> Self {
        self.depth_format = Some(format);
        self
    }

    /// Enable depth testing with default format (Depth32Float).
    pub fn with_depth_default(mut self) -> Self {
        self.depth_format = Some(wgpu::TextureFormat::Depth32Float);
        self
    }

    /// Disable depth testing.
    pub fn without_depth(mut self) -> Self {
        self.depth_format = None;
        self
    }
}

/// Builder for creating [`UiRenderer`] with custom configuration.
///
/// # Example
///
/// ```rust,no_run
/// # use astrelis_ui::{UiRenderer, UiRendererBuilder};
/// # use astrelis_render::{GraphicsContext, RenderWindow};
/// # use std::sync::Arc;
/// # fn example(graphics: Arc<GraphicsContext>, window: &RenderWindow) {
/// // Recommended: inherit formats from window using the static constructor
/// let renderer = UiRendererBuilder::from_window(window)
///     .name("Game HUD")
///     .build(graphics.clone());
///
/// // Or configure manually
/// let renderer = UiRenderer::builder()
///     .name("Debug Overlay")
///     .surface_format(astrelis_render::wgpu::TextureFormat::Rgba8UnormSrgb)
///     .with_depth_default()
///     .build(graphics);
/// # }
/// ```
pub struct UiRendererBuilder {
    descriptor: UiRendererDescriptor,
}

impl Default for UiRendererBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl UiRendererBuilder {
    /// Create a new builder with default configuration.
    pub fn new() -> Self {
        Self {
            descriptor: UiRendererDescriptor::default(),
        }
    }

    /// Initialize from a window, inheriting its format configuration.
    ///
    /// This is the **recommended** starting point as it ensures
    /// pipeline-renderpass format compatibility automatically.
    pub fn from_window(window: &RenderWindow) -> Self {
        Self {
            descriptor: UiRendererDescriptor::from_window(window),
        }
    }

    /// Set the renderer name (appears in GPU debugger/profiler).
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.descriptor.name = name.into();
        self
    }

    /// Set surface format (should match window surface format).
    pub fn surface_format(mut self, format: wgpu::TextureFormat) -> Self {
        self.descriptor.surface_format = format;
        self
    }

    /// Enable depth testing with specified format.
    pub fn with_depth(mut self, format: wgpu::TextureFormat) -> Self {
        self.descriptor.depth_format = Some(format);
        self
    }

    /// Enable depth testing with default Depth32Float format.
    pub fn with_depth_default(mut self) -> Self {
        self.descriptor.depth_format = Some(wgpu::TextureFormat::Depth32Float);
        self
    }

    /// Disable depth testing.
    pub fn without_depth(mut self) -> Self {
        self.descriptor.depth_format = None;
        self
    }

    /// Get the current descriptor configuration.
    pub fn descriptor(&self) -> &UiRendererDescriptor {
        &self.descriptor
    }

    /// Build the renderer.
    pub fn build(self, context: Arc<GraphicsContext>) -> UiRenderer {
        UiRenderer::with_descriptor(context, self.descriptor)
    }
}

/// UI renderer for rendering all widgets.
pub struct UiRenderer {
    renderer: Renderer,
    font_renderer: FontRenderer,
    context: Arc<GraphicsContext>,

    /// Current configuration (stored for reconfigure and descriptor access).
    descriptor: UiRendererDescriptor,

    // Bind group layouts (needed for pipeline recreation during reconfigure)
    projection_bind_group_layout: wgpu::BindGroupLayout,
    text_atlas_bind_group_layout: wgpu::BindGroupLayout,
    text_projection_bind_group_layout: wgpu::BindGroupLayout,

    // Opaque pipelines (depth write ON, depth test ON)
    quad_opaque_pipeline: wgpu::RenderPipeline,
    image_opaque_pipeline: wgpu::RenderPipeline,
    // Transparent pipelines (depth write OFF, depth test ON)
    quad_transparent_pipeline: wgpu::RenderPipeline,
    text_pipeline_render: wgpu::RenderPipeline, // text is always transparent
    image_transparent_pipeline: wgpu::RenderPipeline,

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
    /// Overlay batch rendered after all regular clip batches (for docking previews, ghost tabs, etc.)
    overlay_batch: Option<ClipBatch>,
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

    /// Current theme colors for resolving widget defaults
    theme_colors: ColorPalette,
}

impl UiRenderer {
    /// Create a builder for configuring the renderer.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use astrelis_ui::UiRenderer;
    /// # use astrelis_render::{GraphicsContext, RenderWindow};
    /// # use std::sync::Arc;
    /// # fn example(graphics: Arc<GraphicsContext>, window: &RenderWindow) {
    /// // Configure manually without a window reference
    /// let renderer = UiRenderer::builder()
    ///     .name("Debug Overlay")
    ///     .surface_format(astrelis_render::wgpu::TextureFormat::Bgra8UnormSrgb)
    ///     .with_depth_default()
    ///     .build(graphics);
    /// # }
    /// ```
    pub fn builder() -> UiRendererBuilder {
        UiRendererBuilder::new()
    }

    /// Create a new UI renderer with default configuration (no depth testing).
    ///
    /// **Warning:** This creates a renderer without depth testing. If your render pass
    /// has a depth attachment, use [`from_window`](Self::from_window) instead to ensure
    /// pipeline-renderpass compatibility.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use astrelis_ui::UiRenderer;
    /// # use astrelis_render::GraphicsContext;
    /// # use std::sync::Arc;
    /// # fn example(graphics: Arc<GraphicsContext>) {
    /// // For simple use without depth testing
    /// let renderer = UiRenderer::new(graphics);
    /// # }
    /// ```
    pub fn new(context: Arc<GraphicsContext>) -> Self {
        Self::with_descriptor(context, UiRendererDescriptor::default())
    }

    /// Create renderer from a [`RenderWindow`], matching its format configuration.
    ///
    /// This is the **recommended** constructor as it ensures the renderer's pipelines
    /// are compatible with the window's render pass configuration.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use astrelis_ui::UiRenderer;
    /// # use astrelis_render::{GraphicsContext, RenderWindow};
    /// # use std::sync::Arc;
    /// # fn example(graphics: Arc<GraphicsContext>, window: &RenderWindow) {
    /// // Automatically inherits surface_format and depth_format from window
    /// let renderer = UiRenderer::from_window(graphics, window);
    /// # }
    /// ```
    pub fn from_window(context: Arc<GraphicsContext>, window: &RenderWindow) -> Self {
        Self::with_descriptor(context, UiRendererDescriptor::from_window(window))
    }

    /// Create renderer with explicit configuration.
    ///
    /// Use this when you need full control over the renderer configuration,
    /// or when the target is not a `RenderWindow`.
    pub fn with_descriptor(
        context: Arc<GraphicsContext>,
        descriptor: UiRendererDescriptor,
    ) -> Self {
        let renderer = Renderer::new(context.clone());

        // Create font renderer for text
        let font_system = FontSystem::with_system_fonts();
        let font_renderer = FontRenderer::new(context.clone(), font_system);

        // 1. Create unit quad VBO for instanced rendering
        let unit_quad_vertices = QuadVertex::unit_quad();
        let unit_quad_vbo =
            context
                .device()
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Unit Quad VBO"),
                    contents: bytemuck::cast_slice(&unit_quad_vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });

        // 2. Create projection uniform buffer
        let projection_buffer = context.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{} Projection Uniform", descriptor.name)),
            size: 64, // mat4x4<f32>
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 3. Create bind group layouts
        let projection_bind_group_layout = renderer.create_bind_group_layout(
            Some(&format!("{} Projection Bind Group Layout", descriptor.name)),
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
            Some(&format!("{} Text Atlas Bind Group Layout", descriptor.name)),
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
            Some(&format!(
                "{} Text Projection Bind Group Layout",
                descriptor.name
            )),
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

        // 4. Create bind groups
        let projection_bind_group = renderer.create_bind_group(
            Some(&format!("{} Projection Bind Group", descriptor.name)),
            &projection_bind_group_layout,
            &[wgpu::BindGroupEntry {
                binding: 0,
                resource: projection_buffer.as_entire_binding(),
            }],
        );

        // Atlas bind group (group 0 for text shader)
        let text_atlas_bind_group = renderer.create_bind_group(
            Some(&format!("{} Text Atlas Bind Group", descriptor.name)),
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
            Some(&format!("{} Text Projection Bind Group", descriptor.name)),
            &text_projection_bind_group_layout,
            &[wgpu::BindGroupEntry {
                binding: 0,
                resource: projection_buffer.as_entire_binding(),
            }],
        );

        // Image texture bind group layout (group 0 for image shader)
        let image_texture_bind_group_layout = renderer.create_bind_group_layout(
            Some(&format!(
                "{} Image Texture Bind Group Layout",
                descriptor.name
            )),
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

        // 5. Create pipelines (5 total: opaque/transparent for quads and images, transparent-only for text)
        let (
            quad_opaque_pipeline,
            quad_transparent_pipeline,
            text_pipeline_render,
            image_opaque_pipeline,
            image_transparent_pipeline,
        ) = Self::create_pipelines(
            &renderer,
            &descriptor,
            &projection_bind_group_layout,
            &text_atlas_bind_group_layout,
            &text_projection_bind_group_layout,
            &image_texture_bind_group_layout,
        );

        // 6. Initialize retained components
        let text_pipeline = TextPipeline::new();
        let draw_list = DrawList::new();
        let quad_instances = InstanceBuffer::new(
            context.device(),
            Some(&format!("{} Quad Instances", descriptor.name)),
            1024,
        );
        let text_instances = InstanceBuffer::new(
            context.device(),
            Some(&format!("{} Text Instances", descriptor.name)),
            4096,
        );
        let image_instances = InstanceBuffer::new(
            context.device(),
            Some(&format!("{} Image Instances", descriptor.name)),
            256,
        );

        Self {
            renderer,
            font_renderer,
            context,
            descriptor,
            projection_bind_group_layout,
            text_atlas_bind_group_layout,
            text_projection_bind_group_layout,
            quad_opaque_pipeline,
            quad_transparent_pipeline,
            text_pipeline_render,
            image_opaque_pipeline,
            image_transparent_pipeline,
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
            overlay_batch: None,
            has_clipping: false,
            scale_factor: 1.0,
            // Pre-allocate persistent frame buffers
            frame_quad_instances: Vec::with_capacity(1024),
            frame_text_instances: Vec::with_capacity(4096),
            frame_image_instances: Vec::with_capacity(256),
            theme_colors: ColorPalette::dark(),
        }
    }

    /// Get the current renderer configuration.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use astrelis_ui::UiRenderer;
    /// # use astrelis_render::GraphicsContext;
    /// # use std::sync::Arc;
    /// # fn example(renderer: &UiRenderer) {
    /// let desc = renderer.descriptor();
    /// println!("Surface format: {:?}", desc.surface_format);
    /// println!("Depth format: {:?}", desc.depth_format);
    /// # }
    /// ```
    pub fn descriptor(&self) -> &UiRendererDescriptor {
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
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use astrelis_ui::{UiRenderer, UiRendererDescriptor};
    /// # use astrelis_render::{GraphicsContext, RenderWindow};
    /// # fn example(renderer: &mut UiRenderer, window: &RenderWindow) {
    /// // Window moved to different monitor
    /// renderer.reconfigure(UiRendererDescriptor::from_window(window));
    /// # }
    /// ```
    pub fn reconfigure(&mut self, descriptor: UiRendererDescriptor) {
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
        let (quad_opaque, quad_transparent, text, image_opaque, image_transparent) =
            Self::create_pipelines(
                &self.renderer,
                &self.descriptor,
                &self.projection_bind_group_layout,
                &self.text_atlas_bind_group_layout,
                &self.text_projection_bind_group_layout,
                &self.image_texture_bind_group_layout,
            );

        self.quad_opaque_pipeline = quad_opaque;
        self.quad_transparent_pipeline = quad_transparent;
        self.text_pipeline_render = text;
        self.image_opaque_pipeline = image_opaque;
        self.image_transparent_pipeline = image_transparent;
    }

    /// Reconfigure from a window, inheriting its format configuration.
    ///
    /// Convenience method equivalent to:
    /// ```rust,ignore
    /// renderer.reconfigure(UiRendererDescriptor::from_window(window));
    /// ```
    pub fn reconfigure_from_window(&mut self, window: &RenderWindow) {
        self.reconfigure(
            UiRendererDescriptor::from_window(window).with_name(self.descriptor.name.clone()),
        );
    }

    /// Create all render pipelines with the given configuration.
    ///
    /// Returns 5 pipelines: (quad_opaque, quad_transparent, text, image_opaque, image_transparent)
    /// - Opaque pipelines: depth write ON, depth test ON
    /// - Transparent pipelines: depth write OFF, depth test ON
    /// - Text pipeline: always uses transparent depth stencil (glyphs are alpha-blended)
    fn create_pipelines(
        renderer: &Renderer,
        descriptor: &UiRendererDescriptor,
        projection_bind_group_layout: &wgpu::BindGroupLayout,
        text_atlas_bind_group_layout: &wgpu::BindGroupLayout,
        text_projection_bind_group_layout: &wgpu::BindGroupLayout,
        image_texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> (
        wgpu::RenderPipeline,
        wgpu::RenderPipeline,
        wgpu::RenderPipeline,
        wgpu::RenderPipeline,
        wgpu::RenderPipeline,
    ) {
        // Load shaders
        let quad_instanced_shader = renderer.create_shader(
            Some(&format!("{} Quad Shader", descriptor.name)),
            include_str!("../shaders/quad_instanced.wgsl"),
        );
        let text_instanced_shader = renderer.create_shader(
            Some(&format!("{} Text Shader", descriptor.name)),
            include_str!("../shaders/text_instanced.wgsl"),
        );
        let image_instanced_shader = renderer.create_shader(
            Some(&format!("{} Image Shader", descriptor.name)),
            include_str!("../shaders/image_instanced.wgsl"),
        );

        // Create depth stencil states from descriptor
        // Uses reverse-Z for better depth precision (higher z_index = closer to camera)
        // Opaque: depth write ON, depth test ON
        let depth_stencil_opaque = descriptor
            .depth_format
            .map(|format| wgpu::DepthStencilState {
                format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::GreaterEqual, // Reverse-Z
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            });

        // Transparent: depth write OFF, depth test ON
        let depth_stencil_transparent =
            descriptor
                .depth_format
                .map(|format| wgpu::DepthStencilState {
                    format,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::GreaterEqual, // Reverse-Z
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                });

        let common_primitive = wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        };

        let common_multisample = wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        };

        // --- Quad pipelines (opaque + transparent) ---
        let quad_layout = renderer.create_pipeline_layout(
            Some(&format!("{} Quad Pipeline Layout", descriptor.name)),
            &[projection_bind_group_layout],
            &[],
        );

        let quad_color_target = [Some(wgpu::ColorTargetState {
            format: descriptor.surface_format,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        })];

        let quad_opaque_pipeline =
            renderer.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(&format!("{} Quad Opaque Pipeline", descriptor.name)),
                layout: Some(&quad_layout),
                vertex: wgpu::VertexState {
                    module: &quad_instanced_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[QuadVertex::vertex_layout(), QuadInstance::vertex_layout()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &quad_instanced_shader,
                    entry_point: Some("fs_main"),
                    targets: &quad_color_target,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: common_primitive,
                depth_stencil: depth_stencil_opaque.clone(),
                multisample: common_multisample,
                multiview: None,
                cache: None,
            });

        let quad_transparent_pipeline =
            renderer.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(&format!("{} Quad Transparent Pipeline", descriptor.name)),
                layout: Some(&quad_layout),
                vertex: wgpu::VertexState {
                    module: &quad_instanced_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[QuadVertex::vertex_layout(), QuadInstance::vertex_layout()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &quad_instanced_shader,
                    entry_point: Some("fs_main"),
                    targets: &quad_color_target,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: common_primitive,
                depth_stencil: depth_stencil_transparent.clone(),
                multisample: common_multisample,
                multiview: None,
                cache: None,
            });

        // --- Text pipeline (always transparent) ---
        let text_layout = renderer.create_pipeline_layout(
            Some(&format!("{} Text Pipeline Layout", descriptor.name)),
            &[
                text_atlas_bind_group_layout,
                text_projection_bind_group_layout,
            ],
            &[],
        );

        let text_pipeline = renderer.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&format!("{} Text Pipeline", descriptor.name)),
            layout: Some(&text_layout),
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
                    format: descriptor.surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: common_primitive,
            depth_stencil: depth_stencil_transparent.clone(),
            multisample: common_multisample,
            multiview: None,
            cache: None,
        });

        // --- Image pipelines (opaque + transparent) ---
        let image_layout = renderer.create_pipeline_layout(
            Some(&format!("{} Image Pipeline Layout", descriptor.name)),
            &[
                image_texture_bind_group_layout,
                text_projection_bind_group_layout,
            ],
            &[],
        );

        let image_color_target = [Some(wgpu::ColorTargetState {
            format: descriptor.surface_format,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        })];

        let image_opaque_pipeline =
            renderer.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(&format!("{} Image Opaque Pipeline", descriptor.name)),
                layout: Some(&image_layout),
                vertex: wgpu::VertexState {
                    module: &image_instanced_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[QuadVertex::vertex_layout(), ImageInstance::vertex_layout()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &image_instanced_shader,
                    entry_point: Some("fs_main"),
                    targets: &image_color_target,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: common_primitive,
                depth_stencil: depth_stencil_opaque,
                multisample: common_multisample,
                multiview: None,
                cache: None,
            });

        let image_transparent_pipeline =
            renderer.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(&format!("{} Image Transparent Pipeline", descriptor.name)),
                layout: Some(&image_layout),
                vertex: wgpu::VertexState {
                    module: &image_instanced_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[QuadVertex::vertex_layout(), ImageInstance::vertex_layout()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &image_instanced_shader,
                    entry_point: Some("fs_main"),
                    targets: &image_color_target,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: common_primitive,
                depth_stencil: depth_stencil_transparent,
                multisample: common_multisample,
                multiview: None,
                cache: None,
            });

        (
            quad_opaque_pipeline,
            quad_transparent_pipeline,
            text_pipeline,
            image_opaque_pipeline,
            image_transparent_pipeline,
        )
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
    fn request_text_shaping_recursive(
        &mut self,
        tree: &UiTree,
        node_id: NodeId,
        widget_registry: &WidgetTypeRegistry,
    ) {
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
    fn build_all_nodes_recursive(
        &mut self,
        tree: &UiTree,
        node_id: NodeId,
        widget_registry: &WidgetTypeRegistry,
    ) {
        self.build_all_nodes_recursive_with_clip(
            tree,
            node_id,
            ClipRect::infinite(),
            widget_registry,
        );
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
        let (node_clip, child_clip) =
            self.compute_node_clip(tree, node_id, inherited_clip, widget_registry);

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
                        self.build_all_nodes_recursive_with_clip(
                            tree,
                            child_id,
                            child_clip,
                            widget_registry,
                        );
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
                        self.build_all_nodes_recursive_with_clip(
                            tree,
                            child_id,
                            child_clip,
                            widget_registry,
                        );
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
                        && let Some(scroll_offset_fn) = desc.scroll_offset
                    {
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
    fn compute_inherited_clip(
        &self,
        tree: &UiTree,
        node_id: NodeId,
        widget_registry: &WidgetTypeRegistry,
    ) -> ClipRect {
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
        &mut self,
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
        let (_, child_clip) =
            self.compute_node_clip(tree, node_id, inherited_clip, widget_registry);

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
                        self.collect_dirty_nodes_with_clips(
                            tree,
                            child_id,
                            child_clip,
                            dirty_nodes,
                            widget_registry,
                        );
                    }
                }
                TraversalBehavior::OnlyChild(index) => {
                    // Clear draw commands for inactive children so stale content
                    // from reparented nodes (e.g. tab merge) doesn't overlap.
                    for (i, &child_id) in widget.children().iter().enumerate() {
                        if i != index {
                            self.clear_node_recursive(tree, child_id);
                        }
                    }
                    // Only recurse into the active child
                    if let Some(&child_id) = widget.children().get(index) {
                        self.collect_dirty_nodes_with_clips(
                            tree,
                            child_id,
                            child_clip,
                            dirty_nodes,
                            widget_registry,
                        );
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
                    && let Some(scroll_offset_fn) = desc.scroll_offset
                {
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
                && let Some(render_fn) = descriptor.render
            {
                let mut render_ctx = WidgetRenderContext {
                    abs_position: Vec2::new(abs_x, abs_y),
                    layout_size: Vec2::new(layout.width, layout.height),
                    clip_rect,
                    theme_colors: &self.theme_colors,
                    text_pipeline: &mut self.text_pipeline,
                    parent_z_index: tree
                        .get_node(node_id)
                        .map(|n| n.computed_z_index)
                        .unwrap_or(0),
                };
                commands = render_fn(widget.as_any(), &mut render_ctx);
            }
        }

        // Apply render layer from widget's style to all commands
        let render_layer = widget.style().render_layer;
        if render_layer != crate::draw_list::RenderLayer::Base {
            for cmd in &mut commands {
                match cmd {
                    DrawCommand::Quad(q) => q.render_layer = render_layer,
                    DrawCommand::Text(t) => t.render_layer = render_layer,
                    DrawCommand::Image(i) => i.render_layer = render_layer,
                }
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

    /// Render layer used for docking overlay elements (previews, ghost tabs).
    #[cfg(feature = "docking")]
    const DOCKING_OVERLAY_LAYER: crate::draw_list::RenderLayer =
        crate::draw_list::RenderLayer::Overlay(6);

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

                let mut fill_cmd = crate::draw_list::QuadCommand::rounded(
                    Vec2::new(bounds.x, bounds.y),
                    Vec2::new(bounds.width, bounds.height),
                    fill_color,
                    4.0,
                    10, // High Z to render on top of everything
                );
                fill_cmd.render_layer = Self::DOCKING_OVERLAY_LAYER;

                let mut border_cmd = crate::draw_list::QuadCommand::bordered(
                    Vec2::new(bounds.x, bounds.y),
                    Vec2::new(bounds.width, bounds.height),
                    border_color,
                    2.0,
                    4.0,
                    11, // Even higher Z for border
                );
                border_cmd.render_layer = Self::DOCKING_OVERLAY_LAYER;

                let commands = vec![DrawCommand::Quad(fill_cmd), DrawCommand::Quad(border_cmd)];

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
                let mut bg_cmd = crate::draw_list::QuadCommand::rounded(
                    ghost.position,
                    ghost.size,
                    Color::from_rgba_u8(60, 80, 120, alpha),
                    4.0,
                    12, // High Z-index above everything
                );
                bg_cmd.render_layer = Self::DOCKING_OVERLAY_LAYER;
                commands.push(DrawCommand::Quad(bg_cmd));

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
                        let mut text_cmd = crate::draw_list::TextCommand::new(
                            Vec2::new(text_x, text_y),
                            shaped.clone(),
                            Color::from_rgba_u8(220, 220, 220, text_alpha),
                            13,
                        );
                        text_cmd.render_layer = Self::DOCKING_OVERLAY_LAYER;
                        commands.push(DrawCommand::Text(text_cmd));

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
                let mut tab_bg_cmd = crate::draw_list::QuadCommand::rounded(
                    ghost.position,
                    ghost.size,
                    Color::from_rgba_u8(60, 80, 120, alpha),
                    4.0,
                    12,
                );
                tab_bg_cmd.render_layer = Self::DOCKING_OVERLAY_LAYER;
                commands.push(DrawCommand::Quad(tab_bg_cmd));

                // Tab label text
                let request_id =
                    self.text_pipeline
                        .request_shape(ghost.label.clone(), 0, 13.0, None);

                if let Some(shaped) = self.text_pipeline.get_completed(request_id) {
                    let text_height = shaped.bounds().1;
                    let text_x = ghost.position.x + DEFAULT_TAB_PADDING;
                    let text_y = ghost.position.y + (ghost.size.y - text_height) * 0.5;

                    let text_alpha = (ghost.opacity * 200.0) as u8;
                    let mut tab_text_cmd = crate::draw_list::TextCommand::new(
                        Vec2::new(text_x, text_y),
                        shaped,
                        Color::from_rgba_u8(220, 220, 220, text_alpha),
                        13,
                    );
                    tab_text_cmd.render_layer = Self::DOCKING_OVERLAY_LAYER;
                    commands.push(DrawCommand::Text(tab_text_cmd));
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
    /// Uses single-pass bucketing for base commands (O(n)) and a simple
    /// two-pass encode for overlay commands (typically 2-20 commands).
    fn encode_instances(&mut self) {
        profile_function!();

        // Clear and reuse persistent allocations
        self.frame_quad_instances.clear();
        self.frame_text_instances.clear();
        self.frame_image_instances.clear();

        self.has_clipping = false;
        self.clip_batches.clear();
        self.overlay_batch = None;
        self.image_batches.clear();

        // --- Step A: Single pass over base_commands() to bucket by clip rect ---
        // Each bucket stores (opaque_indices, transparent_indices) into the base commands slice.
        let base_cmds = self.draw_list.base_commands();
        let mut clip_rect_to_bucket: HashMap<ClipRect, usize> = HashMap::new();
        // (clip_rect, opaque_cmd_indices, transparent_cmd_indices)
        let mut buckets: Vec<(ClipRect, Vec<usize>, Vec<usize>)> = Vec::new();

        for (i, cmd) in base_cmds.iter().enumerate() {
            let clip = *cmd.clip_rect();
            if !clip.is_infinite() {
                self.has_clipping = true;
            }

            let bucket_idx = match clip_rect_to_bucket.get(&clip) {
                Some(&idx) => idx,
                None => {
                    let idx = buckets.len();
                    clip_rect_to_bucket.insert(clip, idx);
                    buckets.push((clip, Vec::new(), Vec::new()));
                    idx
                }
            };

            if cmd.is_opaque() && !matches!(cmd, DrawCommand::Text(_)) {
                buckets[bucket_idx].1.push(i);
            } else {
                buckets[bucket_idx].2.push(i);
            }
        }

        // Ensure infinite clip bucket is first (for proper draw order)
        if let Some(&inf_idx) = clip_rect_to_bucket.get(&ClipRect::infinite())
            && inf_idx != 0
        {
            buckets.swap(0, inf_idx);
            // Fix up the index map after swap
            let swapped_clip = buckets[inf_idx].0;
            clip_rect_to_bucket.insert(ClipRect::infinite(), 0);
            clip_rect_to_bucket.insert(swapped_clip, inf_idx);
        }

        // --- Step B: Encode each bucket into contiguous GPU buffer ranges ---
        for (clip_rect, opaque_indices, transparent_indices) in &buckets {
            let opaque_quad_start = self.frame_quad_instances.len() as u32;
            let mut opaque_image_groups: HashMap<
                ImageBindGroupKey,
                (ImageTexture, Vec<ImageInstance>),
            > = HashMap::new();

            for &idx in opaque_indices {
                encode_command(
                    &base_cmds[idx],
                    &mut self.frame_quad_instances,
                    &mut self.frame_text_instances,
                    &mut opaque_image_groups,
                    &mut self.font_renderer,
                );
            }
            let opaque_quad_count = self.frame_quad_instances.len() as u32 - opaque_quad_start;

            let transparent_quad_start = self.frame_quad_instances.len() as u32;
            let text_start = self.frame_text_instances.len() as u32;
            let mut transparent_image_groups: HashMap<
                ImageBindGroupKey,
                (ImageTexture, Vec<ImageInstance>),
            > = HashMap::new();

            for &idx in transparent_indices {
                encode_command(
                    &base_cmds[idx],
                    &mut self.frame_quad_instances,
                    &mut self.frame_text_instances,
                    &mut transparent_image_groups,
                    &mut self.font_renderer,
                );
            }
            let transparent_quad_count =
                self.frame_quad_instances.len() as u32 - transparent_quad_start;
            let text_count = self.frame_text_instances.len() as u32 - text_start;

            let image_groups = finalize_image_groups(
                opaque_image_groups,
                transparent_image_groups,
                &mut self.frame_image_instances,
            );

            let has_content = opaque_quad_count > 0
                || transparent_quad_count > 0
                || text_count > 0
                || !image_groups.is_empty();

            if has_content {
                self.clip_batches.push(ClipBatch {
                    clip_rect: *clip_rect,
                    opaque_quad_range: (opaque_quad_start, opaque_quad_count),
                    transparent_quad_range: (transparent_quad_start, transparent_quad_count),
                    text_range: (text_start, text_count),
                    image_groups,
                });
            }
        }

        // --- Step C: Overlay pass (typically 2-20 commands) ---
        let overlay_cmds = self.draw_list.overlay_commands();
        if !overlay_cmds.is_empty() {
            let overlay_opaque_quad_start = self.frame_quad_instances.len() as u32;
            let mut overlay_opaque_image_groups: HashMap<
                ImageBindGroupKey,
                (ImageTexture, Vec<ImageInstance>),
            > = HashMap::new();

            // Opaque overlay commands (text is always transparent — glyph atlas uses alpha)
            for cmd in overlay_cmds {
                if cmd.is_opaque() && !matches!(cmd, DrawCommand::Text(_)) {
                    encode_command(
                        cmd,
                        &mut self.frame_quad_instances,
                        &mut self.frame_text_instances,
                        &mut overlay_opaque_image_groups,
                        &mut self.font_renderer,
                    );
                }
            }
            let overlay_opaque_quad_count =
                self.frame_quad_instances.len() as u32 - overlay_opaque_quad_start;

            // Transparent overlay commands
            let overlay_transparent_quad_start = self.frame_quad_instances.len() as u32;
            let overlay_text_start = self.frame_text_instances.len() as u32;
            let mut overlay_transparent_image_groups: HashMap<
                ImageBindGroupKey,
                (ImageTexture, Vec<ImageInstance>),
            > = HashMap::new();

            for cmd in overlay_cmds {
                if !cmd.is_opaque() || matches!(cmd, DrawCommand::Text(_)) {
                    encode_command(
                        cmd,
                        &mut self.frame_quad_instances,
                        &mut self.frame_text_instances,
                        &mut overlay_transparent_image_groups,
                        &mut self.font_renderer,
                    );
                }
            }
            let overlay_transparent_quad_count =
                self.frame_quad_instances.len() as u32 - overlay_transparent_quad_start;
            let overlay_text_count = self.frame_text_instances.len() as u32 - overlay_text_start;

            let overlay_image_groups = finalize_image_groups(
                overlay_opaque_image_groups,
                overlay_transparent_image_groups,
                &mut self.frame_image_instances,
            );

            let has_overlay_content = overlay_opaque_quad_count > 0
                || overlay_transparent_quad_count > 0
                || overlay_text_count > 0
                || !overlay_image_groups.is_empty();

            if has_overlay_content {
                self.overlay_batch = Some(ClipBatch {
                    clip_rect: ClipRect::infinite(),
                    opaque_quad_range: (overlay_opaque_quad_start, overlay_opaque_quad_count),
                    transparent_quad_range: (
                        overlay_transparent_quad_start,
                        overlay_transparent_quad_count,
                    ),
                    text_range: (overlay_text_start, overlay_text_count),
                    image_groups: overlay_image_groups,
                });
            }
        }

        // Ensure bind groups exist for all image textures
        {
            let mut image_keys: Vec<(ImageBindGroupKey, ImageTexture)> = self
                .clip_batches
                .iter()
                .flat_map(|batch| {
                    batch
                        .image_groups
                        .iter()
                        .map(|group| (group.bind_group_key, group.texture.clone()))
                })
                .collect();

            if let Some(ref overlay) = self.overlay_batch {
                for group in &overlay.image_groups {
                    image_keys.push((group.bind_group_key, group.texture.clone()));
                }
            }

            for (key, texture) in image_keys {
                self.get_or_create_image_bind_group(key, &texture);
            }
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

        self.render_clip_batches(
            render_pass,
            viewport_width,
            viewport_height,
            viewport.scale_factor.0,
        );
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

        self.render_clip_batches(
            render_pass,
            viewport_width,
            viewport_height,
            viewport.scale_factor.0,
        );
    }

    /// Shared rendering logic: draw clip batches with opaque-then-transparent ordering.
    fn render_clip_batches(
        &self,
        render_pass: &mut wgpu::RenderPass,
        viewport_width: u32,
        viewport_height: u32,
        scale_factor: f64,
    ) {
        profile_scope!("render_clip_batches");

        for batch in &self.clip_batches {
            // Set scissor rect for this batch
            if batch.clip_rect.is_infinite() {
                render_pass.set_scissor_rect(0, 0, viewport_width, viewport_height);
            } else {
                let physical = batch.clip_rect.to_physical(scale_factor);
                let clamped = physical.clamp_to_viewport(viewport_width, viewport_height);
                if clamped.width == 0 || clamped.height == 0 {
                    continue;
                }
                render_pass.set_scissor_rect(clamped.x, clamped.y, clamped.width, clamped.height);
            }

            self.render_batch(render_pass, batch);
        }

        // Overlay pass: render AFTER all regular clip batches so overlays appear on top
        if let Some(ref overlay) = self.overlay_batch {
            render_pass.set_scissor_rect(0, 0, viewport_width, viewport_height);
            self.render_batch(render_pass, overlay);
        }
    }

    /// Render a single clip batch: opaque quads/images, then transparent quads/images, then text.
    fn render_batch(&self, render_pass: &mut wgpu::RenderPass, batch: &ClipBatch) {
        // Pass 1: Opaque quads (depth write ON)
        if batch.opaque_quad_range.1 > 0 {
            render_pass.set_pipeline(&self.quad_opaque_pipeline);
            render_pass.set_bind_group(0, &self.projection_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.unit_quad_vbo.slice(..));
            render_pass.set_vertex_buffer(1, self.quad_instances.buffer().slice(..));
            let (start, count) = batch.opaque_quad_range;
            render_pass.draw(0..6, start..(start + count));
        }

        // Pass 1b: Opaque images per texture (depth write ON)
        for group in &batch.image_groups {
            if group.opaque_range.1 > 0 {
                render_pass.set_pipeline(&self.image_opaque_pipeline);
                render_pass.set_bind_group(1, &self.text_projection_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.unit_quad_vbo.slice(..));
                render_pass.set_vertex_buffer(1, self.image_instances.buffer().slice(..));
                if let Some(bind_group) = self.image_bind_group_cache.get(&group.bind_group_key) {
                    render_pass.set_bind_group(0, bind_group, &[]);
                    let (start, count) = group.opaque_range;
                    render_pass.draw(0..6, start..(start + count));
                }
            }
        }

        // Pass 2: Transparent quads (depth write OFF, depth test ON)
        if batch.transparent_quad_range.1 > 0 {
            render_pass.set_pipeline(&self.quad_transparent_pipeline);
            render_pass.set_bind_group(0, &self.projection_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.unit_quad_vbo.slice(..));
            render_pass.set_vertex_buffer(1, self.quad_instances.buffer().slice(..));
            let (start, count) = batch.transparent_quad_range;
            render_pass.draw(0..6, start..(start + count));
        }

        // Pass 2b: Transparent images per texture (depth write OFF, depth test ON)
        for group in &batch.image_groups {
            if group.transparent_range.1 > 0 {
                render_pass.set_pipeline(&self.image_transparent_pipeline);
                render_pass.set_bind_group(1, &self.text_projection_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.unit_quad_vbo.slice(..));
                render_pass.set_vertex_buffer(1, self.image_instances.buffer().slice(..));
                if let Some(bind_group) = self.image_bind_group_cache.get(&group.bind_group_key) {
                    render_pass.set_bind_group(0, bind_group, &[]);
                    let (start, count) = group.transparent_range;
                    render_pass.draw(0..6, start..(start + count));
                }
            }
        }

        // Pass 3: Text (always transparent)
        if batch.text_range.1 > 0 {
            render_pass.set_pipeline(&self.text_pipeline_render);
            render_pass.set_bind_group(0, &self.text_atlas_bind_group, &[]);
            render_pass.set_bind_group(1, &self.text_projection_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.unit_quad_vbo.slice(..));
            render_pass.set_vertex_buffer(1, self.text_instances.buffer().slice(..));
            let (start, count) = batch.text_range;
            render_pass.draw(0..6, start..(start + count));
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

/// Encode a single draw command into the appropriate GPU instance buffers.
///
/// Converts `DrawCommand` variants into `QuadInstance`, `TextInstance`, or `ImageInstance`
/// and appends them to the corresponding buffers.
fn encode_command(
    cmd: &DrawCommand,
    quad_instances: &mut Vec<QuadInstance>,
    text_instances: &mut Vec<TextInstance>,
    image_groups: &mut HashMap<ImageBindGroupKey, (ImageTexture, Vec<ImageInstance>)>,
    font_renderer: &mut FontRenderer,
) {
    match cmd {
        DrawCommand::Quad(q) => {
            quad_instances.push(QuadInstance {
                position: [q.position.x, q.position.y],
                size: [q.size.x, q.size.y],
                color: [q.color.r, q.color.g, q.color.b, q.color.a],
                border_radius: q.border_radius,
                border_thickness: q.border_thickness,
                z_depth: z_index_to_depth(q.z_index, q.render_layer),
                _padding: 0.0,
            });
        }
        DrawCommand::Text(t) => {
            glyphs_to_instances_into(
                font_renderer,
                &t.shaped_text.inner.glyphs,
                t.position,
                t.color,
                z_index_to_depth(t.z_index, t.render_layer),
                text_instances,
            );
        }
        DrawCommand::Image(i) => {
            let bind_group_key = ImageBindGroupKey {
                texture_ptr: std::sync::Arc::as_ptr(&i.texture) as usize,
                sampling: i.sampling,
            };

            image_groups
                .entry(bind_group_key)
                .or_insert_with(|| (i.texture.clone(), Vec::new()))
                .1
                .push(ImageInstance {
                    position: [i.position.x, i.position.y],
                    size: [i.size.x, i.size.y],
                    uv_min: [i.uv.u_min, i.uv.v_min],
                    uv_max: [i.uv.u_max, i.uv.v_max],
                    tint: [i.tint.r, i.tint.g, i.tint.b, i.tint.a],
                    border_radius: i.border_radius,
                    texture_index: 0,
                    z_depth: z_index_to_depth(i.z_index, i.render_layer),
                    _padding: 0.0,
                });
        }
    }
}

/// Merge opaque and transparent image groups into contiguous GPU buffer ranges.
///
/// For each unique texture key, appends opaque instances then transparent instances
/// to the shared image instance buffer and returns `ImageClipGroup` entries with
/// the resulting ranges.
fn finalize_image_groups(
    mut opaque: HashMap<ImageBindGroupKey, (ImageTexture, Vec<ImageInstance>)>,
    mut transparent: HashMap<ImageBindGroupKey, (ImageTexture, Vec<ImageInstance>)>,
    image_instances: &mut Vec<ImageInstance>,
) -> Vec<ImageClipGroup> {
    // Collect all unique texture keys from both passes
    let all_keys: Vec<ImageBindGroupKey> = opaque
        .keys()
        .chain(transparent.keys())
        .copied()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    let mut groups = Vec::with_capacity(all_keys.len());

    for key in all_keys {
        let opaque_start = image_instances.len() as u32;
        let mut texture = None;

        if let Some((tex, instances)) = opaque.remove(&key) {
            texture = Some(tex);
            image_instances.extend(instances);
        }
        let opaque_count = image_instances.len() as u32 - opaque_start;

        let transparent_start = image_instances.len() as u32;
        if let Some((tex, instances)) = transparent.remove(&key) {
            if texture.is_none() {
                texture = Some(tex);
            }
            image_instances.extend(instances);
        }
        let transparent_count = image_instances.len() as u32 - transparent_start;

        if let Some(texture) = texture
            && (opaque_count > 0 || transparent_count > 0)
        {
            groups.push(ImageClipGroup {
                bind_group_key: key,
                texture,
                opaque_range: (opaque_start, opaque_count),
                transparent_range: (transparent_start, transparent_count),
            });
        }
    }

    groups
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

/// Convert a z_index (u16) to a depth value for the depth buffer.
///
/// Uses reverse-Z convention where higher z_index values result in depth values
/// closer to 1.0 (nearer to the camera). This provides better depth precision
/// for elements that are closer together in z-order.
///
/// The conversion maps z_index range [0, 65535] to depth range (0.0, 1.0]:
/// - z_index 0 → depth ≈ 0.000015 (furthest from camera)
/// - z_index 65535 → depth = 1.0 (nearest to camera)
#[inline]
fn z_index_to_depth(z_index: u16, render_layer: RenderLayer) -> f32 {
    match render_layer {
        RenderLayer::Base => {
            // Base layer: depth range [~0, 0.5)
            (z_index as f32 + 1.0) / 131072.0
        }
        RenderLayer::Overlay(n) => {
            // Overlay layers: depth range [0.5, ~1.0)
            // Each overlay sub-layer gets a 256-wide z_index band
            0.5 + (n as f32 * 256.0 + z_index.min(255) as f32 + 1.0) / 131072.0
        }
    }
}
