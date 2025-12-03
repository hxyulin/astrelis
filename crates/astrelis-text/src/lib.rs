//! Astrelis Text - Text rendering with cosmic-text
//!
//! This crate provides modular text rendering capabilities:
//! - Font management with system fonts and custom fonts
//! - Text builder with styling (size, color, alignment, etc.)
//! - GPU-accelerated text rendering with FontRenderer
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use astrelis_text::{FontSystem, FontRenderer, Text, Color};
//! use astrelis_render::GraphicsContext;
//! use glam::Vec2;
//!
//! let context = GraphicsContext::new_sync();
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
//!
//! ## Examples
//!
//! Run the examples to see text rendering in action:
//!
//! ```bash
//! cargo run --package astrelis-text --example simple_text
//! cargo run --package astrelis-text --example text_demo
//! ```

pub mod font;
pub mod renderer;
pub mod text;

// Re-export main types
pub use font::{FontAttributes, FontDatabase, FontStretch, FontStyle, FontSystem, FontWeight};
pub use renderer::{FontRenderer, TextBuffer};
pub use text::{Text, TextAlign, TextWrap};

// Re-export Color from astrelis-render
pub use astrelis_render::Color;

// Re-export glam for Vec2
pub use glam;
