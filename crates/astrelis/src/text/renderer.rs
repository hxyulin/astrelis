use super::{
    FontId, TextFont, TextLayout,
    geometry::{Geometry, GeometryPool},
};
use crate::{
    Error, GraphicsContext, Material, MaterialOptions, Rect, RenderFormat, RenderPass, Transform2D,
    VertexLayout,
};
use bytemuck::{Pod, Zeroable};
use std::{
    collections::HashMap,
    ops::Range,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use swash::scale::{
    Render, ScaleContext, Source, StrikeWith,
    image::{Content, Image},
};

/// GPU text resource preparation failure. Drawing uses [`crate::Error`].
#[derive(Debug)]
pub enum TextRenderError {
    /// A device, attachment format, or pipeline error from the rendering layer.
    Graphics(Error),
    /// Invalid atlas limits, raster density/size, or prepared geometry.
    InvalidOptions,
    /// A padded glyph image exceeds the configured atlas page size.
    GlyphTooLarge,
    /// Pages remain leased by prepared texts, recordings, or pending GPU work.
    /// Drop unused prepared texts, clear caches, and poll for completion before retrying.
    AtlasFull,
    /// The backend could not parse a retained font or returned inconsistent image data.
    InvalidRaster,
}
impl From<Error> for TextRenderError {
    fn from(value: Error) -> Self {
        Self::Graphics(value)
    }
}
impl std::fmt::Display for TextRenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Graphics(e) => e.fmt(f),
            Self::InvalidOptions => {
                f.write_str("invalid text renderer, raster density, or prepared geometry")
            }
            Self::GlyphTooLarge => f.write_str("glyph image exceeds atlas page size"),
            Self::AtlasFull => f.write_str("text atlas budget is occupied by leased pages"),
            Self::InvalidRaster => f.write_str("invalid retained font or raster image"),
        }
    }
}
impl std::error::Error for TextRenderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Graphics(e) => Some(e),
            _ => None,
        }
    }
}
/// Bounded glyph atlas/cache configuration, selected at renderer creation.
#[derive(Clone, Debug)]
pub struct TextRendererOptions {
    /// Immutable blend/color-write/depth/stencil settings for coverage, color and MTSDF.
    pub pipeline: crate::PipelineOptions,
    /// Square page dimension, default 1024. Each glyph has a transparent one-texel gutter.
    pub page_size: u32,
    /// Maximum live pages, including prepared/recording/completion leases; default 8.
    /// R8 coverage pages cost size² bytes; linear RGBA8 color/field pages cost size²×4.
    pub max_pages: usize,
    /// Maximum cached glyph keys, including blank results; default 16,384.
    pub max_cached_glyphs: usize,
    /// Maximum physical font size passed to the rasterizer, default 512 pixels.
    pub max_raster_size: f32,
}
impl Default for TextRendererOptions {
    fn default() -> Self {
        Self {
            pipeline: crate::PipelineOptions::default(),
            page_size: 1024,
            max_pages: 8,
            max_cached_glyphs: 16_384,
            max_raster_size: 512.,
        }
    }
}
/// Explicit raster scale and hinting, independent of shaping and draw placement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextRasterOptions {
    /// Positive finite raster texels per layout unit, default 1.
    /// Controls image density only; geometry and measurements retain layout units.
    pub raster_scale: f32,
    /// Rasterizer hinting at the chosen physical size, enabled by default.
    pub hinting: bool,
}
impl Default for TextRasterOptions {
    fn default() -> Self {
        Self::new()
    }
}
impl TextRasterOptions {
    /// Creates 1:1 coverage/color preparation with hinting.
    pub const fn new() -> Self {
        Self {
            raster_scale: 1.,
            hinting: true,
        }
    }
    /// Selects image density without scaling geometry. Match the intended draw scale
    /// for sharp hinted coverage; a Painter/session transform applies geometry scaling.
    pub const fn raster_scale(mut self, value: f32) -> Self {
        self.raster_scale = value;
        self
    }
    /// Enables or disables hinting.
    pub const fn hinting(mut self, value: bool) -> Self {
        self.hinting = value;
        self
    }
}
/// Outline distance-field preparation, independent of font size and drawing transforms.
///
/// Generates an unhinted MTSDF: RGB encodes sharp-corner signed distance and alpha
/// encodes true signed distance. Fill rendering uses RGB; effects are not implemented.
/// Color/bitmap glyphs retain their intrinsic artwork, rasterized at the physical
/// font size with hinting. Keep coverage preparation for small text that needs hinting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MtsdfOptions {
    /// Generation density in texels per EM, default 64. Valid range is `16..=256`.
    pub pixels_per_em: u32,
    /// Full signed distance range in EM, default 0.25 (distances -0.125 through +0.125).
    /// Must be finite, positive, at most 1 EM, and span at least two generation texels.
    /// The image includes half this range outside the outline plus a filtering guard.
    pub range_em: f32,
    /// Positive finite raster texels per layout unit for color/bitmap fallback, default 1.
    /// Outline density comes from pixels_per_em; geometry always retains layout units.
    pub raster_scale: f32,
}
impl Default for MtsdfOptions {
    fn default() -> Self {
        Self::new()
    }
}
impl MtsdfOptions {
    /// Creates 64-texel/EM fields with a 0.25-EM full distance range and 1:1 geometry.
    pub const fn new() -> Self {
        Self {
            pixels_per_em: 64,
            range_em: 0.25,
            raster_scale: 1.,
        }
    }
    /// Selects generation density. Higher values cost more preparation time and atlas space.
    pub const fn pixels_per_em(mut self, value: u32) -> Self {
        self.pixels_per_em = value;
        self
    }
    /// Selects the full signed range, split symmetrically around the outline boundary.
    pub const fn range_em(mut self, value: f32) -> Self {
        self.range_em = value;
        self
    }
    /// Selects fallback image density without changing geometry or outline cache keys.
    pub const fn raster_scale(mut self, value: f32) -> Self {
        self.raster_scale = value;
        self
    }
}
/// Explicit glyph representation selected before recording draws.
///
/// `TextRasterOptions` and `MtsdfOptions` convert into this enum, so callers can
/// pass either directly to `TextRenderer::prepare_text` or `Painter::prepare_text`.
/// Default preparation is hinted coverage; no automatic representation switching occurs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextPreparation {
    /// Size-dependent coverage images and intrinsic color artwork.
    Coverage(TextRasterOptions),
    /// Size-independent outline fields, with size-dependent color/bitmap fallback.
    Mtsdf(MtsdfOptions),
}
impl Default for TextPreparation {
    fn default() -> Self {
        TextRasterOptions::new().into()
    }
}
impl From<TextRasterOptions> for TextPreparation {
    fn from(value: TextRasterOptions) -> Self {
        Self::Coverage(value)
    }
}
impl From<MtsdfOptions> for TextPreparation {
    fn from(value: MtsdfOptions) -> Self {
        Self::Mtsdf(value)
    }
}
impl TextPreparation {
    /// Raster texels per layout unit for coverage/color images; geometry is unchanged.
    pub const fn raster_scale(self) -> f32 {
        match self {
            Self::Coverage(v) => v.raster_scale,
            Self::Mtsdf(v) => v.raster_scale,
        }
    }
}
/// Placement and mask color for text geometry in its original layout units.
///
/// Identity draws interpret these units as viewport-relative pixels. An explicit
/// draw or Painter transform can convert application logical units to pixels.
/// Raster density never applies another geometry scale.
#[derive(Clone, Copy, Debug)]
pub struct TextDraw {
    /// Origin offset before the affine transform.
    pub origin: [f32; 2],
    /// Linear straight RGBA for coverage glyphs; alpha also affects intrinsic color glyphs.
    pub color: [f32; 4],
    /// Additional opacity in `0..=1`, applied to monochrome and intrinsic color glyphs.
    pub opacity: f32,
    /// Transform of layout-unit geometry and origin; X right/Y down.
    pub transform: Transform2D,
}
impl Default for TextDraw {
    fn default() -> Self {
        Self::new([0., 0.])
    }
}
impl TextDraw {
    /// Creates white text at the given layout-unit origin with opacity one.
    pub const fn new(origin: [f32; 2]) -> Self {
        Self {
            origin,
            color: [1.; 4],
            opacity: 1.,
            transform: Transform2D::IDENTITY,
        }
    }
    /// Selects linear straight RGBA; RGB affects masks, alpha affects all glyphs.
    pub const fn color(mut self, value: [f32; 4]) -> Self {
        self.color = value;
        self
    }
    /// Selects additional opacity.
    pub const fn opacity(mut self, value: f32) -> Self {
        self.opacity = value;
        self
    }
    /// Selects an affine geometry transform. Coverage images filter; fields reconstruct fill.
    pub fn transform(mut self, value: impl Into<Transform2D>) -> Self {
        self.transform = value.into();
        self
    }
}
/// Cumulative renderer work counters and current atlas/cache occupancy.
#[derive(Clone, Copy, Debug, Default)]
pub struct TextRendererStats {
    /// Glyph preparation cache hits, including blank entries.
    pub cache_hits: u64,
    /// Glyph preparation cache misses.
    pub cache_misses: u64,
    /// Atlas upload payload bytes, excluding implicit texture initialization.
    pub uploaded_bytes: u64,
    /// Glyph geometry payload bytes uploaded during preparation, excluding allocation padding.
    pub geometry_bytes: u64,
    /// New glyph geometry buffer allocations. Empty preparations allocate nothing.
    pub geometry_buffer_allocations: u64,
    /// Geometry uploads that reused a buffer released by its last text/recording/completion owner.
    pub geometry_buffer_reuses: u64,
    /// Recording-owned placement/color payload bytes, 48 per nonempty text draw.
    pub parameter_bytes: u64,
    /// GPU draws recorded, one per consecutive atlas-page batch.
    pub draw_calls: u64,
    /// Live atlas page allocations, including recording and GPU completion leases.
    pub live_pages: usize,
    /// Logical atlas texture bytes for those live allocations, excluding backend overhead.
    pub atlas_bytes: usize,
    /// Current bounded cache key count.
    pub cached_glyphs: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Mask,
    Color,
    Mtsdf,
}
impl Kind {
    fn bytes(self) -> usize {
        if self == Self::Mask { 1 } else { 4 }
    }
}
#[derive(Debug)]
struct Budget {
    pages: AtomicUsize,
    bytes: AtomicUsize,
}
#[derive(Debug)]
struct Page {
    id: u64,
    kind: Kind,
    texture: wgpu::Texture,
    group: wgpu::BindGroup,
    allocation: Arc<Allocation>,
}
#[derive(Debug)]
struct Allocation {
    bytes: usize,
    budget: Arc<Budget>,
}
impl Drop for Allocation {
    fn drop(&mut self) {
        self.budget.pages.fetch_sub(1, Ordering::Relaxed);
        self.budget.bytes.fetch_sub(self.bytes, Ordering::Relaxed);
    }
}
#[derive(Debug)]
struct Shelf {
    page: Arc<Page>,
    x: u32,
    y: u32,
    row: u32,
    last: u64,
}
impl Shelf {
    fn allocate(&mut self, w: u32, h: u32, size: u32) -> Option<[u32; 2]> {
        let (mut x, mut y, mut row) = (self.x, self.y, self.row);
        if x + w > size {
            x = 0;
            y += row;
            row = 0;
        }
        if y + h > size {
            return None;
        }
        self.x = x + w;
        self.y = y;
        self.row = row.max(h);
        Some([x + 1, y + 1])
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Key {
    font: FontId,
    glyph: u16,
    weight: u16,
    representation: Representation,
    italic: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Representation {
    Coverage { size: u32, hint: bool },
    Mtsdf { pixels_per_em: u32, range_em: u32 },
}
#[derive(Clone, Debug)]
enum GlyphSource {
    Image(GlyphImage),
    Blank,
    // Cached outline-source classification; image lookup uses a size-dependent coverage key.
    RasterFallback,
}
#[derive(Clone, Debug)]
struct GlyphImage {
    page: Arc<Page>,
    uv: [f32; 4],
    left: i32,
    top: i32,
    width: u32,
    height: u32,
}
#[derive(Debug)]
struct Cached {
    source: GlyphSource,
    last: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct GlyphData {
    rect: [f32; 4],
    uv: [f32; 4],
    kind: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct DrawData {
    origin_axis_x: [f32; 4],
    axis_y: [f32; 4],
    color: [f32; 4],
}
#[derive(Debug)]
struct TextDraft {
    glyphs: Vec<GlyphData>,
    batches: Vec<Batch>,
    size: [f32; 2],
    bounds: Option<Rect>,
    skipped: Vec<usize>,
    preparation: TextPreparation,
}

#[derive(Debug)]
struct Batch {
    page: Arc<Page>,
    range: Range<u32>,
}
#[derive(Debug)]
enum GeometryBuffer {
    Dedicated(Geometry),
    Shared(Arc<Geometry>),
}
impl std::ops::Deref for GeometryBuffer {
    type Target = Geometry;
    fn deref(&self) -> &Geometry {
        match self {
            Self::Dedicated(geometry) => geometry,
            Self::Shared(geometry) => geometry,
        }
    }
}

#[derive(Debug)]
struct PreparedData {
    buffer: Option<GeometryBuffer>,
    buffer_range: Range<u64>,
    batches: Vec<Batch>,
    raster_pipeline: bool,
    field_pipeline: bool,
    lease: Option<Arc<TextLease>>,
}
// GPU completion callbacks require Send even on WebGPU, whose buffer/texture
// handles are not Send. Retain only ownership tokens in the callback; wgpu owns
// the actual resources referenced by recorded commands.
#[derive(Debug)]
struct TextLease {
    _geometry: Option<Arc<()>>,
    _pages: Vec<Arc<Allocation>>,
}
/// Immutable glyph geometry and atlas leases, independent of later preparation/cache clearing.
/// Clones share GPU storage. Draws upload only a 48-byte placement/color record, not glyph geometry.
#[derive(Clone, Debug)]
pub struct PreparedText {
    graphics: GraphicsContext,
    data: Arc<PreparedData>,
    size: [f32; 2],
    bounds: Option<Rect>,
    glyph_count: usize,
    skipped: Arc<[usize]>,
    preparation: TextPreparation,
}
impl PreparedText {
    /// Advance/line-box measurement in the original layout units, independent of raster density.
    pub fn size(&self) -> [f32; 2] {
        self.size
    }
    /// Image-quad bounds in layout units, distinct from advance/line-box measurement.
    /// Coverage rounding/hinting can change these bounds with raster density.
    /// Distance-field quads include distance-range padding and filtering guards.
    pub fn ink_bounds(&self) -> Option<Rect> {
        self.bounds
    }
    /// Number of drawable glyph quads.
    pub fn glyph_count(&self) -> usize {
        self.glyph_count
    }
    /// Original layout glyph indexes with no raster image, including blank spaces and unsupported sources.
    pub fn skipped_glyphs(&self) -> &[usize] {
        &self.skipped
    }
    /// Representation and geometry scale selected during preparation. Color glyphs may
    /// use raster fallback even when this returns `TextPreparation::Mtsdf`.
    pub fn preparation(&self) -> TextPreparation {
        self.preparation
    }
    /// Raster image density selected during preparation, independent of geometry scale.
    pub fn raster_scale(&self) -> f32 {
        self.preparation.raster_scale()
    }
}
/// Independent coverage/color and outline-distance-field renderer. Preparation generates and uploads before drawing.
///
/// Uses R8 coverage, linear premultiplied RGBA8 color, and linear RGBA8 MTSDF pages.
/// One-texel gutters separate images; fields include additional distance-range padding.
/// Coverage/color draws use a separate fragment shader without field reconstruction.
/// Raster phase is always zero; fractional movement filters existing images without rerasterizing.
/// Only consecutive glyphs on the same page are instanced together, preserving layout order.
/// Leased pages are never evicted or overwritten. Budget exhaustion returns `AtlasFull` without
/// waiting; callers control dropping resources and device polling. Font-size/raster limits bound
/// image work, while Swash keeps a fixed eight-entry scaler cache and no CPU glyph-image cache.
/// Changed text can reuse released geometry buffers. Recycling is bounded to 64
/// buffers / 1 MiB (each at most 64 KiB), including buffers awaiting completion.
/// Prepared texts, clones, recordings and GPU work prevent their storage being overwritten.
/// Larger geometry remains exact-sized and is released rather than recycled.
pub struct TextRenderer {
    graphics: GraphicsContext,
    options: TextRendererOptions,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    material: Material,
    field_material: Material,
    pipelines: crate::mesh_renderer::Pipelines,
    scale: ScaleContext,
    fonts: HashMap<FontId, swash::CacheKey>,
    cache: HashMap<Key, Cached>,
    pages: Vec<Shelf>,
    budget: Arc<Budget>,
    clock: u64,
    next_page: u64,
    stats: TextRendererStats,
    geometry_pool: Arc<Mutex<GeometryPool>>,
}
impl std::fmt::Debug for TextRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextRenderer")
            .field("options", &self.options)
            .field("stats", &self.stats())
            .finish_non_exhaustive()
    }
}
impl TextRenderer {
    /// Creates a renderer with default bounded atlas settings.
    pub fn new(graphics: &GraphicsContext) -> Self {
        Self::with_options(graphics, TextRendererOptions::default())
            .expect("default text options fit supported wgpu limits")
    }
    /// Validates atlas/raster limits and creates device resources, without allocating atlas pages.
    pub fn with_options(
        g: &GraphicsContext,
        options: TextRendererOptions,
    ) -> Result<Self, TextRenderError> {
        if options.page_size < 4
            || options.page_size > g.device().limits().max_texture_dimension_2d
            || options.max_pages == 0
            || options.max_cached_glyphs == 0
            || !options.max_raster_size.is_finite()
            || options.max_raster_size <= 0.
            || options.max_raster_size > 4096.
        {
            return Err(TextRenderError::InvalidOptions);
        }
        let layout = g
            .device()
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Astrelis text atlas"),
                entries: &[
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
            });
        let sampler = g.device().create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Astrelis text filtering"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shader = g
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Astrelis text"),
                source: wgpu::ShaderSource::Wgsl(include_str!("text.wgsl").into()),
            });
        let layouts = [
            VertexLayout {
                stride: 48,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: wgpu::vertex_attr_array![0=>Float32x4,1=>Float32x4,2=>Float32x4]
                    .to_vec(),
            },
            VertexLayout {
                stride: 0,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: wgpu::vertex_attr_array![3=>Float32x4,4=>Float32x4,5=>Float32x4]
                    .to_vec(),
            },
        ];
        let material = g.create_material(
            options.pipeline.mesh(
                MaterialOptions::new(&shader)
                    .vertex_layouts(&layouts)
                    .bind_group_layouts(&[Some(&layout)])
                    .entry_points(
                        "vertex_main",
                        if options.pipeline.writes_attachment() {
                            "fragment_covered"
                        } else {
                            "fragment_main"
                        },
                    ),
            ),
        );
        let field_material = g.create_material(
            options.pipeline.mesh(
                MaterialOptions::new(&shader)
                    .vertex_layouts(&layouts)
                    .bind_group_layouts(&[Some(&layout)])
                    .entry_points(
                        "vertex_main",
                        if options.pipeline.writes_attachment() {
                            "fragment_mtsdf_covered"
                        } else {
                            "fragment_mtsdf"
                        },
                    ),
            ),
        );
        Ok(Self {
            graphics: g.clone(),
            options,
            layout,
            sampler,
            material,
            field_material,
            pipelines: Default::default(),
            scale: ScaleContext::new(),
            fonts: HashMap::new(),
            cache: HashMap::new(),
            pages: Vec::new(),
            geometry_pool: Arc::default(),
            budget: Arc::new(Budget {
                pages: AtomicUsize::new(0),
                bytes: AtomicUsize::new(0),
            }),
            clock: 0,
            next_page: 0,
            stats: Default::default(),
        })
    }
    /// Returns cumulative work and current live allocation counters.
    pub fn stats(&self) -> TextRendererStats {
        TextRendererStats {
            live_pages: self.budget.pages.load(Ordering::Relaxed),
            atlas_bytes: self.budget.bytes.load(Ordering::Relaxed),
            cached_glyphs: self.cache.len(),
            ..self.stats
        }
    }
    /// Releases lookup caches, page ownership, and recycled geometry storage.
    /// Existing prepared texts/recordings remain usable.
    /// Live leased pages continue counting against this renderer's budget until dropped/completed.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
        self.pages.clear();
        self.fonts.clear();
        self.scale = ScaleContext::new();
        self.geometry_pool = Arc::default();
    }
    fn pipeline(
        &mut self,
        color: wgpu::TextureFormat,
        sample_count: u32,
        depth_stencil: Option<wgpu::TextureFormat>,
        field: bool,
    ) -> Result<wgpu::RenderPipeline, Error> {
        Ok(crate::mesh_renderer::pipeline(
            &self.graphics,
            &mut self.pipelines,
            if field {
                &self.field_material
            } else {
                &self.material
            },
            color,
            sample_count,
            depth_stencil,
            true,
        )
        .map_err(|error| match error {
            Error::UnsupportedMeshFormat { format } => Error::UnsupportedTextFormat { format },
            other => other,
        })?
        .clone())
    }
    /// Prepares coverage and MTSDF pipelines without shaping or uploading glyphs.
    pub fn prepare(&mut self, format: &RenderFormat) -> Result<(), Error> {
        let color = format.single_color()?;
        self.pipeline(color, format.sample_count, format.depth_stencil, false)?;
        self.pipeline(color, format.sample_count, format.depth_stencil, true)?;
        Ok(())
    }
    /// Prepares a surface target's attachment variants, checking device identity.
    pub fn prepare_for_target(&mut self, target: &crate::RenderTarget<'_>) -> Result<(), Error> {
        if !self.graphics.same_device(target.graphics()) {
            return Err(Error::DeviceMismatch);
        }
        self.prepare(&target.render_format())
    }
    /// Generates missing glyph images and creates immutable geometry. Coverage is size-dependent;
    /// MTSDF outlines reuse images across font sizes/DPI while color/bitmap fallback remains
    /// size-dependent. Pass either `TextRasterOptions` or `MtsdfOptions` directly.
    /// Cold field generation can be expensive: prepare ahead of drawing or stage groups of labels.
    /// No TextSystem borrow is
    /// needed: layouts retain font sources. Preparation can populate/evict caches even on failure;
    /// previous prepared texts remain valid. Repeated preparation reuses atlas images but creates
    /// a new geometry payload, reusing released buffers when available; keep PreparedText
    /// for unchanged content. Unsupported/blank
    /// sources are skipped and reported by original glyph index. Physical coverage/color sizes
    /// must fit `max_raster_size`; pure outline fields instead use bounded generation density.
    pub fn prepare_text(
        &mut self,
        layout: &TextLayout,
        preparation: impl Into<TextPreparation>,
    ) -> Result<PreparedText, TextRenderError> {
        let draft = self.prepare_geometry(layout, preparation.into())?;
        let bytes = draft.glyphs.len() as u64 * 48;
        let geometry = (!draft.glyphs.is_empty())
            .then(|| GeometryBuffer::Dedicated(self.upload_geometry(&draft.glyphs)));
        Ok(self.finish_text(draft, geometry, 0..bytes, false))
    }
    /// Prepares layouts in input order, sharing geometry uploads for consecutive small texts.
    ///
    /// Accepts owned/borrowed layouts and `Arc<TextLayout>` collections. Every result
    /// remains independently drawable, with its own measurement, skipped glyphs and
    /// atlas-page ownership. Coverage/MTSDF settings and scale apply to all inputs.
    /// No shaping, command recording, submission, or completion wait occurs here.
    ///
    /// Small texts share buffers containing at most 64 KiB of glyph payload; a text
    /// is never split between buffers. Larger texts use a dedicated exact-sized buffer.
    /// Retaining one result or recording keeps its shared geometry buffer alive, but
    /// does not keep other results' atlas pages leased. Empty/blank layouts produce
    /// corresponding empty results without geometry uploads or storage leases.
    ///
    /// An error returns no partial result vector. Earlier work may populate caches
    /// and upload geometry, as with repeated [`Self::prepare_text`] calls; existing
    /// resources stay valid. Retain the results until layout/settings change.
    ///
    /// ```no_run
    /// # use astrelis::{TextRenderer, TextLayout, TextRasterOptions, TextRenderError};
    /// # use std::sync::Arc;
    /// fn prepare(renderer: &mut TextRenderer, layouts: &[Arc<TextLayout>], dpi: f32)
    ///     -> Result<(), TextRenderError> {
    ///     let texts = renderer.prepare_texts(layouts,
    ///         TextRasterOptions::new().raster_scale(dpi))?;
    ///     assert_eq!(texts.len(), layouts.len());
    ///     // Store these resources; each may be drawn with its own placement/style.
    ///     Ok(())
    /// }
    /// ```
    pub fn prepare_texts<L: AsRef<TextLayout>>(
        &mut self,
        layouts: impl IntoIterator<Item = L>,
        preparation: impl Into<TextPreparation>,
    ) -> Result<Vec<PreparedText>, TextRenderError> {
        let preparation = preparation.into();
        // Validate settings even for an empty collection.
        validate_preparation(preparation)?;
        let mut result = Vec::new();
        let mut pending = Vec::new();
        let mut glyphs = Vec::new();
        let chunk_bytes = (64 * 1024).min(self.graphics.device().limits().max_buffer_size);
        for layout in layouts {
            let mut draft = self.prepare_geometry(layout.as_ref(), preparation)?;
            let bytes = draft.glyphs.len() as u64 * 48;
            if bytes > chunk_bytes {
                self.flush_texts(&mut result, &mut pending, &mut glyphs);
                let geometry = self.upload_geometry(&draft.glyphs);
                result.push(self.finish_text(
                    draft,
                    Some(GeometryBuffer::Dedicated(geometry)),
                    0..bytes,
                    true,
                ));
                continue;
            }
            if (glyphs.len() as u64 * 48) + bytes > chunk_bytes {
                self.flush_texts(&mut result, &mut pending, &mut glyphs);
            }
            let start = glyphs.len() as u64 * 48;
            glyphs.extend(std::mem::take(&mut draft.glyphs));
            pending.push((draft, start..start + bytes));
        }
        self.flush_texts(&mut result, &mut pending, &mut glyphs);
        Ok(result)
    }
    fn flush_texts(
        &mut self,
        result: &mut Vec<PreparedText>,
        pending: &mut Vec<(TextDraft, Range<u64>)>,
        glyphs: &mut Vec<GlyphData>,
    ) {
        let geometry = (!glyphs.is_empty()).then(|| Arc::new(self.upload_geometry(glyphs)));
        for (draft, range) in pending.drain(..) {
            let buffer = (!range.is_empty())
                .then(|| GeometryBuffer::Shared(geometry.as_ref().unwrap().clone()));
            result.push(self.finish_text(draft, buffer, range, true));
        }
        glyphs.clear();
    }
    fn prepare_geometry(
        &mut self,
        layout: &TextLayout,
        preparation: TextPreparation,
    ) -> Result<TextDraft, TextRenderError> {
        validate_preparation(preparation)?;
        let scale = preparation.raster_scale();
        if !scale.is_finite()
            || scale <= 0.
            || layout.glyphs().iter().any(|g| {
                let size = g.font_size * scale;
                !size.is_finite()
                    || size <= 0.
                    || (matches!(preparation, TextPreparation::Coverage(_))
                        && size > self.options.max_raster_size)
            })
        {
            return Err(TextRenderError::InvalidOptions);
        }
        let inverse_scale = 1. / scale;
        let size = layout.size();
        if size.iter().any(|v| !v.is_finite())
            || (layout.glyphs().len() as u64) * 48 > self.graphics.device().limits().max_buffer_size
            || layout.glyphs().len() > u32::MAX as usize
        {
            return Err(TextRenderError::InvalidOptions);
        }
        let mut data = Vec::with_capacity(layout.glyphs().len());
        let mut batches: Vec<Batch> = Vec::new();
        let mut skipped = Vec::new();
        let mut bounds: Option<[f32; 4]> = None;
        for (index, glyph) in layout.glyphs().iter().enumerate() {
            let font = &layout.fonts()[glyph.font_index];
            let physical_size = glyph.font_size * scale;
            let coverage = match preparation {
                TextPreparation::Coverage(raster) => Representation::Coverage {
                    size: physical_size.to_bits(),
                    hint: raster.hinting,
                },
                TextPreparation::Mtsdf(_) => Representation::Coverage {
                    size: physical_size.to_bits(),
                    hint: true,
                },
            };
            let key = Key {
                font: font.id(),
                glyph: glyph.glyph_id,
                weight: font.weight(),
                representation: match preparation {
                    TextPreparation::Coverage(_) => coverage,
                    TextPreparation::Mtsdf(v) => Representation::Mtsdf {
                        pixels_per_em: v.pixels_per_em,
                        range_em: v.range_em.to_bits(),
                    },
                },
                italic: glyph.synthetic_italic,
            };
            let source = self.image(key, font)?;
            let source = if matches!(source, GlyphSource::RasterFallback) {
                if physical_size > self.options.max_raster_size {
                    return Err(TextRenderError::InvalidOptions);
                }
                self.image(
                    Key {
                        representation: coverage,
                        ..key
                    },
                    font,
                )?
            } else {
                source
            };
            let GlyphSource::Image(image) = source else {
                skipped.push(index);
                continue;
            };
            let factor = if let Representation::Mtsdf { pixels_per_em, .. } = key.representation
                && image.page.kind == Kind::Mtsdf
            {
                glyph.font_size / pixels_per_em as f32
            } else {
                inverse_scale
            };
            let x = glyph.position[0] + image.left as f32 * factor;
            let y = glyph.position[1] - image.top as f32 * factor;
            let w = image.width as f32 * factor;
            let h = image.height as f32 * factor;
            if [x, y, x + w, y + h].iter().any(|v| !v.is_finite()) {
                return Err(TextRenderError::InvalidOptions);
            }
            bounds = Some(match bounds {
                None => [x, y, x + w, y + h],
                Some(b) => [b[0].min(x), b[1].min(y), b[2].max(x + w), b[3].max(y + h)],
            });
            let start = data.len() as u32;
            data.push(GlyphData {
                rect: [x, y, w, h],
                uv: image.uv,
                kind: [
                    match image.page.kind {
                        Kind::Mask => 0.,
                        Kind::Color => 1.,
                        Kind::Mtsdf => 2.,
                    },
                    0.,
                    0.,
                    0.,
                ],
            });
            if let Some(batch) = batches
                .last_mut()
                .filter(|b| Arc::ptr_eq(&b.page, &image.page))
            {
                batch.range.end += 1;
            } else {
                batches.push(Batch {
                    page: image.page,
                    range: start..start + 1,
                });
            }
        }
        if bounds.is_some_and(|b| !(b[2] - b[0]).is_finite() || !(b[3] - b[1]).is_finite()) {
            return Err(TextRenderError::InvalidOptions);
        }
        Ok(TextDraft {
            glyphs: data,
            batches,
            size,
            bounds: bounds.map(|b| Rect::new(b[0], b[1], b[2] - b[0], b[3] - b[1])),
            skipped,
            preparation,
        })
    }
    fn upload_geometry(&mut self, glyphs: &[GlyphData]) -> Geometry {
        let (geometry, reused) = GeometryPool::upload(
            &self.geometry_pool,
            &self.graphics,
            bytemuck::cast_slice(glyphs),
        );
        if reused {
            self.stats.geometry_buffer_reuses += 1;
        } else {
            self.stats.geometry_buffer_allocations += 1;
        }
        self.stats.geometry_bytes += glyphs.len() as u64 * 48;
        geometry
    }
    fn finish_text(
        &self,
        draft: TextDraft,
        buffer: Option<GeometryBuffer>,
        buffer_range: Range<u64>,
        shared: bool,
    ) -> PreparedText {
        let glyph_count = (buffer_range.end - buffer_range.start) as usize / 48;
        let lease = (!shared).then(|| {
            Arc::new(TextLease {
                _geometry: buffer.as_ref().map(|geometry| geometry.lease.clone()),
                _pages: draft
                    .batches
                    .iter()
                    .map(|batch| batch.page.allocation.clone())
                    .collect(),
            })
        });
        PreparedText {
            graphics: self.graphics.clone(),
            data: Arc::new(PreparedData {
                raster_pipeline: draft.batches.iter().any(|b| b.page.kind != Kind::Mtsdf),
                field_pipeline: draft.batches.iter().any(|b| b.page.kind == Kind::Mtsdf),
                buffer,
                buffer_range,
                batches: draft.batches,
                lease,
            }),
            size: draft.size,
            bounds: draft.bounds,
            glyph_count,
            skipped: draft.skipped.into(),
            preparation: draft.preparation,
        }
    }
    /// Records ordered prepared batches into a frame-owned pass, honoring its viewport/scissor.
    /// Restores owned pipeline/bindings/raster state each call; never shapes or rasterizes.
    /// Default shading ignores depth/stencil; explicit pipeline options select tests/writes.
    /// Attachment compatibility and read-only restrictions are checked before drawing.
    pub fn draw(
        &mut self,
        pass: &mut RenderPass<'_>,
        text: &PreparedText,
        draw: TextDraw,
    ) -> Result<(), Error> {
        if !pass.same_device(&self.graphics) || !self.graphics.same_device(&text.graphics) {
            return Err(Error::DeviceMismatch);
        }
        let parameters = draw_parameters(text.bounds, draw, pass.viewport_size())?;
        crate::mesh_renderer::validate_aspects(pass, &self.material)?;
        let Some(buffer) = &text.data.buffer else {
            return Ok(());
        };
        let color = pass.single_color_format()?;
        // Validate/create every required variant before recording commands.
        let raster_pipeline = if text.data.raster_pipeline {
            Some(self.pipeline(
                color,
                pass.sample_count(),
                pass.depth_stencil_format(),
                false,
            )?)
        } else {
            None
        };
        let field_pipeline = if text.data.field_pipeline {
            Some(self.pipeline(
                color,
                pass.sample_count(),
                pass.depth_stencil_format(),
                true,
            )?)
        } else {
            None
        };
        let (draw_buffer, range) = pass.upload_instances(bytemuck::bytes_of(&parameters), 4);
        self.stats.parameter_bytes += 48;
        // Keep both atlas and geometry immutable through recording and GPU completion.
        if let Some(lease) = &text.data.lease {
            pass.retain_resource(lease.clone());
        } else {
            // Shared geometry needs one token per buffer, not one per label.
            // Atlas ownership stays specific to the text's actual page batches.
            pass.retain_resource(buffer.lease.clone());
            for batch in &text.data.batches {
                pass.retain_resource(batch.page.allocation.clone());
            }
        }
        pass.apply_raster_state();
        pass.set_vertex_buffer(0, &buffer.buffer, text.data.buffer_range.clone());
        pass.set_vertex_buffer(1, &draw_buffer, range);
        let mut previous = None;
        for batch in &text.data.batches {
            let field = batch.page.kind == Kind::Mtsdf;
            if previous != Some(field) {
                pass.set_pipeline(if field {
                    field_pipeline.as_ref().unwrap()
                } else {
                    raster_pipeline.as_ref().unwrap()
                });
                previous = Some(field);
            }
            pass.set_bind_group(0, &batch.page.group, &[]);
            pass.inner.draw(0..6, batch.range.clone());
            self.stats.draw_calls += 1;
        }
        Ok(())
    }
    fn image(&mut self, key: Key, font: &TextFont) -> Result<GlyphSource, TextRenderError> {
        self.clock += 1;
        if let Some(cached) = self.cache.get_mut(&key) {
            self.stats.cache_hits += 1;
            cached.last = self.clock;
            if let GlyphSource::Image(image) = &cached.source
                && let Some(shelf) = self
                    .pages
                    .iter_mut()
                    .find(|s| Arc::ptr_eq(&s.page, &image.page))
            {
                shelf.last = self.clock;
            }
            return Ok(cached.source.clone());
        }
        self.stats.cache_misses += 1;
        let mut font_ref = swash::FontRef::from_index(font.data(), font.face_index() as usize)
            .ok_or(TextRenderError::InvalidRaster)?;
        if !self.fonts.contains_key(&font.id()) && self.fonts.len() >= 64 {
            self.fonts.clear();
        }
        font_ref.key = *self.fonts.entry(font.id()).or_default();
        let weight = swash::Tag::from_be_bytes(*b"wght");
        let coords = font_ref
            .variations()
            .normalized_coords([(weight, f32::from(key.weight))]);
        let (size, hint) = match key.representation {
            Representation::Coverage { size, hint } => (f32::from_bits(size), hint),
            Representation::Mtsdf { pixels_per_em, .. } => (pixels_per_em as f32, false),
        };
        let mut scaler = self
            .scale
            .builder(font_ref)
            .size(size)
            .hint(hint)
            .normalized_coords(coords)
            .build();
        let glyph = key.glyph;
        let slant = key.italic.then(|| {
            swash::zeno::Transform::skew(
                swash::zeno::Angle::from_degrees(14.),
                swash::zeno::Angle::from_degrees(0.),
            )
        });
        let source = match key.representation {
            Representation::Mtsdf {
                pixels_per_em,
                range_em,
            } => {
                // Probe artwork before monochrome outlines: some color glyphs have both.
                // Classification is size independent. Raster fallback is generated separately
                // at the requested physical size, preserving the existing source priority.
                if scaler.scale_color_outline(glyph).is_some()
                    || scaler
                        .scale_color_bitmap(glyph, StrikeWith::BestFit)
                        .is_some()
                {
                    GlyphSource::RasterFallback
                } else if let Some(mut outline) = scaler.scale_outline(glyph) {
                    if let Some(t) = &slant {
                        outline.transform(t);
                    }
                    match super::distance_field::generate(
                        &outline,
                        pixels_per_em,
                        f32::from_bits(range_em),
                        self.options.page_size,
                    )? {
                        Some(image) => GlyphSource::Image(self.upload(image, Kind::Mtsdf)?),
                        None => GlyphSource::Blank,
                    }
                } else if scaler.scale_bitmap(glyph, StrikeWith::BestFit).is_some() {
                    GlyphSource::RasterFallback
                } else {
                    GlyphSource::Blank
                }
            }
            Representation::Coverage { .. } => {
                // Check outline dimensions before Swash allocates its output image.
                // Bitmap dimensions are validated after decoding; Swash owns decode scratch.
                if let Some(mut outline) = scaler
                    .scale_color_outline(glyph)
                    .or_else(|| scaler.scale_outline(glyph))
                {
                    if let Some(t) = &slant {
                        outline.transform(t);
                    }
                    let bounds = outline.bounds();
                    if [bounds.width(), bounds.height()]
                        .iter()
                        .any(|v| !v.is_finite() || v.ceil() + 4. > self.options.page_size as f32)
                    {
                        return Err(TextRenderError::GlyphTooLarge);
                    }
                }
                let mut render = Render::new(&[
                    Source::ColorOutline(0),
                    Source::ColorBitmap(StrikeWith::BestFit),
                    Source::Outline,
                    Source::Bitmap(StrikeWith::BestFit),
                ]);
                render.format(swash::zeno::Format::Alpha);
                render.transform(slant);
                match render.render(&mut scaler, glyph) {
                    Some(image) if image.placement.width > 0 && image.placement.height > 0 => {
                        GlyphSource::Image(self.upload_image(image)?)
                    }
                    _ => GlyphSource::Blank,
                }
            }
        };
        if self.cache.len() >= self.options.max_cached_glyphs {
            let victim = *self.cache.iter().min_by_key(|(_, v)| v.last).unwrap().0;
            self.cache.remove(&victim);
        }
        self.cache.insert(
            key,
            Cached {
                source: source.clone(),
                last: self.clock,
            },
        );
        Ok(source)
    }
    fn upload_image(&mut self, mut image: Image) -> Result<GlyphImage, TextRenderError> {
        let kind = match image.content {
            Content::Mask => Kind::Mask,
            Content::Color => Kind::Color,
            Content::SubpixelMask => return Err(TextRenderError::InvalidRaster),
        };
        if kind == Kind::Color {
            // COLR layers are premultiplied sRGB; PNG bitmap images are straight sRGB.
            linearize_color(
                &mut image.data,
                matches!(image.source, Source::ColorOutline(_)),
            );
        }
        self.upload(image, kind)
    }
    fn upload(&mut self, image: Image, kind: Kind) -> Result<GlyphImage, TextRenderError> {
        let (w, h) = (image.placement.width, image.placement.height);
        let size = self.options.page_size;
        if w.checked_add(2).is_none_or(|v| v > size) || h.checked_add(2).is_none_or(|v| v > size) {
            return Err(TextRenderError::GlyphTooLarge);
        }
        if image.data.len() != (w as usize) * (h as usize) * kind.bytes() {
            return Err(TextRenderError::InvalidRaster);
        }
        let mut allocation = None;
        for shelf in &mut self.pages {
            if shelf.page.kind == kind
                && let Some(pos) = shelf.allocate(w + 2, h + 2, size)
            {
                shelf.last = self.clock;
                allocation = Some((shelf.page.clone(), pos));
                break;
            }
        }
        let (page, pos) = match allocation {
            Some(a) => a,
            None => {
                self.reclaim_page();
                if self.budget.pages.load(Ordering::Relaxed) >= self.options.max_pages {
                    return Err(TextRenderError::AtlasFull);
                }
                let texture = self
                    .graphics
                    .device()
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some("Astrelis glyph atlas page"),
                        size: wgpu::Extent3d {
                            width: size,
                            height: size,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: if kind == Kind::Mask {
                            wgpu::TextureFormat::R8Unorm
                        } else {
                            wgpu::TextureFormat::Rgba8Unorm
                        },
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });
                let view = texture.create_view(&Default::default());
                let group = self
                    .graphics
                    .device()
                    .create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Astrelis glyph atlas binding"),
                        layout: &self.layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(&view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(&self.sampler),
                            },
                        ],
                    });
                let bytes = (size as usize) * (size as usize) * kind.bytes();
                self.budget.pages.fetch_add(1, Ordering::Relaxed);
                self.budget.bytes.fetch_add(bytes, Ordering::Relaxed);
                self.next_page += 1;
                let page = Arc::new(Page {
                    id: self.next_page,
                    kind,
                    texture,
                    group,
                    allocation: Arc::new(Allocation {
                        bytes,
                        budget: self.budget.clone(),
                    }),
                });
                let mut shelf = Shelf {
                    page: page.clone(),
                    x: 0,
                    y: 0,
                    row: 0,
                    last: self.clock,
                };
                let pos = shelf.allocate(w + 2, h + 2, size).unwrap();
                self.pages.push(shelf);
                (page, pos)
            }
        };
        self.graphics.queue().write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &page.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: pos[0],
                    y: pos[1],
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &image.data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * kind.bytes() as u32),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.stats.uploaded_bytes += image.data.len() as u64;
        let s = size as f32;
        Ok(GlyphImage {
            page,
            uv: [
                pos[0] as f32 / s,
                pos[1] as f32 / s,
                w as f32 / s,
                h as f32 / s,
            ],
            left: image.placement.left,
            top: image.placement.top,
            width: w,
            height: h,
        })
    }
    fn reclaim_page(&mut self) {
        if self.budget.pages.load(Ordering::Relaxed) < self.options.max_pages {
            return;
        }
        let victim = self
            .pages
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                let cache_refs = self
                    .cache
                    .values()
                    .filter(|v| {
                        matches!(&v.source, GlyphSource::Image(i) if Arc::ptr_eq(&i.page, &s.page))
                    })
                    .count();
                Arc::strong_count(&s.page) == 1 + cache_refs
                    && Arc::strong_count(&s.page.allocation) == 1
            })
            .min_by_key(|(_, s)| s.last)
            .map(|(i, _)| i);
        if let Some(index) = victim {
            let page = self.pages.swap_remove(index).page;
            self.cache
                .retain(|_, v| !matches!(&v.source, GlyphSource::Image(i) if i.page.id == page.id));
        }
    }
}

fn linearize_color(data: &mut [u8], premultiplied: bool) {
    for p in data.as_chunks_mut::<4>().0 {
        let alpha = f32::from(p[3]) / 255.;
        for channel in &mut p[..3] {
            let mut s = f32::from(*channel) / 255.;
            if premultiplied {
                s = if alpha > 0. { (s / alpha).min(1.) } else { 0. };
            }
            let linear = if s <= 0.04045 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            };
            *channel = (linear * alpha * 255.).round() as u8;
        }
    }
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;

fn validate_preparation(preparation: TextPreparation) -> Result<(), TextRenderError> {
    if let TextPreparation::Mtsdf(options) = preparation
        && (!(16..=256).contains(&options.pixels_per_em)
            || !options.range_em.is_finite()
            || options.range_em <= 0.
            || options.range_em > 1.
            || options.range_em * (options.pixels_per_em as f32) < 2.)
    {
        return Err(TextRenderError::InvalidOptions);
    }
    let scale = preparation.raster_scale();
    if !scale.is_finite() || scale <= 0. {
        return Err(TextRenderError::InvalidOptions);
    }
    Ok(())
}

fn draw_parameters(
    bounds: Option<Rect>,
    draw: TextDraw,
    viewport: [f32; 2],
) -> Result<DrawData, Error> {
    if draw
        .origin
        .iter()
        .chain(&draw.color)
        .chain([&draw.opacity])
        .any(|v| !v.is_finite())
        || !(0. ..=1.).contains(&draw.color[3])
        || !(0. ..=1.).contains(&draw.opacity)
        || !draw.transform.valid()
    {
        return Err(Error::InvalidTextDraw);
    }
    let [a, b, c, d, tx, ty] = draw.transform.0.map(f64::from);
    let [ox, oy] = draw.origin.map(f64::from);
    let [vw, vh] = viewport.map(|v| if v > 0. { f64::from(v) } else { 1. });
    let origin = [(a * ox + c * oy + tx) / vw, (b * ox + d * oy + ty) / vh];
    let p = DrawData {
        origin_axis_x: [
            origin[0] as f32,
            origin[1] as f32,
            (a / vw) as f32,
            (b / vh) as f32,
        ],
        axis_y: [(c / vw) as f32, (d / vh) as f32, 0., 0.],
        color: [
            draw.color[0],
            draw.color[1],
            draw.color[2],
            draw.color[3] * draw.opacity,
        ],
    };
    if bytemuck::cast_slice::<DrawData, f32>(&[p])
        .iter()
        .any(|v| !v.is_finite())
    {
        return Err(Error::InvalidTextDraw);
    }
    if let Some(r) = bounds {
        for x in [r.x, r.x + r.width] {
            for y in [r.y, r.y + r.height] {
                let px = (origin[0] + (a * f64::from(x) + c * f64::from(y)) / vw) as f32;
                let py = (origin[1] + (b * f64::from(x) + d * f64::from(y)) / vh) as f32;
                if !(px * 2.).is_finite() || !(py * 2.).is_finite() {
                    return Err(Error::InvalidTextDraw);
                }
            }
        }
    }
    Ok(p)
}
