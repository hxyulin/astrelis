//! Bitmap-only text renderer.
//!
//! This module provides [`BitmapTextRenderer`], a lightweight text renderer
//! that uses only bitmap glyph atlas (~8 MB with default atlas size).
//!
//! # When to Use
//!
//! Use `BitmapTextRenderer` when:
//! - You only need small text (< 24px)
//! - You don't need text effects (shadows, outlines, glows)
//! - Memory is constrained
//!
//! # Memory Usage
//!
//! | Config | Atlas Size | GPU Memory | CPU Memory | Total |
//! |--------|------------|------------|------------|-------|
//! | small() | 512x512 | ~0.25 MB | ~0.25 MB | ~0.5 MB |
//! | medium() | 1024x1024 | ~1 MB | ~1 MB | ~2 MB |
//! | large() | 2048x2048 | ~4 MB | ~4 MB | ~8 MB |
//!
//! # Example
//!
//! ```ignore
//! use astrelis_text::{BitmapTextRenderer, Text, FontSystem};
//! use astrelis_core::math::Vec2;
//!
//! let font_system = FontSystem::with_system_fonts();
//! let mut renderer = BitmapTextRenderer::new(context, font_system);
//!
//! let text = Text::new("Hello, World!")
//!     .size(14.0)
//!     .color(Color::WHITE);
//!
//! let mut buffer = renderer.prepare(&text);
//! renderer.draw_text(&mut buffer, Vec2::new(10.0, 10.0));
//! renderer.render(&mut render_pass);
//! ```

use std::sync::Arc;

use astrelis_core::alloc::HashMap;
use astrelis_core::math::Vec2;
use astrelis_core::profiling::profile_function;
use cosmic_text::{CacheKey, Color as CosmicColor, Metrics};

use astrelis_render::{AsWgpu, GpuTexture, GraphicsContext, Viewport, wgpu};

use crate::font::FontSystem;
use crate::text::{Text, TextMetrics};

use super::orthographic_projection;
use super::shared::{
    AtlasEntry, AtlasPacker, GlyphPlacement, SharedContext, TextBuffer, TextRender,
    TextRendererConfig, TextVertex,
};

/// Bitmap text renderer backend.
///
/// Manages the bitmap glyph atlas and rendering pipeline.
pub(crate) struct BitmapBackend {
    // GPU resources
    pub(crate) pipeline: wgpu::RenderPipeline,
    #[allow(dead_code)]
    pub(crate) bind_group_layout: wgpu::BindGroupLayout,
    /// GPU texture with cached view and metadata.
    pub(crate) atlas: GpuTexture,
    pub(crate) sampler: wgpu::Sampler,
    pub(crate) bind_group: wgpu::BindGroup,

    // Atlas management
    pub(crate) atlas_data: Vec<u8>,
    pub(crate) atlas_entries: HashMap<CacheKey, AtlasEntry>,
    pub(crate) atlas_packer: AtlasPacker,
    pub(crate) atlas_dirty: bool,
}

impl BitmapBackend {
    /// Create a new bitmap backend.
    pub fn new(shared: &SharedContext, atlas_size: u32) -> Self {
        let renderer = &shared.renderer;

        // Create shader
        let shader =
            renderer.create_shader(Some("Text Shader"), include_str!("../../shaders/text.wgsl"));

        // Create atlas texture using GpuTexture
        let atlas = renderer.create_gpu_texture_2d(
            Some("Text Atlas"),
            atlas_size,
            atlas_size,
            wgpu::TextureFormat::R8Unorm,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );

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
                    resource: wgpu::BindingResource::TextureView(atlas.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        );

        // Create pipeline layout
        let pipeline_layout = renderer.create_pipeline_layout(
            Some("Text Pipeline Layout"),
            &[&bind_group_layout, &shared.uniform_bind_group_layout],
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

        Self {
            pipeline,
            bind_group_layout,
            atlas,
            sampler,
            bind_group,
            atlas_data,
            atlas_entries: HashMap::new(),
            atlas_packer: AtlasPacker::new(atlas_size),
            atlas_dirty: false,
        }
    }

    /// Ensure a glyph is in the atlas, rasterizing and uploading if needed.
    pub fn ensure_glyph(&mut self, shared: &SharedContext, cache_key: CacheKey) -> Option<&AtlasEntry> {
        // Check if already in atlas
        if self.atlas_entries.contains_key(&cache_key) {
            return self.atlas_entries.get(&cache_key);
        }

        // Rasterize the glyph
        let mut font_system = shared.font_system.write().unwrap();
        let mut swash_cache = shared.swash_cache.write().unwrap();
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
        let atlas_size = self.atlas.width();
        for y in 0..height {
            for x in 0..width {
                let src_idx = (y * width + x) as usize;
                let dst_idx = ((entry.y + y) * atlas_size + (entry.x + x)) as usize;
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
    pub fn upload_atlas(&mut self, shared: &SharedContext) {
        if !self.atlas_dirty {
            return;
        }

        let atlas_size = self.atlas.width();
        shared.renderer.queue().write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: self.atlas.as_wgpu(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &self.atlas_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(atlas_size),
                rows_per_image: Some(atlas_size),
            },
            wgpu::Extent3d {
                width: atlas_size,
                height: atlas_size,
                depth_or_array_layers: 1,
            },
        );

        self.atlas_dirty = false;
    }

    /// Clear the atlas (called when scale factor changes).
    pub fn clear(&mut self) {
        self.atlas_entries.clear();
        self.atlas_packer.reset();
        self.atlas_dirty = true;
    }
}

/// Bitmap-only text renderer.
///
/// Uses only a bitmap glyph atlas for rendering, with no SDF support.
/// This provides the lowest memory footprint (~8 MB with default atlas).
///
/// Best for UI text, labels, and other small text that doesn't need effects.
pub struct BitmapTextRenderer {
    shared: SharedContext,
    backend: BitmapBackend,

    // Staging data
    vertices: Vec<TextVertex>,
    indices: Vec<u16>,
}

impl BitmapTextRenderer {
    /// Create a new bitmap text renderer with default configuration.
    pub fn new(context: Arc<GraphicsContext>, font_system: FontSystem) -> Self {
        Self::with_config(context, font_system, TextRendererConfig::default())
    }

    /// Create a new bitmap text renderer with custom configuration.
    pub fn with_config(
        context: Arc<GraphicsContext>,
        font_system: FontSystem,
        config: TextRendererConfig,
    ) -> Self {
        let shared = SharedContext::new(context, font_system.inner());
        let backend = BitmapBackend::new(&shared, config.atlas_size);

        Self {
            shared,
            backend,
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// Measure text dimensions without rendering.
    pub fn measure_text(&self, text: &Text) -> (f32, f32) {
        profile_function!();
        let scale = self.shared.scale_factor();
        let mut font_system = self.shared.font_system.write().unwrap();
        let mut buffer = TextBuffer::new(&mut font_system);
        buffer.set_text(&mut font_system, text, scale);
        buffer.layout(&mut font_system);
        let (width, height) = buffer.bounds();
        (width / scale, height / scale)
    }

    /// Get the logical (unscaled) bounds of a prepared text buffer.
    pub fn buffer_bounds(&self, buffer: &TextBuffer) -> (f32, f32) {
        let scale = self.shared.scale_factor();
        let (width, height) = buffer.bounds();
        (width / scale, height / scale)
    }

    /// Get font metrics for the given text style.
    pub fn get_text_metrics(&self, text: &Text) -> TextMetrics {
        profile_function!();
        let scale = self.shared.scale_factor();
        let font_size = text.get_font_size();
        let line_height_multiplier = text.get_line_height();

        let metrics = Metrics::new(
            font_size * scale,
            font_size * scale * line_height_multiplier,
        );

        let line_height = metrics.line_height / scale;
        let ascent = font_size * 0.8;
        let descent = font_size * 0.2;

        TextMetrics {
            ascent,
            descent,
            line_height,
            baseline_offset: ascent,
        }
    }

    /// Set the viewport for rendering.
    pub fn set_viewport(&mut self, viewport: Viewport) {
        if viewport.scale_factor != self.shared.viewport.scale_factor {
            tracing::trace!(
                "BitmapTextRenderer scale factor changed: {:?} -> {:?}",
                self.shared.viewport.scale_factor,
                viewport.scale_factor
            );
            self.backend.clear();
        }
        self.shared.set_viewport(viewport);
    }

    /// Prepare text for rendering.
    pub fn prepare(&mut self, text: &Text) -> TextBuffer {
        profile_function!();
        let mut font_system = self.shared.font_system.write().unwrap();
        let mut buffer = TextBuffer::new(&mut font_system);
        buffer.set_text(&mut font_system, text, self.shared.scale_factor());
        buffer.layout(&mut font_system);
        buffer
    }

    /// Draw text at a position.
    pub fn draw_text(&mut self, buffer: &mut TextBuffer, position: Vec2) {
        profile_function!();

        let scale = self.shared.scale_factor();
        let mut font_system = self.shared.font_system.write().unwrap();
        buffer.layout(&mut font_system);
        drop(font_system);

        // Render glyphs
        for run in buffer.buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                let physical_glyph =
                    glyph.physical((position.x * scale, position.y * scale + run.line_y), 1.0);
                let cache_key = physical_glyph.cache_key;

                // Ensure glyph is in atlas
                let entry = match self.backend.ensure_glyph(&self.shared, cache_key) {
                    Some(e) => e.clone(),
                    None => continue,
                };

                // Get glyph placement info
                let mut font_system = self.shared.font_system.write().unwrap();
                let mut swash_cache = self.shared.swash_cache.write().unwrap();

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

                    let (u0, v0, u1, v1) = entry.uv_coords(self.backend.atlas.width());

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
    }

    /// Render all queued text to the given render pass.
    pub fn render(&mut self, render_pass: &mut wgpu::RenderPass) {
        profile_function!();

        debug_assert!(
            self.shared.viewport.is_valid(),
            "Viewport size must be set before rendering text."
        );

        if self.vertices.is_empty() {
            return;
        }

        self.backend.upload_atlas(&self.shared);

        // Create buffers
        let vertex_buffer = self
            .shared
            .renderer
            .create_vertex_buffer(Some("Text Vertex Buffer"), &self.vertices);

        let index_buffer = self
            .shared
            .renderer
            .create_index_buffer(Some("Text Index Buffer"), &self.indices);

        // Create projection uniform
        let size = self.shared.viewport.to_logical();
        let projection = orthographic_projection(size.width, size.height);
        let uniform_buffer = self
            .shared
            .renderer
            .create_uniform_buffer(Some("Text Projection"), &projection);

        // Create uniform bind group
        let uniform_bind_group = self.shared.renderer.create_bind_group(
            Some("Text Uniform Bind Group"),
            &self.shared.uniform_bind_group_layout,
            &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        );

        // Render
        render_pass.set_pipeline(&self.backend.pipeline);
        render_pass.set_bind_group(0, &self.backend.bind_group, &[]);
        render_pass.set_bind_group(1, &uniform_bind_group, &[]);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.indices.len() as u32, 0, 0..1);

        // Clear for next frame
        self.vertices.clear();
        self.indices.clear();
    }

    /// Get the font system.
    pub fn font_system(&self) -> std::sync::Arc<std::sync::RwLock<cosmic_text::FontSystem>> {
        self.shared.font_system.clone()
    }

    /// Get the swash cache.
    pub fn swash_cache(&self) -> std::sync::Arc<std::sync::RwLock<cosmic_text::SwashCache>> {
        self.shared.swash_cache.clone()
    }

    /// Get the atlas size in pixels.
    pub fn atlas_size(&self) -> u32 {
        self.backend.atlas.width()
    }

    /// Get the atlas texture view for binding.
    pub fn atlas_texture_view(&self) -> &wgpu::TextureView {
        self.backend.atlas.view()
    }

    /// Get the atlas sampler for binding.
    pub fn atlas_sampler(&self) -> &wgpu::Sampler {
        &self.backend.sampler
    }

    /// Check if the atlas has pending changes.
    pub fn is_atlas_dirty(&self) -> bool {
        self.backend.atlas_dirty
    }

    /// Upload atlas data to GPU if dirty.
    pub fn upload_atlas_if_dirty(&mut self) {
        profile_function!();
        self.backend.upload_atlas(&self.shared);
    }

    /// Ensure a glyph is in the atlas using a cache key.
    pub fn ensure_glyph_in_atlas(&mut self, cache_key: CacheKey) -> Option<&AtlasEntry> {
        self.backend.ensure_glyph(&self.shared, cache_key)
    }

    /// Get glyph placement information.
    pub fn get_glyph_placement(&mut self, cache_key: CacheKey) -> Option<GlyphPlacement> {
        let mut font_system = self.shared.font_system.write().unwrap();
        let mut swash_cache = self.shared.swash_cache.write().unwrap();

        let image = swash_cache
            .get_image(&mut font_system, cache_key)
            .as_ref()?;

        let scale = self.shared.scale_factor();

        Some(GlyphPlacement {
            left: image.placement.left as f32 / scale,
            top: image.placement.top as f32 / scale,
            width: image.placement.width as f32 / scale,
            height: image.placement.height as f32 / scale,
        })
    }

    /// Ensure a glyph is in the atlas and get its placement info.
    pub fn ensure_glyph_with_placement(
        &mut self,
        cache_key: CacheKey,
    ) -> Option<(AtlasEntry, GlyphPlacement)> {
        let atlas_entry = self.backend.ensure_glyph(&self.shared, cache_key)?.clone();

        let mut font_system = self.shared.font_system.write().unwrap();
        let mut swash_cache = self.shared.swash_cache.write().unwrap();

        let image = swash_cache
            .get_image(&mut font_system, cache_key)
            .as_ref()?;

        let scale = self.shared.scale_factor();

        let placement = GlyphPlacement {
            left: image.placement.left as f32 / scale,
            top: image.placement.top as f32 / scale,
            width: image.placement.width as f32 / scale,
            height: image.placement.height as f32 / scale,
        };

        Some((atlas_entry, placement))
    }

    /// Get an atlas entry by cache key (if it exists).
    pub fn get_atlas_entry(&self, cache_key: CacheKey) -> Option<&AtlasEntry> {
        self.backend.atlas_entries.get(&cache_key)
    }
}

impl TextRender for BitmapTextRenderer {
    fn prepare(&mut self, text: &Text) -> TextBuffer {
        BitmapTextRenderer::prepare(self, text)
    }

    fn draw_text(&mut self, buffer: &mut TextBuffer, position: Vec2) {
        BitmapTextRenderer::draw_text(self, buffer, position)
    }

    fn render(&mut self, render_pass: &mut wgpu::RenderPass) {
        BitmapTextRenderer::render(self, render_pass)
    }

    fn measure_text(&self, text: &Text) -> (f32, f32) {
        BitmapTextRenderer::measure_text(self, text)
    }

    fn set_viewport(&mut self, viewport: Viewport) {
        BitmapTextRenderer::set_viewport(self, viewport)
    }

    fn buffer_bounds(&self, buffer: &TextBuffer) -> (f32, f32) {
        BitmapTextRenderer::buffer_bounds(self, buffer)
    }
}
