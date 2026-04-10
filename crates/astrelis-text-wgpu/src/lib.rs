//! GPU-accelerated text rendering for the Astrelis game engine using wgpu.
//!
//! This crate provides three text renderers with different memory footprints:
//!
//! | Renderer | Memory | Use Case |
//! |----------|--------|----------|
//! | [`BitmapTextRenderer`] | ~8 MB | Small text, UI labels, no effects |
//! | [`SdfTextRenderer`] | ~8 MB | Large text, titles, effects |
//! | [`FontRenderer`] | ~16 MB | Mixed usage (default) |
//!
//! # Limitation
//!
//! This crate depends directly on `wgpu` and `astrelis-gpu`.

#![warn(missing_docs)]

pub mod atlas;
pub mod config;
pub mod renderer;

// Re-export main types
pub use config::TextRendererConfig;
pub use renderer::{
    BitmapTextRenderer, DecorationVertex, FontRenderer, SdfTextRenderer, TextBuffer, TextVertex,
};
