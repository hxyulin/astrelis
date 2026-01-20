//! Shared types and resources for text rendering.
//!
//! This module contains types that are common to all text rendering backends:
//! - [`SharedContext`]: Common resources (font system, viewport, projections)
//! - [`TextBuffer`]: Cached text buffer with layout information
//! - [`TextVertex`]: Vertex data for text rendering
//! - [`DecorationVertex`]: Vertex data for decoration rendering
//! - [`DecorationRenderer`]: Shared renderer for text decorations
//! - [`AtlasEntry`]: Position and size in atlas texture
//! - [`GlyphPlacement`]: Glyph metrics for positioning
//! - [`SdfCacheKey`]: Size-independent cache key for SDF glyphs
//! - [`SdfAtlasEntry`]: SDF glyph entry with scaling metadata
//! - [`SdfParams`]: SDF rendering parameters for shaders
//! - [`TextRendererConfig`]: Configuration for atlas sizes

use std::sync::{Arc, RwLock};

use astrelis_core::math::Vec2;
use astrelis_core::profiling::profile_function;
use cosmic_text::{Buffer, CacheKey, Metrics, Shaping, SwashCache};

use astrelis_render::{GraphicsContext, Renderer, Viewport, wgpu};

use crate::{
    decoration::{DecorationQuad, TextBounds, TextDecoration, generate_decoration_quads},
    effects::TextEffects,
    sdf::SdfConfig,
    text::{Text, color_to_cosmic},
};

use super::orthographic_projection;

/// Configuration for text renderer backends.
///
/// Controls atlas texture sizes which directly impact memory usage.
/// Smaller atlases use less memory but may need to evict glyphs more frequently.
///
/// # Memory Usage
///
/// | Config | Atlas Size | Memory/Atlas | Bitmap Total | Hybrid Total |
/// |--------|------------|--------------|--------------|--------------|
/// | small() | 512x512 | 0.5 MB | 1 MB | 2 MB |
/// | medium() | 1024x1024 | 2 MB | 4 MB | 8 MB |
/// | large() | 2048x2048 | 4 MB | 8 MB | 16 MB |
///
/// # Example
///
/// ```ignore
/// use astrelis_text::{BitmapTextRenderer, TextRendererConfig};
///
/// // For memory-constrained environments
/// let renderer = BitmapTextRenderer::with_config(
///     context,
///     font_system,
///     TextRendererConfig::small()
/// );
///
/// // For text-heavy applications
/// let renderer = BitmapTextRenderer::with_config(
///     context,
///     font_system,
///     TextRendererConfig::large()
/// );
/// ```
#[derive(Clone, Debug)]
pub struct TextRendererConfig {
    /// Atlas texture size (width and height, must be power of 2).
    /// Default: 2048
    pub atlas_size: u32,
    /// SDF-specific settings (only used by SDF/Hybrid renderers).
    pub sdf: SdfConfig,
}

impl Default for TextRendererConfig {
    fn default() -> Self {
        Self {
            atlas_size: 2048,
            sdf: SdfConfig::default(),
        }
    }
}

impl TextRendererConfig {
    /// Create default configuration (2048x2048 atlas, ~8 MB per atlas).
    pub fn new() -> Self {
        Self::default()
    }

    /// Small config for memory-constrained environments (512x512, ~0.5 MB per atlas).
    ///
    /// Best for applications with limited text or embedded devices.
    pub fn small() -> Self {
        Self {
            atlas_size: 512,
            ..Default::default()
        }
    }

    /// Medium config (~1024x1024, ~2 MB per atlas).
    ///
    /// Good balance for most applications.
    pub fn medium() -> Self {
        Self {
            atlas_size: 1024,
            ..Default::default()
        }
    }

    /// Large config for text-heavy applications (2048x2048, ~4 MB per atlas).
    ///
    /// Best for applications with lots of unique glyphs or fonts.
    pub fn large() -> Self {
        Self {
            atlas_size: 2048,
            ..Default::default()
        }
    }

    /// Set custom atlas size.
    ///
    /// # Arguments
    ///
    /// * `size` - Atlas width and height (should be power of 2)
    pub fn with_atlas_size(mut self, size: u32) -> Self {
        self.atlas_size = size;
        self
    }

    /// Set SDF configuration.
    pub fn with_sdf_config(mut self, config: SdfConfig) -> Self {
        self.sdf = config;
        self
    }
}

/// Common trait for text renderers.
///
/// This trait defines the interface shared by all text renderer implementations.
/// Use this trait for generic code that needs to work with any text renderer.
///
/// # Example
///
/// ```ignore
/// fn render_ui<R: TextRender>(renderer: &mut R, render_pass: &mut wgpu::RenderPass) {
///     let text = Text::new("Hello").size(16.0);
///     let mut buffer = renderer.prepare(&text);
///     renderer.draw_text(&mut buffer, Vec2::new(10.0, 10.0));
///     renderer.render(render_pass);
/// }
/// ```
pub trait TextRender {
    /// Prepare text for rendering.
    ///
    /// Returns a `TextBuffer` that can be cached and reused for rendering
    /// the same text multiple times.
    fn prepare(&mut self, text: &Text) -> TextBuffer;

    /// Draw text at a position.
    ///
    /// The position represents the top-left corner of the text's bounding box.
    fn draw_text(&mut self, buffer: &mut TextBuffer, position: Vec2);

    /// Render all queued text to the given render pass.
    fn render(&mut self, render_pass: &mut wgpu::RenderPass);

    /// Measure text dimensions without rendering.
    fn measure_text(&self, text: &Text) -> (f32, f32);

    /// Set the viewport for rendering.
    fn set_viewport(&mut self, viewport: Viewport);

    /// Get the logical (unscaled) bounds of a prepared text buffer.
    fn buffer_bounds(&self, buffer: &TextBuffer) -> (f32, f32);
}

/// Shared context containing resources common to all text renderers.
///
/// This includes the font system, swash cache, viewport, and projection uniform
/// layout. By sharing these resources, multiple renderers can coexist without
/// duplicating font data.
pub struct SharedContext {
    /// The font system containing loaded fonts.
    pub font_system: Arc<RwLock<cosmic_text::FontSystem>>,
    /// Cache for rasterized glyph images.
    pub swash_cache: Arc<RwLock<SwashCache>>,
    /// Current viewport configuration.
    pub viewport: Viewport,
    /// Low-level renderer for creating GPU resources.
    pub renderer: Renderer,
    /// Bind group layout for projection matrix uniform.
    pub uniform_bind_group_layout: wgpu::BindGroupLayout,
}

impl SharedContext {
    /// Create a new shared context.
    ///
    /// # Arguments
    ///
    /// * `context` - Graphics context for GPU resource creation
    /// * `font_system` - Arc-wrapped font system to share between renderers
    pub fn new(
        context: Arc<GraphicsContext>,
        font_system: Arc<RwLock<cosmic_text::FontSystem>>,
    ) -> Self {
        let renderer = Renderer::new(context);
        let swash_cache = Arc::new(RwLock::new(SwashCache::new()));

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

        Self {
            font_system,
            swash_cache,
            viewport: Viewport::default(),
            renderer,
            uniform_bind_group_layout,
        }
    }

    /// Set the viewport.
    pub fn set_viewport(&mut self, viewport: Viewport) {
        self.viewport = viewport;
    }

    /// Get the current scale factor.
    pub fn scale_factor(&self) -> f32 {
        self.viewport.scale_factor.0 as f32
    }
}

/// A cached text buffer with layout information.
///
/// This buffer stores shaped text that can be rendered multiple times.
/// Cache and reuse buffers when rendering the same text repeatedly.
pub struct TextBuffer {
    pub(crate) buffer: Buffer,
    pub(crate) needs_layout: bool,
}

impl TextBuffer {
    /// Create a new text buffer.
    pub fn new(font_system: &mut cosmic_text::FontSystem) -> Self {
        let mut buffer = Buffer::new(font_system, Metrics::new(16.0, 20.0));
        buffer.set_wrap(font_system, cosmic_text::Wrap::Word);
        Self {
            buffer,
            needs_layout: true,
        }
    }

    /// Set the text content and style.
    pub fn set_text(&mut self, font_system: &mut cosmic_text::FontSystem, text: &Text, scale: f32) {
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
        self.buffer.set_size(
            font_system,
            text.get_max_width().map(|w| w * scale),
            text.get_max_height().map(|h| h * scale),
        );

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

    /// Perform text layout if needed.
    pub fn layout(&mut self, font_system: &mut cosmic_text::FontSystem) {
        profile_function!();
        if self.needs_layout {
            self.buffer.shape_until_scroll(font_system, false);
            self.needs_layout = false;
        }
    }

    /// Get the bounds of the laid out text.
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
pub struct TextVertex {
    pub position: [f32; 2],
    pub tex_coords: [f32; 2],
    pub color: [f32; 4],
}

/// Vertex data for decoration rendering (solid colored quads).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DecorationVertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
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
    /// Calculate UV coordinates for this atlas entry.
    pub fn uv_coords(&self, atlas_size: u32) -> (f32, f32, f32, f32) {
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
    /// since SDF glyphs are resolution-independent.
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
#[derive(Debug, Clone)]
pub struct SdfAtlasEntry {
    /// Base atlas entry (position and size in atlas)
    pub entry: AtlasEntry,
    /// The SDF spread used when generating this glyph
    pub spread: f32,
    /// Base font size at which the glyph was rasterized
    pub base_size: f32,
    /// Original glyph metrics at base size
    pub base_placement: GlyphPlacement,
}

/// SDF rendering parameters passed to shaders for text effects.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SdfParams {
    /// Edge softness for anti-aliasing (0.0 to 1.0)
    pub edge_softness: f32,
    /// Outline width in SDF space (0.0 = no outline)
    pub outline_width: f32,
    /// Outline color (RGBA)
    pub outline_color: [f32; 4],
    /// Shadow offset in pixels (x, y)
    pub shadow_offset: [f32; 2],
    /// Shadow blur radius (0.0 = hard shadow)
    pub shadow_blur: f32,
    /// Shadow color (RGBA)
    pub shadow_color: [f32; 4],
    /// Glow radius in pixels (0.0 = no glow)
    pub glow_radius: f32,
    /// Glow color (RGBA)
    pub glow_color: [f32; 4],
    /// Padding for GPU alignment
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
    pub fn from_effects(effects: &TextEffects, config: &SdfConfig) -> Self {
        let mut params = Self {
            edge_softness: config.edge_softness,
            ..Default::default()
        };

        for effect in effects.sorted_by_priority() {
            match &effect.effect_type {
                crate::effects::TextEffectType::Shadow {
                    offset,
                    blur_radius,
                    color,
                } => {
                    params.shadow_offset = [offset.x, offset.y];
                    params.shadow_blur = *blur_radius;
                    params.shadow_color = [color.r, color.g, color.b, color.a];
                }
                crate::effects::TextEffectType::Outline { width, color } => {
                    params.outline_width = *width;
                    params.outline_color = [color.r, color.g, color.b, color.a];
                }
                crate::effects::TextEffectType::Glow {
                    radius,
                    color,
                    intensity: _,
                } => {
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

/// Simple row-based atlas packer.
pub(crate) struct AtlasPacker {
    size: u32,
    current_x: u32,
    current_y: u32,
    row_height: u32,
}

impl AtlasPacker {
    pub fn new(size: u32) -> Self {
        Self {
            size,
            current_x: 0,
            current_y: 0,
            row_height: 0,
        }
    }

    pub fn pack(&mut self, width: u32, height: u32) -> Option<AtlasEntry> {
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

    pub fn reset(&mut self) {
        self.current_x = 0;
        self.current_y = 0;
        self.row_height = 0;
    }
}

/// Shared renderer for text decorations (underlines, strikethrough, backgrounds).
///
/// This struct manages the GPU pipeline and rendering state for decorations.
/// It's designed to be shared by all text renderer backends (bitmap, SDF, hybrid).
///
/// # Usage
///
/// ```ignore
/// // Create during renderer initialization
/// let decoration_renderer = DecorationRenderer::new(&renderer, &uniform_bind_group_layout);
///
/// // Queue decoration quads for rendering
/// decoration_renderer.queue_quad(&quad, scale);
///
/// // Render all queued decorations (backgrounds first)
/// decoration_renderer.render_backgrounds(&mut render_pass, &viewport);
///
/// // ... render text glyphs ...
///
/// // Render lines after text
/// decoration_renderer.render_lines(&mut render_pass, &viewport);
/// ```
pub struct DecorationRenderer {
    /// Render pipeline for decoration quads.
    pipeline: wgpu::RenderPipeline,
    /// Bind group layout for uniforms.
    uniform_bind_group_layout: wgpu::BindGroupLayout,

    /// Vertices for background quads (rendered before text).
    background_vertices: Vec<DecorationVertex>,
    /// Indices for background quads.
    background_indices: Vec<u16>,

    /// Vertices for line quads (underline, strikethrough - rendered after text).
    line_vertices: Vec<DecorationVertex>,
    /// Indices for line quads.
    line_indices: Vec<u16>,
}

impl DecorationRenderer {
    /// Create a new decoration renderer.
    pub fn new(renderer: &Renderer, _uniform_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        // Create shader
        let shader = renderer.create_shader(
            Some("Decoration Shader"),
            include_str!("../../shaders/decoration.wgsl"),
        );

        // Create bind group layout for uniforms (projection matrix)
        let decoration_uniform_layout = renderer.create_bind_group_layout(
            Some("Decoration Uniform Layout"),
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

        // Create pipeline layout
        let pipeline_layout = renderer.create_pipeline_layout(
            Some("Decoration Pipeline Layout"),
            &[&decoration_uniform_layout],
            &[],
        );

        // Create pipeline
        let pipeline = renderer.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Decoration Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<DecorationVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2,  // position
                        1 => Float32x4,  // color
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

        Self {
            pipeline,
            uniform_bind_group_layout: decoration_uniform_layout,
            background_vertices: Vec::new(),
            background_indices: Vec::new(),
            line_vertices: Vec::new(),
            line_indices: Vec::new(),
        }
    }

    /// Queue a decoration quad for rendering.
    ///
    /// Background quads are rendered before text (so text appears on top).
    /// Line quads (underline, strikethrough) are rendered after text.
    pub fn queue_quad(&mut self, quad: &DecorationQuad, _scale: f32) {
        let (x, y, width, height) = quad.bounds;
        let color = [quad.color.r, quad.color.g, quad.color.b, quad.color.a];

        // Determine which buffer to use
        let (vertices, indices) = if quad.is_background() {
            (&mut self.background_vertices, &mut self.background_indices)
        } else {
            (&mut self.line_vertices, &mut self.line_indices)
        };

        // Create quad vertices
        let idx = vertices.len() as u16;

        vertices.push(DecorationVertex {
            position: [x, y],
            color,
        });
        vertices.push(DecorationVertex {
            position: [x + width, y],
            color,
        });
        vertices.push(DecorationVertex {
            position: [x + width, y + height],
            color,
        });
        vertices.push(DecorationVertex {
            position: [x, y + height],
            color,
        });

        indices.extend_from_slice(&[idx, idx + 1, idx + 2, idx, idx + 2, idx + 3]);
    }

    /// Queue all decoration quads from a list.
    pub fn queue_quads(&mut self, quads: &[DecorationQuad], scale: f32) {
        for quad in quads {
            self.queue_quad(quad, scale);
        }
    }

    /// Generate and queue decoration quads from text bounds and decoration config.
    pub fn queue_from_text(
        &mut self,
        bounds: &TextBounds,
        decoration: &TextDecoration,
        scale: f32,
    ) {
        let quads = generate_decoration_quads(bounds, decoration);
        self.queue_quads(&quads, scale);
    }

    /// Render background decorations (should be called before rendering text).
    pub fn render_backgrounds(&mut self, render_pass: &mut wgpu::RenderPass, renderer: &Renderer, viewport: &Viewport) {
        profile_function!();

        if self.background_vertices.is_empty() {
            return;
        }

        self.render_vertices(
            render_pass,
            renderer,
            viewport,
            &self.background_vertices,
            &self.background_indices,
        );

        self.background_vertices.clear();
        self.background_indices.clear();
    }

    /// Render line decorations (underline, strikethrough - should be called after rendering text).
    pub fn render_lines(&mut self, render_pass: &mut wgpu::RenderPass, renderer: &Renderer, viewport: &Viewport) {
        profile_function!();

        if self.line_vertices.is_empty() {
            return;
        }

        self.render_vertices(
            render_pass,
            renderer,
            viewport,
            &self.line_vertices,
            &self.line_indices,
        );

        self.line_vertices.clear();
        self.line_indices.clear();
    }

    /// Internal method to render a set of vertices.
    fn render_vertices(
        &self,
        render_pass: &mut wgpu::RenderPass,
        renderer: &Renderer,
        viewport: &Viewport,
        vertices: &[DecorationVertex],
        indices: &[u16],
    ) {
        if vertices.is_empty() {
            return;
        }

        // Create buffers
        let vertex_buffer = renderer.create_vertex_buffer(Some("Decoration Vertex Buffer"), vertices);
        let index_buffer = renderer.create_index_buffer(Some("Decoration Index Buffer"), indices);

        // Create projection uniform
        let size = viewport.to_logical();
        let projection = orthographic_projection(size.width, size.height);
        let uniform_buffer = renderer.create_uniform_buffer(Some("Decoration Projection"), &projection);

        // Create uniform bind group
        let uniform_bind_group = renderer.create_bind_group(
            Some("Decoration Uniform Bind Group"),
            &self.uniform_bind_group_layout,
            &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        );

        // Render
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &uniform_bind_group, &[]);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
    }

    /// Check if there are any queued decorations.
    pub fn has_queued(&self) -> bool {
        !self.background_vertices.is_empty() || !self.line_vertices.is_empty()
    }

    /// Clear all queued decorations without rendering.
    pub fn clear(&mut self) {
        self.background_vertices.clear();
        self.background_indices.clear();
        self.line_vertices.clear();
        self.line_indices.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use astrelis_render::Color;

    #[test]
    fn test_sdf_cache_key_basic() {
        let key1 = SdfCacheKey {
            glyph_id: 100,
            font_id: 12345,
        };
        let key2 = SdfCacheKey {
            glyph_id: 100,
            font_id: 12345,
        };
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_sdf_cache_key_different_glyphs() {
        let key1 = SdfCacheKey {
            glyph_id: 100,
            font_id: 12345,
        };
        let key2 = SdfCacheKey {
            glyph_id: 200,
            font_id: 12345,
        };
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_sdf_cache_key_hash() {
        use std::collections::HashMap;

        let mut map = HashMap::new();
        let key = SdfCacheKey {
            glyph_id: 65,
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
        assert_eq!(params.shadow_offset, [0.0, 0.0]);
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
    }

    #[test]
    fn test_renderer_config_presets() {
        let small = TextRendererConfig::small();
        assert_eq!(small.atlas_size, 512);

        let medium = TextRendererConfig::medium();
        assert_eq!(medium.atlas_size, 1024);

        let large = TextRendererConfig::large();
        assert_eq!(large.atlas_size, 2048);
    }

    #[test]
    fn test_atlas_packer() {
        let mut packer = AtlasPacker::new(100);

        // First glyph: starts at (0, 0)
        let entry1 = packer.pack(30, 20).unwrap();
        assert_eq!(entry1.x, 0);
        assert_eq!(entry1.y, 0);

        // Second glyph: same row at x=30
        let entry2 = packer.pack(30, 20).unwrap();
        assert_eq!(entry2.x, 30);
        assert_eq!(entry2.y, 0);

        // Third glyph: 50 width doesn't fit (60 + 50 > 100), moves to next row
        let entry3 = packer.pack(50, 25).unwrap();
        assert_eq!(entry3.x, 0);
        assert_eq!(entry3.y, 20); // Previous row height was 20

        // Fourth glyph: fits on same row as entry3
        let entry4 = packer.pack(40, 30).unwrap();
        assert_eq!(entry4.x, 50);
        assert_eq!(entry4.y, 20);
    }

    #[test]
    fn test_atlas_entry_uv_coords() {
        let entry = AtlasEntry {
            x: 100,
            y: 50,
            width: 20,
            height: 30,
        };
        let (u0, v0, u1, v1) = entry.uv_coords(1000);
        assert_eq!(u0, 0.1);
        assert_eq!(v0, 0.05);
        assert_eq!(u1, 0.12);
        assert_eq!(v1, 0.08);
    }
}
