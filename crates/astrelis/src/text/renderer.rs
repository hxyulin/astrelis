use super::{FontId, TextFont, TextLayout};
use crate::{
    Error, GraphicsContext, Material, MaterialOptions, Rect, RenderFormat, RenderPass, Transform2D,
    VertexLayout,
};
use bytemuck::{Pod, Zeroable};
use std::{
    collections::HashMap,
    ops::Range,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use swash::scale::{
    Render, ScaleContext, Source, StrikeWith,
    image::{Content, Image},
};

/// GPU text preparation or drawing failure. Failed draws record no text commands.
#[derive(Debug)]
pub enum TextRenderError {
    /// A device, attachment format, or pipeline error from the rendering layer.
    Graphics(Error),
    /// Coverage/color shading requires a blendable floating-point color attachment.
    UnsupportedFormat {
        /// Incompatible pass color format.
        format: wgpu::TextureFormat,
    },
    /// Invalid atlas size/budget, raster scale/size, geometry, color, or opacity.
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
            Self::UnsupportedFormat { format } => write!(
                f,
                "text shading requires blendable floating-point color, got {format:?}"
            ),
            Self::InvalidOptions => f.write_str("invalid text renderer, raster, or draw options"),
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
#[derive(Clone, Copy, Debug)]
pub struct TextRendererOptions {
    /// Square page dimension, default 1024. Each glyph has a transparent one-texel gutter.
    pub page_size: u32,
    /// Maximum live pages, including prepared/recording/completion leases; default 8.
    /// R8 coverage pages cost size² bytes; linear RGBA8 color pages cost size²×4.
    pub max_pages: usize,
    /// Maximum cached glyph keys, including blank results; default 16,384.
    pub max_cached_glyphs: usize,
    /// Maximum physical font size passed to the rasterizer, default 512 pixels.
    pub max_raster_size: f32,
}
impl Default for TextRendererOptions {
    fn default() -> Self {
        Self {
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
    /// Positive finite physical pixels per layout unit. Scales glyphs and layout positions once.
    pub scale_factor: f32,
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
            scale_factor: 1.,
            hinting: true,
        }
    }
    /// Selects explicit DPI/raster scaling. Draw origin remains in physical pixels.
    pub const fn scale_factor(mut self, value: f32) -> Self {
        self.scale_factor = value;
        self
    }
    /// Enables or disables hinting.
    pub const fn hinting(mut self, value: bool) -> Self {
        self.hinting = value;
        self
    }
}
/// Placement and mask color for immutable prepared text in viewport-relative physical pixels.
#[derive(Clone, Copy, Debug)]
pub struct TextDraw {
    /// Origin offset before the affine transform.
    pub origin: [f32; 2],
    /// Linear straight RGBA for coverage glyphs; alpha also affects intrinsic color glyphs.
    pub color: [f32; 4],
    /// Additional opacity in `0..=1`, applied to mask and intrinsic color glyphs.
    pub opacity: f32,
    /// Transform of prepared pixel geometry and origin; X right/Y down.
    pub transform: Transform2D,
}
impl Default for TextDraw {
    fn default() -> Self {
        Self::new([0., 0.])
    }
}
impl TextDraw {
    /// Creates white text at the given pixel origin with opacity one.
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
    /// Selects an affine pixel transform. Magnification filters prepared coverage images.
    pub const fn transform_2d(mut self, value: Transform2D) -> Self {
        self.transform = value;
        self
    }
}
/// Cumulative renderer work counters and current atlas/cache occupancy.
#[derive(Clone, Copy, Debug, Default)]
pub struct TextRendererStats {
    /// Glyph raster cache hits, including blank entries.
    pub cache_hits: u64,
    /// Glyph raster cache misses.
    pub cache_misses: u64,
    /// Atlas upload payload bytes, excluding implicit texture initialization.
    pub uploaded_bytes: u64,
    /// Immutable glyph-buffer bytes created during preparation.
    pub geometry_bytes: u64,
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
    size: u32,
    italic: bool,
    hint: bool,
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
    image: Option<GlyphImage>,
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
struct Batch {
    page: Arc<Page>,
    range: Range<u32>,
}
#[derive(Debug)]
struct PreparedData {
    buffer: Option<wgpu::Buffer>,
    batches: Vec<Batch>,
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
    raster: TextRasterOptions,
}
impl PreparedText {
    /// Advance/line-box measurement scaled to physical pixels once.
    pub fn size(&self) -> [f32; 2] {
        self.size
    }
    /// Bounds of prepared image quads in physical pixels, distinct from layout measurement.
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
    /// Raster settings used by this immutable prepared resource.
    pub fn raster_options(&self) -> TextRasterOptions {
        self.raster
    }
}
/// Independent coverage/color renderer. Preparation rasterizes and uploads before drawing.
///
/// Uses R8 coverage and linear premultiplied RGBA8 color pages, with one-texel gutters.
/// Raster phase is always zero; fractional movement filters existing images without rerasterizing.
/// Only consecutive glyphs on the same page are instanced together, preserving layout order.
/// Leased pages are never evicted or overwritten. Budget exhaustion returns `AtlasFull` without
/// waiting; callers control dropping resources and device polling. Font-size/raster limits bound
/// image work, while Swash keeps a fixed eight-entry scaler cache and no CPU glyph-image cache.
pub struct TextRenderer {
    graphics: GraphicsContext,
    options: TextRendererOptions,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    material: Material,
    pipelines: crate::mesh_renderer::Pipelines,
    scale: ScaleContext,
    fonts: HashMap<FontId, swash::CacheKey>,
    cache: HashMap<Key, Cached>,
    pages: Vec<Shelf>,
    budget: Arc<Budget>,
    clock: u64,
    next_page: u64,
    stats: TextRendererStats,
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
            MaterialOptions::new(&shader)
                .vertex_layouts(&layouts)
                .bind_group_layouts(&[Some(&layout)]),
        );
        Ok(Self {
            graphics: g.clone(),
            options,
            layout,
            sampler,
            material,
            pipelines: Default::default(),
            scale: ScaleContext::new(),
            fonts: HashMap::new(),
            cache: HashMap::new(),
            pages: Vec::new(),
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
    /// Releases lookup caches and page ownership. Existing prepared texts/recordings remain usable.
    /// Live leased pages continue counting against this renderer's budget until dropped/completed.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
        self.pages.clear();
        self.fonts.clear();
        self.scale = ScaleContext::new();
    }
    fn pipeline(&mut self, format: &RenderFormat) -> Result<wgpu::RenderPipeline, TextRenderError> {
        let color = if format.colors.len() == 1 {
            format.colors[0]
        } else {
            None
        }
        .ok_or(Error::ExpectedSingleColor)?;
        Ok(crate::mesh_renderer::pipeline(
            &self.graphics,
            &mut self.pipelines,
            &self.material,
            color,
            format.sample_count,
            format.depth_stencil,
            true,
        )
        .map_err(|error| match error {
            Error::UnsupportedMeshFormat { format } => {
                TextRenderError::UnsupportedFormat { format }
            }
            other => TextRenderError::Graphics(other),
        })?
        .clone())
    }
    /// Prepares the attachment/MSAA/depth variant without rasterizing or drawing.
    pub fn prepare(&mut self, format: &RenderFormat) -> Result<(), TextRenderError> {
        self.pipeline(format)?;
        Ok(())
    }
    /// Prepares a compatible surface target's attachment variant.
    pub fn prepare_for_target(
        &mut self,
        target: &crate::RenderTarget<'_>,
    ) -> Result<(), TextRenderError> {
        if !self.graphics.same_device(target.graphics()) {
            return Err(Error::DeviceMismatch.into());
        }
        self.prepare(&target.render_format())
    }
    /// Rasterizes missing glyph images and creates immutable geometry. No TextSystem borrow is
    /// needed: layouts retain font sources. Preparation can populate/evict caches even on failure;
    /// previous prepared texts remain valid. Repeated preparation reuses atlas images but creates
    /// a new geometry buffer; keep PreparedText for unchanged content. Unsupported/blank raster
    /// sources are skipped and reported by original glyph index.
    pub fn prepare_text(
        &mut self,
        layout: &TextLayout,
        raster: TextRasterOptions,
    ) -> Result<PreparedText, TextRenderError> {
        let scale = raster.scale_factor;
        if !scale.is_finite()
            || scale <= 0.
            || layout.glyphs().iter().any(|g| {
                let size = g.font_size * scale;
                !size.is_finite() || size <= 0. || size > self.options.max_raster_size
            })
        {
            return Err(TextRenderError::InvalidOptions);
        }
        let size = layout.size().map(|v| v * scale);
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
            let key = Key {
                font: font.id(),
                glyph: glyph.glyph_id,
                weight: font.weight(),
                size: (glyph.font_size * scale).to_bits(),
                italic: glyph.synthetic_italic,
                hint: raster.hinting,
            };
            // Glyph ID is part of cache identity, separately from font/raster settings.
            let image = self.image(key, glyph.glyph_id, font)?;
            let Some(image) = image else {
                skipped.push(index);
                continue;
            };
            let x = glyph.position[0] * scale + image.left as f32;
            let y = glyph.position[1] * scale - image.top as f32;
            let w = image.width as f32;
            let h = image.height as f32;
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
                    if image.page.kind == Kind::Mask {
                        0.
                    } else {
                        1.
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
        let buffer = if data.is_empty() {
            None
        } else {
            use wgpu::util::DeviceExt;
            Some(
                self.graphics
                    .device()
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Astrelis prepared glyphs"),
                        contents: bytemuck::cast_slice(&data),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
            )
        };
        self.stats.geometry_bytes += data.len() as u64 * 48;
        Ok(PreparedText {
            graphics: self.graphics.clone(),
            data: Arc::new(PreparedData { buffer, batches }),
            size,
            bounds: bounds.map(|b| Rect::new(b[0], b[1], b[2] - b[0], b[3] - b[1])),
            glyph_count: data.len(),
            skipped: skipped.into(),
            raster,
        })
    }
    /// Records ordered prepared batches into a frame-owned pass, honoring its viewport/scissor.
    /// Restores owned pipeline/bindings/raster state each call; never shapes or rasterizes.
    /// Depth/stencil tests/writes are disabled, allowing overlays in depth-enabled passes.
    pub fn draw(
        &mut self,
        pass: &mut RenderPass<'_>,
        text: &PreparedText,
        draw: TextDraw,
    ) -> Result<(), TextRenderError> {
        if !pass.same_device(&self.graphics) || !self.graphics.same_device(&text.graphics) {
            return Err(Error::DeviceMismatch.into());
        }
        let parameters = draw_parameters(text.bounds, draw, pass.viewport_size())?;
        let Some(buffer) = &text.data.buffer else {
            return Ok(());
        };
        let pipeline = self.pipeline(&pass.render_format())?;
        let (draw_buffer, range) = pass.upload_instances(bytemuck::bytes_of(&parameters), 4);
        self.stats.parameter_bytes += 48;
        for batch in &text.data.batches {
            pass.retain_resource(batch.page.allocation.clone());
        }
        pass.apply_raster_state();
        pass.set_pipeline(&pipeline);
        pass.set_vertex_buffer(0, buffer, 0..buffer.size());
        pass.set_vertex_buffer(1, &draw_buffer, range);
        for batch in &text.data.batches {
            pass.set_bind_group(0, &batch.page.group, &[]);
            pass.inner.draw(0..6, batch.range.clone());
            self.stats.draw_calls += 1;
        }
        Ok(())
    }
    fn image(
        &mut self,
        key: Key,
        glyph: u16,
        font: &TextFont,
    ) -> Result<Option<GlyphImage>, TextRenderError> {
        self.clock += 1;
        if let Some(cached) = self.cache.get_mut(&key) {
            self.stats.cache_hits += 1;
            cached.last = self.clock;
            if let Some(image) = &cached.image
                && let Some(shelf) = self
                    .pages
                    .iter_mut()
                    .find(|s| Arc::ptr_eq(&s.page, &image.page))
            {
                shelf.last = self.clock;
            }
            return Ok(cached.image.clone());
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
        let mut scaler = self
            .scale
            .builder(font_ref)
            .size(f32::from_bits(key.size))
            .hint(key.hint)
            .normalized_coords(coords)
            .build();
        // Check outline dimensions before Swash allocates its output image.
        // Bitmap dimensions are validated after decoding; Swash owns decode scratch.
        let slant = key.italic.then(|| {
            swash::zeno::Transform::skew(
                swash::zeno::Angle::from_degrees(14.),
                swash::zeno::Angle::from_degrees(0.),
            )
        });
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
        ]);
        render.format(swash::zeno::Format::Alpha);
        render.transform(slant);
        let image = render.render(&mut scaler, glyph);
        let image = match image {
            Some(image) if image.placement.width > 0 && image.placement.height > 0 => {
                Some(self.upload_image(image)?)
            }
            _ => None,
        };
        if self.cache.len() >= self.options.max_cached_glyphs {
            let victim = *self.cache.iter().min_by_key(|(_, v)| v.last).unwrap().0;
            self.cache.remove(&victim);
        }
        self.cache.insert(
            key,
            Cached {
                image: image.clone(),
                last: self.clock,
            },
        );
        Ok(image)
    }
    fn upload_image(&mut self, mut image: Image) -> Result<GlyphImage, TextRenderError> {
        let kind = match image.content {
            Content::Mask => Kind::Mask,
            Content::Color => Kind::Color,
            Content::SubpixelMask => return Err(TextRenderError::InvalidRaster),
        };
        let (w, h) = (image.placement.width, image.placement.height);
        let size = self.options.page_size;
        if w.checked_add(2).is_none_or(|v| v > size) || h.checked_add(2).is_none_or(|v| v > size) {
            return Err(TextRenderError::GlyphTooLarge);
        }
        if image.data.len() != (w as usize) * (h as usize) * kind.bytes() {
            return Err(TextRenderError::InvalidRaster);
        }
        if kind == Kind::Color {
            // Swash COLR layer blits produce premultiplied sRGB; PNG bitmap
            // sources decode to straight sRGB. Normalize both to linear premultiplied RGBA.
            linearize_color(
                &mut image.data,
                matches!(image.source, Source::ColorOutline(_)),
            );
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
                        v.image
                            .as_ref()
                            .is_some_and(|i| Arc::ptr_eq(&i.page, &s.page))
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
                .retain(|_, v| v.image.as_ref().is_none_or(|i| i.page.id != page.id));
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

fn draw_parameters(
    bounds: Option<Rect>,
    draw: TextDraw,
    viewport: [f32; 2],
) -> Result<DrawData, TextRenderError> {
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
        return Err(TextRenderError::InvalidOptions);
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
        return Err(TextRenderError::InvalidOptions);
    }
    if let Some(r) = bounds {
        for x in [r.x, r.x + r.width] {
            for y in [r.y, r.y + r.height] {
                let px = (origin[0] + (a * f64::from(x) + c * f64::from(y)) / vw) as f32;
                let py = (origin[1] + (b * f64::from(x) + d * f64::from(y)) / vh) as f32;
                if !(px * 2.).is_finite() || !(py * 2.).is_finite() {
                    return Err(TextRenderError::InvalidOptions);
                }
            }
        }
    }
    Ok(p)
}
