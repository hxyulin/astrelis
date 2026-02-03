//! GPU-accelerated scatter renderer for charts.

use super::super::rect::Rect;
use super::super::types::Chart;
use super::line::SeriesGpuState;
use astrelis_core::profiling::profile_scope;
use astrelis_render::{wgpu, GraphicsContext, Point, PointRenderer, Viewport};
use glam::Vec2;
use std::sync::Arc;

/// GPU-accelerated scatter renderer for charts.
///
/// Uses `PointRenderer` for efficient instanced rendering of large scatter datasets.
/// Points are stored in data coordinates; the GPU transforms them
/// to screen coordinates, so pan/zoom only updates a uniform buffer.
pub struct GpuChartScatterRenderer {
    point_renderer: PointRenderer,
    /// Per-series state tracking.
    series_states: Vec<SeriesGpuState>,
    /// Global data version counter.
    data_version: u64,
}

impl std::fmt::Debug for GpuChartScatterRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuChartScatterRenderer")
            .field("series_states", &self.series_states)
            .field("data_version", &self.data_version)
            .field("point_count", &self.point_renderer.point_count())
            .finish()
    }
}

impl GpuChartScatterRenderer {
    /// Create a new GPU chart scatter renderer.
    ///
    /// The `target_format` must match the render target this renderer will draw into.
    pub fn new(context: Arc<GraphicsContext>, target_format: wgpu::TextureFormat) -> Self {
        Self {
            point_renderer: PointRenderer::new(context, target_format),
            series_states: Vec::new(),
            data_version: 0,
        }
    }

    /// Increment the data version, forcing a rebuild on next prepare.
    pub fn mark_data_changed(&mut self) {
        self.data_version = self.data_version.wrapping_add(1);
    }

    /// Prepare scatter points for rendering.
    ///
    /// Returns `true` if the buffer was rebuilt.
    pub fn prepare(&mut self, chart: &Chart) -> bool {
        profile_scope!("gpu_chart_scatter_prepare");

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

        // Rebuild all points
        self.point_renderer.clear();

        let default_point_style = super::super::style::PointStyle::default();

        for (series_idx, series) in chart.series.iter().enumerate() {
            let point_style = series
                .style
                .point_style
                .as_ref()
                .unwrap_or(&default_point_style);

            let color = series.style.color;
            let size = point_style.size;

            // Add points in DATA coordinates (not screen coordinates).
            for point in &series.data {
                self.point_renderer.add(Point::new(
                    Vec2::new(point.x as f32, point.y as f32),
                    size,
                    color,
                ));
            }

            // Update series state
            self.series_states[series_idx].last_point_count = series.data.len();
            self.series_states[series_idx].data_version = self.data_version;
        }

        // Upload to GPU
        self.point_renderer.prepare();

        tracing::trace!(
            "GPU chart scatter renderer: rebuilt {} points",
            self.point_renderer.point_count()
        );

        true
    }

    /// Render scatter points using GPU instancing with data coordinate transformation.
    pub fn render(
        &self,
        pass: &mut wgpu::RenderPass,
        viewport: Viewport,
        plot_area: &Rect,
        chart: &Chart,
    ) {
        profile_scope!("gpu_chart_scatter_render");

        if self.point_renderer.point_count() == 0 {
            return;
        }

        // Set scissor rect to clip points to the plot area
        let scale_factor = viewport.scale_factor.0 as f32;
        let scissor_x = (plot_area.x * scale_factor).round() as u32;
        let scissor_y = (plot_area.y * scale_factor).round() as u32;
        let scissor_width = (plot_area.width * scale_factor).round() as u32;
        let scissor_height = (plot_area.height * scale_factor).round() as u32;

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

        self.point_renderer.render_with_data_transform(
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

    /// Get the number of points.
    pub fn point_count(&self) -> usize {
        self.point_renderer.point_count()
    }
}
