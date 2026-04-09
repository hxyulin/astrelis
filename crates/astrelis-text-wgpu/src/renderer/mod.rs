//! Text rendering backends.
//!
//! Provides three renderers with different memory/feature trade-offs:
//! - [`BitmapTextRenderer`] - Bitmap-only (~8 MB)
//! - [`SdfTextRenderer`] - SDF-only with effects (~8 MB)
//! - [`FontRenderer`] - Hybrid auto-selecting (~16 MB)

mod bitmap;
mod decoration;
mod hybrid;
mod sdf;
pub mod vertex;

pub use bitmap::BitmapTextRenderer;
pub use decoration::DecorationRenderer;
pub use hybrid::FontRenderer;
pub use sdf::SdfTextRenderer;
pub use vertex::{DecorationVertex, TextVertex};

use std::sync::{Arc, RwLock};

use astrelis_gpu_wgpu::WgpuDevice;
use astrelis_text::FontSystem;
use cosmic_text::{Metrics, SwashCache};

/// SDF uniform parameters for shader effects.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SdfParams {
    /// Edge softness for anti-aliasing.
    pub edge_softness: f32,
    /// Outline width.
    pub outline_width: f32,
    /// Padding for alignment.
    pub _pad0: [f32; 2],
    /// Outline color.
    pub outline_color: [f32; 4],
    /// Shadow offset.
    pub shadow_offset: [f32; 2],
    /// Shadow blur radius.
    pub shadow_blur: f32,
    /// Padding for alignment.
    pub _pad1: f32,
    /// Shadow color.
    pub shadow_color: [f32; 4],
    /// Glow radius.
    pub glow_radius: f32,
    /// Padding for alignment.
    pub _pad2: [f32; 3],
    /// Glow color.
    pub glow_color: [f32; 4],
    /// Padding to reach 16-byte alignment.
    pub _padding: [f32; 2],
    /// More padding.
    pub _pad3: [f32; 2],
}

impl Default for SdfParams {
    fn default() -> Self {
        Self {
            edge_softness: 0.05,
            outline_width: 0.0,
            _pad0: [0.0; 2],
            outline_color: [0.0, 0.0, 0.0, 0.0],
            shadow_offset: [0.0, 0.0],
            shadow_blur: 0.0,
            _pad1: 0.0,
            shadow_color: [0.0, 0.0, 0.0, 0.0],
            glow_radius: 0.0,
            _pad2: [0.0; 3],
            glow_color: [0.0, 0.0, 0.0, 0.0],
            _padding: [0.0; 2],
            _pad3: [0.0; 2],
        }
    }
}

impl SdfParams {
    /// Create SDF params from text effects.
    pub fn from_effects(effects: &astrelis_text::TextEffects, config: &astrelis_text::SdfConfig) -> Self {
        let mut params = Self {
            edge_softness: config.edge_softness,
            outline_width: config.outline_width,
            ..Self::default()
        };

        for effect in effects.effects() {
            if !effect.is_enabled() {
                continue;
            }
            match effect.effect_type() {
                astrelis_text::TextEffectType::Shadow {
                    offset,
                    blur_radius,
                    color,
                } => {
                    params.shadow_offset = [offset.x, offset.y];
                    params.shadow_blur = *blur_radius;
                    params.shadow_color = [color.r, color.g, color.b, color.a];
                }
                astrelis_text::TextEffectType::Outline { width, color } => {
                    params.outline_width = *width;
                    params.outline_color = [color.r, color.g, color.b, color.a];
                }
                astrelis_text::TextEffectType::Glow {
                    radius,
                    color,
                    intensity,
                } => {
                    params.glow_radius = *radius;
                    params.glow_color = [color.r, color.g, color.b, color.a * intensity];
                }
                astrelis_text::TextEffectType::InnerShadow { .. } => {}
            }
        }

        params
    }
}

/// Cached text buffer with layout information.
pub struct TextBuffer {
    pub(crate) buffer: cosmic_text::Buffer,
    pub(crate) needs_layout: bool,
    pub(crate) scale: f32,
}

impl TextBuffer {
    /// Create a new text buffer.
    pub fn new(font_system: &mut cosmic_text::FontSystem, scale: f32) -> Self {
        let metrics = Metrics::new(16.0, 19.2);
        let buffer = cosmic_text::Buffer::new(font_system, metrics);
        Self {
            buffer,
            needs_layout: true,
            scale,
        }
    }

    /// Set text content and style.
    pub fn set_text(
        &mut self,
        font_system: &mut cosmic_text::FontSystem,
        text: &astrelis_text::Text,
        scale: f32,
    ) {
        self.scale = scale;
        let metrics = Metrics::new(
            text.font_size * scale,
            text.font_size * scale * text.line_height,
        );
        self.buffer.set_metrics(font_system, metrics);

        let mut attrs = cosmic_text::Attrs::new();
        if let Some(ref family) = text.font_family {
            attrs = attrs.family(cosmic_text::Family::Name(family));
        }
        attrs = attrs.weight(text.weight.to_cosmic());
        attrs = attrs.style(text.font_style.to_cosmic());
        attrs = attrs.stretch(text.stretch.to_cosmic());
        attrs = attrs.color(astrelis_text::text::color_to_cosmic(text.text_color));

        self.buffer.set_text(
            font_system,
            &text.content,
            &attrs,
            cosmic_text::Shaping::Advanced,
            None,
        );

        if let Some(width) = text.max_width {
            self.buffer
                .set_size(font_system, Some(width * scale), None);
        }

        self.needs_layout = true;
    }

    /// Layout the text if needed.
    pub fn layout(&mut self, font_system: &mut cosmic_text::FontSystem) {
        if self.needs_layout {
            self.buffer.shape_until_scroll(font_system, false);
            self.needs_layout = false;
        }
    }

    /// Get the measured bounds `(width, height)`.
    pub fn bounds(&self) -> (f32, f32) {
        let mut max_x = 0.0f32;
        let mut max_y = 0.0f32;

        for run in self.buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                let physical = glyph.physical((0., 0.), 1.0);
                max_x = max_x.max(physical.x as f32 + glyph.w);
                max_y = max_y.max(run.line_y + run.line_height);
            }
        }

        (max_x / self.scale, max_y / self.scale)
    }
}

/// Shared rendering context with font system and swash cache.
pub(crate) struct SharedContext {
    pub font_system: Arc<RwLock<cosmic_text::FontSystem>>,
    pub swash_cache: SwashCache,
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub uniform_bind_group_layout: wgpu::BindGroupLayout,
}

impl SharedContext {
    /// Create a new shared context.
    pub fn new(device: &WgpuDevice, font_system: FontSystem) -> Self {
        let uniform_bind_group_layout =
            device
                .wgpu_device()
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("text_uniform_layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        Self {
            font_system: font_system.inner().clone(),
            swash_cache: SwashCache::new(),
            viewport_width: 800.0,
            viewport_height: 600.0,
            uniform_bind_group_layout,
        }
    }

    /// Set the viewport dimensions.
    pub fn set_viewport(&mut self, width: f32, height: f32) {
        self.viewport_width = width;
        self.viewport_height = height;
    }
}

/// Create an orthographic projection matrix for screen-space rendering.
///
/// Transforms from top-left origin (Y down) to NDC.
pub(crate) fn orthographic_projection(width: f32, height: f32) -> [[f32; 4]; 4] {
    [
        [2.0 / width, 0.0, 0.0, 0.0],
        [0.0, -2.0 / height, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0, 1.0],
    ]
}
