//! GPU-accelerated chart renderers.
//!
//! This module provides specialized renderers for different chart types,
//! all using GPU instancing for efficient rendering of large datasets.

pub mod area;
pub mod bar;
pub mod line;
pub mod scatter;

pub use area::GpuChartAreaRenderer;
pub use bar::GpuChartBarRenderer;
pub use line::{GpuChartLineRenderer, SeriesGpuState, GPU_RENDER_THRESHOLD};
pub use scatter::GpuChartScatterRenderer;
