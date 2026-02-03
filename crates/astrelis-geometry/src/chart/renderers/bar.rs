//! GPU-accelerated bar renderer for charts.

use super::super::rect::Rect;
use super::super::types::Chart;
use super::line::SeriesGpuState;
use astrelis_core::profiling::profile_scope;
use astrelis_render::{wgpu, GraphicsContext, Quad, QuadRenderer, Viewport};
use std::sync::Arc;

/// GPU-accelerated bar renderer for charts.
///
/// Uses `QuadRenderer` for efficient instanced rendering of bar charts.
/// Bars are stored in data coordinates; the GPU transforms them
/// to screen coordinates, so pan/zoom only updates a uniform buffer.
pub struct GpuChartBarRenderer {
    quad_renderer: QuadRenderer,
    /// Per-series state tracking.
    series_states: Vec<SeriesGpuState>,
    /// Global data version counter.
    data_version: u64,
}

impl std::fmt::Debug for GpuChartBarRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuChartBarRenderer")
            .field("series_states", &self.series_states)
            .field("data_version", &self.data_version)
            .field("quad_count", &self.quad_renderer.quad_count())
            .finish()
    }
}

impl GpuChartBarRenderer {
    /// Create a new GPU chart bar renderer.
    ///
    /// The `target_format` must match the render target this renderer will draw into.
    pub fn new(context: Arc<GraphicsContext>, target_format: wgpu::TextureFormat) -> Self {
        Self {
            quad_renderer: QuadRenderer::new(context, target_format),
            series_states: Vec::new(),
            data_version: 0,
        }
    }

    /// Increment the data version, forcing a rebuild on next prepare.
    pub fn mark_data_changed(&mut self) {
        self.data_version = self.data_version.wrapping_add(1);
    }

    /// Prepare bar quads for rendering.
    ///
    /// Returns `true` if the buffer was rebuilt.
    pub fn prepare(&mut self, chart: &Chart) -> bool {
        profile_scope!("gpu_chart_bar_prepare");

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

        // Rebuild all bars
        self.quad_renderer.clear();

        let bar_width = chart.bar_config.bar_width;
        let gap = chart.bar_config.gap;
        let series_count = chart.series.len() as f32;
        let _total_width = bar_width * series_count + gap * (series_count - 1.0);

        // Convert bar_width to data units (approximate - assumes uniform scale)
        // Note: This is a simplification. For precise bar widths, we'd need
        // to know the data range during prepare. For now, we use a fraction
        // of the data range.
        let (x_min, x_max) = chart.x_range();
        let x_range = x_max - x_min;
        let data_bar_width = (bar_width as f64 / 800.0) * x_range; // Assume ~800px width
        let data_total_width = data_bar_width * series_count as f64;
        let data_gap = (gap as f64 / 800.0) * x_range;

        for (series_idx, series) in chart.series.iter().enumerate() {
            let (y_min, _) = chart.axis_range(series.y_axis);
            let data_offset = series_idx as f64 * (data_bar_width + data_gap) - data_total_width * 0.5;

            let color = series.style.color;

            // Add bars in DATA coordinates
            for point in &series.data {
                let x_center = point.x + data_offset;
                let y_bottom = y_min;
                let y_top = point.y;

                self.quad_renderer.add(Quad::bar(
                    x_center as f32,
                    data_bar_width as f32,
                    y_bottom as f32,
                    y_top as f32,
                    color,
                ));
            }

            // Update series state
            self.series_states[series_idx].last_point_count = series.data.len();
            self.series_states[series_idx].data_version = self.data_version;
        }

        // Upload to GPU
        self.quad_renderer.prepare();

        tracing::trace!(
            "GPU chart bar renderer: rebuilt {} quads",
            self.quad_renderer.quad_count()
        );

        true
    }

    /// Render bars using GPU instancing with data coordinate transformation.
    pub fn render(
        &self,
        pass: &mut wgpu::RenderPass,
        viewport: Viewport,
        plot_area: &Rect,
        chart: &Chart,
    ) {
        profile_scope!("gpu_chart_bar_render");

        if self.quad_renderer.quad_count() == 0 {
            return;
        }

        // Set scissor rect to clip bars to the plot area
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

        self.quad_renderer.render_with_data_transform(
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

    /// Get the number of quads (bars).
    pub fn quad_count(&self) -> usize {
        self.quad_renderer.quad_count()
    }
}
