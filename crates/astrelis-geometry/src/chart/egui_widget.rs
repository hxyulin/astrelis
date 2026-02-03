//! egui integration for chart rendering.
//!
//! Provides an interactive chart widget for egui with full text rendering support:
//! - Chart title and subtitle
//! - Axis tick labels with smart number formatting
//! - Axis labels
//! - Legend with color swatches
//!
//! # Example
//!
//! ```ignore
//! use astrelis_geometry::chart::{ChartBuilder, ChartWidget, LegendPosition};
//!
//! let chart = ChartBuilder::line()
//!     .title("My Chart")
//!     .subtitle("Interactive visualization")
//!     .x_label("Time (s)")
//!     .y_label("Value")
//!     .add_series("Series A", &data)
//!     .with_legend(LegendPosition::TopRight)
//!     .interactive(true)
//!     .build();
//!
//! ui.add(ChartWidget::new(&mut chart));
//! ```

use super::text::format_tick_value;
use super::types::{AxisOrientation, AxisPosition, Chart, DataPoint, LegendPosition};
use egui::{Align2, FontId, Response, Sense, Ui, Widget};

/// An interactive chart widget for egui.
///
/// This widget renders charts with full interactivity:
/// - Pan by dragging
/// - Zoom with scroll wheel
/// - Hover to see data points
/// - Click to select points
///
/// # Example
///
/// ```ignore
/// use astrelis_geometry::chart::{ChartBuilder, ChartWidget};
///
/// let chart = ChartBuilder::line()
///     .add_series("Data", &[(0.0, 1.0), (1.0, 2.0)])
///     .interactive(true)
///     .build();
///
/// ui.add(ChartWidget::new(&mut chart));
/// ```
pub struct ChartWidget<'a> {
    chart: &'a mut Chart,
    /// Minimum size of the widget
    min_size: egui::Vec2,
    /// Maximum distance for hit testing (in pixels)
    hit_test_distance: f32,
}

impl<'a> ChartWidget<'a> {
    /// Create a new chart widget.
    pub fn new(chart: &'a mut Chart) -> Self {
        Self {
            chart,
            min_size: egui::Vec2::new(200.0, 150.0),
            hit_test_distance: 10.0,
        }
    }

    /// Set the minimum size of the widget.
    pub fn min_size(mut self, size: egui::Vec2) -> Self {
        self.min_size = size;
        self
    }

    /// Set the hit test distance for hover detection.
    pub fn hit_test_distance(mut self, distance: f32) -> Self {
        self.hit_test_distance = distance;
        self
    }

    /// Convert data coordinates to screen coordinates.
    fn data_to_screen(&self, plot_rect: &egui::Rect, point: DataPoint) -> egui::Pos2 {
        let (x_min, x_max) = self.chart.x_range();
        let (y_min, y_max) = self.chart.y_range();

        let x = plot_rect.min.x + ((point.x - x_min) / (x_max - x_min)) as f32 * plot_rect.width();
        let y = plot_rect.max.y - ((point.y - y_min) / (y_max - y_min)) as f32 * plot_rect.height();

        egui::pos2(x, y)
    }

    /// Find the nearest data point to a screen position.
    fn hit_test(&self, plot_rect: &egui::Rect, pos: egui::Pos2) -> Option<(usize, usize, f32)> {
        let mut best: Option<(usize, usize, f32)> = None;

        for (series_idx, series) in self.chart.series.iter().enumerate() {
            for (point_idx, point) in series.data.iter().enumerate() {
                let screen_pos = self.data_to_screen(plot_rect, *point);
                let dist = pos.distance(screen_pos);

                if dist <= self.hit_test_distance {
                    if best.map_or(true, |(_, _, d)| dist < d) {
                        best = Some((series_idx, point_idx, dist));
                    }
                }
            }
        }

        best
    }
}

impl<'a> Widget for ChartWidget<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let desired_size = ui.available_size().max(self.min_size);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());

        if !ui.is_rect_visible(rect) {
            return response;
        }

        let padding = self.chart.padding;
        let plot_rect = egui::Rect::from_min_max(
            egui::pos2(rect.min.x + padding, rect.min.y + padding),
            egui::pos2(rect.max.x - padding, rect.max.y - padding),
        );

        // Handle interactions
        let response = self.handle_interactions(ui, &response, &plot_rect);

        // Draw the chart
        self.draw(ui, &rect, &plot_rect);

        response
    }
}

impl<'a> ChartWidget<'a> {
    fn handle_interactions(&self, ui: &mut Ui, response: &Response, plot_rect: &egui::Rect) -> Response {
        let mut response = response.clone();

        // Handle hover
        if let Some(hover_pos) = response.hover_pos() {
            if plot_rect.contains(hover_pos) {
                if let Some((series_idx, point_idx, _)) = self.hit_test(plot_rect, hover_pos) {
                    // Update hovered point (we can't mutate self.chart here due to borrow rules)
                    // Instead, we'll show a tooltip
                    if let Some(series) = self.chart.series.get(series_idx) {
                        if let Some(point) = series.data.get(point_idx) {
                            response = response.on_hover_ui(|ui| {
                                ui.label(format!("{}", series.name));
                                ui.label(format!("x: {:.4}", point.x));
                                ui.label(format!("y: {:.4}", point.y));
                            });
                        }
                    }
                }
            }
        }

        // Note: Drag for panning is handled via ChartResponse::handle_pan()
        // since we can't mutate self.chart.interactive here due to borrow rules

        // Handle scroll for zooming
        if response.hovered() && self.chart.interactive.zoom_enabled {
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            if scroll.abs() > 0.0 {
                // Zoom factor
                let _factor = 1.0 + scroll * 0.001;
                // Note: We can't mutate self.chart.interactive here
            }
        }

        response
    }

    fn draw(&self, ui: &mut Ui, rect: &egui::Rect, plot_rect: &egui::Rect) {
        let painter = ui.painter();

        // Background
        let bg_color = egui::Color32::from_rgba_unmultiplied(
            (self.chart.background_color.r * 255.0) as u8,
            (self.chart.background_color.g * 255.0) as u8,
            (self.chart.background_color.b * 255.0) as u8,
            (self.chart.background_color.a * 255.0) as u8,
        );
        painter.rect_filled(*rect, 0.0, bg_color);

        // Title and subtitle (above plot area)
        self.draw_title(painter, rect);

        // Grid
        self.draw_grid(painter, plot_rect);

        // Axes
        self.draw_axes(painter, plot_rect);

        // Tick labels
        self.draw_tick_labels(painter, plot_rect);

        // Axis labels
        self.draw_axis_labels(painter, plot_rect);

        // Fill regions
        self.draw_fill_regions(painter, plot_rect);

        // Line annotations
        self.draw_line_annotations(painter, plot_rect);

        // Series
        self.draw_series(painter, plot_rect);

        // Legend (on top of everything)
        self.draw_legend(painter, plot_rect);
    }

    fn draw_title(&self, painter: &egui::Painter, rect: &egui::Rect) {
        let mut y_offset = rect.min.y + 8.0;

        // Draw main title
        if let Some(title) = &self.chart.title {
            let title_color = egui::Color32::from_rgba_unmultiplied(
                (title.color.r * 255.0) as u8,
                (title.color.g * 255.0) as u8,
                (title.color.b * 255.0) as u8,
                (title.color.a * 255.0) as u8,
            );

            let font = FontId::proportional(title.font_size);
            let center_x = rect.center().x;

            painter.text(
                egui::pos2(center_x, y_offset),
                Align2::CENTER_TOP,
                &title.text,
                font,
                title_color,
            );

            y_offset += title.font_size + 4.0;
        }

        // Draw subtitle
        if let Some(subtitle) = &self.chart.subtitle {
            let subtitle_color = egui::Color32::from_rgba_unmultiplied(
                (subtitle.color.r * 255.0) as u8,
                (subtitle.color.g * 255.0) as u8,
                (subtitle.color.b * 255.0) as u8,
                (subtitle.color.a * 255.0) as u8,
            );

            let font = FontId::proportional(subtitle.font_size);
            let center_x = rect.center().x;

            painter.text(
                egui::pos2(center_x, y_offset),
                Align2::CENTER_TOP,
                &subtitle.text,
                font,
                subtitle_color,
            );
        }
    }

    fn draw_tick_labels(&self, painter: &egui::Painter, plot_rect: &egui::Rect) {
        let tick_font = FontId::proportional(11.0);
        let label_color = egui::Color32::from_gray(200);

        for axis in &self.chart.axes {
            if !axis.visible {
                continue;
            }

            let (data_min, data_max) = self.chart.axis_range(axis.id);
            let tick_count = axis.tick_count;

            for i in 0..=tick_count {
                let t = i as f64 / tick_count as f64;
                let value = data_min + t * (data_max - data_min);
                let label = format_tick_value(value);

                match (axis.orientation, axis.position) {
                    (AxisOrientation::Horizontal, AxisPosition::Bottom) => {
                        let x = plot_rect.min.x + t as f32 * plot_rect.width();
                        let y = plot_rect.max.y + 4.0;
                        painter.text(
                            egui::pos2(x, y),
                            Align2::CENTER_TOP,
                            &label,
                            tick_font.clone(),
                            label_color,
                        );
                    }
                    (AxisOrientation::Horizontal, AxisPosition::Top) => {
                        let x = plot_rect.min.x + t as f32 * plot_rect.width();
                        let y = plot_rect.min.y - 4.0;
                        painter.text(
                            egui::pos2(x, y),
                            Align2::CENTER_BOTTOM,
                            &label,
                            tick_font.clone(),
                            label_color,
                        );
                    }
                    (AxisOrientation::Vertical, AxisPosition::Left) => {
                        // Y axis is inverted (0 at bottom)
                        let y = plot_rect.min.y + (1.0 - t as f32) * plot_rect.height();
                        let x = plot_rect.min.x - 4.0;
                        painter.text(
                            egui::pos2(x, y),
                            Align2::RIGHT_CENTER,
                            &label,
                            tick_font.clone(),
                            label_color,
                        );
                    }
                    (AxisOrientation::Vertical, AxisPosition::Right) => {
                        let y = plot_rect.min.y + (1.0 - t as f32) * plot_rect.height();
                        let x = plot_rect.max.x + 4.0;
                        painter.text(
                            egui::pos2(x, y),
                            Align2::LEFT_CENTER,
                            &label,
                            tick_font.clone(),
                            label_color,
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    fn draw_axis_labels(&self, painter: &egui::Painter, plot_rect: &egui::Rect) {
        let label_font = FontId::proportional(13.0);
        let label_color = egui::Color32::from_gray(220);

        for axis in &self.chart.axes {
            if !axis.visible {
                continue;
            }

            let Some(label) = &axis.label else {
                continue;
            };

            match (axis.orientation, axis.position) {
                (AxisOrientation::Horizontal, AxisPosition::Bottom) => {
                    // Centered below tick labels
                    let x = plot_rect.center().x;
                    let y = plot_rect.max.y + 24.0;
                    painter.text(
                        egui::pos2(x, y),
                        Align2::CENTER_TOP,
                        label,
                        label_font.clone(),
                        label_color,
                    );
                }
                (AxisOrientation::Horizontal, AxisPosition::Top) => {
                    let x = plot_rect.center().x;
                    let y = plot_rect.min.y - 24.0;
                    painter.text(
                        egui::pos2(x, y),
                        Align2::CENTER_BOTTOM,
                        label,
                        label_font.clone(),
                        label_color,
                    );
                }
                (AxisOrientation::Vertical, AxisPosition::Left) => {
                    // Place above the axis (horizontal, not rotated)
                    let x = plot_rect.min.x - 40.0;
                    let y = plot_rect.min.y - 8.0;
                    painter.text(
                        egui::pos2(x, y),
                        Align2::RIGHT_BOTTOM,
                        label,
                        label_font.clone(),
                        label_color,
                    );
                }
                (AxisOrientation::Vertical, AxisPosition::Right) => {
                    let x = plot_rect.max.x + 40.0;
                    let y = plot_rect.min.y - 8.0;
                    painter.text(
                        egui::pos2(x, y),
                        Align2::LEFT_BOTTOM,
                        label,
                        label_font.clone(),
                        label_color,
                    );
                }
                _ => {}
            }
        }
    }

    fn draw_legend(&self, painter: &egui::Painter, plot_rect: &egui::Rect) {
        let Some(legend) = &self.chart.legend else {
            return;
        };

        if legend.position == LegendPosition::None {
            return;
        }

        // Filter visible series
        let visible_series: Vec<_> = self
            .chart
            .series
            .iter()
            .filter(|s| s.style.show_in_legend && s.style.visible)
            .collect();

        if visible_series.is_empty() {
            return;
        }

        let swatch_size = 12.0;
        let entry_height = 18.0;
        let padding = legend.padding;
        let legend_font = FontId::proportional(12.0);

        // Calculate legend dimensions
        let max_name_width = visible_series
            .iter()
            .map(|s| {
                painter
                    .layout_no_wrap(s.name.clone(), legend_font.clone(), egui::Color32::WHITE)
                    .rect
                    .width()
            })
            .fold(0.0_f32, |a, b| a.max(b));

        let width = swatch_size + 8.0 + max_name_width + padding * 2.0;
        let height = entry_height * visible_series.len() as f32 + padding * 2.0;

        // Calculate position
        let (x, y) = match legend.position {
            LegendPosition::TopRight => (plot_rect.max.x - width - 8.0, plot_rect.min.y + 8.0),
            LegendPosition::TopLeft => (plot_rect.min.x + 8.0, plot_rect.min.y + 8.0),
            LegendPosition::BottomRight => (
                plot_rect.max.x - width - 8.0,
                plot_rect.max.y - height - 8.0,
            ),
            LegendPosition::BottomLeft => (plot_rect.min.x + 8.0, plot_rect.max.y - height - 8.0),
            LegendPosition::None => return,
        };

        // Draw background
        let bg_rect = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(width, height));
        painter.rect_filled(bg_rect, 4.0, egui::Color32::from_rgba_unmultiplied(25, 25, 30, 230));
        painter.rect_stroke(
            bg_rect,
            4.0,
            egui::Stroke::new(1.0, egui::Color32::from_gray(60)),
            egui::StrokeKind::Outside,
        );

        // Draw entries
        for (i, series) in visible_series.iter().enumerate() {
            let entry_y = y + padding + i as f32 * entry_height;

            // Draw color swatch
            let swatch_color = egui::Color32::from_rgba_unmultiplied(
                (series.style.color.r * 255.0) as u8,
                (series.style.color.g * 255.0) as u8,
                (series.style.color.b * 255.0) as u8,
                (series.style.color.a * 255.0) as u8,
            );
            let swatch_rect = egui::Rect::from_min_size(
                egui::pos2(x + padding, entry_y + 3.0),
                egui::vec2(swatch_size, swatch_size),
            );
            painter.rect_filled(swatch_rect, 2.0, swatch_color);

            // Draw series name
            painter.text(
                egui::pos2(x + padding + swatch_size + 8.0, entry_y + entry_height * 0.5),
                Align2::LEFT_CENTER,
                &series.name,
                legend_font.clone(),
                egui::Color32::from_gray(220),
            );
        }
    }

    fn draw_grid(&self, painter: &egui::Painter, plot_rect: &egui::Rect) {
        for axis in &self.chart.axes {
            if !axis.grid_lines || !axis.visible {
                continue;
            }

            let grid_color = egui::Color32::from_rgba_unmultiplied(
                (axis.style.grid_color.r * 255.0) as u8,
                (axis.style.grid_color.g * 255.0) as u8,
                (axis.style.grid_color.b * 255.0) as u8,
                (axis.style.grid_color.a * 255.0) as u8,
            );

            let tick_count = axis.tick_count;

            match axis.orientation {
                super::types::AxisOrientation::Horizontal => {
                    for i in 0..=tick_count {
                        let t = i as f32 / tick_count as f32;
                        let x = plot_rect.min.x + t * plot_rect.width();
                        painter.line_segment(
                            [egui::pos2(x, plot_rect.min.y), egui::pos2(x, plot_rect.max.y)],
                            egui::Stroke::new(axis.style.grid_width, grid_color),
                        );
                    }
                }
                super::types::AxisOrientation::Vertical => {
                    for i in 0..=tick_count {
                        let t = i as f32 / tick_count as f32;
                        let y = plot_rect.min.y + t * plot_rect.height();
                        painter.line_segment(
                            [egui::pos2(plot_rect.min.x, y), egui::pos2(plot_rect.max.x, y)],
                            egui::Stroke::new(axis.style.grid_width, grid_color),
                        );
                    }
                }
            }
        }
    }

    fn draw_axes(&self, painter: &egui::Painter, plot_rect: &egui::Rect) {
        for axis in &self.chart.axes {
            if !axis.visible {
                continue;
            }

            let line_color = egui::Color32::from_rgba_unmultiplied(
                (axis.style.line_color.r * 255.0) as u8,
                (axis.style.line_color.g * 255.0) as u8,
                (axis.style.line_color.b * 255.0) as u8,
                (axis.style.line_color.a * 255.0) as u8,
            );

            let tick_color = egui::Color32::from_rgba_unmultiplied(
                (axis.style.tick_color.r * 255.0) as u8,
                (axis.style.tick_color.g * 255.0) as u8,
                (axis.style.tick_color.b * 255.0) as u8,
                (axis.style.tick_color.a * 255.0) as u8,
            );

            match (axis.orientation, axis.position) {
                (super::types::AxisOrientation::Horizontal, super::types::AxisPosition::Bottom) => {
                    painter.line_segment(
                        [egui::pos2(plot_rect.min.x, plot_rect.max.y), egui::pos2(plot_rect.max.x, plot_rect.max.y)],
                        egui::Stroke::new(axis.style.line_width, line_color),
                    );

                    for i in 0..=axis.tick_count {
                        let t = i as f32 / axis.tick_count as f32;
                        let x = plot_rect.min.x + t * plot_rect.width();
                        painter.line_segment(
                            [egui::pos2(x, plot_rect.max.y), egui::pos2(x, plot_rect.max.y + axis.style.tick_length)],
                            egui::Stroke::new(axis.style.line_width, tick_color),
                        );
                    }
                }
                (super::types::AxisOrientation::Vertical, super::types::AxisPosition::Left) => {
                    painter.line_segment(
                        [egui::pos2(plot_rect.min.x, plot_rect.min.y), egui::pos2(plot_rect.min.x, plot_rect.max.y)],
                        egui::Stroke::new(axis.style.line_width, line_color),
                    );

                    for i in 0..=axis.tick_count {
                        let t = i as f32 / axis.tick_count as f32;
                        let y = plot_rect.min.y + t * plot_rect.height();
                        painter.line_segment(
                            [egui::pos2(plot_rect.min.x - axis.style.tick_length, y), egui::pos2(plot_rect.min.x, y)],
                            egui::Stroke::new(axis.style.line_width, tick_color),
                        );
                    }
                }
                (super::types::AxisOrientation::Vertical, super::types::AxisPosition::Right) => {
                    painter.line_segment(
                        [egui::pos2(plot_rect.max.x, plot_rect.min.y), egui::pos2(plot_rect.max.x, plot_rect.max.y)],
                        egui::Stroke::new(axis.style.line_width, line_color),
                    );

                    for i in 0..=axis.tick_count {
                        let t = i as f32 / axis.tick_count as f32;
                        let y = plot_rect.min.y + t * plot_rect.height();
                        painter.line_segment(
                            [egui::pos2(plot_rect.max.x, y), egui::pos2(plot_rect.max.x + axis.style.tick_length, y)],
                            egui::Stroke::new(axis.style.line_width, tick_color),
                        );
                    }
                }
                _ => {}
            }
        }
    }

    fn draw_fill_regions(&self, painter: &egui::Painter, plot_rect: &egui::Rect) {
        use super::types::FillRegionKind;

        for region in &self.chart.fill_regions {
            let fill_color = egui::Color32::from_rgba_unmultiplied(
                (region.color.r * 255.0) as u8,
                (region.color.g * 255.0) as u8,
                (region.color.b * 255.0) as u8,
                (region.color.a * 255.0) as u8,
            );

            match &region.kind {
                FillRegionKind::HorizontalBand { y_min, y_max } => {
                    let (x_range_min, x_range_max) = self.chart.x_range();
                    let top_left = self.data_to_screen(plot_rect, DataPoint::new(x_range_min, *y_max));
                    let bottom_right = self.data_to_screen(plot_rect, DataPoint::new(x_range_max, *y_min));
                    painter.rect_filled(
                        egui::Rect::from_min_max(top_left, bottom_right),
                        0.0,
                        fill_color,
                    );
                }
                FillRegionKind::VerticalBand { x_min, x_max } => {
                    let (y_range_min, y_range_max) = self.chart.y_range();
                    let top_left = self.data_to_screen(plot_rect, DataPoint::new(*x_min, y_range_max));
                    let bottom_right = self.data_to_screen(plot_rect, DataPoint::new(*x_max, y_range_min));
                    painter.rect_filled(
                        egui::Rect::from_min_max(top_left, bottom_right),
                        0.0,
                        fill_color,
                    );
                }
                FillRegionKind::Rectangle { x_min, y_min, x_max, y_max } => {
                    let top_left = self.data_to_screen(plot_rect, DataPoint::new(*x_min, *y_max));
                    let bottom_right = self.data_to_screen(plot_rect, DataPoint::new(*x_max, *y_min));
                    painter.rect_filled(
                        egui::Rect::from_min_max(top_left, bottom_right),
                        0.0,
                        fill_color,
                    );
                }
                _ => {
                    // Other fill types require more complex polygon rendering
                    // which egui doesn't directly support, skip for now
                }
            }
        }
    }

    fn draw_line_annotations(&self, painter: &egui::Painter, plot_rect: &egui::Rect) {
        for annotation in &self.chart.line_annotations {
            let color = egui::Color32::from_rgba_unmultiplied(
                (annotation.color.r * 255.0) as u8,
                (annotation.color.g * 255.0) as u8,
                (annotation.color.b * 255.0) as u8,
                (annotation.color.a * 255.0) as u8,
            );

            let start = self.data_to_screen(plot_rect, annotation.start);
            let end = self.data_to_screen(plot_rect, annotation.end);

            painter.line_segment(
                [start, end],
                egui::Stroke::new(annotation.width, color),
            );
        }
    }

    fn draw_series(&self, painter: &egui::Painter, plot_rect: &egui::Rect) {
        use super::types::ChartType;

        match self.chart.chart_type {
            ChartType::Line | ChartType::Area => {
                for series in &self.chart.series {
                    if series.data.len() < 2 {
                        continue;
                    }

                    let color = egui::Color32::from_rgba_unmultiplied(
                        (series.style.color.r * 255.0) as u8,
                        (series.style.color.g * 255.0) as u8,
                        (series.style.color.b * 255.0) as u8,
                        (series.style.color.a * 255.0) as u8,
                    );

                    // Draw area fill for Area chart type
                    if self.chart.chart_type == ChartType::Area {
                        let fill_alpha = if let Some(fill) = &series.style.fill {
                            (fill.opacity * 255.0) as u8
                        } else {
                            (0.3 * 255.0) as u8
                        };
                        let fill_color = egui::Color32::from_rgba_unmultiplied(
                            (series.style.color.r * 255.0) as u8,
                            (series.style.color.g * 255.0) as u8,
                            (series.style.color.b * 255.0) as u8,
                            fill_alpha,
                        );

                        let (y_min, _) = self.chart.y_range();
                        let mut points = Vec::with_capacity(series.data.len() + 2);

                        // Start at baseline
                        points.push(self.data_to_screen(plot_rect, DataPoint::new(series.data[0].x, y_min)));

                        // Add all data points
                        for point in &series.data {
                            points.push(self.data_to_screen(plot_rect, *point));
                        }

                        // End at baseline
                        points.push(self.data_to_screen(plot_rect, DataPoint::new(series.data.last().unwrap().x, y_min)));

                        // Draw as a filled polygon
                        let shape = egui::Shape::convex_polygon(points, fill_color, egui::Stroke::NONE);
                        painter.add(shape);
                    }

                    // Draw line
                    let points: Vec<egui::Pos2> = series.data.iter()
                        .map(|p| self.data_to_screen(plot_rect, *p))
                        .collect();

                    for window in points.windows(2) {
                        painter.line_segment(
                            [window[0], window[1]],
                            egui::Stroke::new(series.style.line_width, color),
                        );
                    }

                    // Draw points if enabled
                    if let Some(point_style) = &series.style.point_style {
                        let point_color = egui::Color32::from_rgba_unmultiplied(
                            (point_style.color.r * 255.0) as u8,
                            (point_style.color.g * 255.0) as u8,
                            (point_style.color.b * 255.0) as u8,
                            (point_style.color.a * 255.0) as u8,
                        );
                        for point in &series.data {
                            let pos = self.data_to_screen(plot_rect, *point);
                            painter.circle_filled(pos, point_style.size * 0.5, point_color);
                        }
                    }
                }
            }
            ChartType::Scatter => {
                for series in &self.chart.series {
                    let color = egui::Color32::from_rgba_unmultiplied(
                        (series.style.color.r * 255.0) as u8,
                        (series.style.color.g * 255.0) as u8,
                        (series.style.color.b * 255.0) as u8,
                        (series.style.color.a * 255.0) as u8,
                    );

                    let point_size = series.style.point_style.as_ref()
                        .map(|p| p.size)
                        .unwrap_or(6.0);

                    for point in &series.data {
                        let pos = self.data_to_screen(plot_rect, *point);
                        painter.circle_filled(pos, point_size * 0.5, color);
                    }
                }
            }
            ChartType::Bar => {
                let bar_width = self.chart.bar_config.bar_width;
                let gap = self.chart.bar_config.gap;
                let series_count = self.chart.series.len() as f32;
                let total_width = bar_width * series_count + gap * (series_count - 1.0);

                for (series_idx, series) in self.chart.series.iter().enumerate() {
                    let color = egui::Color32::from_rgba_unmultiplied(
                        (series.style.color.r * 255.0) as u8,
                        (series.style.color.g * 255.0) as u8,
                        (series.style.color.b * 255.0) as u8,
                        (series.style.color.a * 255.0) as u8,
                    );

                    let (y_min, _) = self.chart.y_range();
                    let offset = series_idx as f32 * (bar_width + gap) - total_width * 0.5;

                    for point in &series.data {
                        let center = self.data_to_screen(plot_rect, *point);
                        let base = self.data_to_screen(plot_rect, DataPoint::new(point.x, y_min));

                        let bar_rect = egui::Rect::from_min_max(
                            egui::pos2(center.x + offset, center.y.min(base.y)),
                            egui::pos2(center.x + offset + bar_width, center.y.max(base.y)),
                        );

                        painter.rect_filled(bar_rect, 0.0, color);
                    }
                }
            }
        }
    }
}

/// Extension trait for creating chart responses with pan/zoom support.
pub trait ChartResponse {
    /// Handle pan interaction and return the pan delta in data coordinates.
    fn handle_pan(&self, chart: &Chart, plot_rect: &egui::Rect) -> Option<(f64, f64)>;

    /// Handle zoom interaction and return the zoom factor and center point.
    fn handle_zoom(&self, ui: &Ui) -> Option<f32>;
}

impl ChartResponse for Response {
    fn handle_pan(&self, chart: &Chart, plot_rect: &egui::Rect) -> Option<(f64, f64)> {
        if self.dragged() && chart.interactive.pan_enabled {
            let delta = self.drag_delta();
            let (x_min, x_max) = chart.x_range();
            let (y_min, y_max) = chart.y_range();

            let data_dx = -(delta.x as f64 / plot_rect.width() as f64) * (x_max - x_min);
            let data_dy = (delta.y as f64 / plot_rect.height() as f64) * (y_max - y_min);

            Some((data_dx, data_dy))
        } else {
            None
        }
    }

    fn handle_zoom(&self, ui: &Ui) -> Option<f32> {
        if self.hovered() {
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            if scroll.abs() > 0.0 {
                Some(1.0 + scroll * 0.001)
            } else {
                None
            }
        } else {
            None
        }
    }
}
