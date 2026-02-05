//! Mathematical charting module.
//!
//! Provides chart types for data visualization:
//! - Line charts
//! - Bar charts
//! - Scatter plots
//! - Function plots
//! - Area charts
//!
//! # Features
//!
//! - **Multiple axes**: Support for primary and secondary axes on all sides
//! - **Custom axes**: Unlimited named axes with different scales
//! - **Grid configuration**: Major/minor/tertiary grid lines with dash patterns
//! - **Annotations**: Text, line, and region annotations
//! - **Fill regions**: Fill between series, horizontal/vertical bands
//! - **Interactivity**: Pan, zoom, and hover support
//! - **Caching**: Coordinate caching and spatial indexing for large datasets
//! - **Streaming**: Ring buffers for efficient real-time data
//!
//! # Example
//!
//! ```ignore
//! use astrelis_geometry::chart::*;
//!
//! let chart = ChartBuilder::line()
//!     .title("Temperature Over Time")
//!     .x_label("Time (hours)")
//!     .y_label("Temperature (C)")
//!     .add_series("Indoor", &[(0.0, 20.0), (1.0, 21.0), (2.0, 22.5)])
//!     .add_series("Outdoor", &[(0.0, 15.0), (1.0, 18.0), (2.0, 16.0)])
//!     .with_grid()
//!     .interactive(true)
//!     .build();
//!
//! chart_renderer.draw(&chart, bounds);
//! ```

// Core modules
mod axis;
mod builder;
mod cache;
mod data;
mod gpu;
mod grid;
pub mod rect;
mod renderer;
pub mod renderers;
mod streaming;
mod style;
mod types;

// Text rendering module (requires chart-text feature)
#[cfg(feature = "chart-text")]
mod text;

// Re-exports
pub use axis::*;
pub use builder::*;
pub use cache::*;
pub use data::*;
pub use gpu::*;
pub use grid::*;
pub use rect::Rect;
pub use renderer::*;
pub use renderers::*;
pub use streaming::*;
pub use style::*;
pub use types::*;

#[cfg(feature = "chart-text")]
pub use text::*;

#[cfg(feature = "egui-integration")]
mod egui_widget;
#[cfg(feature = "egui-integration")]
pub use egui_widget::*;

#[cfg(feature = "ui-integration")]
mod ui_widget;
#[cfg(feature = "ui-integration")]
pub use ui_widget::*;
