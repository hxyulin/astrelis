//! Modular text rendering system with zero-cost abstraction.
//!
//! This module provides three text renderers with different memory footprints:
//!
//! - [`BitmapTextRenderer`]: Bitmap-only rendering (~8 MB with default atlas)
//! - [`SdfTextRenderer`]: SDF-only rendering (~8 MB with default atlas)
//! - [`FontRenderer`]: Hybrid rendering with both backends (~16 MB, backwards-compatible default)
//!
//! # Architecture
//!
//! All renderers share a common [`SharedContext`] containing:
//! - Font system and swash cache for glyph rasterization
//! - Viewport and projection matrix handling
//! - Common GPU resources (uniform bind group layout)
//!
//! Each renderer has its own backend that manages atlas textures and pipelines.
//!
//! # Memory Costs
//!
//! | Renderer | GPU | CPU | Total | Use Case |
//! |----------|-----|-----|-------|----------|
//! | BitmapTextRenderer | 4 MB | 4 MB | 8 MB | UI labels, small text |
//! | SdfTextRenderer | 4 MB | 4 MB | 8 MB | Titles, effects, scaling |
//! | FontRenderer | 8 MB | 8 MB | 16 MB | Mixed (default) |
//!
//! Memory can be further reduced using [`TextRendererConfig`]:
//! - `small()`: 512x512 atlas (~1 MB per atlas)
//! - `medium()`: 1024x1024 atlas (~4 MB per atlas)
//! - `large()`: 2048x2048 atlas (~8 MB per atlas, default)
//!
//! # Examples
//!
//! ## Bitmap-only (for UI labels, ~8 MB)
//!
//! ```ignore
//! use astrelis_text::{BitmapTextRenderer, Text, FontSystem};
//!
//! let mut renderer = BitmapTextRenderer::new(context, font_system);
//! let text = Text::new("UI Label").size(14.0);
//! let mut buffer = renderer.prepare(&text);
//! renderer.draw_text(&mut buffer, Vec2::new(10.0, 10.0));
//! renderer.render(&mut render_pass);
//! ```
//!
//! ## SDF-only with effects (for titles, ~8 MB)
//!
//! ```ignore
//! use astrelis_text::{SdfTextRenderer, Text, TextEffectsBuilder};
//!
//! let mut renderer = SdfTextRenderer::new(context, font_system);
//! let text = Text::new("Title").size(48.0);
//! let mut buffer = renderer.prepare(&text);
//! let effects = TextEffectsBuilder::new()
//!     .shadow(Vec2::new(2.0, 2.0), Color::BLACK)
//!     .build();
//! renderer.draw_text_with_effects(&mut buffer, position, &effects);
//! renderer.render(&mut render_pass);
//! ```
//!
//! ## Hybrid (backwards compatible, ~16 MB)
//!
//! ```ignore
//! use astrelis_text::{FontRenderer, Text};
//!
//! let mut renderer = FontRenderer::new(context, font_system);
//! // Automatically selects best mode: small text -> bitmap, large/effects -> SDF
//! let text = Text::new("Hello World").size(16.0);
//! let mut buffer = renderer.prepare(&text);
//! renderer.draw_text(&mut buffer, Vec2::new(100.0, 100.0));
//! renderer.render(&mut render_pass);
//! ```

mod bitmap;
mod hybrid;
mod sdf;
mod shared;

// Re-export public types
pub use bitmap::BitmapTextRenderer;
pub use hybrid::FontRenderer;
pub use sdf::SdfTextRenderer;
pub use shared::{
    AtlasEntry, DecorationRenderer, DecorationVertex, GlyphPlacement, SdfAtlasEntry, SdfCacheKey,
    SdfParams, SharedContext, TextBuffer, TextRender, TextRendererConfig, TextVertex,
};

/// Create an orthographic projection matrix for screen-space rendering.
///
/// This matrix transforms from screen coordinates (top-left origin, Y down)
/// to normalized device coordinates (NDC) where:
/// - X ranges from -1 (left) to +1 (right)
/// - Y ranges from -1 (bottom) to +1 (top)
///
/// The negative Y scale factor (-2.0 / height) flips the Y axis to convert
/// from top-left origin (UI convention) to bottom-left origin (OpenGL/NDC convention).
pub(crate) fn orthographic_projection(width: f32, height: f32) -> [[f32; 4]; 4] {
    [
        [2.0 / width, 0.0, 0.0, 0.0],
        [0.0, -2.0 / height, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0, 1.0],
    ]
}

/// Base size for SDF glyph rasterization.
/// Glyphs are rasterized at this size, then scaled via shader.
pub(crate) const SDF_BASE_SIZE: f32 = 48.0;

/// Default SDF spread in pixels.
pub(crate) const SDF_DEFAULT_SPREAD: f32 = 4.0;
