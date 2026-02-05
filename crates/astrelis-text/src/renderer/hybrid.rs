//! Hybrid text renderer (backwards-compatible default).
//!
//! This module provides [`FontRenderer`], the hybrid text renderer that combines
//! both bitmap and SDF backends (~16 MB with default atlas size).
//!
//! This is the **default and backwards-compatible** renderer that automatically
//! selects the best rendering mode based on text size and effects.
//!
//! # When to Use
//!
//! Use `FontRenderer` (the default) when:
//! - You need both small UI text and large display text
//! - You want automatic mode selection for optimal quality
//! - Backwards compatibility with existing code is important
//!
//! # Memory Usage
//!
//! | Config | Atlas Size | GPU Memory | CPU Memory | Total |
//! |--------|------------|------------|------------|-------|
//! | small() | 512x512 | ~0.5 MB | ~0.5 MB | ~1 MB |
//! | medium() | 1024x1024 | ~2 MB | ~2 MB | ~4 MB |
//! | large() | 2048x2048 | ~8 MB | ~8 MB | ~16 MB |
//!
//! # Example
//!
//! ```ignore
//! use astrelis_text::{FontRenderer, Text, FontSystem, Color};
//! use astrelis_core::math::Vec2;
//!
//! let font_system = FontSystem::with_system_fonts();
//! let mut renderer = FontRenderer::new(context, font_system);
//!
//! // Small text -> automatically uses bitmap
//! let small_text = Text::new("UI Label").size(14.0);
//! let mut small_buffer = renderer.prepare(&small_text);
//! renderer.draw_text(&mut small_buffer, Vec2::new(10.0, 10.0));
//!
//! // Large text -> automatically uses SDF
//! let large_text = Text::new("Title").size(48.0);
//! let mut large_buffer = renderer.prepare(&large_text);
//! renderer.draw_text(&mut large_buffer, Vec2::new(100.0, 100.0));
//!
//! // Effects always use SDF
//! renderer.draw_text_with_effects(&mut large_buffer, position, &effects);
//!
//! renderer.render(&mut render_pass);
//! ```

use std::sync::Arc;

use astrelis_core::math::Vec2;
use astrelis_core::profiling::profile_function;
use cosmic_text::{CacheKey, Color as CosmicColor, Metrics};

use astrelis_render::{GraphicsContext, Viewport, wgpu};

use crate::effects::TextEffects;
use crate::font::FontSystem;
use crate::sdf::{SdfConfig, TextRenderMode};
use crate::text::{Text, TextMetrics};

use crate::decoration::TextBounds;

use super::bitmap::BitmapBackend;
use super::sdf::SdfBackend;
use super::shared::{
    AtlasEntry, DecorationRenderer, GlyphPlacement, SdfParams, SharedContext, TextBuffer,
    TextRender, TextRendererConfig, TextVertex,
};
use super::{SDF_DEFAULT_SPREAD, orthographic_projection};

/// Helper macro to handle RwLock write poisoning gracefully.
macro_rules! lock_or_recover {
    ($lock:expr, $error_msg:expr, $default:expr) => {
        match $lock.write() {
            Ok(guard) => guard,
            Err(e) => {
                tracing::error!("{}: {}. Attempting recovery.", $error_msg, e);
                $lock.write().unwrap_or_else(|poisoned| {
                    tracing::warn!("Clearing poisoned lock");
                    poisoned.into_inner()
                })
            }
        }
    };
    ($lock:expr, $error_msg:expr) => {
        match $lock.write() {
            Ok(guard) => guard,
            Err(e) => {
                tracing::error!("{}: {}. Returning default.", $error_msg, e);
                return Default::default();
            }
        }
    };
}

/// Font renderer for rendering text with WGPU.
///
/// This is the **hybrid renderer** that combines both bitmap and SDF backends,
/// automatically selecting the best mode based on text size and effects.
///
/// - Small text (< 24px) without effects: uses bitmap for sharpness
/// - Large text (>= 24px) or text with effects: uses SDF for quality
///
/// This is the backwards-compatible default renderer.
pub struct FontRenderer {
    shared: SharedContext,
    bitmap: BitmapBackend,
    sdf: SdfBackend,
    decoration: DecorationRenderer,

    // Render mode configuration
    render_mode: TextRenderMode,

    // Staging data
    vertices: Vec<TextVertex>,
    indices: Vec<u16>,
}

impl FontRenderer {
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
        Self::with_config(
            context,
            font_system,
            TextRendererConfig {
                atlas_size,
                ..Default::default()
            },
        )
    }

    /// Create a new font renderer with custom configuration.
    pub fn with_config(
        context: Arc<GraphicsContext>,
        font_system: FontSystem,
        config: TextRendererConfig,
    ) -> Self {
        let shared = SharedContext::new(context, font_system.inner());
        let bitmap = BitmapBackend::new(&shared, config.atlas_size);
        let sdf = SdfBackend::new(&shared, config.atlas_size, config.sdf);
        let decoration =
            DecorationRenderer::new(&shared.renderer, &shared.uniform_bind_group_layout);

        Self {
            shared,
            bitmap,
            sdf,
            decoration,
            render_mode: TextRenderMode::default(),
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// Measure text dimensions without rendering.
    pub fn measure_text(&self, text: &Text) -> (f32, f32) {
        profile_function!();
        let scale = self.shared.scale_factor();
        let mut font_system = lock_or_recover!(
            self.shared.font_system,
            "Font system lock poisoned during measure"
        );
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

    /// Get the baseline offset from the top of the text bounding box.
    pub fn get_baseline_offset(&self, text: &Text) -> f32 {
        let metrics = self.get_text_metrics(text);
        metrics.baseline_offset
    }

    /// Set the text render mode (Bitmap or SDF).
    pub fn set_render_mode(&mut self, mode: TextRenderMode) {
        self.render_mode = mode;
    }

    /// Get the current render mode.
    pub fn render_mode(&self) -> TextRenderMode {
        self.render_mode
    }

    /// Set SDF configuration.
    pub fn set_sdf_config(&mut self, config: SdfConfig) {
        if config.mode.is_sdf() {
            self.render_mode = config.mode;
        }
        self.sdf.config = config;
    }

    /// Get the current SDF configuration.
    pub fn sdf_config(&self) -> &SdfConfig {
        &self.sdf.config
    }

    /// Determine the appropriate render mode based on font size and effects.
    ///
    /// - Small text (< 24px) without effects: use Bitmap for sharpness
    /// - Large text (>= 24px) or text with effects: use SDF for quality
    pub fn select_render_mode(font_size: f32, has_effects: bool) -> TextRenderMode {
        if has_effects {
            return TextRenderMode::SDF {
                spread: SDF_DEFAULT_SPREAD,
            };
        }
        if font_size >= 24.0 {
            return TextRenderMode::SDF {
                spread: SDF_DEFAULT_SPREAD,
            };
        }
        TextRenderMode::Bitmap
    }

    /// Set the viewport for rendering.
    pub fn set_viewport(&mut self, viewport: Viewport) {
        if viewport.scale_factor != self.shared.viewport.scale_factor {
            tracing::trace!(
                "FontRenderer scale factor changed: {:?} -> {:?}",
                self.shared.viewport.scale_factor,
                viewport.scale_factor
            );
            // Clear bitmap atlas on scale factor change
            self.bitmap.clear();
            // Note: SDF atlas doesn't need to be cleared (resolution-independent)
        }
        self.shared.set_viewport(viewport);
    }

    /// Prepare text for rendering.
    pub fn prepare(&mut self, text: &Text) -> TextBuffer {
        profile_function!();
        let mut font_system = lock_or_recover!(
            self.shared.font_system,
            "Font system lock poisoned during prepare",
            TextBuffer::default()
        );
        let mut buffer = TextBuffer::new(&mut font_system);
        buffer.set_text(&mut font_system, text, self.shared.scale_factor());
        buffer.layout(&mut font_system);
        buffer
    }

    /// Draw text at a position.
    ///
    /// The position represents the **top-left corner** of the text's bounding box.
    pub fn draw_text(&mut self, buffer: &mut TextBuffer, position: Vec2) {
        profile_function!();

        if self.render_mode.is_sdf() {
            // Use default params for SDF without effects
            let params = SdfParams::default();
            self.sdf.update_params(&self.shared, &params);
            self.draw_text_sdf_internal(buffer, position);
        } else {
            self.draw_text_bitmap_internal(buffer, position);
        }
    }

    /// Draw text with effects at a position using SDF rendering.
    pub fn draw_text_with_effects(
        &mut self,
        buffer: &mut TextBuffer,
        position: Vec2,
        effects: &TextEffects,
    ) {
        profile_function!();

        // Always use SDF mode when effects are present
        if effects.has_enabled_effects() && !self.render_mode.is_sdf() {
            self.render_mode = TextRenderMode::SDF {
                spread: SDF_DEFAULT_SPREAD,
            };
        }

        // Update SDF params from effects
        let sdf_params = SdfParams::from_effects(effects, &self.sdf.config);
        self.sdf.update_params(&self.shared, &sdf_params);

        // Use SDF drawing path
        self.draw_text_sdf_internal(buffer, position);
    }

    /// Draw text with decoration (underline, strikethrough, background).
    ///
    /// This method handles both the text rendering and any decorations.
    /// Decorations are rendered in the correct order:
    /// - Background: behind text
    /// - Text glyphs
    /// - Underline/strikethrough: on top of text
    ///
    /// # Arguments
    ///
    /// * `buffer` - The prepared text buffer
    /// * `position` - Top-left position of the text
    /// * `text` - The Text object containing decoration configuration
    pub fn draw_text_with_decoration(
        &mut self,
        buffer: &mut TextBuffer,
        position: Vec2,
        text: &Text,
    ) {
        profile_function!();

        // Queue decoration if present
        if let Some(decoration) = text.get_decoration() {
            // Get text bounds from buffer
            let (width, height) = self.buffer_bounds(buffer);
            let metrics = self.get_text_metrics(text);

            let bounds = TextBounds::new(
                position.x,
                position.y,
                width,
                height,
                metrics.baseline_offset,
            );

            self.decoration
                .queue_from_text(&bounds, decoration, self.shared.scale_factor());
        }

        // Draw the text
        self.draw_text(buffer, position);
    }

    /// Internal bitmap text drawing implementation.
    fn draw_text_bitmap_internal(&mut self, buffer: &mut TextBuffer, position: Vec2) {
        profile_function!();

        let scale = self.shared.scale_factor();
        let mut font_system = lock_or_recover!(
            self.shared.font_system,
            "Font system lock poisoned in draw_text_bitmap_internal"
        );
        buffer.layout(&mut font_system);
        drop(font_system);

        // Render glyphs
        for run in buffer.buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                let physical_glyph =
                    glyph.physical((position.x * scale, position.y * scale + run.line_y), 1.0);
                let cache_key = physical_glyph.cache_key;

                // Ensure glyph is in atlas
                let entry = match self.bitmap.ensure_glyph(&self.shared, cache_key) {
                    Some(e) => e.clone(),
                    None => continue,
                };

                // Get glyph placement info
                let mut font_system = lock_or_recover!(
                    self.shared.font_system,
                    "Font system lock poisoned in draw_text_bitmap_internal (glyph loop)"
                );
                let mut swash_cache = lock_or_recover!(
                    self.shared.swash_cache,
                    "Swash cache lock poisoned in draw_text_bitmap_internal (glyph loop)"
                );

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

                    let (u0, v0, u1, v1) = entry.uv_coords(self.bitmap.atlas.width());

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

    /// Internal SDF text drawing implementation.
    fn draw_text_sdf_internal(&mut self, buffer: &mut TextBuffer, position: Vec2) {
        profile_function!();

        let scale = self.shared.scale_factor();
        let mut font_system = lock_or_recover!(
            self.shared.font_system,
            "Font system lock poisoned in draw_text_sdf_internal"
        );
        buffer.layout(&mut font_system);
        drop(font_system);

        // Render glyphs using SDF atlas
        for run in buffer.buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                let physical_glyph = glyph.physical((position.x, position.y + run.line_y), 1.0);
                let cache_key = physical_glyph.cache_key;

                // Ensure glyph is in SDF atlas
                let sdf_entry = match self.sdf.ensure_glyph(&self.shared, cache_key) {
                    Some(e) => e.clone(),
                    None => continue,
                };

                // Calculate scale factor from base size to target size
                let target_size = f32::from_bits(cache_key.font_size_bits);
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

                let (u0, v0, u1, v1) = sdf_entry.entry.uv_coords(self.sdf.atlas.width());

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
    ///
    /// Automatically selects bitmap or SDF pipeline based on the current render mode.
    /// Renders in the correct order:
    /// 1. Background decorations (behind text)
    /// 2. Text glyphs
    /// 3. Line decorations (underline, strikethrough - on top of text)
    pub fn render(&mut self, render_pass: &mut wgpu::RenderPass) {
        profile_function!();

        debug_assert!(
            self.shared.viewport.is_valid(),
            "Viewport size must be set before rendering text."
        );

        // 1. Render background decorations first (behind text)
        self.decoration.render_backgrounds(
            render_pass,
            &self.shared.renderer,
            &self.shared.viewport,
        );

        // 2. Render text glyphs
        if !self.vertices.is_empty() {
            // Upload appropriate atlas based on render mode
            if self.render_mode.is_sdf() {
                self.sdf.upload_atlas(&self.shared);
            } else {
                self.bitmap.upload_atlas(&self.shared);
            }

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

            // Render with appropriate pipeline
            if self.render_mode.is_sdf() {
                // SDF pipeline
                render_pass.set_pipeline(&self.sdf.pipeline);
                render_pass.set_bind_group(0, &self.sdf.bind_group, &[]);
                render_pass.set_bind_group(1, &uniform_bind_group, &[]);
                render_pass.set_bind_group(2, &self.sdf.params_bind_group, &[]);
            } else {
                // Bitmap pipeline
                render_pass.set_pipeline(&self.bitmap.pipeline);
                render_pass.set_bind_group(0, &self.bitmap.bind_group, &[]);
                render_pass.set_bind_group(1, &uniform_bind_group, &[]);
            }

            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..self.indices.len() as u32, 0, 0..1);

            // Clear for next frame
            self.vertices.clear();
            self.indices.clear();
        }

        // 3. Render line decorations (underline, strikethrough - on top of text)
        self.decoration
            .render_lines(render_pass, &self.shared.renderer, &self.shared.viewport);
    }

    /// Get the font system.
    pub fn font_system(&self) -> Arc<std::sync::RwLock<cosmic_text::FontSystem>> {
        self.shared.font_system.clone()
    }

    /// Get the swash cache.
    pub fn swash_cache(&self) -> Arc<std::sync::RwLock<cosmic_text::SwashCache>> {
        self.shared.swash_cache.clone()
    }

    /// Get the atlas size in pixels.
    pub fn atlas_size(&self) -> u32 {
        self.bitmap.atlas.width()
    }

    /// Get the atlas texture view for binding.
    pub fn atlas_texture_view(&self) -> &wgpu::TextureView {
        self.bitmap.atlas.view()
    }

    /// Get the atlas sampler for binding.
    pub fn atlas_sampler(&self) -> &wgpu::Sampler {
        &self.bitmap.sampler
    }

    /// Check if the atlas has pending changes.
    pub fn is_atlas_dirty(&self) -> bool {
        self.bitmap.atlas_dirty
    }

    /// Upload atlas data to GPU if dirty.
    pub fn upload_atlas_if_dirty(&mut self) {
        profile_function!();
        self.bitmap.upload_atlas(&self.shared);
    }

    /// Ensure a glyph is in the atlas using a cache key.
    pub fn ensure_glyph_in_atlas(&mut self, cache_key: CacheKey) -> Option<&AtlasEntry> {
        self.bitmap.ensure_glyph(&self.shared, cache_key)
    }

    /// Get glyph placement information.
    pub fn get_glyph_placement(&mut self, cache_key: CacheKey) -> Option<GlyphPlacement> {
        let mut font_system = self.shared.font_system.write().ok()?;
        let mut swash_cache = self.shared.swash_cache.write().ok()?;

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
        let atlas_entry = self.bitmap.ensure_glyph(&self.shared, cache_key)?.clone();

        let mut font_system = self.shared.font_system.write().ok()?;
        let mut swash_cache = self.shared.swash_cache.write().ok()?;

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
        self.bitmap.atlas_entries.get(&cache_key)
    }
}

impl TextRender for FontRenderer {
    fn prepare(&mut self, text: &Text) -> TextBuffer {
        FontRenderer::prepare(self, text)
    }

    fn draw_text(&mut self, buffer: &mut TextBuffer, position: Vec2) {
        FontRenderer::draw_text(self, buffer, position)
    }

    fn render(&mut self, render_pass: &mut wgpu::RenderPass) {
        FontRenderer::render(self, render_pass)
    }

    fn measure_text(&self, text: &Text) -> (f32, f32) {
        FontRenderer::measure_text(self, text)
    }

    fn set_viewport(&mut self, viewport: Viewport) {
        FontRenderer::set_viewport(self, viewport)
    }

    fn buffer_bounds(&self, buffer: &TextBuffer) -> (f32, f32) {
        FontRenderer::buffer_bounds(self, buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
