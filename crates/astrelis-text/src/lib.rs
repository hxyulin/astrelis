//! Astrelis Text - Text rendering with cosmic-text
//!
//! This crate provides modular text rendering capabilities:
//! - Font management with system fonts and custom fonts
//! - Text builder with styling (size, color, alignment, etc.)
//! - GPU-accelerated text rendering with zero-cost backend selection
//! - Signed Distance Field (SDF) rendering for scalable text and effects
//!
//! ## Zero-Cost Renderer Selection
//!
//! Choose the renderer that fits your memory budget:
//!
//! | Renderer | Memory | Use Case |
//! |----------|--------|----------|
//! | [`BitmapTextRenderer`] | ~8 MB | Small text, UI labels, no effects needed |
//! | [`SdfTextRenderer`] | ~8 MB | Large text, titles, needs shadows/outlines/glows |
//! | [`FontRenderer`] | ~16 MB | Mixed usage, backwards compatibility (default) |
//!
//! Memory can be further reduced with [`TextRendererConfig`]:
//! - `small()`: 512x512 atlas (~1 MB per renderer)
//! - `medium()`: 1024x1024 atlas (~4 MB per renderer)
//! - `large()`: 2048x2048 atlas (~8 MB per renderer, default)
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use astrelis_text::{FontSystem, FontRenderer, Text, Color};
//! use astrelis_render::GraphicsContext;
//! use astrelis_core::math::Vec2;
//!
//! let context = GraphicsContext::new_owned_sync();
//! let font_system = FontSystem::with_system_fonts();
//! let mut renderer = FontRenderer::new(context, font_system);
//!
//! // Create styled text with builder pattern
//! let text = Text::new("Hello, World!")
//!     .size(24.0)
//!     .color(Color::WHITE)
//!     .bold();
//!
//! // Prepare and draw
//! let mut buffer = renderer.prepare(&text);
//! renderer.draw_text(&mut buffer, Vec2::new(100.0, 100.0));
//!
//! // Render to a render pass
//! // renderer.render(render_pass, viewport_size);
//! ```
//!
//! ## Features
//!
//! - **System Fonts**: Automatically loads all system fonts
//! - **Custom Fonts**: Load .ttf and .otf files from disk or memory
//! - **Rich Styling**: Font size, weight, style, color, alignment, wrapping
//! - **Builder Pattern**: Fluent API for text configuration
//! - **GPU Accelerated**: WGPU-based rendering with texture atlas
//! - **Text Layout**: Multi-line text with automatic wrapping
//! - **Asset Integration**: Load fonts through the asset system (with `asset` feature)
//! - **SDF Rendering**: Resolution-independent text scaling and effects
//! - **Text Effects**: Shadows, outlines, glows, and more
//!
//! ## SDF (Signed Distance Field) Rendering
//!
//! SDF rendering enables sharp text at any scale and high-quality effects. The renderer uses
//! a hybrid approach for optimal quality:
//!
//! - **Bitmap atlas** for small text (< 24px) without effects - sharper at small sizes
//! - **SDF atlas** for large text (>= 24px) or text with effects - scalable and smooth
//!
//! ### When to Use SDF
//!
//! SDF rendering is automatically enabled for:
//! - Large text (24px and above)
//! - Text with effects (shadows, outlines, glows)
//! - Text that needs to scale dynamically
//!
//! ### Basic SDF Usage
//!
//! ```rust,no_run
//! use astrelis_text::{Text, TextEffect, Color};
//! use astrelis_core::math::Vec2;
//!
//! // Text with a drop shadow
//! let text = Text::new("Hello")
//!     .size(32.0)
//!     .with_shadow(Vec2::new(2.0, 2.0), Color::rgba(0.0, 0.0, 0.0, 0.5));
//!
//! // Text with an outline
//! let text = Text::new("Bold")
//!     .size(48.0)
//!     .with_outline(2.0, Color::BLACK);
//!
//! // Combine multiple effects
//! let text = Text::new("Glowing")
//!     .size(36.0)
//!     .with_shadow(Vec2::new(1.0, 1.0), Color::BLACK)
//!     .with_outline(1.5, Color::WHITE)
//!     .with_glow(4.0, Color::BLUE, 0.8);
//! ```
//!
//! ### Force SDF Mode
//!
//! You can force SDF rendering for better scalability:
//!
//! ```rust,no_run
//! use astrelis_text::Text;
//!
//! let text = Text::new("Scalable")
//!     .size(16.0)
//!     .sdf();  // Force SDF even for small text
//! ```
//!
//! ## Examples
//!
//! Run the examples to see text rendering in action:
//!
//! ```bash
//! cargo run --package astrelis-text --example text_demo
//! cargo run --package astrelis-text --example text_effects
//! cargo run --package astrelis-text --example rich_text_demo
//! ```

pub mod cache;
pub mod decoration;
pub mod editor;
pub mod effects;
pub mod font;
pub mod pipeline;
pub mod renderer;
pub mod rich_text;
pub mod sdf;
pub mod shaping;
pub mod text;

#[cfg(feature = "asset")]
pub mod asset;

// Re-export main types
pub use cache::{ShapeKey, ShapedTextData, TextShapeCache};
pub use decoration::{
    BackgroundGeometry, DecorationGeometry, DecorationQuad, DecorationQuadType, LineStyle,
    StrikethroughStyle, TextBounds, TextDecoration, UnderlineStyle, generate_decoration_geometry,
    generate_decoration_quads,
};
pub use editor::{TextCursor, TextEditor, TextSelection};
pub use effects::{
    EffectRenderConfig, TextEffect, TextEffectType, TextEffects, TextEffectsBuilder,
};
pub use font::{FontAttributes, FontDatabase, FontStretch, FontStyle, FontSystem, FontWeight};
pub use pipeline::{
    RequestId, ShapedTextResult as PipelineShapedTextResult, SyncTextShaper, TextPipeline,
    TextShapeRequest, TextShaper,
};
pub use renderer::{
    // Renderers
    BitmapTextRenderer, FontRenderer, SdfTextRenderer,
    // Common types
    AtlasEntry, DecorationRenderer, DecorationVertex, GlyphPlacement, SdfAtlasEntry, SdfCacheKey,
    SdfParams, SharedContext, TextBuffer, TextRender, TextRendererConfig, TextVertex,
};
pub use rich_text::{RichText, RichTextBuilder, TextSpan, TextSpanStyle};
pub use sdf::{SdfConfig, TextRenderMode, generate_sdf, generate_sdf_smooth};
pub use shaping::{
    extract_glyphs_from_buffer, measure_text_fast, shape_text, ShapedGlyph, ShapedTextResult,
};
pub use text::{LineBreakConfig, Text, TextAlign, TextMetrics, TextWrap, VerticalAlign};

// Re-export asset types when feature is enabled
#[cfg(feature = "asset")]
pub use asset::{FontAsset, FontFormat, FontLoader};

// Re-export cosmic-text types needed for retained rendering
pub use cosmic_text::CacheKey;

// Re-export Color from astrelis-render
pub use astrelis_render::Color;

// Re-export math types from astrelis-core
pub use astrelis_core::math::Vec2;
