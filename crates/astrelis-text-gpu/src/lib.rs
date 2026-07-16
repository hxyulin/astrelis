//! GPU glyph rasterization and atlas caching for Astrelis text layouts.

#![warn(missing_docs)]

use std::{collections::HashMap, error::Error, fmt};

use astrelis_core::geometry::{LogicalRect, Physical, Rect, Size};
use astrelis_gpu as gpu;
use astrelis_text::{GlyphRun, TextLayout};
use etagere::{AtlasAllocator, size2};
use swash::{
    FontRef,
    scale::{
        Render, ScaleContext, Source, StrikeWith,
        image::{Content, Image},
    },
};

const SOURCES: [Source; 3] = [
    Source::ColorOutline(0),
    Source::ColorBitmap(StrikeWith::BestFit),
    Source::Outline,
];
const PADDING: u32 = 1;

/// Persistent glyph-atlas configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlyphCacheOptions {
    /// Width and height of newly allocated atlas pages.
    pub page_size: u32,
    /// Soft limit for all mask and color atlas texture bytes.
    pub max_bytes: usize,
}

impl Default for GlyphCacheOptions {
    fn default() -> Self {
        Self {
            page_size: 2_048,
            max_bytes: 64 << 20,
        }
    }
}

/// Glyph preparation failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlyphCacheError(String);

impl GlyphCacheError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for GlyphCacheError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Error for GlyphCacheError {}

impl From<gpu::GpuError> for GlyphCacheError {
    fn from(value: gpu::GpuError) -> Self {
        Self::new(value.to_string())
    }
}

/// Atlas content mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AtlasKind {
    /// Single-channel glyph coverage mask.
    Mask,
    /// Straight-alpha sRGB color glyph.
    Color,
}

/// One GPU-ready glyph quad.
#[derive(Clone, Debug)]
pub struct PreparedGlyph {
    /// Glyph rectangle in layout-local logical units.
    pub rect: LogicalRect,
    /// Normalized atlas coordinates `[u0, v0, u1, v1]`.
    pub uv: [f32; 4],
    /// Atlas sampling bind group.
    pub bind_group: gpu::BindGroup,
    /// Atlas content mode.
    pub kind: AtlasKind,
}

/// Statistics from preparing retained text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GlyphCacheStats {
    /// Glyphs found in the persistent cache.
    pub hits: u32,
    /// Newly rasterized glyphs.
    pub misses: u32,
    /// Newly uploaded non-empty glyph images.
    pub uploads: u32,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct GlyphKey {
    font_blob: u64,
    font_index: u32,
    glyph: u32,
    ppem_quarters: u32,
    coords: Vec<i16>,
}

#[derive(Clone)]
struct CachedGlyph {
    page: usize,
    rect: LogicalRect,
    uv: [f32; 4],
    kind: AtlasKind,
}

struct AtlasPage {
    kind: AtlasKind,
    _texture: gpu::Texture,
    bind_group: gpu::BindGroup,
    allocator: AtlasAllocator,
    used: u64,
}

/// Device-bound rasterizer and bounded glyph atlas.
pub struct GlyphCache {
    device: gpu::Device,
    queue: gpu::Queue,
    options: GlyphCacheOptions,
    layout: gpu::BindGroupLayout,
    sampler: gpu::Sampler,
    scale_context: ScaleContext,
    pages: Vec<AtlasPage>,
    glyphs: HashMap<GlyphKey, CachedGlyph>,
    frame: u64,
}

impl GlyphCache {
    /// Creates a cache for one device and queue.
    pub fn new(
        device: gpu::Device,
        queue: gpu::Queue,
        mut options: GlyphCacheOptions,
    ) -> Result<Self, GlyphCacheError> {
        if device.id() != queue.device_id() {
            return Err(GlyphCacheError::new(
                "device and queue belong to different devices",
            ));
        }
        let maximum = device.capabilities().limits.max_texture_dimension_2d;
        options.page_size = options.page_size.clamp(64, maximum);
        let layout = device.create_bind_group_layout(gpu::BindGroupLayoutDescriptor {
            label: Some("text atlas layout".into()),
            entries: vec![
                gpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: gpu::ShaderStages::FRAGMENT,
                    ty: gpu::BindingType::Texture {
                        sample_type: gpu::TextureSampleType::Float,
                        view_dimension: gpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                },
                gpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: gpu::ShaderStages::FRAGMENT,
                    ty: gpu::BindingType::Sampler(gpu::SamplerBindingType::Filtering),
                },
            ],
        });
        let sampler = device.create_sampler(gpu::SamplerDescriptor {
            mag_filter: gpu::FilterMode::Linear,
            min_filter: gpu::FilterMode::Linear,
            ..Default::default()
        });
        Ok(Self {
            device,
            queue,
            options,
            layout,
            sampler,
            scale_context: ScaleContext::new(),
            pages: Vec::new(),
            glyphs: HashMap::new(),
            frame: 0,
        })
    }

    /// Bind-group layout used by text sampling pipelines.
    pub fn bind_group_layout(&self) -> gpu::BindGroupLayout {
        self.layout.clone()
    }

    /// Begins a preparation frame and pins pages used during that frame.
    pub fn begin_frame(&mut self) {
        self.frame = self.frame.wrapping_add(1);
    }

    /// Prepares every visible glyph in a retained layout.
    pub fn prepare_layout(
        &mut self,
        text: &TextLayout,
        physical_scale: f32,
    ) -> Result<(Vec<(usize, PreparedGlyph)>, GlyphCacheStats), GlyphCacheError> {
        if !physical_scale.is_finite() || physical_scale <= 0.0 {
            return Err(GlyphCacheError::new(
                "physical text scale must be finite and positive",
            ));
        }
        let mut prepared = Vec::new();
        let mut stats = GlyphCacheStats::default();
        for (run_index, run) in text.glyph_runs().iter().enumerate() {
            for glyph in run.glyphs.iter() {
                if let Some(value) =
                    self.prepare_glyph(run, glyph.id, physical_scale, &mut stats)?
                {
                    prepared.push((
                        run_index,
                        PreparedGlyph {
                            rect: Rect::from_xywh(
                                glyph.position.x + value.rect.origin.x,
                                glyph.position.y + value.rect.origin.y,
                                value.rect.size.width,
                                value.rect.size.height,
                            ),
                            uv: value.uv,
                            bind_group: self.pages[value.page].bind_group.clone(),
                            kind: value.kind,
                        },
                    ));
                }
            }
        }
        Ok((prepared, stats))
    }

    /// Evicts unpinned whole pages until the soft budget is met.
    pub fn finish_frame(&mut self) {
        while self.bytes() > self.options.max_bytes {
            let Some((index, _)) = self
                .pages
                .iter()
                .enumerate()
                .filter(|(_, page)| page.used != self.frame)
                .min_by_key(|(_, page)| page.used)
            else {
                break;
            };
            self.remove_page(index);
        }
    }

    /// Clears all glyphs and atlas pages.
    pub fn clear(&mut self) {
        self.glyphs.clear();
        self.pages.clear();
    }

    /// Current texture allocation in bytes.
    pub fn bytes(&self) -> usize {
        self.pages
            .iter()
            .map(|page| {
                let channels = match page.kind {
                    AtlasKind::Mask => 1,
                    AtlasKind::Color => 4,
                };
                self.options.page_size as usize * self.options.page_size as usize * channels
            })
            .sum()
    }

    fn prepare_glyph(
        &mut self,
        run: &GlyphRun,
        glyph: u32,
        physical_scale: f32,
        stats: &mut GlyphCacheStats,
    ) -> Result<Option<CachedGlyph>, GlyphCacheError> {
        let ppem_quarters = (run.font_size * physical_scale * 4.0).round().max(1.0) as u32;
        let key = GlyphKey {
            font_blob: run.font.cache_id().0,
            font_index: run.font.cache_id().1,
            glyph,
            ppem_quarters,
            coords: run.normalized_coords.to_vec(),
        };
        if let Some(cached) = self.glyphs.get(&key).cloned() {
            self.pages[cached.page].used = self.frame;
            stats.hits += 1;
            return Ok(Some(cached));
        }
        stats.misses += 1;
        let font = FontRef::from_index(run.font.data(), run.font.index() as usize)
            .ok_or_else(|| GlyphCacheError::new("resolved font could not be parsed by Swash"))?;
        let ppem = ppem_quarters as f32 / 4.0;
        let mut scaler = self
            .scale_context
            .builder_with_id(
                font,
                [run.font.cache_id().0, u64::from(run.font.cache_id().1)],
            )
            .size(ppem)
            .hint(true)
            .normalized_coords(run.normalized_coords.iter())
            .build();
        let Some(image) = Render::new(&SOURCES).render(&mut scaler, glyph as u16) else {
            return Ok(None);
        };
        if image.placement.width == 0 || image.placement.height == 0 {
            return Ok(None);
        }
        let kind = match image.content {
            Content::Mask => AtlasKind::Mask,
            Content::Color => AtlasKind::Color,
            Content::SubpixelMask => AtlasKind::Color,
        };
        let page = self.allocate_page(
            kind,
            image.placement.width + PADDING * 2,
            image.placement.height + PADDING * 2,
        )?;
        let allocation = self.pages[page]
            .allocator
            .allocate(size2(
                (image.placement.width + PADDING * 2) as i32,
                (image.placement.height + PADDING * 2) as i32,
            ))
            .expect("page allocation was checked");
        let x = allocation.rectangle.min.x as u32 + PADDING;
        let y = allocation.rectangle.min.y as u32 + PADDING;
        let data = normalize_image(&image);
        let channels = match kind {
            AtlasKind::Mask => 1,
            AtlasKind::Color => 4,
        };
        self.queue.write_texture(
            &gpu::TextureCopy {
                texture: self.pages[page]._texture.clone(),
                mip_level: 0,
                origin: gpu::Origin3d { x, y, z: 0 },
            },
            &data,
            gpu::TextureDataLayout {
                offset: 0,
                bytes_per_row: Some(image.placement.width * channels),
                rows_per_image: Some(image.placement.height),
            },
            gpu::Extent3d::d2(image.placement.width, image.placement.height),
        )?;
        self.pages[page].used = self.frame;
        stats.uploads += 1;
        let atlas = self.options.page_size as f32;
        let cached = CachedGlyph {
            page,
            rect: Rect::from_xywh(
                image.placement.left as f32 / physical_scale,
                -(image.placement.top as f32) / physical_scale,
                image.placement.width as f32 / physical_scale,
                image.placement.height as f32 / physical_scale,
            ),
            uv: [
                x as f32 / atlas,
                y as f32 / atlas,
                (x + image.placement.width) as f32 / atlas,
                (y + image.placement.height) as f32 / atlas,
            ],
            kind,
        };
        self.glyphs.insert(key, cached.clone());
        Ok(Some(cached))
    }

    fn allocate_page(
        &mut self,
        kind: AtlasKind,
        width: u32,
        height: u32,
    ) -> Result<usize, GlyphCacheError> {
        if width > self.options.page_size || height > self.options.page_size {
            return Err(GlyphCacheError::new(
                "rasterized glyph exceeds atlas page dimensions",
            ));
        }
        if let Some(index) = self.pages.iter_mut().position(|page| {
            page.kind == kind
                && page
                    .allocator
                    .allocate(size2(width as i32, height as i32))
                    .is_some_and(|allocation| {
                        page.allocator.deallocate(allocation.id);
                        true
                    })
        }) {
            return Ok(index);
        }
        let format = match kind {
            AtlasKind::Mask => gpu::TextureFormat::R8Unorm,
            AtlasKind::Color => gpu::TextureFormat::Rgba8UnormSrgb,
        };
        let texture = self.device.create_texture(gpu::TextureDescriptor {
            label: Some("text glyph atlas".into()),
            size: gpu::Extent3d::d2(self.options.page_size, self.options.page_size),
            mip_level_count: 1,
            sample_count: 1,
            dimension: gpu::TextureDimension::D2,
            format,
            usage: gpu::TextureUsages::TEXTURE_BINDING | gpu::TextureUsages::COPY_DST,
        });
        let view = texture.create_view(Default::default());
        let bind_group = self.device.create_bind_group(gpu::BindGroupDescriptor {
            label: Some("text glyph atlas bind group".into()),
            layout: self.layout.clone(),
            entries: vec![
                gpu::BindGroupEntry {
                    binding: 0,
                    resource: gpu::BindingResource::TextureView(view),
                },
                gpu::BindGroupEntry {
                    binding: 1,
                    resource: gpu::BindingResource::Sampler(self.sampler.clone()),
                },
            ],
        })?;
        self.pages.push(AtlasPage {
            kind,
            _texture: texture,
            bind_group,
            allocator: AtlasAllocator::new(size2(
                self.options.page_size as i32,
                self.options.page_size as i32,
            )),
            used: self.frame,
        });
        Ok(self.pages.len() - 1)
    }

    fn remove_page(&mut self, index: usize) {
        self.pages.remove(index);
        self.glyphs.retain(|_, glyph| {
            if glyph.page == index {
                false
            } else {
                if glyph.page > index {
                    glyph.page -= 1;
                }
                true
            }
        });
    }
}

fn normalize_image(image: &Image) -> Vec<u8> {
    match image.content {
        Content::Mask | Content::Color => image.data.clone(),
        Content::SubpixelMask => image
            .data
            .chunks_exact(4)
            .flat_map(|pixel| [255, 255, 255, pixel[0].max(pixel[1]).max(pixel[2])])
            .collect(),
    }
}

#[allow(dead_code)]
fn _physical_size(value: u32) -> Size<Physical, u32> {
    Size::new(value, value)
}
