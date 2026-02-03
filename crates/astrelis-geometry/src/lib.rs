//! Astrelis Geometry - Customizable 2D geometry rendering
//!
//! This crate provides:
//! - Path and shape primitives for 2D vector graphics
//! - Tessellation (converting paths/shapes to triangle meshes)
//! - Style system (strokes, fills, gradients)
//! - GPU-accelerated rendering with instancing
//! - Mathematical charting (optional "chart" feature)
//!
//! # Example
//!
//! ```ignore
//! use astrelis_geometry::*;
//!
//! // Create a geometry renderer
//! let mut renderer = GeometryRenderer::new(context);
//!
//! // Draw shapes
//! let style = Style::fill(Paint::solid(Color::RED));
//! renderer.draw_shape(&Shape::circle(Vec2::new(100.0, 100.0), 50.0), &style);
//!
//! // Draw custom paths
//! let path = PathBuilder::new()
//!     .move_to(Vec2::new(0.0, 0.0))
//!     .line_to(Vec2::new(100.0, 0.0))
//!     .quad_to(Vec2::new(150.0, 50.0), Vec2::new(100.0, 100.0))
//!     .close()
//!     .build();
//! renderer.draw_path(&path, &style);
//!
//! // Render to a pass
//! renderer.render(&mut pass, viewport);
//! ```

// Core primitives
mod curve;
mod path;
mod shape;
mod transform;

// Styling
mod fill;
mod paint;
mod stroke;
mod style;

// Tessellation
mod tessellator;
mod vertex;

// Rendering
mod dirty_ranges;
mod gpu_types;
mod instance_buffer;
mod pipeline;
mod renderer;

// Chart module (optional)
#[cfg(feature = "chart")]
pub mod chart;

// Re-exports
pub use curve::*;
pub use path::*;
pub use shape::*;
pub use transform::*;

pub use fill::*;
pub use paint::*;
pub use stroke::*;
pub use style::*;

pub use tessellator::*;
pub use vertex::*;

pub use gpu_types::*;
pub use renderer::*;
