//! Hybrid text renderer that auto-selects bitmap or SDF.
//!
//! Automatically uses bitmap rendering for small text (<24px) without effects,
//! and SDF rendering for large text or text with effects.

use astrelis_core::math::Vec2;
use astrelis_gpu_wgpu::WgpuDevice;
use astrelis_text::{FontSystem, SdfConfig, Text, TextEffects, TextRenderMode};

use crate::config::TextRendererConfig;

use super::{BitmapTextRenderer, SdfTextRenderer, TextBuffer};

/// Hybrid text renderer (~16 MB with default atlas).
///
/// Auto-selects the best backend:
/// - Small text (<24px) without effects → bitmap (sharp)
/// - Large text (>=24px) or effects → SDF (scalable)
pub struct FontRenderer {
    bitmap: BitmapTextRenderer,
    sdf: SdfTextRenderer,
}

impl FontRenderer {
    /// Create a new hybrid text renderer.
    pub fn new(
        device: &WgpuDevice,
        font_system: FontSystem,
        config: TextRendererConfig,
    ) -> Self {
        astrelis_profiling::profile_function!();
        let bitmap = BitmapTextRenderer::new(device, font_system.clone(), config.clone());
        let sdf = SdfTextRenderer::new(device, font_system, config);

        Self { bitmap, sdf }
    }

    /// Select the best render mode for the given text.
    pub fn select_render_mode(font_size: f32, has_effects: bool) -> TextRenderMode {
        if has_effects || font_size >= 24.0 {
            TextRenderMode::SDF { spread: 4.0 }
        } else {
            TextRenderMode::Bitmap
        }
    }

    /// Prepare text for rendering.
    pub fn prepare(&mut self, text: &Text) -> TextBuffer {
        astrelis_profiling::profile_function!();
        let mode = text
            .render_mode
            .unwrap_or_else(|| Self::select_render_mode(text.font_size, text.needs_sdf()));

        if mode.is_sdf() {
            self.sdf.prepare(text)
        } else {
            self.bitmap.prepare(text)
        }
    }

    /// Queue text for drawing at the given position.
    pub fn draw_text(&mut self, text: &Text, buffer: &mut TextBuffer, position: Vec2) {
        astrelis_profiling::profile_function!();
        let mode = text
            .render_mode
            .unwrap_or_else(|| Self::select_render_mode(text.font_size, text.needs_sdf()));

        if mode.is_sdf() {
            self.sdf.draw_text(buffer, position);
        } else {
            self.bitmap.draw_text(buffer, position);
        }
    }

    /// Queue text with effects for drawing.
    ///
    /// **Note:** Effects rendering is not yet implemented. Text is rendered
    /// without effects applied.
    pub fn draw_text_with_effects(
        &mut self,
        buffer: &mut TextBuffer,
        position: Vec2,
        effects: &TextEffects,
    ) {
        astrelis_profiling::profile_function!();
        self.sdf.draw_text_with_effects(buffer, position, effects);
    }

    /// Set the viewport dimensions.
    pub fn resize(&mut self, width: f32, height: f32) {
        self.bitmap.resize(width, height);
        self.sdf.resize(width, height);
    }

    /// Get the SDF config.
    pub fn sdf_config(&self) -> &SdfConfig {
        self.sdf.sdf_config()
    }

    /// Set the SDF config.
    pub fn set_sdf_config(&mut self, config: SdfConfig) {
        self.sdf.set_sdf_config(config);
    }

    /// Render all queued text (both bitmap and SDF).
    pub fn render(
        &mut self,
        device: &WgpuDevice,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) {
        astrelis_profiling::profile_function!();
        self.bitmap.render(device, encoder, view, width, height);
        self.sdf.render(device, encoder, view, width, height);
    }
}
