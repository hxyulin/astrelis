//! SDF-only text renderer.
//!
//! This module provides [`SdfTextRenderer`], a text renderer that uses only
//! Signed Distance Field (SDF) glyph atlas (~8 MB with default atlas size).
//!
//! # When to Use
//!
//! Use `SdfTextRenderer` when:
//! - You need text effects (shadows, outlines, glows)
//! - You need text that scales smoothly at any size
//! - You're primarily rendering large text (24px+)
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
//! use astrelis_text::{SdfTextRenderer, Text, TextEffectsBuilder, FontSystem, Color};
//! use astrelis_core::math::Vec2;
//!
//! let font_system = FontSystem::with_system_fonts();
//! let mut renderer = SdfTextRenderer::new(context, font_system);
//!
//! let text = Text::new("Title")
//!     .size(48.0)
//!     .color(Color::WHITE);
//!
//! let mut buffer = renderer.prepare(&text);
//!
//! // Draw with effects
//! let effects = TextEffectsBuilder::new()
//!     .shadow(Vec2::new(2.0, 2.0), Color::BLACK)
//!     .outline(1.5, Color::rgba(0.0, 0.0, 0.0, 0.8))
//!     .build();
//!
//! renderer.draw_text_with_effects(&mut buffer, Vec2::new(100.0, 100.0), &effects);
//! renderer.render(&mut render_pass);
//! ```

use std::sync::Arc;

use astrelis_core::alloc::HashMap;
use astrelis_core::math::Vec2;
use astrelis_core::profiling::profile_function;
use cosmic_text::{CacheKey, Color as CosmicColor, Metrics};

use astrelis_render::{AsWgpu, GpuTexture, GraphicsContext, UniformBuffer, Viewport, wgpu};

use crate::effects::TextEffects;
use crate::font::FontSystem;
use crate::sdf::{SdfConfig, generate_sdf};
use crate::text::{Text, TextMetrics};

use super::{SDF_BASE_SIZE, SDF_DEFAULT_SPREAD, orthographic_projection};
use super::shared::{
    AtlasEntry, AtlasPacker, GlyphPlacement, SdfAtlasEntry, SdfCacheKey, SdfParams, SharedContext,
    TextBuffer, TextRender, TextRendererConfig, TextVertex,
};

/// SDF text renderer backend.
///
/// Manages the SDF glyph atlas and rendering pipeline.
pub(crate) struct SdfBackend {
    // GPU resources
    pub(crate) pipeline: wgpu::RenderPipeline,
    #[allow(dead_code)]
    pub(crate) bind_group_layout: wgpu::BindGroupLayout,
    /// GPU texture with cached view and metadata.
    pub(crate) atlas: GpuTexture,
    #[allow(dead_code)]
    pub(crate) sampler: wgpu::Sampler,
    pub(crate) bind_group: wgpu::BindGroup,
    /// Typed uniform buffer for SDF parameters.
    pub(crate) params_buffer: UniformBuffer<SdfParams>,
    #[allow(dead_code)]
    pub(crate) params_bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) params_bind_group: wgpu::BindGroup,

    // Atlas management
    pub(crate) atlas_data: Vec<u8>,
    pub(crate) atlas_entries: HashMap<SdfCacheKey, SdfAtlasEntry>,
    pub(crate) atlas_packer: AtlasPacker,
    pub(crate) atlas_dirty: bool,

    // Configuration
    pub(crate) config: SdfConfig,
}

impl SdfBackend {
    /// Create a new SDF backend.
    pub fn new(shared: &SharedContext, atlas_size: u32, config: SdfConfig) -> Self {
        let renderer = &shared.renderer;

        // Create SDF shader
        let shader = renderer.create_shader(
            Some("Text SDF Shader"),
            include_str!("../../shaders/text_sdf.wgsl"),
        );

        // Create SDF atlas texture using GpuTexture
        let atlas = renderer.create_gpu_texture_2d(
            Some("SDF Text Atlas"),
            atlas_size,
            atlas_size,
            wgpu::TextureFormat::R8Unorm,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );

        let atlas_data = vec![0u8; (atlas_size * atlas_size) as usize];
        let sampler = renderer.create_linear_sampler(Some("SDF Text Sampler"));

        // SDF bind group layout
        let bind_group_layout = renderer.create_bind_group_layout(
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

        let bind_group = renderer.create_bind_group(
            Some("SDF Text Bind Group"),
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

        // SDF params uniform buffer using UniformBuffer
        let params_buffer = renderer.create_typed_uniform(
            Some("SDF Params Buffer"),
            &SdfParams::default(),
        );

        // SDF params bind group layout
        let params_bind_group_layout = renderer.create_bind_group_layout(
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

        let params_bind_group = renderer.create_bind_group(
            Some("SDF Params Bind Group"),
            &params_bind_group_layout,
            &[wgpu::BindGroupEntry {
                binding: 0,
                resource: params_buffer.as_binding(),
            }],
        );

        // Create SDF pipeline layout
        let pipeline_layout = renderer.create_pipeline_layout(
            Some("SDF Text Pipeline Layout"),
            &[
                &bind_group_layout,
                &shared.uniform_bind_group_layout,
                &params_bind_group_layout,
            ],
            &[],
        );

        // Create SDF pipeline
        let pipeline = renderer.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("SDF Text Pipeline"),
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
            params_buffer,
            params_bind_group_layout,
            params_bind_group,
            atlas_data,
            atlas_entries: HashMap::new(),
            atlas_packer: AtlasPacker::new(atlas_size),
            atlas_dirty: false,
            config,
        }
    }

    /// Ensure a glyph is in the SDF atlas.
    pub fn ensure_glyph(&mut self, shared: &SharedContext, cache_key: CacheKey) -> Option<&SdfAtlasEntry> {
        let sdf_key = SdfCacheKey::from_cache_key(cache_key);

        // Check if already in SDF atlas
        if self.atlas_entries.contains_key(&sdf_key) {
            return self.atlas_entries.get(&sdf_key);
        }

        // Create a cache key at base size for rasterization
        let base_cache_key = CacheKey {
            font_id: cache_key.font_id,
            glyph_id: cache_key.glyph_id,
            font_size_bits: SDF_BASE_SIZE.to_bits(),
            x_bin: cache_key.x_bin,
            y_bin: cache_key.y_bin,
            flags: cache_key.flags,
        };

        // Rasterize the glyph at base size
        let mut font_system = shared.font_system.write()
            .map_err(|e| crate::error::TextError::LockPoisoned(e.to_string()))
            .ok()?;
        let mut swash_cache = shared.swash_cache.write()
            .map_err(|e| crate::error::TextError::LockPoisoned(e.to_string()))
            .ok()?;
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
        let spread = self.config.mode.spread().max(SDF_DEFAULT_SPREAD);
        let sdf_data = generate_sdf(&image, spread);

        if sdf_data.is_empty() {
            return None;
        }

        // Add padding for effects
        let padding = (spread.ceil() as u32) * 2;
        let padded_width = width + padding * 2;
        let padded_height = height + padding * 2;

        // Try to pack into SDF atlas
        let atlas_entry = self.atlas_packer.pack(padded_width, padded_height)?;

        // Copy SDF data into atlas with padding
        let atlas_size = self.atlas.width();
        for y in 0..height {
            for x in 0..width {
                let src_idx = (y * width + x) as usize;
                let dst_x = atlas_entry.x + padding + x;
                let dst_y = atlas_entry.y + padding + y;
                let dst_idx = (dst_y * atlas_size + dst_x) as usize;
                if src_idx < sdf_data.len() && dst_idx < self.atlas_data.len() {
                    self.atlas_data[dst_idx] = sdf_data[src_idx];
                }
            }
        }

        // Store the base placement info
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

        self.atlas_dirty = true;
        self.atlas_entries.insert(sdf_key, sdf_entry);
        self.atlas_entries.get(&sdf_key)
    }

    /// Upload SDF atlas data to GPU if dirty.
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

    /// Update SDF params uniform buffer.
    pub fn update_params(&self, shared: &SharedContext, params: &SdfParams) {
        self.params_buffer
            .write_uniform(&shared.renderer.queue(), params);
    }
}

/// SDF-only text renderer.
///
/// Uses only an SDF glyph atlas for rendering, with no bitmap support.
/// This provides scalable text rendering with effects (~8 MB with default atlas).
///
/// Best for large text, titles, and text that needs effects like shadows/outlines.
pub struct SdfTextRenderer {
    shared: SharedContext,
    backend: SdfBackend,

    // Staging data
    vertices: Vec<TextVertex>,
    indices: Vec<u16>,
}

impl SdfTextRenderer {
    /// Create a new SDF text renderer with default configuration.
    pub fn new(context: Arc<GraphicsContext>, font_system: FontSystem) -> Self {
        Self::with_config(context, font_system, TextRendererConfig::default())
    }

    /// Create a new SDF text renderer with custom configuration.
    pub fn with_config(
        context: Arc<GraphicsContext>,
        font_system: FontSystem,
        config: TextRendererConfig,
    ) -> Self {
        let shared = SharedContext::new(context, font_system.inner());
        let backend = SdfBackend::new(&shared, config.atlas_size, config.sdf);

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

        // Handle lock poisoning gracefully
        let mut font_system = match self.shared.font_system.write() {
            Ok(guard) => guard,
            Err(e) => {
                tracing::error!("Font system lock poisoned: {}. Returning zero size.", e);
                return (0.0, 0.0);
            }
        };

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
        let font_size = text.get_font_size();
        let line_height_multiplier = text.get_line_height();
        let scale = self.shared.scale_factor();

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
    ///
    /// Note: SDF atlas doesn't need to be cleared on scale factor change
    /// because SDF glyphs are resolution-independent.
    pub fn set_viewport(&mut self, viewport: Viewport) {
        self.shared.set_viewport(viewport);
    }

    /// Set SDF configuration.
    pub fn set_sdf_config(&mut self, config: SdfConfig) {
        self.backend.config = config;
    }

    /// Get the current SDF configuration.
    pub fn sdf_config(&self) -> &SdfConfig {
        &self.backend.config
    }

    /// Prepare text for rendering.
    pub fn prepare(&mut self, text: &Text) -> TextBuffer {
        profile_function!();

        // Handle lock poisoning gracefully - create empty buffer on error
        let mut font_system = match self.shared.font_system.write() {
            Ok(guard) => guard,
            Err(e) => {
                tracing::error!("Font system lock poisoned during prepare: {}. Attempting recovery.", e);
                // Try to recover by taking the poisoned lock
                self.shared.font_system.write().unwrap_or_else(|poisoned| {
                    tracing::warn!("Clearing poisoned lock and continuing");
                    poisoned.into_inner()
                })
            }
        };

        let mut buffer = TextBuffer::new(&mut font_system);
        buffer.set_text(&mut font_system, text, self.shared.scale_factor());
        buffer.layout(&mut font_system);
        buffer
    }

    /// Draw text at a position (without effects).
    pub fn draw_text(&mut self, buffer: &mut TextBuffer, position: Vec2) {
        // Use default (no effects) params
        let params = SdfParams::default();
        self.backend.update_params(&self.shared, &params);
        self.draw_text_internal(buffer, position);
    }

    /// Draw text with effects at a position.
    pub fn draw_text_with_effects(
        &mut self,
        buffer: &mut TextBuffer,
        position: Vec2,
        effects: &TextEffects,
    ) {
        profile_function!();

        // Update SDF params from effects
        let sdf_params = SdfParams::from_effects(effects, &self.backend.config);
        self.backend.update_params(&self.shared, &sdf_params);

        // Draw text
        self.draw_text_internal(buffer, position);
    }

    /// Internal SDF text drawing implementation.
    fn draw_text_internal(&mut self, buffer: &mut TextBuffer, position: Vec2) {
        profile_function!();

        let scale = self.shared.scale_factor();

        // Handle lock poisoning gracefully
        let mut font_system = match self.shared.font_system.write() {
            Ok(guard) => guard,
            Err(e) => {
                tracing::error!("Font system lock poisoned during draw: {}. Skipping layout.", e);
                return; // Skip rendering on error
            }
        };

        buffer.layout(&mut font_system);
        drop(font_system);

        // Render glyphs using SDF atlas
        for run in buffer.buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                let physical_glyph = glyph.physical((position.x, position.y + run.line_y), 1.0);
                let cache_key = physical_glyph.cache_key;

                // Ensure glyph is in SDF atlas
                let sdf_entry = match self.backend.ensure_glyph(&self.shared, cache_key) {
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

                let (u0, v0, u1, v1) = sdf_entry.entry.uv_coords(self.backend.atlas.width());

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
            .create_vertex_buffer(Some("SDF Text Vertex Buffer"), &self.vertices);

        let index_buffer = self
            .shared
            .renderer
            .create_index_buffer(Some("SDF Text Index Buffer"), &self.indices);

        // Create projection uniform
        let size = self.shared.viewport.to_logical();
        let projection = orthographic_projection(size.width, size.height);
        let uniform_buffer = self
            .shared
            .renderer
            .create_uniform_buffer(Some("SDF Text Projection"), &projection);

        // Create uniform bind group
        let uniform_bind_group = self.shared.renderer.create_bind_group(
            Some("SDF Text Uniform Bind Group"),
            &self.shared.uniform_bind_group_layout,
            &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        );

        // Render with SDF pipeline
        render_pass.set_pipeline(&self.backend.pipeline);
        render_pass.set_bind_group(0, &self.backend.bind_group, &[]);
        render_pass.set_bind_group(1, &uniform_bind_group, &[]);
        render_pass.set_bind_group(2, &self.backend.params_bind_group, &[]);
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
}

impl TextRender for SdfTextRenderer {
    fn prepare(&mut self, text: &Text) -> TextBuffer {
        SdfTextRenderer::prepare(self, text)
    }

    fn draw_text(&mut self, buffer: &mut TextBuffer, position: Vec2) {
        SdfTextRenderer::draw_text(self, buffer, position)
    }

    fn render(&mut self, render_pass: &mut wgpu::RenderPass) {
        SdfTextRenderer::render(self, render_pass)
    }

    fn measure_text(&self, text: &Text) -> (f32, f32) {
        SdfTextRenderer::measure_text(self, text)
    }

    fn set_viewport(&mut self, viewport: Viewport) {
        SdfTextRenderer::set_viewport(self, viewport)
    }

    fn buffer_bounds(&self, buffer: &TextBuffer) -> (f32, f32) {
        SdfTextRenderer::buffer_bounds(self, buffer)
    }
}
