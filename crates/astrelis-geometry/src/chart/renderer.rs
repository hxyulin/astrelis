//! Chart rendering using the geometry renderer.
//!
//! This module provides two rendering paths:
//! - **Tessellation path**: Uses Lyon for CPU tessellation (slower, but universal)
//! - **GPU path**: Uses specialized renderers for instanced GPU rendering (faster for large datasets)
//!
//! The GPU path is automatically selected for charts with >500 data points per series.

use super::rect::Rect;
use super::renderers::GPU_RENDER_THRESHOLD;
use super::renderers::GpuChartLineRenderer;
use super::types::{
    AxisId, AxisOrientation, AxisPosition, Chart, ChartType, DataPoint, FillRegionKind,
};
use crate::{GeometryRenderer, PathBuilder, ScissorRect, Stroke, Style};
use astrelis_core::profiling::profile_scope;
use astrelis_render::{Color, Viewport, wgpu};
use glam::Vec2;

/// Renders charts using a GeometryRenderer, with optional GPU acceleration.
pub struct ChartRenderer<'a> {
    geometry: &'a mut GeometryRenderer,
    /// Optional GPU line renderer for accelerated line series rendering.
    gpu_line_renderer: Option<&'a mut GpuChartLineRenderer>,
}

impl<'a> ChartRenderer<'a> {
    /// Create a new chart renderer wrapping a geometry renderer.
    pub fn new(geometry: &'a mut GeometryRenderer) -> Self {
        Self {
            geometry,
            gpu_line_renderer: None,
        }
    }

    /// Create a chart renderer with GPU acceleration for line series.
    ///
    /// When GPU acceleration is enabled and a line chart has more than
    /// `GPU_RENDER_THRESHOLD` points, it will use GPU instancing for
    /// much faster rendering.
    pub fn with_gpu_line_renderer(
        geometry: &'a mut GeometryRenderer,
        gpu_line_renderer: &'a mut GpuChartLineRenderer,
    ) -> Self {
        Self {
            geometry,
            gpu_line_renderer: Some(gpu_line_renderer),
        }
    }

    /// Render a chart within the given bounds.
    ///
    /// This method accumulates geometry commands. Call `render()` afterwards
    /// to draw everything to a render pass.
    ///
    /// For GPU-accelerated line charts, use `draw_with_gpu_lines()` instead.
    pub fn draw(&mut self, chart: &Chart, bounds: Rect) {
        self.draw_internal(chart, bounds, false);
    }

    /// Render a chart with GPU-accelerated line series.
    ///
    /// This method accumulates non-line geometry commands. Large line series
    /// (>500 points) will NOT be drawn - they should be rendered separately
    /// via `GpuChartLineRenderer` for much better performance.
    ///
    /// Returns the plot area rectangle for use with GPU line rendering.
    pub fn draw_with_gpu_lines(&mut self, chart: &Chart, bounds: Rect) -> Rect {
        let plot_area = bounds.inset(chart.padding);
        self.draw_internal(chart, bounds, true);
        plot_area
    }

    /// Internal draw implementation with GPU line skip option.
    fn draw_internal(&mut self, chart: &Chart, bounds: Rect, skip_gpu_lines: bool) {
        profile_scope!("chart_draw");

        // When skip_gpu_lines is true, we skip large line series (they'll be rendered via GPU).
        // When false, we draw everything via tessellation.
        let use_gpu_for_lines = skip_gpu_lines && chart.chart_type == ChartType::Line;

        // Draw background
        self.geometry
            .draw_rect(bounds.position(), bounds.size(), chart.background_color);

        // Calculate plot area (inside padding and axis labels)
        let plot_area = bounds.inset(chart.padding);

        // Draw grid lines (outside scissor so they extend to edges)
        {
            profile_scope!("draw_grid");
            self.draw_grid(chart, &plot_area);
        }

        // Draw axes (outside scissor)
        {
            profile_scope!("draw_axes");
            self.draw_all_axes(chart, &plot_area);
        }

        // Set scissor to clip data series to plot area
        self.geometry.set_scissor(ScissorRect::from_f32(
            plot_area.x,
            plot_area.y,
            plot_area.width,
            plot_area.height,
        ));

        // Draw fill regions (clipped to plot area)
        {
            profile_scope!("draw_fill_regions");
            self.draw_fill_regions(chart, &plot_area);
        }

        // Draw line annotations (clipped to plot area)
        {
            profile_scope!("draw_line_annotations");
            self.draw_line_annotations(chart, &plot_area);
        }

        // Draw data series (clipped to plot area)
        {
            profile_scope!("draw_series");
            match chart.chart_type {
                ChartType::Line => {
                    if use_gpu_for_lines {
                        // Only draw small series via tessellation, skip large ones for GPU
                        self.draw_line_series_tessellated_only(chart, &plot_area);
                    } else {
                        self.draw_line_series(chart, &plot_area);
                    }
                }
                ChartType::Scatter => self.draw_scatter_series(chart, &plot_area),
                ChartType::Bar => self.draw_bar_series(chart, &plot_area),
                ChartType::Area => self.draw_area_series(chart, &plot_area),
            }
        }

        // Draw crosshair if enabled and hovering (clipped to plot area)
        if chart.show_crosshair {
            profile_scope!("draw_crosshair");
            self.draw_crosshair(chart, &plot_area);
        }

        // Reset scissor for any subsequent drawing
        self.geometry.reset_scissor();
    }

    /// Draw only small line series via tessellation (for hybrid rendering).
    fn draw_line_series_tessellated_only(&mut self, chart: &Chart, plot_area: &Rect) {
        profile_scope!("draw_line_series_small");
        for series in &chart.series {
            // Skip large series - they'll be rendered via GPU
            if series.data.len() > GPU_RENDER_THRESHOLD {
                continue;
            }

            if series.data.len() < 2 {
                continue;
            }

            self.draw_single_line_series_tessellated(chart, series, plot_area);
        }
    }

    /// Draw a single line series using tessellation.
    fn draw_single_line_series_tessellated(
        &mut self,
        chart: &Chart,
        series: &super::types::Series,
        plot_area: &Rect,
    ) {
        // Get visible X range with buffer for smooth scrolling
        let (x_min, x_max) = chart.axis_range(series.x_axis);
        let x_range = x_max - x_min;
        let buffer = x_range * 0.1;
        let visible_x_min = x_min - buffer;
        let visible_x_max = x_max + buffer;

        // Find visible point range using binary search
        let (start_idx, end_idx) =
            Self::find_visible_range(&series.data, visible_x_min, visible_x_max);

        if end_idx <= start_idx + 1 {
            return;
        }

        // Build path for the visible portion of the line
        let mut builder = PathBuilder::new();
        let first_point = self.data_to_pixel_with_axes(
            chart,
            plot_area,
            series.data[start_idx].x,
            series.data[start_idx].y,
            series.x_axis,
            series.y_axis,
        );
        builder.move_to(first_point);

        for point in &series.data[start_idx + 1..end_idx] {
            let pixel = self.data_to_pixel_with_axes(
                chart,
                plot_area,
                point.x,
                point.y,
                series.x_axis,
                series.y_axis,
            );
            builder.line_to(pixel);
        }

        let path = builder.build();
        let stroke = Stroke::solid(series.style.color, series.style.line_width);
        self.geometry.draw_path_stroke(&path, &stroke);

        // Draw points if enabled
        if let Some(point_style) = &series.style.point_style {
            for point in &series.data[start_idx..end_idx] {
                let pixel = self.data_to_pixel_with_axes(
                    chart,
                    plot_area,
                    point.x,
                    point.y,
                    series.x_axis,
                    series.y_axis,
                );
                self.geometry
                    .draw_circle(pixel, point_style.size * 0.5, point_style.color);
            }
        }
    }

    /// Convert data coordinates to pixel coordinates using specific axes.
    fn data_to_pixel_with_axes(
        &self,
        chart: &Chart,
        plot_area: &Rect,
        x: f64,
        y: f64,
        x_axis_id: AxisId,
        y_axis_id: AxisId,
    ) -> Vec2 {
        let (x_min, x_max) = chart.axis_range(x_axis_id);
        let (y_min, y_max) = chart.axis_range(y_axis_id);

        let px = plot_area.x + ((x - x_min) / (x_max - x_min)) as f32 * plot_area.width;
        // Y is inverted (0 at top in screen coords)
        let py = plot_area.y + plot_area.height
            - ((y - y_min) / (y_max - y_min)) as f32 * plot_area.height;

        Vec2::new(px, py)
    }

    /// Convert data coordinates to pixel coordinates (primary axes).
    fn data_to_pixel(&self, chart: &Chart, plot_area: &Rect, x: f64, y: f64) -> Vec2 {
        self.data_to_pixel_with_axes(chart, plot_area, x, y, AxisId::X_PRIMARY, AxisId::Y_PRIMARY)
    }

    /// Convert pixel coordinates to data coordinates.
    pub fn pixel_to_data(&self, chart: &Chart, plot_area: &Rect, pixel: Vec2) -> DataPoint {
        let (x_min, x_max) = chart.x_range();
        let (y_min, y_max) = chart.y_range();

        let x = x_min + ((pixel.x - plot_area.x) / plot_area.width) as f64 * (x_max - x_min);
        let y = y_max - ((pixel.y - plot_area.y) / plot_area.height) as f64 * (y_max - y_min);

        DataPoint::new(x, y)
    }

    fn draw_fill_regions(&mut self, chart: &Chart, plot_area: &Rect) {
        for region in &chart.fill_regions {
            match &region.kind {
                FillRegionKind::HorizontalBand { y_min, y_max } => {
                    let (x_range_min, x_range_max) = chart.axis_range(region.x_axis);
                    let top_left = self.data_to_pixel_with_axes(
                        chart,
                        plot_area,
                        x_range_min,
                        *y_max,
                        region.x_axis,
                        region.y_axis,
                    );
                    let bottom_right = self.data_to_pixel_with_axes(
                        chart,
                        plot_area,
                        x_range_max,
                        *y_min,
                        region.x_axis,
                        region.y_axis,
                    );

                    self.geometry.draw_rect(
                        Vec2::new(top_left.x, top_left.y),
                        Vec2::new(bottom_right.x - top_left.x, bottom_right.y - top_left.y),
                        region.color,
                    );
                }
                FillRegionKind::VerticalBand { x_min, x_max } => {
                    let (y_range_min, y_range_max) = chart.axis_range(region.y_axis);
                    let top_left = self.data_to_pixel_with_axes(
                        chart,
                        plot_area,
                        *x_min,
                        y_range_max,
                        region.x_axis,
                        region.y_axis,
                    );
                    let bottom_right = self.data_to_pixel_with_axes(
                        chart,
                        plot_area,
                        *x_max,
                        y_range_min,
                        region.x_axis,
                        region.y_axis,
                    );

                    self.geometry.draw_rect(
                        Vec2::new(top_left.x, top_left.y),
                        Vec2::new(bottom_right.x - top_left.x, bottom_right.y - top_left.y),
                        region.color,
                    );
                }
                FillRegionKind::BelowSeries {
                    series_index,
                    y_baseline,
                } => {
                    if let Some(series) = chart.series.get(*series_index) {
                        if series.data.len() < 2 {
                            continue;
                        }

                        let mut builder = PathBuilder::new();

                        // Start at baseline
                        let base_start = self.data_to_pixel_with_axes(
                            chart,
                            plot_area,
                            series.data[0].x,
                            *y_baseline,
                            series.x_axis,
                            series.y_axis,
                        );
                        builder.move_to(base_start);

                        // Line up to first data point
                        let first = self.data_to_pixel_with_axes(
                            chart,
                            plot_area,
                            series.data[0].x,
                            series.data[0].y,
                            series.x_axis,
                            series.y_axis,
                        );
                        builder.line_to(first);

                        // Follow series
                        for point in &series.data[1..] {
                            let p = self.data_to_pixel_with_axes(
                                chart,
                                plot_area,
                                point.x,
                                point.y,
                                series.x_axis,
                                series.y_axis,
                            );
                            builder.line_to(p);
                        }

                        // Close to baseline
                        let base_end = self.data_to_pixel_with_axes(
                            chart,
                            plot_area,
                            series.data.last().unwrap().x,
                            *y_baseline,
                            series.x_axis,
                            series.y_axis,
                        );
                        builder.line_to(base_end);
                        builder.close();

                        let path = builder.build();
                        let style = Style::fill_color(region.color);
                        self.geometry.draw_path(&path, &style);
                    }
                }
                FillRegionKind::BetweenSeries {
                    series_index_1,
                    series_index_2,
                } => {
                    let series1 = chart.series.get(*series_index_1);
                    let series2 = chart.series.get(*series_index_2);

                    if let (Some(s1), Some(s2)) = (series1, series2) {
                        if s1.data.is_empty() || s2.data.is_empty() {
                            continue;
                        }

                        let mut builder = PathBuilder::new();

                        // Forward along series 1
                        let first = self.data_to_pixel_with_axes(
                            chart,
                            plot_area,
                            s1.data[0].x,
                            s1.data[0].y,
                            s1.x_axis,
                            s1.y_axis,
                        );
                        builder.move_to(first);

                        for point in &s1.data[1..] {
                            let p = self.data_to_pixel_with_axes(
                                chart, plot_area, point.x, point.y, s1.x_axis, s1.y_axis,
                            );
                            builder.line_to(p);
                        }

                        // Backward along series 2
                        for point in s2.data.iter().rev() {
                            let p = self.data_to_pixel_with_axes(
                                chart, plot_area, point.x, point.y, s2.x_axis, s2.y_axis,
                            );
                            builder.line_to(p);
                        }

                        builder.close();

                        let path = builder.build();
                        let style = Style::fill_color(region.color);
                        self.geometry.draw_path(&path, &style);
                    }
                }
                FillRegionKind::Rectangle {
                    x_min,
                    y_min,
                    x_max,
                    y_max,
                } => {
                    let top_left = self.data_to_pixel_with_axes(
                        chart,
                        plot_area,
                        *x_min,
                        *y_max,
                        region.x_axis,
                        region.y_axis,
                    );
                    let bottom_right = self.data_to_pixel_with_axes(
                        chart,
                        plot_area,
                        *x_max,
                        *y_min,
                        region.x_axis,
                        region.y_axis,
                    );

                    self.geometry.draw_rect(
                        Vec2::new(top_left.x, top_left.y),
                        Vec2::new(bottom_right.x - top_left.x, bottom_right.y - top_left.y),
                        region.color,
                    );
                }
                FillRegionKind::Polygon { points } => {
                    if points.len() < 3 {
                        continue;
                    }

                    let mut builder = PathBuilder::new();
                    let first = self.data_to_pixel_with_axes(
                        chart,
                        plot_area,
                        points[0].x,
                        points[0].y,
                        region.x_axis,
                        region.y_axis,
                    );
                    builder.move_to(first);

                    for point in &points[1..] {
                        let p = self.data_to_pixel_with_axes(
                            chart,
                            plot_area,
                            point.x,
                            point.y,
                            region.x_axis,
                            region.y_axis,
                        );
                        builder.line_to(p);
                    }
                    builder.close();

                    let path = builder.build();
                    let style = Style::fill_color(region.color);
                    self.geometry.draw_path(&path, &style);
                }
            }
        }
    }

    fn draw_line_annotations(&mut self, chart: &Chart, plot_area: &Rect) {
        for annotation in &chart.line_annotations {
            let start = self.data_to_pixel_with_axes(
                chart,
                plot_area,
                annotation.start.x,
                annotation.start.y,
                annotation.x_axis,
                annotation.y_axis,
            );
            let end = self.data_to_pixel_with_axes(
                chart,
                plot_area,
                annotation.end.x,
                annotation.end.y,
                annotation.x_axis,
                annotation.y_axis,
            );

            self.geometry
                .draw_line(start, end, annotation.width, annotation.color);
        }
    }

    fn draw_crosshair(&mut self, chart: &Chart, plot_area: &Rect) {
        if let Some((series_idx, point_idx)) = chart.interactive.hovered_point
            && let Some(series) = chart.series.get(series_idx)
            && let Some(point) = series.data.get(point_idx)
        {
            let pixel = self.data_to_pixel_with_axes(
                chart,
                plot_area,
                point.x,
                point.y,
                series.x_axis,
                series.y_axis,
            );

            let crosshair_color = Color::rgba(1.0, 1.0, 1.0, 0.5);

            // Vertical line
            self.geometry.draw_line(
                Vec2::new(pixel.x, plot_area.y),
                Vec2::new(pixel.x, plot_area.bottom()),
                1.0,
                crosshair_color,
            );

            // Horizontal line
            self.geometry.draw_line(
                Vec2::new(plot_area.x, pixel.y),
                Vec2::new(plot_area.right(), pixel.y),
                1.0,
                crosshair_color,
            );

            // Highlight point
            self.geometry.draw_circle(pixel, 6.0, series.style.color);
        }
    }

    fn draw_grid(&mut self, chart: &Chart, plot_area: &Rect) {
        // Draw grid for each axis
        for axis in &chart.axes {
            if !axis.grid_lines || !axis.visible {
                continue;
            }

            let style = &axis.style;
            let tick_count = axis.tick_count;

            match axis.orientation {
                AxisOrientation::Horizontal => {
                    // Vertical grid lines
                    for i in 0..=tick_count {
                        let t = i as f32 / tick_count as f32;
                        let x = plot_area.x + t * plot_area.width;
                        self.geometry.draw_line(
                            Vec2::new(x, plot_area.y),
                            Vec2::new(x, plot_area.bottom()),
                            style.grid_width,
                            style.grid_color,
                        );
                    }
                }
                AxisOrientation::Vertical => {
                    // Horizontal grid lines
                    for i in 0..=tick_count {
                        let t = i as f32 / tick_count as f32;
                        let y = plot_area.y + t * plot_area.height;
                        self.geometry.draw_line(
                            Vec2::new(plot_area.x, y),
                            Vec2::new(plot_area.right(), y),
                            style.grid_width,
                            style.grid_color,
                        );
                    }
                }
            }
        }
    }

    fn draw_all_axes(&mut self, chart: &Chart, plot_area: &Rect) {
        for axis in &chart.axes {
            if !axis.visible {
                continue;
            }

            let style = &axis.style;

            match (axis.orientation, axis.position) {
                (AxisOrientation::Horizontal, AxisPosition::Bottom) => {
                    // X axis at bottom
                    self.geometry.draw_line(
                        Vec2::new(plot_area.x, plot_area.bottom()),
                        Vec2::new(plot_area.right(), plot_area.bottom()),
                        style.line_width,
                        style.line_color,
                    );

                    // Ticks
                    for i in 0..=axis.tick_count {
                        let t = i as f32 / axis.tick_count as f32;
                        let x = plot_area.x + t * plot_area.width;
                        let y = plot_area.bottom();
                        self.geometry.draw_line(
                            Vec2::new(x, y),
                            Vec2::new(x, y + style.tick_length),
                            style.line_width,
                            style.tick_color,
                        );
                    }
                }
                (AxisOrientation::Horizontal, AxisPosition::Top) => {
                    // X axis at top
                    self.geometry.draw_line(
                        Vec2::new(plot_area.x, plot_area.y),
                        Vec2::new(plot_area.right(), plot_area.y),
                        style.line_width,
                        style.line_color,
                    );

                    // Ticks
                    for i in 0..=axis.tick_count {
                        let t = i as f32 / axis.tick_count as f32;
                        let x = plot_area.x + t * plot_area.width;
                        let y = plot_area.y;
                        self.geometry.draw_line(
                            Vec2::new(x, y - style.tick_length),
                            Vec2::new(x, y),
                            style.line_width,
                            style.tick_color,
                        );
                    }
                }
                (AxisOrientation::Vertical, AxisPosition::Left) => {
                    // Y axis at left
                    self.geometry.draw_line(
                        Vec2::new(plot_area.x, plot_area.y),
                        Vec2::new(plot_area.x, plot_area.bottom()),
                        style.line_width,
                        style.line_color,
                    );

                    // Ticks
                    for i in 0..=axis.tick_count {
                        let t = i as f32 / axis.tick_count as f32;
                        let x = plot_area.x;
                        let y = plot_area.y + t * plot_area.height;
                        self.geometry.draw_line(
                            Vec2::new(x - style.tick_length, y),
                            Vec2::new(x, y),
                            style.line_width,
                            style.tick_color,
                        );
                    }
                }
                (AxisOrientation::Vertical, AxisPosition::Right) => {
                    // Y axis at right
                    self.geometry.draw_line(
                        Vec2::new(plot_area.right(), plot_area.y),
                        Vec2::new(plot_area.right(), plot_area.bottom()),
                        style.line_width,
                        style.line_color,
                    );

                    // Ticks
                    for i in 0..=axis.tick_count {
                        let t = i as f32 / axis.tick_count as f32;
                        let x = plot_area.right();
                        let y = plot_area.y + t * plot_area.height;
                        self.geometry.draw_line(
                            Vec2::new(x, y),
                            Vec2::new(x + style.tick_length, y),
                            style.line_width,
                            style.tick_color,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    fn draw_line_series(&mut self, chart: &Chart, plot_area: &Rect) {
        profile_scope!("draw_line_series");
        for series in &chart.series {
            if series.data.len() < 2 {
                continue;
            }

            // Get visible X range with buffer for smooth scrolling
            let (x_min, x_max) = chart.axis_range(series.x_axis);
            let x_range = x_max - x_min;
            let buffer = x_range * 0.1; // 10% buffer on each side
            let visible_x_min = x_min - buffer;
            let visible_x_max = x_max + buffer;

            // Find visible point range using binary search (assumes sorted X data)
            let (start_idx, end_idx) =
                Self::find_visible_range(&series.data, visible_x_min, visible_x_max);

            tracing::trace!(
                "Series '{}': rendering {} of {} points (indices {}..{})",
                series.name,
                end_idx - start_idx,
                series.data.len(),
                start_idx,
                end_idx
            );

            // Need at least 2 points to draw a line
            if end_idx <= start_idx + 1 {
                continue;
            }

            // Build path for the visible portion of the line
            let mut builder = PathBuilder::new();
            let first_point = self.data_to_pixel_with_axes(
                chart,
                plot_area,
                series.data[start_idx].x,
                series.data[start_idx].y,
                series.x_axis,
                series.y_axis,
            );
            builder.move_to(first_point);

            for point in &series.data[start_idx + 1..end_idx] {
                let pixel = self.data_to_pixel_with_axes(
                    chart,
                    plot_area,
                    point.x,
                    point.y,
                    series.x_axis,
                    series.y_axis,
                );
                builder.line_to(pixel);
            }

            let path = builder.build();

            // Draw the line
            let stroke = Stroke::solid(series.style.color, series.style.line_width);
            self.geometry.draw_path_stroke(&path, &stroke);

            // Draw points if enabled (only visible ones)
            if let Some(point_style) = &series.style.point_style {
                for point in &series.data[start_idx..end_idx] {
                    let pixel = self.data_to_pixel_with_axes(
                        chart,
                        plot_area,
                        point.x,
                        point.y,
                        series.x_axis,
                        series.y_axis,
                    );
                    self.geometry
                        .draw_circle(pixel, point_style.size * 0.5, point_style.color);
                }
            }
        }
    }

    /// Find the range of visible points using binary search.
    /// Returns (start_idx, end_idx) where end_idx is exclusive.
    /// Includes one extra point on each side for line continuity.
    fn find_visible_range(data: &[DataPoint], x_min: f64, x_max: f64) -> (usize, usize) {
        if data.is_empty() {
            return (0, 0);
        }

        // Binary search for first point >= x_min
        let start = data
            .binary_search_by(|p| p.x.partial_cmp(&x_min).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or_else(|i| i);

        // Binary search for first point > x_max
        let end = data
            .binary_search_by(|p| {
                if p.x <= x_max {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                }
            })
            .unwrap_or_else(|i| i);

        // Include one extra point on each side for line continuity
        let start = start.saturating_sub(1);
        let end = (end + 1).min(data.len());

        (start, end)
    }

    fn draw_scatter_series(&mut self, chart: &Chart, plot_area: &Rect) {
        let default_point_style = super::style::PointStyle::default();
        for series in &chart.series {
            let point_style = series
                .style
                .point_style
                .as_ref()
                .unwrap_or(&default_point_style);

            // Get visible X range with buffer
            let (x_min, x_max) = chart.axis_range(series.x_axis);
            let x_range = x_max - x_min;
            let buffer = x_range * 0.1;
            let (start_idx, end_idx) =
                Self::find_visible_range(&series.data, x_min - buffer, x_max + buffer);

            for point in &series.data[start_idx..end_idx] {
                let pixel = self.data_to_pixel_with_axes(
                    chart,
                    plot_area,
                    point.x,
                    point.y,
                    series.x_axis,
                    series.y_axis,
                );
                self.geometry
                    .draw_circle(pixel, point_style.size * 0.5, series.style.color);
            }
        }
    }

    fn draw_bar_series(&mut self, chart: &Chart, plot_area: &Rect) {
        let bar_width = chart.bar_config.bar_width;
        let gap = chart.bar_config.gap;

        let series_count = chart.series.len() as f32;
        let total_width = bar_width * series_count + gap * (series_count - 1.0);

        for (series_idx, series) in chart.series.iter().enumerate() {
            let (y_min, _) = chart.axis_range(series.y_axis);
            let offset = series_idx as f32 * (bar_width + gap) - total_width * 0.5;

            // Get visible X range with buffer
            let (x_min, x_max) = chart.axis_range(series.x_axis);
            let x_range = x_max - x_min;
            let buffer = x_range * 0.1;
            let (start_idx, end_idx) =
                Self::find_visible_range(&series.data, x_min - buffer, x_max + buffer);

            for point in &series.data[start_idx..end_idx] {
                let center_pixel = self.data_to_pixel_with_axes(
                    chart,
                    plot_area,
                    point.x,
                    point.y,
                    series.x_axis,
                    series.y_axis,
                );
                let base_pixel = self.data_to_pixel_with_axes(
                    chart,
                    plot_area,
                    point.x,
                    y_min,
                    series.x_axis,
                    series.y_axis,
                );

                let bar_x = center_pixel.x + offset;
                let bar_height = (base_pixel.y - center_pixel.y).abs();
                let bar_y = center_pixel.y.min(base_pixel.y);

                self.geometry.draw_rect(
                    Vec2::new(bar_x, bar_y),
                    Vec2::new(bar_width, bar_height),
                    series.style.color,
                );
            }
        }
    }

    fn draw_area_series(&mut self, chart: &Chart, plot_area: &Rect) {
        for series in &chart.series {
            if series.data.len() < 2 {
                continue;
            }

            let (y_min, _) = chart.axis_range(series.y_axis);

            // Get visible X range with buffer for smooth scrolling
            let (x_min, x_max) = chart.axis_range(series.x_axis);
            let x_range = x_max - x_min;
            let buffer = x_range * 0.1; // 10% buffer on each side
            let visible_x_min = x_min - buffer;
            let visible_x_max = x_max + buffer;

            // Find visible point range using binary search
            let (start_idx, end_idx) =
                Self::find_visible_range(&series.data, visible_x_min, visible_x_max);

            // Need at least 2 points to draw an area
            if end_idx <= start_idx + 1 {
                continue;
            }

            let visible_data = &series.data[start_idx..end_idx];

            // Build filled path for visible portion
            let mut builder = PathBuilder::new();

            // Start at baseline
            let first_x = visible_data[0].x;
            let base_start = self.data_to_pixel_with_axes(
                chart,
                plot_area,
                first_x,
                y_min,
                series.x_axis,
                series.y_axis,
            );
            builder.move_to(base_start);

            // Line to first data point
            let first_point = self.data_to_pixel_with_axes(
                chart,
                plot_area,
                first_x,
                visible_data[0].y,
                series.x_axis,
                series.y_axis,
            );
            builder.line_to(first_point);

            // Connect visible data points
            for point in &visible_data[1..] {
                let pixel = self.data_to_pixel_with_axes(
                    chart,
                    plot_area,
                    point.x,
                    point.y,
                    series.x_axis,
                    series.y_axis,
                );
                builder.line_to(pixel);
            }

            // Close to baseline
            let last_x = visible_data.last().unwrap().x;
            let base_end = self.data_to_pixel_with_axes(
                chart,
                plot_area,
                last_x,
                y_min,
                series.x_axis,
                series.y_axis,
            );
            builder.line_to(base_end);
            builder.close();

            let path = builder.build();

            // Draw filled area with transparency
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

            let style = Style::fill_color(fill_color);
            self.geometry.draw_path(&path, &style);

            // Draw line on top (only visible portion)
            let mut builder = PathBuilder::new();
            let first_point = self.data_to_pixel_with_axes(
                chart,
                plot_area,
                visible_data[0].x,
                visible_data[0].y,
                series.x_axis,
                series.y_axis,
            );
            builder.move_to(first_point);

            for point in &visible_data[1..] {
                let pixel = self.data_to_pixel_with_axes(
                    chart,
                    plot_area,
                    point.x,
                    point.y,
                    series.x_axis,
                    series.y_axis,
                );
                builder.line_to(pixel);
            }

            let path = builder.build();
            let stroke = Stroke::solid(series.style.color, series.style.line_width);
            self.geometry.draw_path_stroke(&path, &stroke);
        }
    }

    /// Render the accumulated draw commands.
    pub fn render(&mut self, pass: &mut wgpu::RenderPass, viewport: Viewport) {
        self.geometry.render(pass, viewport);
    }

    /// Render with GPU-accelerated line series.
    ///
    /// Call this after `draw_with_gpu_lines()`. The GPU line renderer must
    /// have been prepared with `GpuChartLineRenderer::prepare()` before this call.
    ///
    /// # Arguments
    ///
    /// * `pass` - The render pass to draw into
    /// * `viewport` - The viewport for rendering
    /// * `chart` - The chart being rendered (for data ranges)
    /// * `plot_area` - The plot area returned by `draw_with_gpu_lines()`
    pub fn render_with_gpu_lines(
        &mut self,
        pass: &mut wgpu::RenderPass,
        viewport: Viewport,
        chart: &Chart,
        plot_area: &Rect,
    ) {
        profile_scope!("chart_render_gpu_lines");

        // First render all non-line geometry
        self.geometry.render(pass, viewport);

        // Then render GPU lines if available
        if let Some(gpu_renderer) = &self.gpu_line_renderer {
            profile_scope!("render_gpu_line_series");
            gpu_renderer.render(pass, viewport, plot_area, chart);
        }
    }
}

/// Hit test result for chart interaction.
#[derive(Debug, Clone)]
pub struct HitTestResult {
    /// Series index
    pub series_index: usize,
    /// Point index within the series
    pub point_index: usize,
    /// Distance from the test point to the data point (in pixels)
    pub distance: f32,
    /// The data point
    pub data_point: DataPoint,
    /// The pixel position of the data point
    pub pixel_position: Vec2,
}

impl ChartRenderer<'_> {
    /// Find the nearest data point to a pixel position.
    pub fn hit_test(
        &self,
        chart: &Chart,
        plot_area: &Rect,
        pixel: Vec2,
        max_distance: f32,
    ) -> Option<HitTestResult> {
        if !plot_area.contains(pixel) {
            return None;
        }

        let mut best: Option<HitTestResult> = None;

        for (series_idx, series) in chart.series.iter().enumerate() {
            for (point_idx, point) in series.data.iter().enumerate() {
                let point_pixel = self.data_to_pixel_with_axes(
                    chart,
                    plot_area,
                    point.x,
                    point.y,
                    series.x_axis,
                    series.y_axis,
                );

                let dist = pixel.distance(point_pixel);

                if dist <= max_distance && best.as_ref().is_none_or(|b| dist < b.distance) {
                    best = Some(HitTestResult {
                        series_index: series_idx,
                        point_index: point_idx,
                        distance: dist,
                        data_point: *point,
                        pixel_position: point_pixel,
                    });
                }
            }
        }

        best
    }
}
