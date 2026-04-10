//! Astrelis Text - CPU-side text shaping and font management.
//!
//! This crate provides modular text capabilities without any GPU dependency:
//! - Font management with system fonts and custom fonts
//! - Text builder with styling (size, color, alignment, etc.)
//! - Rich text with per-span styling (font, size, color, weight, decorations)
//! - Text shaping via cosmic-text
//! - SDF (Signed Distance Field) generation for scalable text effects
//! - Shape caching and pipeline for performance
//! - Asset integration for font loading (with `asset` feature)
//!
//! GPU rendering is provided by the companion crate `astrelis-text-wgpu`.
//!
//! # Features
//!
//! - `asset` (default) — Enables [`FontAsset`] and [`FontLoader`] for
//!   integration with the `astrelis-assets` system.
//!
//! # Quick Start
//!
//! ```
//! use astrelis_text::{Text, FontWeight};
//! use astrelis_core::color::Color;
//!
//! let text = Text::new("Hello, World!")
//!     .size(24.0)
//!     .color(Color::WHITE)
//!     .bold();
//!
//! assert_eq!(text.font_size, 24.0);
//! assert_eq!(text.weight, FontWeight::Bold);
//! ```

#![warn(missing_docs)]

pub mod cache;
pub mod decoration;
pub mod effects;
pub mod error;
pub mod font;
pub mod pipeline;
pub mod rich_text;
pub mod sdf;
pub mod shaping;
pub mod text;

#[cfg(feature = "asset")]
pub mod asset;

// Re-export main types
pub use cache::{HashMapShapeCache, ShapeCache, ShapeKey};
pub use decoration::{
    DecorationQuad, DecorationQuadType, LineStyle, StrikethroughStyle, TextBounds,
    TextDecoration, UnderlineStyle, generate_decoration_quads,
};
pub use effects::{
    EffectRenderConfig, TextEffect, TextEffectType, TextEffects, TextEffectsBuilder,
};
pub use error::{TextError, TextResult};
pub use font::{FontAttributes, FontDatabase, FontStretch, FontStyle, FontSystem, FontWeight};
pub use pipeline::{
    RequestId, ShapedTextResult as PipelineShapedTextResult, SyncTextShaper, TextPipeline,
    TextShapeRequest, TextShaper,
};
pub use rich_text::{RichText, RichTextBuilder, TextSpan, TextSpanStyle};
pub use sdf::{SdfConfig, TextRenderMode, generate_sdf, generate_sdf_smooth, swash_image_to_grayscale};
pub use shaping::{ShapedGlyph, ShapedTextResult, extract_glyphs_from_buffer, measure_text_fast, shape_text};
pub use text::{LineBreakConfig, Text, TextAlign, TextMetrics, TextWrap, VerticalAlign};

// Re-export asset types when feature is enabled
#[cfg(feature = "asset")]
pub use asset::{FontAsset, FontFormat, FontLoader};

// Re-export cosmic-text types needed for retained rendering
pub use cosmic_text::CacheKey;
