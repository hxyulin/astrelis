use std::sync::{Arc, RwLock};

use astrelis_core::alloc::HashMap;
use astrelis_core::math::Vec2;
use astrelis_core::profiling::profile_function;
use cosmic_text::{Buffer, CacheKey, Color as CosmicColor, Metrics, Shaping, SwashCache};

use astrelis_render::{GraphicsContext, Renderer, Viewport, wgpu};

use crate::{
    effects::TextEffects,
    font::FontSystem,
    sdf::{SdfConfig, TextRenderMode, generate_sdf},
    text::{Text, TextMetrics, color_to_cosmic},
};

/// A cached text buffer with layout information.
pub struct TextBuffer {
    buffer: Buffer,
    needs_layout: bool,
}

impl TextBuffer {
    fn new(font_system: &mut cosmic_text::FontSystem) -> Self {
        let mut buffer = Buffer::new(font_system, Metrics::new(16.0, 20.0));
        buffer.set_wrap(font_system, cosmic_text::Wrap::Word);
        Self {
            buffer,
            needs_layout: true,
        }
    }

    fn set_text(&mut self, font_system: &mut cosmic_text::FontSystem, text: &Text, scale: f32) {
        let metrics = Metrics::new(
            text.get_font_size() * scale,
            text.get_font_size() * scale * text.get_line_height(),
        );
        self.buffer.set_metrics(font_system, metrics);

        let attrs = text
            .get_font_attrs()
            .to_cosmic()
            .color(color_to_cosmic(text.get_color()));

        self.buffer
            .set_text(font_system, text.get_content(), attrs, Shaping::Advanced);

        // Set buffer size for wrapping
        self.buffer
            .set_size(font_system, text.get_max_width().map(|w| w * scale), text.get_max_height().map(|h| h * scale));

        // Set wrapping mode
        self.buffer
            .set_wrap(font_system, text.get_wrap().to_cosmic());

        // Set alignment for all lines
        let align = Some(text.get_align().to_cosmic());
        for line in &mut self.buffer.lines {
            line.set_align(align);
        }

        self.needs_layout = true;
    }

    fn layout(&mut self, font_system: &mut cosmic_text::FontSystem) {
        profile_function!();
        if self.needs_layout {
            self.buffer.shape_until_scroll(font_system, false);
            self.needs_layout = false;
        }
    }

    pub fn bounds(&self) -> (f32, f32) {
        let mut width: f32 = 0.0;
        let mut height: f32 = 0.0;

        for run in self.buffer.layout_runs() {
            width = width.max(run.line_w);
            height += run.line_height;
        }

        (width, height)
    }
}

/// Vertex data for text rendering.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct TextVertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
    color: [f32; 4],
}

/// Glyph atlas entry with UV coordinates.
#[derive(Debug, Clone)]
pub struct AtlasEntry {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl AtlasEntry {
    fn uv_coords(&self, atlas_size: u32) -> (f32, f32, f32, f32) {
        let u0 = self.x as f32 / atlas_size as f32;
        let v0 = self.y as f32 / atlas_size as f32;
        let u1 = (self.x + self.width) as f32 / atlas_size as f32;
        let v1 = (self.y + self.height) as f32 / atlas_size as f32;
        (u0, v0, u1, v1)
    }
}

/// Glyph placement information for correct positioning.
#[derive(Debug, Clone, Copy)]
pub struct GlyphPlacement {
    /// Left bearing offset (horizontal offset from origin)
    pub left: f32,
    /// Top bearing offset (vertical offset from baseline)
    pub top: f32,
    /// Glyph width in pixels
    pub width: f32,
    /// Glyph height in pixels
    pub height: f32,
}

/// SDF glyph cache key - size-independent for scale-free rendering.
///
/// Unlike bitmap glyphs which need different cache entries per font size,
/// SDF glyphs are rendered at a fixed base size (48px) and scaled via shader,
/// so we only need `glyph_id` and `font_id` as cache keys.
///
/// # Why Size-Independent?
///
/// Traditional bitmap rendering requires a separate cached glyph for each font size
/// (e.g., 12px, 16px, 24px would need 3 atlas entries). SDF rendering stores distance
/// information that can be sampled at any scale, so a single cached glyph works for
/// all sizes.
///
/// # Example
///
/// ```ignore
/// use cosmic_text::CacheKey;
/// use astrelis_text::SdfCacheKey;
///
/// let bitmap_key = CacheKey { glyph_id: 42, font_id: ..., font_size_bits: 16.0.to_bits(), ... };
/// let sdf_key = SdfCacheKey::from_cache_key(bitmap_key);
/// // sdf_key ignores font_size_bits - same key for all sizes
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SdfCacheKey {
    /// The glyph ID within the font
    pub glyph_id: u16,
    /// The font ID (for supporting multiple fonts)
    pub font_id: u32,
}

impl SdfCacheKey {
    /// Create a new SDF cache key from a cosmic-text CacheKey.
    ///
    /// Extracts `glyph_id` and `font_id`, ignoring size-related fields
    /// (`font_size_bits`, `x_bin`, `y_bin`) since SDF glyphs are resolution-independent.
    ///
    /// We use a hash of the `font_id` since cosmic_text's ID type is opaque.
    ///
    /// # Arguments
    ///
    /// * `cache_key` - A cosmic-text CacheKey containing glyph and font information
    ///
    /// # Returns
    ///
    /// A size-independent cache key suitable for SDF atlas lookups
    pub fn from_cache_key(cache_key: CacheKey) -> Self {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        cache_key.font_id.hash(&mut hasher);
        Self {
            glyph_id: cache_key.glyph_id,
            font_id: hasher.finish() as u32,
        }
    }
}

/// SDF atlas entry with additional metadata for SDF rendering.
///
/// Contains the information needed to render an SDF glyph at any size.
/// The glyph is rasterized once at a fixed base size (48px), and the placement
/// information is used to scale it correctly at render time.
///
/// # Fields
///
/// - `entry`: Position and size in the atlas texture
/// - `spread`: Distance field spread in pixels (typically 4.0)
/// - `base_size`: The size at which the glyph was rasterized (48px)
/// - `base_placement`: Bearing offsets and dimensions at base size
///
/// # Rendering
///
/// At render time, the shader samples the SDF texture and the placement is
/// scaled by `target_size / base_size` to achieve the correct appearance at
/// any font size.
///
/// # Example
///
/// ```ignore
/// // Glyph rasterized at 48px base size with 4px spread
/// let sdf_entry = SdfAtlasEntry {
///     entry: atlas_entry,
///     spread: 4.0,
///     base_size: 48.0,
///     base_placement: GlyphPlacement { left: 2.0, top: 40.0, width: 28.0, height: 35.0 },
/// };
///
/// // Render at 24px: scale factor = 24.0 / 48.0 = 0.5
/// let render_width = sdf_entry.base_placement.width * 0.5;  // 14px
/// ```
#[derive(Debug, Clone)]
pub struct SdfAtlasEntry {
    /// Base atlas entry (position and size in atlas)
    pub entry: AtlasEntry,
    /// The SDF spread used when generating this glyph (distance field radius in pixels)
    pub spread: f32,
    /// Base font size at which the glyph was rasterized (typically 48px)
    pub base_size: f32,
    /// Original glyph metrics at base size (for scaling during rendering)
    pub base_placement: GlyphPlacement,
}

/// SDF rendering parameters passed to shaders for text effects.
///
/// This structure is uploaded to the GPU as a uniform buffer and controls
/// how the SDF shader renders effects like shadows, outlines, and glows.
///
/// # GPU Layout
///
/// The structure is `#[repr(C)]` and `bytemuck::Pod` for safe GPU upload.
/// Padding fields ensure proper alignment for GPU uniform buffers.
///
/// # Usage
///
/// ```ignore
/// use astrelis_text::{SdfParams, TextEffects, SdfConfig};
///
/// let effects = TextEffectsBuilder::new()
///     .shadow(Vec2::new(2.0, 2.0), Color::BLACK)
///     .outline(1.5, Color::WHITE)
///     .build();
///
/// let config = SdfConfig::default();
/// let params = SdfParams::from_effects(&effects, &config);
/// // Upload params to GPU uniform buffer
/// ```
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SdfParams {
    /// Edge softness for anti-aliasing (0.0 to 1.0)
    /// Lower values = sharper edges, higher values = softer edges
    pub edge_softness: f32,
    /// Outline width in SDF space (0.0 = no outline)
    pub outline_width: f32,
    /// Outline color (RGBA)
    pub outline_color: [f32; 4],
    /// Shadow offset in pixels (x, y)
    pub shadow_offset: [f32; 2],
    /// Shadow blur radius (0.0 = hard shadow)
    pub shadow_blur: f32,
    /// Shadow color (RGBA, typically with alpha < 1.0)
    pub shadow_color: [f32; 4],
    /// Glow radius in pixels (0.0 = no glow)
    pub glow_radius: f32,
    /// Glow color (RGBA)
    pub glow_color: [f32; 4],
    /// Padding for GPU alignment (unused)
    pub _padding: [f32; 2],
}

impl Default for SdfParams {
    fn default() -> Self {
        Self {
            edge_softness: 0.05,
            outline_width: 0.0,
            outline_color: [0.0, 0.0, 0.0, 1.0],
            shadow_offset: [0.0, 0.0],
            shadow_blur: 0.0,
            shadow_color: [0.0, 0.0, 0.0, 0.5],
            glow_radius: 0.0,
            glow_color: [1.0, 1.0, 1.0, 0.5],
            _padding: [0.0, 0.0],
        }
    }
}

impl SdfParams {
    /// Create SDF parameters from a collection of text effects.
    ///
    /// Extracts effect parameters (shadow offset, outline width, etc.) from the
    /// `TextEffects` collection and combines them with the SDF configuration to
    /// produce shader-ready parameters.
    ///
    /// # Arguments
    ///
    /// * `effects` - Collection of text effects to convert
    /// * `config` - SDF configuration for edge softness and other global settings
    ///
    /// # Returns
    ///
    /// GPU-ready SDF parameters that can be uploaded to a uniform buffer
    ///
    /// # Example
    ///
    /// ```ignore
    /// let effects = TextEffectsBuilder::new()
    ///     .shadow(Vec2::new(2.0, 2.0), Color::rgba(0.0, 0.0, 0.0, 0.5))
    ///     .outline(1.5, Color::WHITE)
    ///     .build();
    ///
    /// let params = SdfParams::from_effects(&effects, &config);
    /// queue.write_buffer(&uniform_buffer, 0, bytemuck::cast_slice(&[params]));
    /// ```
    pub fn from_effects(effects: &TextEffects, config: &SdfConfig) -> Self {
        let mut params = Self {
            edge_softness: config.edge_softness,
            ..Default::default()
        };

        for effect in effects.sorted_by_priority() {
            match &effect.effect_type {
                crate::effects::TextEffectType::Shadow { offset, blur_radius, color } => {
                    params.shadow_offset = [offset.x, offset.y];
                    params.shadow_blur = *blur_radius;
                    params.shadow_color = [color.r, color.g, color.b, color.a];
                }
                crate::effects::TextEffectType::Outline { width, color } => {
                    params.outline_width = *width;
                    params.outline_color = [color.r, color.g, color.b, color.a];
                }
                crate::effects::TextEffectType::Glow { radius, color, intensity: _ } => {
                    params.glow_radius = *radius;
                    params.glow_color = [color.r, color.g, color.b, color.a];
                }
                crate::effects::TextEffectType::InnerShadow { .. } => {
                    // Inner shadow requires special handling in shader
                }
            }
        }

        params
    }
}

/// Base size for SDF glyph rasterization.
/// Glyphs are rasterized at this size, then scaled via shader.
const SDF_BASE_SIZE: f32 = 48.0;

/// Default SDF spread in pixels.
const SDF_DEFAULT_SPREAD: f32 = 4.0;

/// Simple row-based atlas packer.
struct AtlasPacker {
    size: u32,
    current_x: u32,
    current_y: u32,
    row_height: u32,
}

impl AtlasPacker {
    fn new(size: u32) -> Self {
        Self {
            size,
            current_x: 0,
            current_y: 0,
            row_height: 0,
        }
    }

    fn pack(&mut self, width: u32, height: u32) -> Option<AtlasEntry> {
        // Try to fit in current row
        if self.current_x + width > self.size {
            // Move to next row
            self.current_x = 0;
            self.current_y += self.row_height;
            self.row_height = 0;
        }

        // Check if we have vertical space
        if self.current_y + height > self.size {
            return None; // Atlas full
        }

        let entry = AtlasEntry {
            x: self.current_x,
            y: self.current_y,
            width,
            height,
        };

        self.current_x += width;
        self.row_height = self.row_height.max(height);

        Some(entry)
    }
}

/// Font renderer for rendering text with WGPU.
pub struct FontRenderer {
    renderer: Renderer,
    viewport: Viewport,
    font_system: Arc<RwLock<cosmic_text::FontSystem>>,
    swash_cache: Arc<RwLock<SwashCache>>,

    // GPU resources - Bitmap rendering
    pipeline: wgpu::RenderPipeline,
    /// Kept alive for pipeline - must not be dropped while pipeline exists
    #[allow(dead_code)]
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_bind_group_layout: wgpu::BindGroupLayout,
    atlas_texture: wgpu::Texture,
    atlas_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,

    // Bitmap atlas management
    atlas_size: u32,
    atlas_data: Vec<u8>,
    atlas_entries: HashMap<CacheKey, AtlasEntry>,
    atlas_packer: AtlasPacker,
    atlas_dirty: bool,

    // GPU resources - SDF rendering
    sdf_pipeline: wgpu::RenderPipeline,
    #[allow(dead_code)]
    sdf_bind_group_layout: wgpu::BindGroupLayout,
    sdf_atlas_texture: wgpu::Texture,
    #[allow(dead_code)]
    sdf_atlas_view: wgpu::TextureView,
    sdf_bind_group: wgpu::BindGroup,
    sdf_params_buffer: wgpu::Buffer,
    #[allow(dead_code)]
    sdf_params_bind_group_layout: wgpu::BindGroupLayout,
    sdf_params_bind_group: wgpu::BindGroup,

    // SDF atlas management
    sdf_atlas_data: Vec<u8>,
    sdf_atlas_entries: HashMap<SdfCacheKey, SdfAtlasEntry>,
    sdf_atlas_packer: AtlasPacker,
    sdf_atlas_dirty: bool,

    // Render mode configuration
    render_mode: TextRenderMode,
    sdf_config: SdfConfig,

    // Staging data
    vertices: Vec<TextVertex>,
    indices: Vec<u16>,
}

impl FontRenderer {
    /// Measure text dimensions without rendering.
    pub fn measure_text(&self, text: &Text) -> (f32, f32) {
        profile_function!();
        let scale = self.viewport.scale_factor as f32;
        let mut font_system = self.font_system.write().unwrap();
        let mut buffer = TextBuffer::new(&mut font_system);
        // we increase the text size in order to make it sharper on high-DPI displays
        buffer.set_text(&mut font_system, text, scale);
        buffer.layout(&mut font_system);
        let (width, height) = buffer.bounds();
        (width / scale, height / scale)
    }

    /// Get the logical (unscaled) bounds of a prepared text buffer.
    ///
    /// Use this to get the dimensions for layout purposes after calling `prepare()`.
    /// The returned dimensions are in logical coordinates (not scaled by DPI).
    pub fn buffer_bounds(&self, buffer: &TextBuffer) -> (f32, f32) {
        let scale = self.viewport.scale_factor as f32;
        let (width, height) = buffer.bounds();
        (width / scale, height / scale)
    }

    /// Get font metrics for the given text style.
    ///
    /// Returns metrics including ascent, descent, line height, and baseline offset
    /// which are useful for precise text positioning and baseline alignment.
    pub fn get_text_metrics(&self, text: &Text) -> TextMetrics {
        profile_function!();
        let scale = self.viewport.scale_factor as f32;
        let font_size = text.get_font_size();
        let line_height_multiplier = text.get_line_height();

        // Create metrics for the given font size and line height
        let metrics = Metrics::new(
            font_size * scale,
            font_size * scale * line_height_multiplier,
        );

        // The line_height from cosmic_text includes both ascent and descent
        let line_height = metrics.line_height / scale;

        // For cosmic-text, the ascent is typically about 80% of font size
        // and descent is about 20% of font size (these are approximations)
        // We can get better metrics by actually querying the font
        let ascent = font_size * 0.8;  // Approximate ascent
        let descent = font_size * 0.2; // Approximate descent

        TextMetrics {
            ascent,
            descent,
            line_height,
            baseline_offset: ascent, // Baseline is at ascent distance from top
        }
    }

    /// Get the baseline offset from the top of the text bounding box.
    ///
    /// This is the distance from the top of the text's bounding box to the baseline
    /// of the first line of text. Useful for aligning text by baseline.
    pub fn get_baseline_offset(&self, text: &Text) -> f32 {
        let metrics = self.get_text_metrics(text);
        metrics.baseline_offset
    }

    /// Create a new font renderer.
    pub fn new(context: Arc<GraphicsContext>, font_system: FontSystem) -> Self {
        Self::new_with_atlas_size(context, font_system, 2048)
    }

    /// Create a new font renderer with a custom atlas size.
    pub fn new_with_atlas_size(
        context: Arc<GraphicsContext>,
        font_system: FontSystem,
        atlas_size: u32,
    ) -> Self {
        let renderer = Renderer::new(context.clone());
        let swash_cache = Arc::new(RwLock::new(SwashCache::new()));

        // Create shader
        let shader =
            renderer.create_shader(Some("Text Shader"), include_str!("../shaders/text.wgsl"));

        // Create atlas texture
        let atlas_texture = renderer.create_texture(&wgpu::TextureDescriptor {
            label: Some("Text Atlas"),
            size: wgpu::Extent3d {
                width: atlas_size,
                height: atlas_size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = renderer.create_linear_sampler(Some("Text Sampler"));

        // Initialize atlas data
        let atlas_data = vec![0u8; (atlas_size * atlas_size) as usize];

        // Create bind group layout
        let bind_group_layout = renderer.create_bind_group_layout(
            Some("Text Bind Group Layout"),
            &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
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

        let bind_group = renderer.create_bind_group(
            Some("Text Bind Group"),
            &bind_group_layout,
            &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        );

        // Create uniform bind group layout for projection matrix
        let uniform_bind_group_layout = renderer.create_bind_group_layout(
            Some("Text Uniform Layout"),
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

        // Create pipeline layout with both bind groups
        let pipeline_layout = renderer.create_pipeline_layout(
            Some("Text Pipeline Layout"),
            &[&bind_group_layout, &uniform_bind_group_layout],
            &[],
        );

        // Create pipeline
        let pipeline = renderer.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Text Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<TextVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2,
                        1 => Float32x2,
                        2 => Float32x4,
                    ],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
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

        // ========== SDF Rendering Setup ==========

        // Create SDF shader
        let sdf_shader = renderer.create_shader(
            Some("Text SDF Shader"),
            include_str!("../shaders/text_sdf.wgsl"),
        );

        // Create SDF atlas texture (same size as bitmap atlas)
        let sdf_atlas_texture = renderer.create_texture(&wgpu::TextureDescriptor {
            label: Some("SDF Text Atlas"),
            size: wgpu::Extent3d {
                width: atlas_size,
                height: atlas_size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let sdf_atlas_view = sdf_atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sdf_atlas_data = vec![0u8; (atlas_size * atlas_size) as usize];

        // SDF bind group layout (texture + sampler, same structure as bitmap)
        let sdf_bind_group_layout = renderer.create_bind_group_layout(
            Some("SDF Text Bind Group Layout"),
            &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
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

        let sdf_bind_group = renderer.create_bind_group(
            Some("SDF Text Bind Group"),
            &sdf_bind_group_layout,
            &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&sdf_atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        );

        // SDF params uniform buffer
        let sdf_params_buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SDF Params Buffer"),
            size: std::mem::size_of::<SdfParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // SDF params bind group layout
        let sdf_params_bind_group_layout = renderer.create_bind_group_layout(
            Some("SDF Params Bind Group Layout"),
            &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        );

        let sdf_params_bind_group = renderer.create_bind_group(
            Some("SDF Params Bind Group"),
            &sdf_params_bind_group_layout,
            &[wgpu::BindGroupEntry {
                binding: 0,
                resource: sdf_params_buffer.as_entire_binding(),
            }],
        );

        // Create SDF pipeline layout (atlas + projection + sdf_params)
        let sdf_pipeline_layout = renderer.create_pipeline_layout(
            Some("SDF Text Pipeline Layout"),
            &[
                &sdf_bind_group_layout,
                &uniform_bind_group_layout,
                &sdf_params_bind_group_layout,
            ],
            &[],
        );

        // Create SDF pipeline
        let sdf_pipeline = renderer.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("SDF Text Pipeline"),
            layout: Some(&sdf_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &sdf_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<TextVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2,
                        1 => Float32x2,
                        2 => Float32x4,
                    ],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &sdf_shader,
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

        Self {
            viewport: Viewport::default(),
            renderer,
            font_system: font_system.inner(),
            swash_cache,
            // Bitmap rendering
            pipeline,
            bind_group_layout,
            uniform_bind_group_layout,
            atlas_texture,
            atlas_view,
            sampler,
            bind_group,
            atlas_size,
            atlas_data,
            atlas_entries: HashMap::new(),
            atlas_packer: AtlasPacker::new(atlas_size),
            atlas_dirty: false,
            // SDF rendering
            sdf_pipeline,
            sdf_bind_group_layout,
            sdf_atlas_texture,
            sdf_atlas_view,
            sdf_bind_group,
            sdf_params_buffer,
            sdf_params_bind_group_layout,
            sdf_params_bind_group,
            sdf_atlas_data,
            sdf_atlas_entries: HashMap::new(),
            sdf_atlas_packer: AtlasPacker::new(atlas_size),
            sdf_atlas_dirty: false,
            // Configuration
            render_mode: TextRenderMode::default(),
            sdf_config: SdfConfig::default(),
            // Staging data
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// Ensure a glyph is in the atlas, rasterizing and uploading if needed.
    fn ensure_glyph(&mut self, cache_key: CacheKey) -> Option<&AtlasEntry> {
        // Check if already in atlas
        if self.atlas_entries.contains_key(&cache_key) {
            return self.atlas_entries.get(&cache_key);
        }

        // Rasterize the glyph
        let mut font_system = self.font_system.write().unwrap();
        let mut swash_cache = self.swash_cache.write().unwrap();
        let image = match swash_cache.get_image(&mut font_system, cache_key) {
            Some(img) => img,
            None => return None,
        };

        let width = image.placement.width;
        let height = image.placement.height;

        if width == 0 || height == 0 {
            return None;
        }

        // Try to pack into atlas
        let entry = self.atlas_packer.pack(width, height)?;

        // Copy glyph data into atlas
        for y in 0..height {
            for x in 0..width {
                let src_idx = (y * width + x) as usize;
                let dst_idx = ((entry.y + y) * self.atlas_size + (entry.x + x)) as usize;
                if src_idx < image.data.len() && dst_idx < self.atlas_data.len() {
                    self.atlas_data[dst_idx] = image.data[src_idx];
                }
            }
        }

        self.atlas_dirty = true;
        self.atlas_entries.insert(cache_key, entry.clone());
        self.atlas_entries.get(&cache_key)
    }

    /// Upload atlas data to GPU if dirty.
    fn upload_atlas(&mut self) {
        if !self.atlas_dirty {
            return;
        }

        self.renderer.queue().write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &self.atlas_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.atlas_size),
                rows_per_image: Some(self.atlas_size),
            },
            wgpu::Extent3d {
                width: self.atlas_size,
                height: self.atlas_size,
                depth_or_array_layers: 1,
            },
        );

        self.atlas_dirty = false;
    }

    /// Upload SDF atlas data to GPU if dirty.
    fn upload_sdf_atlas(&mut self) {
        if !self.sdf_atlas_dirty {
            return;
        }

        self.renderer.queue().write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.sdf_atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &self.sdf_atlas_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.atlas_size),
                rows_per_image: Some(self.atlas_size),
            },
            wgpu::Extent3d {
                width: self.atlas_size,
                height: self.atlas_size,
                depth_or_array_layers: 1,
            },
        );

        self.sdf_atlas_dirty = false;
    }

    /// Ensure a glyph is in the SDF atlas, rasterizing at base size and generating SDF if needed.
    ///
    /// SDF glyphs are size-independent: they're rasterized at a fixed base size (48px)
    /// and scaled via shader. This allows a single cached glyph to work at any display size.
    fn ensure_glyph_sdf(&mut self, cache_key: CacheKey) -> Option<&SdfAtlasEntry> {
        let sdf_key = SdfCacheKey::from_cache_key(cache_key);

        // Check if already in SDF atlas
        if self.sdf_atlas_entries.contains_key(&sdf_key) {
            return self.sdf_atlas_entries.get(&sdf_key);
        }

        // Create a cache key at base size for rasterization
        // We need to create a new CacheKey with the base size for consistent SDF generation
        // font_size_bits stores the f32 representation of font size as bits
        let base_cache_key = CacheKey {
            font_id: cache_key.font_id,
            glyph_id: cache_key.glyph_id,
            font_size_bits: SDF_BASE_SIZE.to_bits(),
            x_bin: cache_key.x_bin,
            y_bin: cache_key.y_bin,
            flags: cache_key.flags,
        };

        // Rasterize the glyph at base size
        let mut font_system = self.font_system.write().unwrap();
        let mut swash_cache = self.swash_cache.write().unwrap();
        let image = match swash_cache.get_image(&mut font_system, base_cache_key) {
            Some(img) => img.clone(),
            None => return None,
        };

        drop(font_system);
        drop(swash_cache);

        let width = image.placement.width;
        let height = image.placement.height;

        if width == 0 || height == 0 {
            return None;
        }

        // Generate SDF from the rasterized bitmap
        let spread = self.sdf_config.mode.spread().max(SDF_DEFAULT_SPREAD);
        let sdf_data = generate_sdf(&image, spread);

        if sdf_data.is_empty() {
            return None;
        }

        // Add padding for effects (shadow, glow can extend beyond glyph bounds)
        let padding = (spread.ceil() as u32) * 2;
        let padded_width = width + padding * 2;
        let padded_height = height + padding * 2;

        // Try to pack into SDF atlas
        let atlas_entry = self.sdf_atlas_packer.pack(padded_width, padded_height)?;

        // Copy SDF data into atlas with padding
        for y in 0..height {
            for x in 0..width {
                let src_idx = (y * width + x) as usize;
                let dst_x = atlas_entry.x + padding + x;
                let dst_y = atlas_entry.y + padding + y;
                let dst_idx = (dst_y * self.atlas_size + dst_x) as usize;
                if src_idx < sdf_data.len() && dst_idx < self.sdf_atlas_data.len() {
                    self.sdf_atlas_data[dst_idx] = sdf_data[src_idx];
                }
            }
        }

        // Store the base placement info for proper scaling at render time
        let base_placement = GlyphPlacement {
            left: image.placement.left as f32,
            top: image.placement.top as f32,
            width: width as f32,
            height: height as f32,
        };

        let sdf_entry = SdfAtlasEntry {
            entry: AtlasEntry {
                x: atlas_entry.x + padding,
                y: atlas_entry.y + padding,
                width,
                height,
            },
            spread,
            base_size: SDF_BASE_SIZE,
            base_placement,
        };

        self.sdf_atlas_dirty = true;
        self.sdf_atlas_entries.insert(sdf_key, sdf_entry);
        self.sdf_atlas_entries.get(&sdf_key)
    }

    /// Set the text render mode (Bitmap or SDF).
    ///
    /// Controls which rendering pipeline is used for subsequent text rendering.
    /// This affects all text drawn until the mode is changed again.
    ///
    /// # Arguments
    ///
    /// * `mode` - The render mode to use:
    ///   - `TextRenderMode::Bitmap` - Traditional bitmap atlas (sharper at small sizes)
    ///   - `TextRenderMode::SDF { spread }` - Signed distance field (scalable, supports effects)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use astrelis_text::{FontRenderer, TextRenderMode};
    ///
    /// let mut renderer = FontRenderer::new(context, font_system);
    ///
    /// // Use SDF for large, scalable text
    /// renderer.set_render_mode(TextRenderMode::SDF { spread: 4.0 });
    ///
    /// // Switch back to bitmap for small UI text
    /// renderer.set_render_mode(TextRenderMode::Bitmap);
    /// ```
    pub fn set_render_mode(&mut self, mode: TextRenderMode) {
        self.render_mode = mode;
    }

    /// Get the current render mode.
    pub fn render_mode(&self) -> TextRenderMode {
        self.render_mode
    }

    /// Set SDF configuration.
    pub fn set_sdf_config(&mut self, config: SdfConfig) {
        // Update render mode if config specifies SDF
        if config.mode.is_sdf() {
            self.render_mode = config.mode;
        }
        self.sdf_config = config;
    }

    /// Get the current SDF configuration.
    pub fn sdf_config(&self) -> &SdfConfig {
        &self.sdf_config
    }

    /// Determine the appropriate render mode based on font size and effects.
    ///
    /// This is a helper function that implements the hybrid rendering strategy:
    /// - Small text (< 24px) without effects: use Bitmap for sharpness
    /// - Large text (>= 24px) or text with effects: use SDF for quality and effects
    ///
    /// # Arguments
    ///
    /// * `font_size` - Font size in pixels
    /// * `has_effects` - Whether the text has effects (shadows, outlines, glows)
    ///
    /// # Returns
    ///
    /// The recommended `TextRenderMode` for optimal quality
    ///
    /// # Example
    ///
    /// ```ignore
    /// use astrelis_text::{FontRenderer, Text};
    ///
    /// let text = Text::new("Hello").size(32.0).with_shadow(...);
    ///
    /// let mode = FontRenderer::select_render_mode(
    ///     text.get_font_size(),
    ///     text.has_effects()
    /// );
    /// // Returns TextRenderMode::SDF { spread: 4.0 } because size >= 24px
    /// ```
    ///
    /// # Hybrid Strategy
    ///
    /// The 24px threshold is chosen based on typical UI text sizes:
    /// - UI labels, buttons, and body text are typically 12-18px (bitmap is sharper)
    /// - Headings and display text are typically 24px+ (SDF scales better)
    /// - Any text with effects automatically uses SDF regardless of size
    pub fn select_render_mode(font_size: f32, has_effects: bool) -> TextRenderMode {
        if has_effects {
            return TextRenderMode::SDF { spread: SDF_DEFAULT_SPREAD };
        }
        if font_size >= 24.0 {
            return TextRenderMode::SDF { spread: SDF_DEFAULT_SPREAD };
        }
        TextRenderMode::Bitmap
    }

    /// Draw text with effects at a position using SDF rendering.
    ///
    /// This method automatically switches to SDF mode when effects are present and
    /// configures the shader parameters from the provided effects collection.
    ///
    /// Effects include shadows, outlines, and glows. Multiple effects can be combined
    /// and are rendered in a single draw call using the SDF shader.
    ///
    /// # Arguments
    ///
    /// * `buffer` - Prepared text buffer containing shaped glyphs
    /// * `position` - Top-left position to draw the text
    /// * `effects` - Collection of effects to apply (shadows, outlines, glows)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use astrelis_text::{FontRenderer, Text, TextEffectsBuilder, Color};
    /// use astrelis_core::math::Vec2;
    ///
    /// let mut renderer = FontRenderer::new(context, font_system);
    ///
    /// let text = Text::new("Glowing Text").size(48.0);
    /// let mut buffer = renderer.prepare(&text);
    ///
    /// let effects = TextEffectsBuilder::new()
    ///     .shadow(Vec2::new(2.0, 2.0), Color::rgba(0.0, 0.0, 0.0, 0.5))
    ///     .outline(2.0, Color::WHITE)
    ///     .glow(5.0, Color::CYAN, 0.8)
    ///     .build();
    ///
    /// renderer.draw_text_with_effects(&mut buffer, Vec2::new(100.0, 100.0), &effects);
    /// ```
    ///
    /// # Performance
    ///
    /// All effects are rendered in a single pass using the SDF shader. The shader
    /// samples the distance field and applies effects based on the uploaded uniform
    /// parameters.
    pub fn draw_text_with_effects(
        &mut self,
        buffer: &mut TextBuffer,
        position: Vec2,
        effects: &TextEffects,
    ) {
        profile_function!();

        // Always use SDF mode when effects are present
        if effects.has_enabled_effects() && !self.render_mode.is_sdf() {
            self.render_mode = TextRenderMode::SDF { spread: SDF_DEFAULT_SPREAD };
        }

        // Update SDF params from effects
        let sdf_params = SdfParams::from_effects(effects, &self.sdf_config);
        self.renderer.queue().write_buffer(
            &self.sdf_params_buffer,
            0,
            bytemuck::cast_slice(&[sdf_params]),
        );

        // Use SDF drawing path
        self.draw_text_sdf_internal(buffer, position);
    }

    /// Internal SDF text drawing implementation.
    fn draw_text_sdf_internal(&mut self, buffer: &mut TextBuffer, position: Vec2) {
        profile_function!();

        let scale = self.viewport.scale_factor as f32;
        let mut font_system = self.font_system.write().unwrap();
        buffer.layout(&mut font_system);
        drop(font_system);

        // Render glyphs using SDF atlas
        for run in buffer.buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                let physical_glyph = glyph.physical((position.x, position.y + run.line_y), 1.0);
                let cache_key = physical_glyph.cache_key;

                // Ensure glyph is in SDF atlas
                let sdf_entry = match self.ensure_glyph_sdf(cache_key) {
                    Some(e) => e.clone(),
                    None => continue,
                };

                // Calculate scale factor from base size to target size
                let target_size = f32::from_bits(cache_key.font_size_bits as u32);
                let size_scale = target_size / sdf_entry.base_size;

                // Scale placement based on size ratio
                let scaled_left = sdf_entry.base_placement.left * size_scale;
                let scaled_top = sdf_entry.base_placement.top * size_scale;
                let scaled_width = sdf_entry.base_placement.width * size_scale;
                let scaled_height = sdf_entry.base_placement.height * size_scale;

                let x = physical_glyph.x as f32 + scaled_left;
                let y = physical_glyph.y as f32 - scaled_top;
                let w = scaled_width;
                let h = scaled_height;

                let x = x / scale;
                let y = y / scale;
                let w = w / scale;
                let h = h / scale;

                let (u0, v0, u1, v1) = sdf_entry.entry.uv_coords(self.atlas_size);

                let color = glyph.color_opt.unwrap_or(CosmicColor::rgb(255, 255, 255));
                let color_f = [
                    color.r() as f32 / 255.0,
                    color.g() as f32 / 255.0,
                    color.b() as f32 / 255.0,
                    color.a() as f32 / 255.0,
                ];

                // Pixel snapping for crisp rendering
                let x = (x * scale).round() / scale;
                let y = (y * scale).round() / scale;

                // Create quad
                let idx = self.vertices.len() as u16;

                self.vertices.push(TextVertex {
                    position: [x, y],
                    tex_coords: [u0, v0],
                    color: color_f,
                });
                self.vertices.push(TextVertex {
                    position: [x + w, y],
                    tex_coords: [u1, v0],
                    color: color_f,
                });
                self.vertices.push(TextVertex {
                    position: [x + w, y + h],
                    tex_coords: [u1, v1],
                    color: color_f,
                });
                self.vertices.push(TextVertex {
                    position: [x, y + h],
                    tex_coords: [u0, v1],
                    color: color_f,
                });

                self.indices
                    .extend_from_slice(&[idx, idx + 1, idx + 2, idx, idx + 2, idx + 3]);
            }
        }
    }

    pub fn set_viewport(&mut self, viewport: Viewport) {
        if viewport.scale_factor != self.viewport.scale_factor {
            tracing::trace!(
                "FontRenderer scale factor changed: {} -> {}",
                self.viewport.scale_factor,
                viewport.scale_factor
            );
            // Clear bitmap atlas and repack on scale factor change
            self.atlas_entries.clear();
            self.atlas_packer = AtlasPacker::new(self.atlas_size);
            self.atlas_dirty = true;

            // Note: SDF atlas doesn't need to be cleared on scale factor change
            // because SDF glyphs are resolution-independent (rendered at fixed base size)
        }
        self.viewport = viewport;
    }

    /// Prepare text for rendering. Returns a TextBuffer handle.
    ///
    /// This buffer can be cached and reused for rendering the same text multiple times,
    /// but must be revalidated if the text content, style, or scale factor changes.
    pub fn prepare(&mut self, text: &Text) -> TextBuffer {
        profile_function!();
        let mut font_system = self.font_system.write().unwrap();
        let mut buffer = TextBuffer::new(&mut font_system);
        buffer.set_text(&mut font_system, text, self.viewport.scale_factor as f32);
        buffer.layout(&mut font_system);
        buffer
    }

    /// Draw text at a position.
    ///
    /// The position represents the **top-left corner** of the text's bounding box.
    /// This is consistent with UI layout conventions (CSS, Flutter) where elements
    /// are positioned by their top-left corner.
    pub fn draw_text(&mut self, buffer: &mut TextBuffer, position: Vec2) {
        profile_function!();

        let scale = self.viewport.scale_factor as f32;
        let mut font_system = self.font_system.write().unwrap();
        buffer.layout(&mut font_system);
        drop(font_system);

        // Render glyphs
        for run in buffer.buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                // Use run.line_y for proper multi-line positioning
                // Position is in logical coordinates, but run.line_y is in scaled coordinates
                // So we scale the position to physical space before combining
                let physical_glyph = glyph.physical((position.x * scale, position.y * scale + run.line_y), 1.0);
                let cache_key = physical_glyph.cache_key;

                // Ensure glyph is in atlas
                let entry = match self.ensure_glyph(cache_key) {
                    Some(e) => e.clone(),
                    None => continue,
                };

                // Get glyph placement info
                let mut font_system = self.font_system.write().unwrap();
                let mut swash_cache = self.swash_cache.write().unwrap();

                if let Some(image) = swash_cache.get_image(&mut font_system, cache_key) {
                    let x = physical_glyph.x as f32 + image.placement.left as f32;
                    let y = physical_glyph.y as f32 - image.placement.top as f32;
                    let w = image.placement.width as f32;
                    let h = image.placement.height as f32;

                    let x = x / scale;
                    let y = y / scale;
                    let w = w / scale;
                    let h = h / scale;

                    drop(font_system);
                    drop(swash_cache);

                    let (u0, v0, u1, v1) = entry.uv_coords(self.atlas_size);

                    let color = glyph.color_opt.unwrap_or(CosmicColor::rgb(255, 255, 255));
                    let color_f = [
                        color.r() as f32 / 255.0,
                        color.g() as f32 / 255.0,
                        color.b() as f32 / 255.0,
                        color.a() as f32 / 255.0,
                    ];

                    // TODO: Do we want to do pixel snapping here?
                    let x = (x * scale).round() / scale;
                    let y = (y * scale).round() / scale;

                    // Create quad
                    let idx = self.vertices.len() as u16;

                    self.vertices.push(TextVertex {
                        position: [x, y],
                        tex_coords: [u0, v0],
                        color: color_f,
                    });
                    self.vertices.push(TextVertex {
                        position: [x + w, y],
                        tex_coords: [u1, v0],
                        color: color_f,
                    });
                    self.vertices.push(TextVertex {
                        position: [x + w, y + h],
                        tex_coords: [u1, v1],
                        color: color_f,
                    });
                    self.vertices.push(TextVertex {
                        position: [x, y + h],
                        tex_coords: [u0, v1],
                        color: color_f,
                    });

                    // Create indices for two triangles
                    self.indices
                        .extend_from_slice(&[idx, idx + 1, idx + 2, idx, idx + 2, idx + 3]);
                }
            }
        }
    }

    /// Render all queued text to the given render pass.
    ///
    /// Automatically selects bitmap or SDF pipeline based on the current render mode.
    pub fn render(&mut self, render_pass: &mut wgpu::RenderPass) {
        profile_function!();

        debug_assert!(
            self.viewport.is_valid(),
            "Viewport size must be set before rendering text."
        );

        if self.vertices.is_empty() {
            return;
        }

        // Upload appropriate atlas based on render mode
        if self.render_mode.is_sdf() {
            self.upload_sdf_atlas();
        } else {
            self.upload_atlas();
        }

        // Create buffers
        let vertex_buffer = self
            .renderer
            .create_vertex_buffer(Some("Text Vertex Buffer"), &self.vertices);

        let index_buffer = self
            .renderer
            .create_index_buffer(Some("Text Index Buffer"), &self.indices);

        // Create projection uniform
        let size = self.viewport.to_logical();
        let projection = orthographic_projection(size.width, size.height);
        let uniform_buffer = self
            .renderer
            .create_uniform_buffer(Some("Text Projection"), &projection);

        // Create uniform bind group
        let uniform_bind_group = self.renderer.create_bind_group(
            Some("Text Uniform Bind Group"),
            &self.uniform_bind_group_layout,
            &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        );

        // Render with appropriate pipeline
        if self.render_mode.is_sdf() {
            // SDF pipeline: atlas (group 0) + projection (group 1) + sdf_params (group 2)
            render_pass.set_pipeline(&self.sdf_pipeline);
            render_pass.set_bind_group(0, &self.sdf_bind_group, &[]);
            render_pass.set_bind_group(1, &uniform_bind_group, &[]);
            render_pass.set_bind_group(2, &self.sdf_params_bind_group, &[]);
        } else {
            // Bitmap pipeline: atlas (group 0) + projection (group 1)
            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.set_bind_group(1, &uniform_bind_group, &[]);
        }

        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.indices.len() as u32, 0, 0..1);

        // Clear for next frame
        self.vertices.clear();
        self.indices.clear();
    }

    /// Get the font system.
    pub fn font_system(&self) -> Arc<RwLock<cosmic_text::FontSystem>> {
        self.font_system.clone()
    }

    /// Get the atlas size in pixels.
    pub fn atlas_size(&self) -> u32 {
        self.atlas_size
    }

    /// Ensure a glyph is in the atlas using a cache key.
    ///
    /// This is a public wrapper around the internal ensure_glyph method
    /// for use by the retained rendering system.
    pub fn ensure_glyph_in_atlas(&mut self, cache_key: CacheKey) -> Option<&AtlasEntry> {
        self.ensure_glyph(cache_key)
    }

    /// Get glyph placement information (left/top offsets, width, height).
    ///
    /// Returns the placement metrics needed to correctly position a glyph on screen.
    /// This includes the bearing offsets that position the glyph relative to its baseline.
    pub fn get_glyph_placement(&mut self, cache_key: CacheKey) -> Option<GlyphPlacement> {
        let mut font_system = self.font_system.write().unwrap();
        let mut swash_cache = self.swash_cache.write().unwrap();

        let image = swash_cache
            .get_image(&mut font_system, cache_key)
            .as_ref()?;

        let scale = self.viewport.scale_factor as f32;

        Some(GlyphPlacement {
            left: image.placement.left as f32 / scale,
            top: image.placement.top as f32 / scale,
            width: image.placement.width as f32 / scale,
            height: image.placement.height as f32 / scale,
        })
    }

    /// Ensure a glyph is in the atlas and get its placement info.
    ///
    /// This is a combined operation to avoid multiple mutable borrows.
    /// Returns both the atlas entry and glyph placement information.
    pub fn ensure_glyph_with_placement(
        &mut self,
        cache_key: CacheKey,
    ) -> Option<(AtlasEntry, GlyphPlacement)> {
        // First ensure the glyph is in the atlas
        let atlas_entry = self.ensure_glyph(cache_key)?.clone();

        // Then get the placement info
        let mut font_system = self.font_system.write().unwrap();
        let mut swash_cache = self.swash_cache.write().unwrap();

        let image = swash_cache
            .get_image(&mut font_system, cache_key)
            .as_ref()?;

        let scale = self.viewport.scale_factor as f32;

        let placement = GlyphPlacement {
            left: image.placement.left as f32 / scale,
            top: image.placement.top as f32 / scale,
            width: image.placement.width as f32 / scale,
            height: image.placement.height as f32 / scale,
        };

        Some((atlas_entry, placement))
    }

    /// Get swash cache for external glyph operations.
    pub fn swash_cache(&self) -> Arc<RwLock<cosmic_text::SwashCache>> {
        self.swash_cache.clone()
    }

    /// Get the atlas texture view for binding.
    pub fn atlas_texture_view(&self) -> &wgpu::TextureView {
        &self.atlas_view
    }

    /// Get the atlas sampler for binding.
    pub fn atlas_sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    /// Upload atlas data to GPU if dirty (public wrapper).
    pub fn upload_atlas_if_dirty(&mut self) {
        profile_function!();

        self.upload_atlas();
    }

    /// Check if the atlas has pending changes.
    pub fn is_atlas_dirty(&self) -> bool {
        self.atlas_dirty
    }

    /// Get an atlas entry by cache key (if it exists).
    pub fn get_atlas_entry(&self, cache_key: CacheKey) -> Option<&AtlasEntry> {
        self.atlas_entries.get(&cache_key)
    }
}

/// Create an orthographic projection matrix for screen-space rendering.
///
/// This matrix transforms from screen coordinates (top-left origin, Y down)
/// to normalized device coordinates (NDC) where:
/// - X ranges from -1 (left) to +1 (right)
/// - Y ranges from -1 (bottom) to +1 (top)
///
/// The negative Y scale factor (-2.0 / height) flips the Y axis to convert
/// from top-left origin (UI convention) to bottom-left origin (OpenGL/NDC convention).
fn orthographic_projection(width: f32, height: f32) -> [[f32; 4]; 4] {
    [
        [2.0 / width, 0.0, 0.0, 0.0],
        [0.0, -2.0 / height, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0, 1.0],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use astrelis_render::Color;

    #[test]
    fn test_sdf_cache_key_basic() {
        // Test SdfCacheKey creation directly
        let key1 = SdfCacheKey {
            glyph_id: 100,
            font_id: 12345,
        };
        let key2 = SdfCacheKey {
            glyph_id: 100,
            font_id: 12345,
        };
        // Same glyph_id and font_id should be equal
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_sdf_cache_key_different_glyphs() {
        // Test that different glyph IDs produce different cache keys
        let key1 = SdfCacheKey {
            glyph_id: 100,
            font_id: 12345,
        };
        let key2 = SdfCacheKey {
            glyph_id: 200,
            font_id: 12345,
        };
        // Different glyph IDs should produce different keys
        assert_ne!(key1.glyph_id, key2.glyph_id);
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_sdf_cache_key_different_fonts() {
        // Test that different font IDs produce different cache keys
        let key1 = SdfCacheKey {
            glyph_id: 100,
            font_id: 12345,
        };
        let key2 = SdfCacheKey {
            glyph_id: 100,
            font_id: 67890,
        };
        // Different font IDs should produce different keys
        assert_ne!(key1.font_id, key2.font_id);
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_sdf_cache_key_hash() {
        // Test that SdfCacheKey can be used as a HashMap key
        use std::collections::HashMap;

        let mut map = HashMap::new();
        let key = SdfCacheKey {
            glyph_id: 65, // 'A'
            font_id: 1,
        };
        map.insert(key, "test_value");

        assert_eq!(map.get(&key), Some(&"test_value"));
    }

    #[test]
    fn test_sdf_params_default() {
        let params = SdfParams::default();

        assert_eq!(params.edge_softness, 0.05);
        assert_eq!(params.outline_width, 0.0);
        assert_eq!(params.outline_color, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(params.shadow_offset, [0.0, 0.0]);
        assert_eq!(params.shadow_blur, 0.0);
        assert_eq!(params.shadow_color, [0.0, 0.0, 0.0, 0.5]);
        assert_eq!(params.glow_radius, 0.0);
        assert_eq!(params.glow_color, [1.0, 1.0, 1.0, 0.5]);
    }

    #[test]
    fn test_sdf_params_from_effects_shadow() {
        use crate::effects::{TextEffect, TextEffects};

        let mut effects = TextEffects::new();
        effects.add(TextEffect::shadow_blurred(
            Vec2::new(2.0, 3.0),
            1.5,
            Color::rgba(0.1, 0.2, 0.3, 0.8),
        ));

        let config = SdfConfig::default();
        let params = SdfParams::from_effects(&effects, &config);

        assert_eq!(params.shadow_offset, [2.0, 3.0]);
        assert_eq!(params.shadow_blur, 1.5);
        assert_eq!(params.shadow_color[0], 0.1);
        assert_eq!(params.shadow_color[1], 0.2);
        assert_eq!(params.shadow_color[2], 0.3);
        assert_eq!(params.shadow_color[3], 0.8);
    }

    #[test]
    fn test_sdf_params_from_effects_outline() {
        use crate::effects::{TextEffect, TextEffects};

        let mut effects = TextEffects::new();
        effects.add(TextEffect::outline(2.5, Color::rgba(1.0, 0.0, 0.0, 1.0)));

        let config = SdfConfig::default();
        let params = SdfParams::from_effects(&effects, &config);

        assert_eq!(params.outline_width, 2.5);
        assert_eq!(params.outline_color, [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_sdf_params_from_effects_glow() {
        use crate::effects::{TextEffect, TextEffects};

        let mut effects = TextEffects::new();
        effects.add(TextEffect::glow(
            5.0,
            Color::rgba(0.0, 1.0, 0.0, 0.9),
            0.7,
        ));

        let config = SdfConfig::default();
        let params = SdfParams::from_effects(&effects, &config);

        assert_eq!(params.glow_radius, 5.0);
        assert_eq!(params.glow_color, [0.0, 1.0, 0.0, 0.9]);
    }

    #[test]
    fn test_sdf_params_from_effects_multiple() {
        use crate::effects::{TextEffect, TextEffects};

        let mut effects = TextEffects::new();
        effects.add(TextEffect::shadow(Vec2::new(1.0, 1.0), Color::BLACK));
        effects.add(TextEffect::outline(1.0, Color::WHITE));
        effects.add(TextEffect::glow(3.0, Color::BLUE, 0.5));

        let config = SdfConfig::default();
        let params = SdfParams::from_effects(&effects, &config);

        // All effects should be present
        assert_eq!(params.shadow_offset, [1.0, 1.0]);
        assert_eq!(params.outline_width, 1.0);
        assert_eq!(params.glow_radius, 3.0);
    }

    #[test]
    fn test_sdf_params_from_effects_custom_edge_softness() {
        use crate::effects::TextEffects;

        let effects = TextEffects::new();
        let config = SdfConfig::default().edge_softness(0.15);
        let params = SdfParams::from_effects(&effects, &config);

        assert_eq!(params.edge_softness, 0.15);
    }

    #[test]
    fn test_select_render_mode_small_text_no_effects() {
        let mode = FontRenderer::select_render_mode(12.0, false);
        assert!(!mode.is_sdf());
        assert_eq!(mode, TextRenderMode::Bitmap);
    }

    #[test]
    fn test_select_render_mode_large_text_no_effects() {
        let mode = FontRenderer::select_render_mode(32.0, false);
        assert!(mode.is_sdf());
        assert_eq!(mode.spread(), SDF_DEFAULT_SPREAD);
    }

    #[test]
    fn test_select_render_mode_small_text_with_effects() {
        let mode = FontRenderer::select_render_mode(12.0, true);
        assert!(mode.is_sdf());
        assert_eq!(mode.spread(), SDF_DEFAULT_SPREAD);
    }

    #[test]
    fn test_select_render_mode_boundary() {
        // Exactly at 24px boundary
        let mode = FontRenderer::select_render_mode(24.0, false);
        assert!(mode.is_sdf());

        // Just below boundary
        let mode = FontRenderer::select_render_mode(23.9, false);
        assert!(!mode.is_sdf());
    }
}
