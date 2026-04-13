//! 2D rendering pipeline for the Astrelis engine.
//!
//! Provides an immediate-mode drawing API with automatic sorting and
//! batching. Supports textured sprites, shape primitives (rectangles,
//! circles, lines), and an orthographic camera.
//!
//! # Architecture
//!
//! - [`Renderer2D`] — the main drawing API (begin/end pattern)
//! - [`Camera2D`] — orthographic camera producing a view-projection matrix
//! - [`TextureHandle`] — handle to a registered texture
//! - [`SpriteOptions`] — per-sprite drawing options (tint, flip, scale)
//!
//! # Example
//!
//! ```ignore
//! let mut renderer = Renderer2D::new(&gpu, surface_format);
//! let tex = renderer.register_texture(&gpu, &view, &sampler, 64, 64);
//!
//! renderer.begin(&camera);
//! renderer.draw_sprite(tex, Vec2::new(100.0, 100.0), &SpriteOptions::default());
//! renderer.draw_rect_filled(Vec2::new(200.0, 200.0), Vec2::new(50.0, 50.0), Color::RED);
//! renderer.draw_circle_filled(Vec2::new(400.0, 300.0), 25.0, Color::BLUE);
//! renderer.end(&gpu, &mut encoder, &surface_view, &camera);
//! ```

#![warn(missing_docs)]

pub mod batch;
pub mod camera;
pub mod instance;
mod pipeline;
pub mod renderer;
pub mod shapes;
pub mod sprite;

pub use batch::{BatchRenderStats, BatchRenderer2D, RenderTier};
pub use camera::Camera2D;
pub use renderer::{Renderer2D, TextureHandle};
pub use sprite::{SpriteOptions, SpriteRegion};
