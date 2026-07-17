//! GPU renderer for backend-independent Astrelis display lists.

#![warn(missing_docs)]

use std::{collections::HashMap, error::Error, fmt, mem::size_of, ops::Range};

use astrelis_core::{
    color::Color,
    geometry::{LogicalRect, Physical, Rect, Size},
    math::{Affine2, Vec2},
};
use astrelis_gpu as gpu;
use astrelis_paint::{
    Brush, Command, CornerRadii, DisplayList, FillRule, Image, ImageOptions, ImageSampling,
    LineCap, LineJoin, LinearGradient, Path, PathVerb, RadialGradient, RoundedRect, StrokeStyle,
};
use astrelis_text_gpu::{AtlasKind, GlyphCache, GlyphCacheOptions};
use bytemuck::{Pod, Zeroable};
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, StrokeOptions, StrokeTessellator,
    StrokeVertex, VertexBuffers, math::point, path::Path as LyonPath,
};

/// Renderer antialiasing mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Antialiasing {
    /// Single-sample rasterization.
    None,
    /// Four-sample multisampling.
    #[default]
    Msaa4,
}

impl Antialiasing {
    fn samples(self) -> u32 {
        match self {
            Self::None => 1,
            Self::Msaa4 => 4,
        }
    }
}

/// Persistent renderer cache budgets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheLimits {
    /// Maximum cached tessellated mesh bytes.
    pub mesh_bytes: usize,
    /// Maximum cached uploaded image bytes.
    pub image_bytes: usize,
    /// Maximum cached gradient buffer bytes.
    pub gradient_bytes: usize,
    /// Maximum glyph atlas texture bytes.
    pub glyph_bytes: usize,
}

impl Default for CacheLimits {
    fn default() -> Self {
        Self {
            mesh_bytes: 32 << 20,
            image_bytes: 128 << 20,
            gradient_bytes: 4 << 20,
            glyph_bytes: 64 << 20,
        }
    }
}

/// Device-bound renderer configuration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RendererOptions {
    /// Edge antialiasing mode.
    pub antialiasing: Antialiasing,
    /// Persistent cache limits.
    pub cache_limits: CacheLimits,
}

/// One complete paint destination.
#[derive(Clone, Debug)]
pub struct RenderTarget {
    /// Destination texture view.
    pub view: gpu::TextureView,
    /// Destination pixel format.
    pub format: gpu::TextureFormat,
    /// Physical dimensions.
    pub size: Size<Physical, u32>,
    /// Logical-to-physical scale factor.
    pub scale_factor: f32,
    /// Linear-space clear color.
    pub clear_color: Color,
}

/// Statistics from preparing and recording one display list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderStats {
    /// Recorded content draws.
    pub draws: u32,
    /// Recorded triangles, including clip geometry.
    pub triangles: u32,
    /// Reused tessellated meshes.
    pub mesh_cache_hits: u32,
    /// Newly tessellated meshes.
    pub mesh_cache_misses: u32,
    /// Reused uploaded images.
    pub image_cache_hits: u32,
    /// Newly uploaded images.
    pub image_cache_misses: u32,
    /// Reused uploaded gradient resources.
    pub gradient_cache_hits: u32,
    /// Newly uploaded gradient resources.
    pub gradient_cache_misses: u32,
    /// Reused rasterized glyphs.
    pub glyph_cache_hits: u32,
    /// Newly rasterized glyphs.
    pub glyph_cache_misses: u32,
    /// Newly uploaded glyph images.
    pub glyph_uploads: u32,
}

/// Display-list rendering failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderError(String);

impl RenderError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for RenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Error for RenderError {}

impl From<gpu::GpuError> for RenderError {
    fn from(value: gpu::GpuError) -> Self {
        Self::new(value.to_string())
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
    local_position: [f32; 2],
}

#[derive(Clone)]
struct Mesh {
    vertices: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

impl Mesh {
    fn bytes(&self) -> usize {
        self.vertices.len() * 8 + self.indices.len() * 4
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum MeshKind {
    Fill(FillRule),
    Stroke(u32, LineCap, LineJoin, u32),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct MeshKey(u64, MeshKind, i16);

struct CachedMesh {
    mesh: Mesh,
    used: u64,
}

struct CachedImage {
    _texture: gpu::Texture,
    nearest: gpu::BindGroup,
    linear: gpu::BindGroup,
    bytes: usize,
    used: u64,
}

struct CachedGradient {
    _header: gpu::Buffer,
    _stops: gpu::Buffer,
    bind_group: gpu::BindGroup,
    bytes: usize,
    used: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct PipelineKey(gpu::TextureFormat, u32);

struct Pipelines {
    solid: gpu::RenderPipeline,
    gradient: gpu::RenderPipeline,
    image: gpu::RenderPipeline,
    text_mask: gpu::RenderPipeline,
    text_color: gpu::RenderPipeline,
    clip_push: gpu::RenderPipeline,
    clip_pop: gpu::RenderPipeline,
}

struct Attachments {
    key: (u32, u32, gpu::TextureFormat, u32),
    _color: Option<gpu::Texture>,
    color: Option<gpu::TextureView>,
    _stencil: gpu::Texture,
    stencil: gpu::TextureView,
}

struct FrameBuffer {
    buffer: gpu::Buffer,
    capacity: usize,
}

#[derive(Clone, Copy)]
struct Scissor {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl Scissor {
    fn full(size: Size<Physical, u32>) -> Self {
        Self {
            x: 0,
            y: 0,
            width: size.width,
            height: size.height,
        }
    }

    fn intersect(self, other: Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = (self.x + self.width).min(other.x + other.width);
        let bottom = (self.y + self.height).min(other.y + other.height);
        Self {
            x,
            y,
            width: right.saturating_sub(x),
            height: bottom.saturating_sub(y),
        }
    }
}

#[derive(Clone)]
struct Clip {
    mesh: Mesh,
    transform: Affine2,
    scissor: Scissor,
}

#[derive(Clone)]
struct State {
    transform: Affine2,
    scissor: Scissor,
    clips: Vec<Clip>,
    opacity: f32,
}

enum DrawKind {
    Solid,
    Gradient(gpu::BindGroup),
    Image(gpu::BindGroup),
    TextMask(gpu::BindGroup),
    TextColor(gpu::BindGroup),
    ClipPush,
    ClipPop,
}

struct Draw {
    kind: DrawKind,
    indices: Range<u32>,
    scissor: Scissor,
    stencil: u32,
}

/// GPU display-list renderer tied to one device and queue.
pub struct Renderer {
    device: gpu::Device,
    queue: gpu::Queue,
    options: RendererOptions,
    image_layout: gpu::BindGroupLayout,
    gradient_layout: gpu::BindGroupLayout,
    pipelines: HashMap<PipelineKey, Pipelines>,
    attachments: Option<Attachments>,
    meshes: HashMap<MeshKey, CachedMesh>,
    images: HashMap<u64, CachedImage>,
    gradients: HashMap<u64, CachedGradient>,
    glyphs: GlyphCache,
    vertex_buffer: Option<FrameBuffer>,
    index_buffer: Option<FrameBuffer>,
    clock: u64,
}

impl Renderer {
    /// Creates a renderer for one device/queue pair.
    pub fn new(
        device: gpu::Device,
        queue: gpu::Queue,
        options: RendererOptions,
    ) -> Result<Self, RenderError> {
        if device.id() != queue.device_id() {
            return Err(RenderError::new(
                "device and queue belong to different devices",
            ));
        }
        let image_layout = device.create_bind_group_layout(gpu::BindGroupLayoutDescriptor {
            label: Some("paint image layout".into()),
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
        let gradient_layout = device.create_bind_group_layout(gpu::BindGroupLayoutDescriptor {
            label: Some("paint gradient layout".into()),
            entries: vec![
                gpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: gpu::ShaderStages::FRAGMENT,
                    ty: gpu::BindingType::Buffer {
                        ty: gpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                },
                gpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: gpu::ShaderStages::FRAGMENT,
                    ty: gpu::BindingType::Buffer {
                        ty: gpu::BufferBindingType::ReadOnlyStorage,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                },
            ],
        });
        let glyphs = GlyphCache::new(
            device.clone(),
            queue.clone(),
            GlyphCacheOptions {
                max_bytes: options.cache_limits.glyph_bytes,
                ..Default::default()
            },
        )
        .map_err(|error| RenderError::new(error.to_string()))?;
        Ok(Self {
            device,
            queue,
            options,
            image_layout,
            gradient_layout,
            pipelines: HashMap::new(),
            attachments: None,
            meshes: HashMap::new(),
            images: HashMap::new(),
            gradients: HashMap::new(),
            glyphs,
            vertex_buffer: None,
            index_buffer: None,
            clock: 0,
        })
    }

    /// Records a complete display-list render into an existing encoder.
    pub fn render(
        &mut self,
        encoder: &mut gpu::CommandEncoder,
        list: &DisplayList,
        target: RenderTarget,
    ) -> Result<RenderStats, RenderError> {
        if target.view.device_id() != self.device.id() {
            return Err(RenderError::new("render target belongs to another device"));
        }
        if !target.scale_factor.is_finite() || target.scale_factor <= 0.0 {
            return Err(RenderError::new("scale factor must be finite and positive"));
        }
        if target.size.width == 0 || target.size.height == 0 {
            return Ok(RenderStats::default());
        }
        self.clock = self.clock.wrapping_add(1);
        self.glyphs.begin_frame();
        let samples = self.options.antialiasing.samples();
        self.ensure_pipelines(target.format, samples)?;
        self.ensure_attachments(target.size, target.format, samples);

        let dpi = Affine2::from_scale(Vec2::splat(target.scale_factor));
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut draws = Vec::new();
        let mut stats = RenderStats::default();
        let mut state = State {
            transform: Affine2::IDENTITY,
            scissor: Scissor::full(target.size),
            clips: Vec::new(),
            opacity: 1.0,
        };
        let mut stack = Vec::new();
        for (index, command) in list.commands().iter().enumerate() {
            if let Err(error) = self.compile(
                command,
                list,
                dpi,
                target.size,
                &mut state,
                &mut stack,
                &mut vertices,
                &mut indices,
                &mut draws,
                &mut stats,
            ) {
                return Err(RenderError::new(format!("command {index}: {error}")));
            }
        }

        self.upload(&vertices, &indices)?;
        let attachments = self.attachments.as_ref().expect("attachments exist");
        let pipeline = self
            .pipelines
            .get(&PipelineKey(target.format, samples))
            .expect("pipelines exist");
        let view = attachments
            .color
            .clone()
            .unwrap_or_else(|| target.view.clone());
        let resolve_target = attachments.color.as_ref().map(|_| target.view.clone());
        let mut pass = encoder.begin_render_pass(gpu::RenderPassDescriptor {
            label: Some("astrelis paint".into()),
            color_attachments: vec![Some(gpu::RenderPassColorAttachment {
                view,
                resolve_target,
                load: gpu::LoadOp::Clear(gpu::Color {
                    r: target.clear_color.r as f64,
                    g: target.clear_color.g as f64,
                    b: target.clear_color.b as f64,
                    a: target.clear_color.a as f64,
                }),
                store: gpu::StoreOp::Store,
            })],
            depth_stencil_attachment: Some(gpu::RenderPassDepthStencilAttachment {
                view: attachments.stencil.clone(),
                depth_ops: None,
                stencil_ops: Some(gpu::AttachmentOperations {
                    load: gpu::LoadOpValue::Clear(0),
                    store: gpu::StoreOp::Discard,
                }),
            }),
            timestamp_writes: None,
        })?;
        if !indices.is_empty() {
            let vb = &self.vertex_buffer.as_ref().expect("vertex buffer").buffer;
            let ib = &self.index_buffer.as_ref().expect("index buffer").buffer;
            pass.set_vertex_buffer(0, vb, 0..(vertices.len() * size_of::<Vertex>()) as u64)?;
            pass.set_index_buffer(
                ib,
                0..(indices.len() * size_of::<u32>()) as u64,
                gpu::IndexFormat::Uint32,
            )?;
            for draw in draws {
                if draw.scissor.width == 0 || draw.scissor.height == 0 {
                    continue;
                }
                pass.set_scissor_rect(
                    draw.scissor.x,
                    draw.scissor.y,
                    draw.scissor.width,
                    draw.scissor.height,
                );
                pass.set_stencil_reference(draw.stencil);
                match draw.kind {
                    DrawKind::Solid => pass.set_pipeline(&pipeline.solid)?,
                    DrawKind::Gradient(bind) => {
                        pass.set_pipeline(&pipeline.gradient)?;
                        pass.set_bind_group(0, &bind, &[])?;
                    }
                    DrawKind::Image(bind) => {
                        pass.set_pipeline(&pipeline.image)?;
                        pass.set_bind_group(0, &bind, &[])?;
                    }
                    DrawKind::TextMask(bind) => {
                        pass.set_pipeline(&pipeline.text_mask)?;
                        pass.set_bind_group(0, &bind, &[])?;
                    }
                    DrawKind::TextColor(bind) => {
                        pass.set_pipeline(&pipeline.text_color)?;
                        pass.set_bind_group(0, &bind, &[])?;
                    }
                    DrawKind::ClipPush => pass.set_pipeline(&pipeline.clip_push)?,
                    DrawKind::ClipPop => pass.set_pipeline(&pipeline.clip_pop)?,
                }
                pass.draw_indexed(draw.indices, 0, 0..1);
            }
        }
        drop(pass);
        self.glyphs.finish_frame();
        self.evict();
        Ok(stats)
    }

    /// Clears persistent image and tessellation caches.
    pub fn trim_caches(&mut self) {
        self.meshes.clear();
        self.images.clear();
        self.gradients.clear();
        self.glyphs.clear();
    }

    #[allow(clippy::too_many_arguments)]
    fn compile(
        &mut self,
        command: &Command,
        list: &DisplayList,
        dpi: Affine2,
        size: Size<Physical, u32>,
        state: &mut State,
        stack: &mut Vec<State>,
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u32>,
        draws: &mut Vec<Draw>,
        stats: &mut RenderStats,
    ) -> Result<(), RenderError> {
        match command {
            Command::Save => stack.push(state.clone()),
            Command::Restore => {
                let restored = stack
                    .pop()
                    .ok_or_else(|| RenderError::new("restore at root"))?;
                while state.clips.len() > restored.clips.len() {
                    let clip = state.clips.pop().expect("clip exists");
                    append(
                        &clip.mesh,
                        dpi * clip.transform,
                        size,
                        [0.0; 4],
                        None,
                        vertices,
                        indices,
                        draws,
                        DrawKind::ClipPop,
                        clip.scissor,
                        state.clips.len() as u32 + 1,
                        stats,
                    );
                }
                *state = restored;
            }
            Command::Transform(value) => state.transform *= *value,
            Command::MultiplyOpacity(value) => state.opacity *= *value,
            Command::ClipRect(rect) => {
                if let Some(scissor) = exact_scissor(*rect, dpi * state.transform, size) {
                    state.scissor = state.scissor.intersect(scissor);
                } else {
                    push_clip(
                        rect_mesh(*rect),
                        state,
                        dpi,
                        size,
                        vertices,
                        indices,
                        draws,
                        stats,
                    )?;
                }
            }
            Command::ClipRoundedRect(rect) => {
                push_clip(
                    rounded_mesh(*rect, local_tolerance(dpi * state.transform))?,
                    state,
                    dpi,
                    size,
                    vertices,
                    indices,
                    draws,
                    stats,
                )?;
            }
            Command::ClipPath { path, rule } => {
                let mesh = self.path_mesh(
                    list.path(*path),
                    MeshKind::Fill(*rule),
                    dpi * state.transform,
                    stats,
                )?;
                push_clip(mesh, state, dpi, size, vertices, indices, draws, stats)?;
            }
            Command::FillRect { rect, brush } => self.draw_brush(
                rect_mesh(*rect),
                brush,
                state,
                dpi,
                size,
                vertices,
                indices,
                draws,
                stats,
            )?,
            Command::FillRoundedRect { rect, brush } => self.draw_brush(
                rounded_mesh(*rect, local_tolerance(dpi * state.transform))?,
                brush,
                state,
                dpi,
                size,
                vertices,
                indices,
                draws,
                stats,
            )?,
            Command::FillEllipse { rect, brush } => self.draw_brush(
                ellipse_mesh(*rect, local_tolerance(dpi * state.transform), None)?,
                brush,
                state,
                dpi,
                size,
                vertices,
                indices,
                draws,
                stats,
            )?,
            Command::StrokeRect { rect, style, brush } => self.draw_brush(
                shape_stroke_mesh(
                    rect_path(*rect)?,
                    *style,
                    local_tolerance(dpi * state.transform),
                )?,
                brush,
                state,
                dpi,
                size,
                vertices,
                indices,
                draws,
                stats,
            )?,
            Command::StrokeRoundedRect { rect, style, brush } => self.draw_brush(
                shape_stroke_mesh(
                    rounded_path(*rect)?,
                    *style,
                    local_tolerance(dpi * state.transform),
                )?,
                brush,
                state,
                dpi,
                size,
                vertices,
                indices,
                draws,
                stats,
            )?,
            Command::StrokeEllipse { rect, style, brush } => self.draw_brush(
                ellipse_mesh(*rect, local_tolerance(dpi * state.transform), Some(*style))?,
                brush,
                state,
                dpi,
                size,
                vertices,
                indices,
                draws,
                stats,
            )?,
            Command::FillPath { path, rule, brush } => {
                let mesh = self.path_mesh(
                    list.path(*path),
                    MeshKind::Fill(*rule),
                    dpi * state.transform,
                    stats,
                )?;
                self.draw_brush(
                    mesh, brush, state, dpi, size, vertices, indices, draws, stats,
                )?;
            }
            Command::StrokePath { path, style, brush } => {
                let mesh = self.path_mesh(
                    list.path(*path),
                    MeshKind::Stroke(
                        style.width.to_bits(),
                        style.cap,
                        style.join,
                        style.miter_limit.to_bits(),
                    ),
                    dpi * state.transform,
                    stats,
                )?;
                self.draw_brush(
                    mesh, brush, state, dpi, size, vertices, indices, draws, stats,
                )?;
            }
            Command::DrawImage {
                image,
                destination,
                options,
            } => {
                if options.opacity == 0.0 {
                    return Ok(());
                }
                let image = list.image(*image);
                let bind = self.image_bind(image, options.sampling, stats)?;
                append(
                    &rect_mesh(*destination),
                    dpi * state.transform,
                    size,
                    [options.opacity * state.opacity; 4],
                    Some(image_uv(image, *options)),
                    vertices,
                    indices,
                    draws,
                    DrawKind::Image(bind),
                    state.scissor,
                    state.clips.len() as u32,
                    stats,
                );
                stats.draws += 1;
            }
            Command::DrawText {
                text,
                origin,
                opacity,
            } => {
                if *opacity == 0.0 {
                    return Ok(());
                }
                let text = list.text(*text);
                let physical_scale = effective_scale(dpi * state.transform);
                let (glyphs, glyph_stats) = self
                    .glyphs
                    .prepare_layout(text, physical_scale)
                    .map_err(|error| RenderError::new(error.to_string()))?;
                stats.glyph_cache_hits += glyph_stats.hits;
                stats.glyph_cache_misses += glyph_stats.misses;
                stats.glyph_uploads += glyph_stats.uploads;
                for run in text.glyph_runs() {
                    for decoration in [run.underline, run.strikethrough].into_iter().flatten() {
                        let rect = Rect::from_xywh(
                            decoration.origin.x + origin.x,
                            decoration.origin.y + origin.y,
                            decoration.size.width,
                            decoration.size.height,
                        );
                        draw_solid(
                            rect_mesh(rect),
                            run.color.with_alpha(run.color.a * *opacity),
                            state,
                            dpi,
                            size,
                            vertices,
                            indices,
                            draws,
                            stats,
                        );
                    }
                }
                for (run_index, glyph) in glyphs {
                    let run = &text.glyph_runs()[run_index];
                    let rect = Rect::from_xywh(
                        glyph.rect.origin.x + origin.x,
                        glyph.rect.origin.y + origin.y,
                        glyph.rect.size.width,
                        glyph.rect.size.height,
                    );
                    let effective_opacity = *opacity * state.opacity;
                    let alpha = (run.color.a * effective_opacity).clamp(0.0, 1.0);
                    let (color, kind) = match glyph.kind {
                        AtlasKind::Mask => (
                            [
                                run.color.r * alpha,
                                run.color.g * alpha,
                                run.color.b * alpha,
                                alpha,
                            ],
                            DrawKind::TextMask(glyph.bind_group),
                        ),
                        AtlasKind::Color => (
                            [effective_opacity; 4],
                            DrawKind::TextColor(glyph.bind_group),
                        ),
                    };
                    append(
                        &rect_mesh(rect),
                        dpi * state.transform,
                        size,
                        color,
                        Some(glyph.uv),
                        vertices,
                        indices,
                        draws,
                        kind,
                        state.scissor,
                        state.clips.len() as u32,
                        stats,
                    );
                    stats.draws += 1;
                }
            }
        }
        Ok(())
    }

    fn path_mesh(
        &mut self,
        path: &Path,
        kind: MeshKind,
        transform: Affine2,
        stats: &mut RenderStats,
    ) -> Result<Mesh, RenderError> {
        let key = MeshKey(path.cache_id(), kind, scale_bucket(transform));
        if let Some(cached) = self.meshes.get_mut(&key) {
            cached.used = self.clock;
            stats.mesh_cache_hits += 1;
            return Ok(cached.mesh.clone());
        }
        stats.mesh_cache_misses += 1;
        let mesh = match kind {
            MeshKind::Fill(rule) => tessellate_fill(path, rule, local_tolerance(transform))?,
            MeshKind::Stroke(width, cap, join, miter) => tessellate_stroke(
                path,
                StrokeStyle {
                    width: f32::from_bits(width),
                    cap,
                    join,
                    miter_limit: f32::from_bits(miter),
                },
                local_tolerance(transform),
            )?,
        };
        if self.options.cache_limits.mesh_bytes > 0 {
            self.meshes.insert(
                key,
                CachedMesh {
                    mesh: mesh.clone(),
                    used: self.clock,
                },
            );
        }
        Ok(mesh)
    }

    fn image_bind(
        &mut self,
        image: &Image,
        sampling: ImageSampling,
        stats: &mut RenderStats,
    ) -> Result<gpu::BindGroup, RenderError> {
        if let Some(cached) = self.images.get_mut(&image.cache_id()) {
            cached.used = self.clock;
            stats.image_cache_hits += 1;
            return Ok(match sampling {
                ImageSampling::Nearest => cached.nearest.clone(),
                ImageSampling::Linear => cached.linear.clone(),
            });
        }
        stats.image_cache_misses += 1;
        let size = image.size();
        if size.width > self.device.capabilities().limits.max_texture_dimension_2d
            || size.height > self.device.capabilities().limits.max_texture_dimension_2d
        {
            return Err(RenderError::new("image exceeds device texture limits"));
        }
        let texture = self.device.create_texture(gpu::TextureDescriptor {
            label: Some("paint image".into()),
            size: gpu::Extent3d::d2(size.width, size.height),
            mip_level_count: 1,
            sample_count: 1,
            dimension: gpu::TextureDimension::D2,
            format: gpu::TextureFormat::Rgba8UnormSrgb,
            usage: gpu::TextureUsages::TEXTURE_BINDING | gpu::TextureUsages::COPY_DST,
        });
        self.queue.write_texture(
            &gpu::TextureCopy {
                texture: texture.clone(),
                mip_level: 0,
                origin: Default::default(),
            },
            image.rgba8(),
            gpu::TextureDataLayout {
                offset: 0,
                bytes_per_row: Some(size.width * 4),
                rows_per_image: Some(size.height),
            },
            gpu::Extent3d::d2(size.width, size.height),
        )?;
        let view = texture.create_view(Default::default());
        let nearest_sampler = self.device.create_sampler(Default::default());
        let linear_sampler = self.device.create_sampler(gpu::SamplerDescriptor {
            mag_filter: gpu::FilterMode::Linear,
            min_filter: gpu::FilterMode::Linear,
            ..Default::default()
        });
        let create = |sampler| {
            self.device.create_bind_group(gpu::BindGroupDescriptor {
                label: Some("paint image bind group".into()),
                layout: self.image_layout.clone(),
                entries: vec![
                    gpu::BindGroupEntry {
                        binding: 0,
                        resource: gpu::BindingResource::TextureView(view.clone()),
                    },
                    gpu::BindGroupEntry {
                        binding: 1,
                        resource: gpu::BindingResource::Sampler(sampler),
                    },
                ],
            })
        };
        let nearest = create(nearest_sampler)?;
        let linear = create(linear_sampler)?;
        let result = match sampling {
            ImageSampling::Nearest => nearest.clone(),
            ImageSampling::Linear => linear.clone(),
        };
        if self.options.cache_limits.image_bytes > 0 {
            self.images.insert(
                image.cache_id(),
                CachedImage {
                    _texture: texture,
                    nearest,
                    linear,
                    bytes: size.width as usize * size.height as usize * 4,
                    used: self.clock,
                },
            );
        }
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_brush(
        &mut self,
        mesh: Mesh,
        brush: &Brush,
        state: &State,
        dpi: Affine2,
        size: Size<Physical, u32>,
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u32>,
        draws: &mut Vec<Draw>,
        stats: &mut RenderStats,
    ) -> Result<(), RenderError> {
        if state.opacity <= 0.0 {
            return Ok(());
        }
        match brush {
            Brush::Solid(color) => draw_solid(
                mesh, *color, state, dpi, size, vertices, indices, draws, stats,
            ),
            Brush::LinearGradient(gradient) => {
                let bind = self.gradient_bind_linear(gradient, stats)?;
                append(
                    &mesh,
                    dpi * state.transform,
                    size,
                    [state.opacity; 4],
                    None,
                    vertices,
                    indices,
                    draws,
                    DrawKind::Gradient(bind),
                    state.scissor,
                    state.clips.len() as u32,
                    stats,
                );
                stats.draws += u32::from(!mesh.indices.is_empty());
            }
            Brush::RadialGradient(gradient) => {
                let bind = self.gradient_bind_radial(gradient, stats)?;
                append(
                    &mesh,
                    dpi * state.transform,
                    size,
                    [state.opacity; 4],
                    None,
                    vertices,
                    indices,
                    draws,
                    DrawKind::Gradient(bind),
                    state.scissor,
                    state.clips.len() as u32,
                    stats,
                );
                stats.draws += u32::from(!mesh.indices.is_empty());
            }
        }
        Ok(())
    }

    fn gradient_bind_linear(
        &mut self,
        gradient: &LinearGradient,
        stats: &mut RenderStats,
    ) -> Result<gpu::BindGroup, RenderError> {
        let start = gradient.start();
        let end = gradient.end();
        self.gradient_bind(
            gradient.cache_id(),
            [
                0.0,
                gradient.stops().len() as f32,
                start.x,
                start.y,
                end.x,
                end.y,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
            ],
            gradient.stops(),
            stats,
        )
    }

    fn gradient_bind_radial(
        &mut self,
        gradient: &RadialGradient,
        stats: &mut RenderStats,
    ) -> Result<gpu::BindGroup, RenderError> {
        let center = gradient.center();
        self.gradient_bind(
            gradient.cache_id(),
            [
                1.0,
                gradient.stops().len() as f32,
                center.x,
                center.y,
                0.0,
                0.0,
                gradient.radius(),
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
            ],
            gradient.stops(),
            stats,
        )
    }

    fn gradient_bind(
        &mut self,
        id: u64,
        header_data: [f32; 12],
        stops: &[astrelis_paint::GradientStop],
        stats: &mut RenderStats,
    ) -> Result<gpu::BindGroup, RenderError> {
        if let Some(cached) = self.gradients.get_mut(&id) {
            cached.used = self.clock;
            stats.gradient_cache_hits += 1;
            return Ok(cached.bind_group.clone());
        }
        stats.gradient_cache_misses += 1;
        let stop_data = stops
            .iter()
            .map(|stop| {
                let alpha = stop.color.a.clamp(0.0, 1.0);
                [
                    stop.color.r * alpha,
                    stop.color.g * alpha,
                    stop.color.b * alpha,
                    alpha,
                    stop.offset,
                    0.0,
                    0.0,
                    0.0,
                ]
            })
            .collect::<Vec<_>>();
        let header = self.device.create_buffer_init(
            &self.queue,
            Some("paint gradient header".into()),
            bytemuck::cast_slice(&header_data),
            gpu::BufferUsages::UNIFORM,
        )?;
        let stop_buffer = self.device.create_buffer_init(
            &self.queue,
            Some("paint gradient stops".into()),
            bytemuck::cast_slice(&stop_data),
            gpu::BufferUsages::STORAGE,
        )?;
        let bind_group = self.device.create_bind_group(gpu::BindGroupDescriptor {
            label: Some("paint gradient bind group".into()),
            layout: self.gradient_layout.clone(),
            entries: vec![
                gpu::BindGroupEntry {
                    binding: 0,
                    resource: gpu::BindingResource::Buffer(gpu::BufferBinding {
                        buffer: header.clone(),
                        offset: 0,
                        size: None,
                    }),
                },
                gpu::BindGroupEntry {
                    binding: 1,
                    resource: gpu::BindingResource::Buffer(gpu::BufferBinding {
                        buffer: stop_buffer.clone(),
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        })?;
        let result = bind_group.clone();
        self.gradients.insert(
            id,
            CachedGradient {
                _header: header,
                _stops: stop_buffer,
                bind_group,
                bytes: size_of::<[f32; 12]>() + stop_data.len() * size_of::<[f32; 8]>(),
                used: self.clock,
            },
        );
        Ok(result)
    }

    fn ensure_pipelines(
        &mut self,
        format: gpu::TextureFormat,
        samples: u32,
    ) -> Result<(), RenderError> {
        let key = PipelineKey(format, samples);
        if self.pipelines.contains_key(&key) {
            return Ok(());
        }
        let shader = self
            .device
            .create_shader_module(gpu::ShaderModuleDescriptor {
                label: Some("paint shader".into()),
                wgsl: SHADER.into(),
            });
        let image_layout = self
            .device
            .create_pipeline_layout(gpu::PipelineLayoutDescriptor {
                label: Some("paint image pipeline layout".into()),
                bind_group_layouts: vec![self.image_layout.clone()],
            })?;
        let text_layout = self
            .device
            .create_pipeline_layout(gpu::PipelineLayoutDescriptor {
                label: Some("paint text pipeline layout".into()),
                bind_group_layouts: vec![self.glyphs.bind_group_layout()],
            })?;
        let gradient_layout =
            self.device
                .create_pipeline_layout(gpu::PipelineLayoutDescriptor {
                    label: Some("paint gradient pipeline layout".into()),
                    bind_group_layouts: vec![self.gradient_layout.clone()],
                })?;
        let vertex = || gpu::VertexState {
            module: shader.clone(),
            entry_point: "vs_main".into(),
            buffers: vec![gpu::VertexBufferLayout {
                array_stride: size_of::<Vertex>() as u64,
                step_mode: gpu::VertexStepMode::Vertex,
                attributes: vec![
                    gpu::VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: gpu::VertexFormat::Float32x2,
                    },
                    gpu::VertexAttribute {
                        offset: 8,
                        shader_location: 1,
                        format: gpu::VertexFormat::Float32x2,
                    },
                    gpu::VertexAttribute {
                        offset: 16,
                        shader_location: 2,
                        format: gpu::VertexFormat::Float32x4,
                    },
                    gpu::VertexAttribute {
                        offset: 32,
                        shader_location: 3,
                        format: gpu::VertexFormat::Float32x2,
                    },
                ],
            }],
        };
        let stencil = |face| gpu::DepthStencilState {
            format: gpu::TextureFormat::Depth24PlusStencil8,
            depth_write_enabled: false,
            depth_compare: gpu::CompareFunction::Always,
            stencil: gpu::StencilState {
                front: face,
                back: face,
                read_mask: 0xff,
                write_mask: 0xff,
            },
            bias_constant: 0,
            bias_slope_scale: 0.0,
            bias_clamp: 0.0,
        };
        let content = gpu::StencilFaceState {
            compare: gpu::CompareFunction::Equal,
            ..gpu::StencilFaceState::IGNORE
        };
        let create = |label: &str,
                      layout: Option<gpu::PipelineLayout>,
                      fragment: &str,
                      writes: gpu::ColorWrites,
                      face: gpu::StencilFaceState| {
            self.device
                .create_render_pipeline(gpu::RenderPipelineDescriptor {
                    label: Some(label.into()),
                    layout,
                    vertex: vertex(),
                    primitive: Default::default(),
                    depth_stencil: Some(stencil(face)),
                    multisample: gpu::MultisampleState {
                        count: samples,
                        ..Default::default()
                    },
                    fragment: Some(gpu::FragmentState {
                        module: shader.clone(),
                        entry_point: fragment.into(),
                        targets: vec![Some(gpu::ColorTargetState {
                            format,
                            blend: Some(gpu::BlendState::PREMULTIPLIED_ALPHA),
                            write_mask: writes,
                        })],
                    }),
                })
        };
        let solid = create(
            "paint solid",
            None,
            "fs_solid",
            gpu::ColorWrites::ALL,
            content,
        )?;
        let gradient = create(
            "paint gradient",
            Some(gradient_layout),
            "fs_gradient",
            gpu::ColorWrites::ALL,
            content,
        )?;
        let image = create(
            "paint image",
            Some(image_layout),
            "fs_image",
            gpu::ColorWrites::ALL,
            content,
        )?;
        let text_mask = create(
            "paint text mask",
            Some(text_layout.clone()),
            "fs_text_mask",
            gpu::ColorWrites::ALL,
            content,
        )?;
        let text_color = create(
            "paint text color",
            Some(text_layout),
            "fs_text_color",
            gpu::ColorWrites::ALL,
            content,
        )?;
        let clip_push = create(
            "paint clip push",
            None,
            "fs_solid",
            gpu::ColorWrites::empty(),
            gpu::StencilFaceState {
                compare: gpu::CompareFunction::Equal,
                pass_op: gpu::StencilOperation::IncrementClamp,
                ..gpu::StencilFaceState::IGNORE
            },
        )?;
        let clip_pop = create(
            "paint clip pop",
            None,
            "fs_solid",
            gpu::ColorWrites::empty(),
            gpu::StencilFaceState {
                compare: gpu::CompareFunction::Equal,
                pass_op: gpu::StencilOperation::DecrementClamp,
                ..gpu::StencilFaceState::IGNORE
            },
        )?;
        self.pipelines.insert(
            key,
            Pipelines {
                solid,
                gradient,
                image,
                text_mask,
                text_color,
                clip_push,
                clip_pop,
            },
        );
        Ok(())
    }

    fn ensure_attachments(
        &mut self,
        size: Size<Physical, u32>,
        format: gpu::TextureFormat,
        samples: u32,
    ) {
        let key = (size.width, size.height, format, samples);
        if self
            .attachments
            .as_ref()
            .is_some_and(|value| value.key == key)
        {
            return;
        }
        let (color_texture, color) = if samples > 1 {
            let texture = self.device.create_texture(gpu::TextureDescriptor {
                label: Some("paint multisample color".into()),
                size: gpu::Extent3d::d2(size.width, size.height),
                mip_level_count: 1,
                sample_count: samples,
                dimension: gpu::TextureDimension::D2,
                format,
                usage: gpu::TextureUsages::RENDER_ATTACHMENT,
            });
            let view = texture.create_view(Default::default());
            (Some(texture), Some(view))
        } else {
            (None, None)
        };
        let stencil_texture = self.device.create_texture(gpu::TextureDescriptor {
            label: Some("paint stencil".into()),
            size: gpu::Extent3d::d2(size.width, size.height),
            mip_level_count: 1,
            sample_count: samples,
            dimension: gpu::TextureDimension::D2,
            format: gpu::TextureFormat::Depth24PlusStencil8,
            usage: gpu::TextureUsages::RENDER_ATTACHMENT,
        });
        let stencil = stencil_texture.create_view(Default::default());
        self.attachments = Some(Attachments {
            key,
            _color: color_texture,
            color,
            _stencil: stencil_texture,
            stencil,
        });
    }

    fn upload(&mut self, vertices: &[Vertex], indices: &[u32]) -> Result<(), RenderError> {
        if vertices.is_empty() {
            return Ok(());
        }
        ensure_buffer(
            &self.device,
            &self.queue,
            &mut self.vertex_buffer,
            bytemuck::cast_slice(vertices),
            gpu::BufferUsages::VERTEX,
            "paint vertices",
        )?;
        ensure_buffer(
            &self.device,
            &self.queue,
            &mut self.index_buffer,
            bytemuck::cast_slice(indices),
            gpu::BufferUsages::INDEX,
            "paint indices",
        )
    }

    fn evict(&mut self) {
        while self
            .meshes
            .values()
            .map(|entry| entry.mesh.bytes())
            .sum::<usize>()
            > self.options.cache_limits.mesh_bytes
        {
            let Some(key) = self
                .meshes
                .iter()
                .min_by_key(|(_, entry)| entry.used)
                .map(|(key, _)| *key)
            else {
                break;
            };
            self.meshes.remove(&key);
        }
        while self.images.values().map(|entry| entry.bytes).sum::<usize>()
            > self.options.cache_limits.image_bytes
        {
            let Some(key) = self
                .images
                .iter()
                .min_by_key(|(_, entry)| entry.used)
                .map(|(key, _)| *key)
            else {
                break;
            };
            self.images.remove(&key);
        }
        while self
            .gradients
            .values()
            .map(|entry| entry.bytes)
            .sum::<usize>()
            > self.options.cache_limits.gradient_bytes
        {
            let Some(key) = self
                .gradients
                .iter()
                .min_by_key(|(_, entry)| entry.used)
                .map(|(key, _)| *key)
            else {
                break;
            };
            self.gradients.remove(&key);
        }
    }
}

fn ensure_buffer(
    device: &gpu::Device,
    queue: &gpu::Queue,
    slot: &mut Option<FrameBuffer>,
    bytes: &[u8],
    usage: gpu::BufferUsages,
    label: &str,
) -> Result<(), RenderError> {
    if slot
        .as_ref()
        .is_none_or(|value| value.capacity < bytes.len())
    {
        let capacity = bytes.len().next_power_of_two().max(256);
        *slot = Some(FrameBuffer {
            buffer: device.create_buffer(gpu::BufferDescriptor {
                label: Some(label.into()),
                size: capacity as u64,
                usage: usage | gpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            capacity,
        });
    }
    queue.write_buffer(&slot.as_ref().expect("buffer exists").buffer, 0, bytes)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn push_clip(
    mesh: Mesh,
    state: &mut State,
    dpi: Affine2,
    size: Size<Physical, u32>,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    draws: &mut Vec<Draw>,
    stats: &mut RenderStats,
) -> Result<(), RenderError> {
    if state.clips.len() == 255 {
        return Err(RenderError::new("complex clip depth exceeds 255"));
    }
    append(
        &mesh,
        dpi * state.transform,
        size,
        [0.0; 4],
        None,
        vertices,
        indices,
        draws,
        DrawKind::ClipPush,
        state.scissor,
        state.clips.len() as u32,
        stats,
    );
    state.clips.push(Clip {
        mesh,
        transform: state.transform,
        scissor: state.scissor,
    });
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn draw_solid(
    mesh: Mesh,
    color: Color,
    state: &State,
    dpi: Affine2,
    size: Size<Physical, u32>,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    draws: &mut Vec<Draw>,
    stats: &mut RenderStats,
) {
    if color.a <= 0.0 {
        return;
    }
    let alpha = (color.a * state.opacity).clamp(0.0, 1.0);
    append(
        &mesh,
        dpi * state.transform,
        size,
        [color.r * alpha, color.g * alpha, color.b * alpha, alpha],
        None,
        vertices,
        indices,
        draws,
        DrawKind::Solid,
        state.scissor,
        state.clips.len() as u32,
        stats,
    );
    stats.draws += u32::from(!mesh.indices.is_empty());
}

#[allow(clippy::too_many_arguments)]
fn append(
    mesh: &Mesh,
    transform: Affine2,
    size: Size<Physical, u32>,
    color: [f32; 4],
    uv_rect: Option<[f32; 4]>,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    draws: &mut Vec<Draw>,
    kind: DrawKind,
    scissor: Scissor,
    stencil: u32,
    stats: &mut RenderStats,
) {
    if mesh.indices.is_empty() {
        return;
    }
    let bounds = mesh_bounds(mesh);
    let base = vertices.len() as u32;
    let first = indices.len() as u32;
    for point in &mesh.vertices {
        let physical = transform.transform_point2(Vec2::from_array(*point));
        let uv = uv_rect.map_or([0.0; 2], |rect| {
            let x = normalized(point[0], bounds[0], bounds[2]);
            let y = normalized(point[1], bounds[1], bounds[3]);
            [
                rect[0] + (rect[2] - rect[0]) * x,
                rect[1] + (rect[3] - rect[1]) * y,
            ]
        });
        vertices.push(Vertex {
            position: [
                physical.x / size.width as f32 * 2.0 - 1.0,
                1.0 - physical.y / size.height as f32 * 2.0,
            ],
            uv,
            color,
            local_position: *point,
        });
    }
    indices.extend(mesh.indices.iter().map(|index| base + index));
    draws.push(Draw {
        kind,
        indices: first..indices.len() as u32,
        scissor,
        stencil,
    });
    stats.triangles += mesh.indices.len() as u32 / 3;
}

fn normalized(value: f32, min: f32, max: f32) -> f32 {
    if max > min {
        (value - min) / (max - min)
    } else {
        0.0
    }
}

fn mesh_bounds(mesh: &Mesh) -> [f32; 4] {
    mesh.vertices.iter().fold(
        [
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ],
        |mut bounds, point| {
            bounds[0] = bounds[0].min(point[0]);
            bounds[1] = bounds[1].min(point[1]);
            bounds[2] = bounds[2].max(point[0]);
            bounds[3] = bounds[3].max(point[1]);
            bounds
        },
    )
}

fn rect_mesh(rect: LogicalRect) -> Mesh {
    Mesh {
        vertices: vec![
            [rect.min_x(), rect.min_y()],
            [rect.max_x(), rect.min_y()],
            [rect.max_x(), rect.max_y()],
            [rect.min_x(), rect.max_y()],
        ],
        indices: if rect.size.width == 0.0 || rect.size.height == 0.0 {
            Vec::new()
        } else {
            vec![0, 1, 2, 0, 2, 3]
        },
    }
}

fn to_lyon(path: &Path) -> LyonPath {
    let mut builder = LyonPath::builder();
    let mut active = false;
    for verb in path.verbs() {
        match *verb {
            PathVerb::MoveTo(value) => {
                if active {
                    builder.end(false);
                }
                builder.begin(point(value.x, value.y));
                active = true;
            }
            PathVerb::LineTo(value) => {
                builder.line_to(point(value.x, value.y));
            }
            PathVerb::QuadTo(control, value) => {
                builder.quadratic_bezier_to(point(control.x, control.y), point(value.x, value.y));
            }
            PathVerb::CubicTo(a, b, value) => {
                builder.cubic_bezier_to(point(a.x, a.y), point(b.x, b.y), point(value.x, value.y));
            }
            PathVerb::Close => {
                builder.close();
                active = false;
            }
        }
    }
    if active {
        builder.end(false);
    }
    builder.build()
}

fn tessellate_fill(path: &Path, rule: FillRule, tolerance: f32) -> Result<Mesh, RenderError> {
    let path = to_lyon(path);
    let mut buffers: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
    FillTessellator::new()
        .tessellate_path(
            &path,
            &FillOptions::default()
                .with_tolerance(tolerance)
                .with_fill_rule(match rule {
                    FillRule::NonZero => lyon_tessellation::FillRule::NonZero,
                    FillRule::EvenOdd => lyon_tessellation::FillRule::EvenOdd,
                }),
            &mut BuffersBuilder::new(&mut buffers, |vertex: FillVertex<'_>| {
                vertex.position().to_array()
            }),
        )
        .map_err(|error| RenderError::new(format!("fill tessellation failed: {error:?}")))?;
    Ok(Mesh {
        vertices: buffers.vertices,
        indices: buffers.indices,
    })
}

fn tessellate_stroke(path: &Path, style: StrokeStyle, tolerance: f32) -> Result<Mesh, RenderError> {
    if style.width == 0.0 {
        return Ok(Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
        });
    }
    let path = to_lyon(path);
    let cap = match style.cap {
        LineCap::Butt => lyon_tessellation::LineCap::Butt,
        LineCap::Square => lyon_tessellation::LineCap::Square,
        LineCap::Round => lyon_tessellation::LineCap::Round,
    };
    let join = match style.join {
        LineJoin::Miter => lyon_tessellation::LineJoin::Miter,
        LineJoin::Bevel => lyon_tessellation::LineJoin::Bevel,
        LineJoin::Round => lyon_tessellation::LineJoin::Round,
    };
    let options = StrokeOptions::default()
        .with_line_width(style.width)
        .with_tolerance(tolerance)
        .with_start_cap(cap)
        .with_end_cap(cap)
        .with_line_join(join)
        .with_miter_limit(style.miter_limit);
    let mut buffers: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
    StrokeTessellator::new()
        .tessellate_path(
            &path,
            &options,
            &mut BuffersBuilder::new(&mut buffers, |vertex: StrokeVertex<'_, '_>| {
                vertex.position().to_array()
            }),
        )
        .map_err(|error| RenderError::new(format!("stroke tessellation failed: {error:?}")))?;
    Ok(Mesh {
        vertices: buffers.vertices,
        indices: buffers.indices,
    })
}

fn rounded_path(rect: RoundedRect) -> Result<Path, RenderError> {
    let r = rect.rect();
    let CornerRadii {
        top_left,
        top_right,
        bottom_right,
        bottom_left,
    } = rect.radii();
    let k = 0.552_284_8;
    let mut path = astrelis_paint::Path::builder();
    let p = astrelis_core::geometry::Point::new;
    path.move_to(p(r.min_x() + top_left, r.min_y()))
        .map_err(paint_error)?;
    path.line_to(p(r.max_x() - top_right, r.min_y()))
        .map_err(paint_error)?;
    path.cubic_to(
        p(r.max_x() - top_right + top_right * k, r.min_y()),
        p(r.max_x(), r.min_y() + top_right - top_right * k),
        p(r.max_x(), r.min_y() + top_right),
    )
    .map_err(paint_error)?;
    path.line_to(p(r.max_x(), r.max_y() - bottom_right))
        .map_err(paint_error)?;
    path.cubic_to(
        p(r.max_x(), r.max_y() - bottom_right + bottom_right * k),
        p(r.max_x() - bottom_right + bottom_right * k, r.max_y()),
        p(r.max_x() - bottom_right, r.max_y()),
    )
    .map_err(paint_error)?;
    path.line_to(p(r.min_x() + bottom_left, r.max_y()))
        .map_err(paint_error)?;
    path.cubic_to(
        p(r.min_x() + bottom_left - bottom_left * k, r.max_y()),
        p(r.min_x(), r.max_y() - bottom_left + bottom_left * k),
        p(r.min_x(), r.max_y() - bottom_left),
    )
    .map_err(paint_error)?;
    path.line_to(p(r.min_x(), r.min_y() + top_left))
        .map_err(paint_error)?;
    path.cubic_to(
        p(r.min_x(), r.min_y() + top_left - top_left * k),
        p(r.min_x() + top_left - top_left * k, r.min_y()),
        p(r.min_x() + top_left, r.min_y()),
    )
    .map_err(paint_error)?;
    path.close().map_err(paint_error)?;
    Ok(path.finish())
}

fn rounded_mesh(rect: RoundedRect, tolerance: f32) -> Result<Mesh, RenderError> {
    tessellate_fill(&rounded_path(rect)?, FillRule::NonZero, tolerance)
}

fn rect_path(rect: LogicalRect) -> Result<Path, RenderError> {
    let p = astrelis_core::geometry::Point::new;
    let mut path = Path::builder();
    path.move_to(p(rect.min_x(), rect.min_y()))
        .map_err(paint_error)?;
    path.line_to(p(rect.max_x(), rect.min_y()))
        .map_err(paint_error)?;
    path.line_to(p(rect.max_x(), rect.max_y()))
        .map_err(paint_error)?;
    path.line_to(p(rect.min_x(), rect.max_y()))
        .map_err(paint_error)?;
    path.close().map_err(paint_error)?;
    Ok(path.finish())
}

fn ellipse_path(rect: LogicalRect) -> Result<Path, RenderError> {
    let center_x = rect.origin.x + rect.size.width * 0.5;
    let center_y = rect.origin.y + rect.size.height * 0.5;
    let radius_x = rect.size.width * 0.5;
    let radius_y = rect.size.height * 0.5;
    let k = 0.552_284_8;
    let p = astrelis_core::geometry::Point::new;
    let mut path = Path::builder();
    path.move_to(p(center_x + radius_x, center_y))
        .map_err(paint_error)?;
    path.cubic_to(
        p(center_x + radius_x, center_y + radius_y * k),
        p(center_x + radius_x * k, center_y + radius_y),
        p(center_x, center_y + radius_y),
    )
    .map_err(paint_error)?;
    path.cubic_to(
        p(center_x - radius_x * k, center_y + radius_y),
        p(center_x - radius_x, center_y + radius_y * k),
        p(center_x - radius_x, center_y),
    )
    .map_err(paint_error)?;
    path.cubic_to(
        p(center_x - radius_x, center_y - radius_y * k),
        p(center_x - radius_x * k, center_y - radius_y),
        p(center_x, center_y - radius_y),
    )
    .map_err(paint_error)?;
    path.cubic_to(
        p(center_x + radius_x * k, center_y - radius_y),
        p(center_x + radius_x, center_y - radius_y * k),
        p(center_x + radius_x, center_y),
    )
    .map_err(paint_error)?;
    path.close().map_err(paint_error)?;
    Ok(path.finish())
}

fn shape_stroke_mesh(path: Path, style: StrokeStyle, tolerance: f32) -> Result<Mesh, RenderError> {
    tessellate_stroke(&path, style, tolerance)
}

fn ellipse_mesh(
    rect: LogicalRect,
    tolerance: f32,
    stroke: Option<StrokeStyle>,
) -> Result<Mesh, RenderError> {
    let path = ellipse_path(rect)?;
    match stroke {
        Some(style) => tessellate_stroke(&path, style, tolerance),
        None => tessellate_fill(&path, FillRule::NonZero, tolerance),
    }
}

fn paint_error(value: astrelis_paint::PaintError) -> RenderError {
    RenderError::new(value.to_string())
}

fn image_uv(image: &Image, options: ImageOptions) -> [f32; 4] {
    let size = image.size();
    let source = options
        .source
        .unwrap_or_else(|| Rect::from_xywh(0.0, 0.0, size.width as f32, size.height as f32));
    [
        source.min_x() / size.width as f32,
        source.min_y() / size.height as f32,
        source.max_x() / size.width as f32,
        source.max_y() / size.height as f32,
    ]
}

fn exact_scissor(
    rect: LogicalRect,
    transform: Affine2,
    size: Size<Physical, u32>,
) -> Option<Scissor> {
    let points = [
        transform.transform_point2(Vec2::new(rect.min_x(), rect.min_y())),
        transform.transform_point2(Vec2::new(rect.max_x(), rect.min_y())),
        transform.transform_point2(Vec2::new(rect.max_x(), rect.max_y())),
        transform.transform_point2(Vec2::new(rect.min_x(), rect.max_y())),
    ];
    let e = 1e-4;
    if (points[0].y - points[1].y).abs() >= e
        || (points[1].x - points[2].x).abs() >= e
        || (points[2].y - points[3].y).abs() >= e
        || (points[3].x - points[0].x).abs() >= e
    {
        return None;
    }
    let min_x = points.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
    let min_y = points.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
    let max_x = points.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
    let max_y = points.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
    if [min_x, min_y, max_x, max_y]
        .into_iter()
        .any(|v| (v - v.round()).abs() >= e)
    {
        return None;
    }
    let x0 = min_x.round().clamp(0.0, size.width as f32) as u32;
    let y0 = min_y.round().clamp(0.0, size.height as f32) as u32;
    let x1 = max_x.round().clamp(0.0, size.width as f32) as u32;
    let y1 = max_y.round().clamp(0.0, size.height as f32) as u32;
    Some(Scissor {
        x: x0.min(x1),
        y: y0.min(y1),
        width: x0.abs_diff(x1),
        height: y0.abs_diff(y1),
    })
}

fn effective_scale(transform: Affine2) -> f32 {
    let matrix = transform.matrix2;
    let a = matrix.x_axis.length_squared();
    let b = matrix.y_axis.length_squared();
    let c = matrix.x_axis.dot(matrix.y_axis);
    let d = ((a - b) * (a - b) + 4.0 * c * c).sqrt();
    ((a + b + d) * 0.5).sqrt().max(1e-6)
}

fn scale_bucket(transform: Affine2) -> i16 {
    effective_scale(transform).log2().ceil().clamp(-32.0, 32.0) as i16
}

fn local_tolerance(transform: Affine2) -> f32 {
    (0.25 / 2.0_f32.powi(scale_bucket(transform) as i32)).max(1e-5)
}

const SHADER: &str = r#"
struct Input {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) local_position: vec2<f32>,
};
struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) local_position: vec2<f32>,
};
@vertex fn vs_main(input: Input) -> Output {
    var output: Output;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.uv = input.uv;
    output.color = input.color;
    output.local_position = input.local_position;
    return output;
}
@fragment fn fs_solid(input: Output) -> @location(0) vec4<f32> {
    return input.color;
}
struct GradientHeader {
    metadata_and_start: vec4<f32>,
    end_and_radius: vec4<f32>,
    reserved: vec4<f32>,
};
struct GradientStopData {
    color: vec4<f32>,
    offset: vec4<f32>,
};
@group(0) @binding(0) var<uniform> gradient: GradientHeader;
@group(0) @binding(1) var<storage, read> gradient_stops: array<GradientStopData>;
@fragment fn fs_gradient(input: Output) -> @location(0) vec4<f32> {
    let kind = gradient.metadata_and_start.x;
    let count = u32(gradient.metadata_and_start.y);
    let start = gradient.metadata_and_start.zw;
    var position = 0.0;
    if kind < 0.5 {
        let direction = gradient.end_and_radius.xy - start;
        position = dot(input.local_position - start, direction) / dot(direction, direction);
    } else {
        position = distance(input.local_position, start) / gradient.end_and_radius.z;
    }
    let t = clamp(position, 0.0, 1.0);
    var color = gradient_stops[0].color;
    for (var index = 1u; index < count; index += 1u) {
        let previous = gradient_stops[index - 1u];
        let next = gradient_stops[index];
        if t <= next.offset.x {
            let span = next.offset.x - previous.offset.x;
            let amount = select(1.0, clamp((t - previous.offset.x) / span, 0.0, 1.0), span > 0.0);
            color = mix(previous.color, next.color, amount);
            break;
        }
        color = next.color;
    }
    return color * input.color.a;
}
@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;
@fragment fn fs_image(input: Output) -> @location(0) vec4<f32> {
    let sample = textureSample(image, image_sampler, input.uv);
    return vec4<f32>(sample.rgb * sample.a, sample.a) * input.color.a;
}
@fragment fn fs_text_mask(input: Output) -> @location(0) vec4<f32> {
    let coverage = textureSample(image, image_sampler, input.uv).r;
    return input.color * coverage;
}
@fragment fn fs_text_color(input: Output) -> @location(0) vec4<f32> {
    let sample = textureSample(image, image_sampler, input.uv);
    return vec4<f32>(sample.rgb * sample.a, sample.a) * input.color.a;
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_bucket_ignores_translation() {
        assert_eq!(
            scale_bucket(Affine2::IDENTITY),
            scale_bucket(Affine2::from_translation(Vec2::new(10.0, 20.0)))
        );
    }

    #[test]
    fn scissor_requires_pixel_alignment() {
        let size = Size::new(100, 100);
        assert!(
            exact_scissor(Rect::from_xywh(1.0, 2.0, 3.0, 4.0), Affine2::IDENTITY, size).is_some()
        );
        assert!(
            exact_scissor(Rect::from_xywh(1.5, 2.0, 3.0, 4.0), Affine2::IDENTITY, size).is_none()
        );
    }
}
