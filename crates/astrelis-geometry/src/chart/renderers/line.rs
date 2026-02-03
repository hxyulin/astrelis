//! GPU-accelerated line renderer for charts.

use super::super::rect::Rect;
use super::super::types::Chart;
use astrelis_core::profiling::profile_scope;
use astrelis_render::{wgpu, GraphicsContext, LineRenderer, LineSegment, Viewport};
use glam::Vec2;
use std::sync::Arc;

/// Threshold for switching to GPU-accelerated rendering.
/// Charts with more than this many points per series will use the GPU path.
pub const GPU_RENDER_THRESHOLD: usize = 500;

/// Per-series GPU state for tracking when to rebuild line buffers.
#[derive(Debug, Default)]
pub struct SeriesGpuState {
    /// Number of data points when the buffer was last built.
    pub last_point_count: usize,
    /// Data version when buffer was last built.
    pub data_version: u64,
}

/// GPU-accelerated line renderer for charts.
///
/// Uses `LineRenderer` for efficient instanced rendering of large datasets.
/// Line segments are stored in data coordinates; the GPU transforms them
/// to screen coordinates, so pan/zoom only updates a uniform buffer.
pub struct GpuChartLineRenderer {
    line_renderer: LineRenderer,
    /// Per-series state tracking.
    series_states: Vec<SeriesGpuState>,
    /// Global data version counter.
    data_version: u64,
}

impl std::fmt::Debug for GpuChartLineRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuChartLineRenderer")
            .field("series_states", &self.series_states)
            .field("data_version", &self.data_version)
            .field("segment_count", &self.line_renderer.segment_count())
            .finish()
    }
}

impl GpuChartLineRenderer {
    /// Create a new GPU chart line renderer.
    ///
    /// The `target_format` must match the render target this renderer will draw into.
    pub fn new(context: Arc<GraphicsContext>, target_format: wgpu::TextureFormat) -> Self {
        Self {
            line_renderer: LineRenderer::new(context, target_format),
            series_states: Vec::new(),
            data_version: 0,
        }
    }

    /// Increment the data version, forcing a rebuild on next prepare.
    pub fn mark_data_changed(&mut self) {
        self.data_version = self.data_version.wrapping_add(1);
    }

    /// Prepare line segments for rendering.
    ///
    /// This checks if data has changed and only rebuilds the instance buffer
    /// when necessary. Returns `true` if the buffer was rebuilt.
    pub fn prepare(&mut self, chart: &Chart) -> bool {
        profile_scope!("gpu_chart_line_prepare");

        // Ensure we have enough series states
        while self.series_states.len() < chart.series.len() {
            self.series_states.push(SeriesGpuState::default());
        }

        // Check if any series needs rebuilding
        let mut needs_rebuild = false;
        for (idx, series) in chart.series.iter().enumerate() {
            let state = &self.series_states[idx];
            if state.last_point_count != series.data.len()
                || state.data_version != self.data_version
            {
                needs_rebuild = true;
                break;
            }
        }

        if !needs_rebuild {
            return false;
        }

        // Rebuild all line segments
        self.line_renderer.clear();

        for (series_idx, series) in chart.series.iter().enumerate() {
            if series.data.len() < 2 {
                continue;
            }

            let color = series.style.color;
            let width = series.style.line_width;

            // Add line segments in DATA coordinates (not screen coordinates).
            // The GPU shader will transform them using the data-to-screen matrix.
            for i in 0..series.data.len() - 1 {
                let p0 = &series.data[i];
                let p1 = &series.data[i + 1];

                self.line_renderer.add_segment(LineSegment::new(
                    Vec2::new(p0.x as f32, p0.y as f32),
                    Vec2::new(p1.x as f32, p1.y as f32),
                    width,
                    color,
                ));
            }

            // Update series state
            self.series_states[series_idx].last_point_count = series.data.len();
            self.series_states[series_idx].data_version = self.data_version;
        }

        // Upload to GPU
        self.line_renderer.prepare();

        tracing::trace!(
            "GPU chart line renderer: rebuilt {} segments",
            self.line_renderer.segment_count()
        );

        true
    }

    /// Prepare only for specific series that changed (partial update).
    ///
    /// Used by streaming charts to only rebuild when data actually changes.
    pub fn prepare_series(&mut self, chart: &Chart, series_indices: &[usize]) -> bool {
        profile_scope!("gpu_chart_line_prepare_partial");

        // For now, just do a full rebuild if any series changed
        // A more sophisticated implementation could do incremental updates
        let mut any_changed = false;
        for &idx in series_indices {
            if let Some(series) = chart.series.get(idx) {
                if idx < self.series_states.len() {
                    let state = &self.series_states[idx];
                    if state.last_point_count != series.data.len()
                        || state.data_version != self.data_version
                    {
                        any_changed = true;
                        break;
                    }
                } else {
                    any_changed = true;
                    break;
                }
            }
        }

        if any_changed {
            self.prepare(chart)
        } else {
            false
        }
    }

    /// Render line series using GPU instancing with data coordinate transformation.
    ///
    /// This is the fast path: line data stays constant, only the transform uniform
    /// is updated for pan/zoom operations.
    ///
    /// **Note**: This method sets a scissor rect to clip lines to the plot area.
    /// You may want to reset the scissor rect after calling this method if you
    /// need to render additional content outside the plot area.
    pub fn render(
        &self,
        pass: &mut wgpu::RenderPass,
        viewport: Viewport,
        plot_area: &Rect,
        chart: &Chart,
    ) {
        profile_scope!("gpu_chart_line_render");

        if self.line_renderer.segment_count() == 0 {
            return;
        }

        // Set scissor rect to clip lines to the plot area
        // This is critical for proper chart rendering - lines outside the plot
        // area should not be visible.
        let scale_factor = viewport.scale_factor.0 as f32;

        // Convert plot area to physical pixels for scissor rect
        let scissor_x = (plot_area.x * scale_factor).round() as u32;
        let scissor_y = (plot_area.y * scale_factor).round() as u32;
        let scissor_width = (plot_area.width * scale_factor).round() as u32;
        let scissor_height = (plot_area.height * scale_factor).round() as u32;

        // Clamp to viewport bounds (in physical pixels)
        let max_width = viewport.size.width as u32;
        let max_height = viewport.size.height as u32;
        let scissor_x = scissor_x.min(max_width);
        let scissor_y = scissor_y.min(max_height);
        let scissor_width = scissor_width.min(max_width.saturating_sub(scissor_x));
        let scissor_height = scissor_height.min(max_height.saturating_sub(scissor_y));

        pass.set_scissor_rect(scissor_x, scissor_y, scissor_width, scissor_height);

        // Get data ranges from chart
        let (x_min, x_max) = chart.x_range();
        let (y_min, y_max) = chart.y_range();

        self.line_renderer.render_with_data_transform(
            pass,
            viewport,
            plot_area.x,
            plot_area.y,
            plot_area.width,
            plot_area.height,
            x_min,
            x_max,
            y_min,
            y_max,
        );
    }

    /// Render without setting scissor rect (caller is responsible for clipping).
    ///
    /// Use this if you've already set the scissor rect appropriately.
    pub fn render_no_scissor(
        &self,
        pass: &mut wgpu::RenderPass,
        viewport: Viewport,
        plot_area: &Rect,
        chart: &Chart,
    ) {
        profile_scope!("gpu_chart_line_render_no_scissor");

        if self.line_renderer.segment_count() == 0 {
            return;
        }

        let (x_min, x_max) = chart.x_range();
        let (y_min, y_max) = chart.y_range();

        self.line_renderer.render_with_data_transform(
            pass,
            viewport,
            plot_area.x,
            plot_area.y,
            plot_area.width,
            plot_area.height,
            x_min,
            x_max,
            y_min,
            y_max,
        );
    }

    /// Get the number of line segments.
    pub fn segment_count(&self) -> usize {
        self.line_renderer.segment_count()
    }

    /// Get the underlying line renderer for advanced use.
    pub fn line_renderer(&self) -> &LineRenderer {
        &self.line_renderer
    }

    /// Get mutable access to the underlying line renderer.
    pub fn line_renderer_mut(&mut self) -> &mut LineRenderer {
        &mut self.line_renderer
    }
}
