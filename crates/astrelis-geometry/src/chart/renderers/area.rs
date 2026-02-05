//! GPU-accelerated area renderer for charts.

use super::super::rect::Rect;
use super::super::types::Chart;
use super::line::SeriesGpuState;
use astrelis_core::profiling::profile_scope;
use astrelis_render::{
    Color, GraphicsContext, LineRenderer, LineSegment, Quad, QuadRenderer, Viewport, wgpu,
};
use glam::Vec2;
use std::sync::Arc;

/// GPU-accelerated area renderer for charts.
///
/// Uses `LineRenderer` for the outline and a triangle-based fill approach.
/// The fill triangles are generated on CPU when data changes and uploaded
/// to GPU. The GPU transforms vertices using the data-to-screen matrix.
pub struct GpuChartAreaRenderer {
    line_renderer: LineRenderer,
    quad_renderer: QuadRenderer,
    /// Per-series state tracking.
    series_states: Vec<SeriesGpuState>,
    /// Global data version counter.
    data_version: u64,
}

impl std::fmt::Debug for GpuChartAreaRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuChartAreaRenderer")
            .field("series_states", &self.series_states)
            .field("data_version", &self.data_version)
            .field("line_segment_count", &self.line_renderer.segment_count())
            .field("fill_quad_count", &self.quad_renderer.quad_count())
            .finish()
    }
}

impl GpuChartAreaRenderer {
    /// Create a new GPU chart area renderer.
    ///
    /// The `target_format` must match the render target this renderer will draw into.
    pub fn new(context: Arc<GraphicsContext>, target_format: wgpu::TextureFormat) -> Self {
        Self {
            line_renderer: LineRenderer::new(context.clone(), target_format),
            quad_renderer: QuadRenderer::new(context, target_format),
            series_states: Vec::new(),
            data_version: 0,
        }
    }

    /// Increment the data version, forcing a rebuild on next prepare.
    pub fn mark_data_changed(&mut self) {
        self.data_version = self.data_version.wrapping_add(1);
    }

    /// Prepare area geometry for rendering.
    ///
    /// Returns `true` if the buffer was rebuilt.
    pub fn prepare(&mut self, chart: &Chart) -> bool {
        profile_scope!("gpu_chart_area_prepare");

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

        // Rebuild all area geometry
        self.line_renderer.clear();
        self.quad_renderer.clear();

        for (series_idx, series) in chart.series.iter().enumerate() {
            if series.data.len() < 2 {
                continue;
            }

            let (y_min, _) = chart.axis_range(series.y_axis);

            // Get fill color
            let fill_color = if let Some(fill) = &series.style.fill {
                Color::rgba(fill.color.r, fill.color.g, fill.color.b, fill.opacity)
            } else {
                Color::rgba(
                    series.style.color.r,
                    series.style.color.g,
                    series.style.color.b,
                    0.3,
                )
            };

            // Generate fill quads (vertical strips from baseline to data point)
            // This creates a series of quads that fill the area below the line
            for i in 0..series.data.len() - 1 {
                let p0 = &series.data[i];
                let p1 = &series.data[i + 1];

                // Create a quad from baseline to the line segment
                // Using the trapezoid formed by two adjacent points
                let x0 = p0.x as f32;
                let x1 = p1.x as f32;
                let y0 = p0.y as f32;
                let y1 = p1.y as f32;
                let y_base = y_min as f32;

                // For a proper trapezoid fill, we'd need a custom shader
                // For simplicity, we approximate with vertical strips
                // Each strip goes from baseline to midpoint of the two heights
                let y_avg = (y0 + y1) * 0.5;
                self.quad_renderer.add(Quad::new(
                    Vec2::new(x0, y_base),
                    Vec2::new(x1, y_avg),
                    fill_color,
                ));

                // Add small quad for the triangle portion above/below the average
                // This approximates the trapezoid shape
                if (y0 - y1).abs() > 0.001 {
                    if y0 < y1 {
                        // Rising edge - add upper triangle as quad approximation
                        self.quad_renderer.add(Quad::new(
                            Vec2::new(x0, y_avg),
                            Vec2::new(x1, y1),
                            fill_color,
                        ));
                    } else {
                        // Falling edge - add lower portion
                        self.quad_renderer.add(Quad::new(
                            Vec2::new(x0, y_avg),
                            Vec2::new(x1, y0),
                            fill_color,
                        ));
                    }
                }
            }

            // Add line segments for the outline
            let line_color = series.style.color;
            let line_width = series.style.line_width;

            for i in 0..series.data.len() - 1 {
                let p0 = &series.data[i];
                let p1 = &series.data[i + 1];

                self.line_renderer.add_segment(LineSegment::new(
                    Vec2::new(p0.x as f32, p0.y as f32),
                    Vec2::new(p1.x as f32, p1.y as f32),
                    line_width,
                    line_color,
                ));
            }

            // Update series state
            self.series_states[series_idx].last_point_count = series.data.len();
            self.series_states[series_idx].data_version = self.data_version;
        }

        // Upload to GPU
        self.quad_renderer.prepare();
        self.line_renderer.prepare();

        tracing::trace!(
            "GPU chart area renderer: rebuilt {} fill quads, {} line segments",
            self.quad_renderer.quad_count(),
            self.line_renderer.segment_count()
        );

        true
    }

    /// Render area fill and outline using GPU instancing.
    pub fn render(
        &self,
        pass: &mut wgpu::RenderPass,
        viewport: Viewport,
        plot_area: &Rect,
        chart: &Chart,
    ) {
        profile_scope!("gpu_chart_area_render");

        // Set scissor rect to clip to the plot area
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

        // Render fill first (behind the line)
        if self.quad_renderer.quad_count() > 0 {
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

        // Render outline on top
        if self.line_renderer.segment_count() > 0 {
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
    }

    /// Get the number of fill quads.
    pub fn quad_count(&self) -> usize {
        self.quad_renderer.quad_count()
    }

    /// Get the number of line segments.
    pub fn segment_count(&self) -> usize {
        self.line_renderer.segment_count()
    }
}
