//! Chart text rendering using astrelis-text.
//!
//! Provides text rendering for chart elements:
//! - Chart title and subtitle
//! - Tick labels (numeric values)
//! - Axis labels
//! - Legend with color swatches
//!
//! This module is only available when the `chart-text` feature is enabled.
//!
//! # Example
//!
//! ```ignore
//! use astrelis_geometry::chart::*;
//! use astrelis_text::FontSystem;
//!
//! let font_system = FontSystem::with_system_fonts();
//! let mut text_renderer = ChartTextRenderer::new(context.clone(), font_system);
//!
//! // In render loop:
//! text_renderer.set_viewport(viewport);
//! text_renderer.draw_title(&chart, &bounds);
//! text_renderer.draw_tick_labels(&chart, &plot_area);
//! text_renderer.draw_axis_labels(&chart, &plot_area);
//! text_renderer.draw_legend(&chart, &plot_area, &mut geometry);
//! text_renderer.render(&mut pass);
//! ```

use std::sync::Arc;

use astrelis_render::{Color, GraphicsContext, Viewport, wgpu};
use astrelis_text::{FontRenderer, FontSystem, Text, TextAlign};
use glam::Vec2;

use super::rect::Rect;
use super::types::{Axis, AxisOrientation, AxisPosition, Chart, LegendPosition};
use crate::GeometryRenderer;

/// Configuration for chart text rendering.
#[derive(Debug, Clone)]
pub struct ChartTextConfig {
    /// Font size for tick labels (default: 11.0)
    pub tick_label_size: f32,
    /// Font size for axis labels (default: 13.0)
    pub axis_label_size: f32,
    /// Font size for chart title (default: 18.0)
    pub title_size: f32,
    /// Font size for subtitle (default: 12.0)
    pub subtitle_size: f32,
    /// Font size for legend entries (default: 12.0)
    pub legend_size: f32,
    /// Default text color (default: white)
    pub text_color: Color,
    /// Padding between tick marks and labels
    pub tick_label_padding: f32,
    /// Padding between axis labels and tick labels
    pub axis_label_padding: f32,
    /// Title padding from top
    pub title_padding: f32,
}

impl Default for ChartTextConfig {
    fn default() -> Self {
        Self {
            tick_label_size: 11.0,
            axis_label_size: 13.0,
            title_size: 18.0,
            subtitle_size: 12.0,
            legend_size: 12.0,
            text_color: Color::WHITE,
            tick_label_padding: 4.0,
            axis_label_padding: 8.0,
            title_padding: 8.0,
        }
    }
}

/// Margins calculated from chart text elements.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChartMargins {
    /// Top margin (for title/subtitle)
    pub top: f32,
    /// Bottom margin (for X axis tick labels and label)
    pub bottom: f32,
    /// Left margin (for Y axis tick labels and label)
    pub left: f32,
    /// Right margin (for secondary Y axis)
    pub right: f32,
}

impl ChartMargins {
    /// Create margins with uniform padding.
    pub fn uniform(padding: f32) -> Self {
        Self {
            top: padding,
            bottom: padding,
            left: padding,
            right: padding,
        }
    }

    /// Add another margins struct to this one.
    pub fn add(&self, other: &ChartMargins) -> Self {
        Self {
            top: self.top + other.top,
            bottom: self.bottom + other.bottom,
            left: self.left + other.left,
            right: self.right + other.right,
        }
    }
}

/// Text renderer for chart elements.
///
/// Wraps a `FontRenderer` and provides high-level methods for rendering
/// chart titles, axis labels, tick labels, and legends.
pub struct ChartTextRenderer {
    font_renderer: FontRenderer,
    config: ChartTextConfig,
    viewport: Viewport,
}

impl ChartTextRenderer {
    /// Create a new chart text renderer.
    pub fn new(context: Arc<GraphicsContext>, font_system: FontSystem) -> Self {
        let font_renderer = FontRenderer::new(context, font_system);
        Self {
            font_renderer,
            config: ChartTextConfig::default(),
            viewport: Viewport::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(
        context: Arc<GraphicsContext>,
        font_system: FontSystem,
        config: ChartTextConfig,
    ) -> Self {
        let font_renderer = FontRenderer::new(context, font_system);
        Self {
            font_renderer,
            config,
            viewport: Viewport::default(),
        }
    }

    /// Set the viewport for rendering.
    pub fn set_viewport(&mut self, viewport: Viewport) {
        self.viewport = viewport;
        self.font_renderer.set_viewport(viewport);
    }

    /// Get the current configuration.
    pub fn config(&self) -> &ChartTextConfig {
        &self.config
    }

    /// Set the configuration.
    pub fn set_config(&mut self, config: ChartTextConfig) {
        self.config = config;
    }

    /// Measure text dimensions.
    fn measure_text(&self, text: &str, size: f32) -> (f32, f32) {
        let text_obj = Text::new(text).size(size);
        self.font_renderer.measure_text(&text_obj)
    }

    /// Calculate margins needed for chart text elements.
    ///
    /// This should be called before rendering to determine how much space
    /// to reserve for titles, labels, and tick marks.
    pub fn calculate_margins(&self, chart: &Chart) -> ChartMargins {
        let mut margins = ChartMargins::default();

        // Title and subtitle
        if let Some(title) = &chart.title {
            let (_, h) = self.measure_text(&title.text, title.font_size);
            margins.top += h + self.config.title_padding;
        }
        if let Some(subtitle) = &chart.subtitle {
            let (_, h) = self.measure_text(&subtitle.text, subtitle.font_size);
            margins.top += h + 4.0;
        }

        // Axis margins
        for axis in &chart.axes {
            if !axis.visible {
                continue;
            }

            let tick_label_height = self.config.tick_label_size + self.config.tick_label_padding;
            let axis_label_height = if axis.label.is_some() {
                self.config.axis_label_size + self.config.axis_label_padding
            } else {
                0.0
            };

            match (axis.orientation, axis.position) {
                (AxisOrientation::Horizontal, AxisPosition::Bottom) => {
                    margins.bottom += tick_label_height + axis_label_height;
                }
                (AxisOrientation::Horizontal, AxisPosition::Top) => {
                    margins.top += tick_label_height + axis_label_height;
                }
                (AxisOrientation::Vertical, AxisPosition::Left) => {
                    // Y axis needs more space for numeric labels
                    margins.left += 50.0 + axis_label_height;
                }
                (AxisOrientation::Vertical, AxisPosition::Right) => {
                    margins.right += 50.0 + axis_label_height;
                }
                _ => {}
            }
        }

        margins
    }

    /// Draw the chart title and subtitle.
    ///
    /// Returns the total height consumed by titles.
    pub fn draw_title(&mut self, chart: &Chart, bounds: &Rect) -> f32 {
        let mut y_offset = bounds.y + self.config.title_padding;

        // Draw main title
        if let Some(title) = &chart.title {
            let text = Text::new(&title.text)
                .size(title.font_size)
                .color(title.color)
                .align(TextAlign::Center);

            let (w, h) = self.font_renderer.measure_text(&text);
            let x = bounds.x + (bounds.width - w) / 2.0;

            let mut buffer = self.font_renderer.prepare(&text);
            self.font_renderer
                .draw_text(&mut buffer, Vec2::new(x, y_offset));

            y_offset += h + 4.0;
        }

        // Draw subtitle
        if let Some(subtitle) = &chart.subtitle {
            let text = Text::new(&subtitle.text)
                .size(subtitle.font_size)
                .color(subtitle.color)
                .align(TextAlign::Center);

            let (w, h) = self.font_renderer.measure_text(&text);
            let x = bounds.x + (bounds.width - w) / 2.0;

            let mut buffer = self.font_renderer.prepare(&text);
            self.font_renderer
                .draw_text(&mut buffer, Vec2::new(x, y_offset));

            y_offset += h + 4.0;
        }

        y_offset - bounds.y
    }

    /// Draw tick labels for all visible axes.
    pub fn draw_tick_labels(&mut self, chart: &Chart, plot_area: &Rect) {
        for axis in &chart.axes {
            if !axis.visible {
                continue;
            }
            self.draw_axis_tick_labels(chart, axis, plot_area);
        }
    }

    /// Draw tick labels for a single axis.
    fn draw_axis_tick_labels(&mut self, chart: &Chart, axis: &Axis, plot_area: &Rect) {
        let (data_min, data_max) = chart.axis_range(axis.id);
        let tick_count = axis.tick_count;
        let text_color = axis.style.label_color;

        // Generate tick labels (custom or auto)
        let ticks: Vec<(f64, String)> = if let Some(custom) = &axis.custom_ticks {
            custom.clone()
        } else {
            (0..=tick_count)
                .map(|i| {
                    let t = i as f64 / tick_count as f64;
                    let value = data_min + t * (data_max - data_min);
                    (value, format_tick_value(value))
                })
                .collect()
        };

        for (value, label) in ticks {
            // Skip if value is outside visible range
            if value < data_min || value > data_max {
                continue;
            }

            let t = (value - data_min) / (data_max - data_min);

            let text = Text::new(&label)
                .size(self.config.tick_label_size)
                .color(text_color);

            let (w, h) = self.font_renderer.measure_text(&text);

            let (x, y) = match (axis.orientation, axis.position) {
                (AxisOrientation::Horizontal, AxisPosition::Bottom) => {
                    let px = plot_area.x + t as f32 * plot_area.width;
                    (
                        px - w / 2.0,
                        plot_area.bottom() + self.config.tick_label_padding,
                    )
                }
                (AxisOrientation::Horizontal, AxisPosition::Top) => {
                    let px = plot_area.x + t as f32 * plot_area.width;
                    (
                        px - w / 2.0,
                        plot_area.y - self.config.tick_label_padding - h,
                    )
                }
                (AxisOrientation::Vertical, AxisPosition::Left) => {
                    // Y axis is inverted (0 at bottom, max at top)
                    let py = plot_area.y + (1.0 - t as f32) * plot_area.height;
                    (
                        plot_area.x - self.config.tick_label_padding - w,
                        py - h / 2.0,
                    )
                }
                (AxisOrientation::Vertical, AxisPosition::Right) => {
                    let py = plot_area.y + (1.0 - t as f32) * plot_area.height;
                    (
                        plot_area.right() + self.config.tick_label_padding,
                        py - h / 2.0,
                    )
                }
                _ => continue,
            };

            let mut buffer = self.font_renderer.prepare(&text);
            self.font_renderer.draw_text(&mut buffer, Vec2::new(x, y));
        }
    }

    /// Draw axis labels (e.g., "Time (s)", "Temperature (°C)").
    pub fn draw_axis_labels(&mut self, chart: &Chart, plot_area: &Rect) {
        for axis in &chart.axes {
            if !axis.visible {
                continue;
            }

            let Some(label) = &axis.label else {
                continue;
            };

            let text = Text::new(label)
                .size(self.config.axis_label_size)
                .color(axis.style.label_color);

            let (w, h) = self.font_renderer.measure_text(&text);

            let (x, y) = match (axis.orientation, axis.position) {
                (AxisOrientation::Horizontal, AxisPosition::Bottom) => {
                    // Centered below tick labels
                    let px = plot_area.x + (plot_area.width - w) / 2.0;
                    let py = plot_area.bottom()
                        + self.config.tick_label_padding
                        + self.config.tick_label_size
                        + self.config.axis_label_padding;
                    (px, py)
                }
                (AxisOrientation::Horizontal, AxisPosition::Top) => {
                    let px = plot_area.x + (plot_area.width - w) / 2.0;
                    let py = plot_area.y
                        - self.config.tick_label_padding
                        - self.config.tick_label_size
                        - self.config.axis_label_padding
                        - h;
                    (px, py)
                }
                (AxisOrientation::Vertical, AxisPosition::Left) => {
                    // Place at top-left of plot area (horizontal, not rotated)
                    let px = plot_area.x - 50.0 - self.config.axis_label_padding;
                    let py = plot_area.y - h - 4.0;
                    // Right-align the label
                    (px - w + 50.0, py)
                }
                (AxisOrientation::Vertical, AxisPosition::Right) => {
                    let px = plot_area.right() + 50.0 + self.config.axis_label_padding;
                    let py = plot_area.y - h - 4.0;
                    (px, py)
                }
                _ => continue,
            };

            let mut buffer = self.font_renderer.prepare(&text);
            self.font_renderer.draw_text(&mut buffer, Vec2::new(x, y));
        }
    }

    /// Draw the legend.
    ///
    /// Uses the geometry renderer to draw color swatches and the text renderer
    /// for series names.
    pub fn draw_legend(
        &mut self,
        chart: &Chart,
        plot_area: &Rect,
        geometry: &mut GeometryRenderer,
    ) {
        let Some(legend) = &chart.legend else {
            return;
        };

        if legend.position == LegendPosition::None {
            return;
        }

        // Filter visible series
        let visible_series: Vec<_> = chart
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

        // Calculate legend dimensions
        let max_name_width = visible_series
            .iter()
            .map(|s| self.measure_text(&s.name, self.config.legend_size).0)
            .fold(0.0_f32, |a, b| a.max(b));

        let width = swatch_size + 8.0 + max_name_width + padding * 2.0;
        let height = entry_height * visible_series.len() as f32 + padding * 2.0;

        // Calculate position
        let (x, y) = match legend.position {
            LegendPosition::TopRight => (plot_area.right() - width - 8.0, plot_area.y + 8.0),
            LegendPosition::TopLeft => (plot_area.x + 8.0, plot_area.y + 8.0),
            LegendPosition::BottomRight => (
                plot_area.right() - width - 8.0,
                plot_area.bottom() - height - 8.0,
            ),
            LegendPosition::BottomLeft => (plot_area.x + 8.0, plot_area.bottom() - height - 8.0),
            LegendPosition::None => return,
        };

        // Draw background
        let bg_color = Color::rgba(0.1, 0.1, 0.12, 0.9);
        geometry.draw_rect(Vec2::new(x, y), Vec2::new(width, height), bg_color);

        // Draw entries
        for (i, series) in visible_series.iter().enumerate() {
            let entry_y = y + padding + i as f32 * entry_height;

            // Draw color swatch
            geometry.draw_rect(
                Vec2::new(x + padding, entry_y + 3.0),
                Vec2::new(swatch_size, swatch_size),
                series.style.color,
            );

            // Draw series name
            let text = Text::new(&series.name)
                .size(self.config.legend_size)
                .color(self.config.text_color);

            let mut buffer = self.font_renderer.prepare(&text);
            self.font_renderer.draw_text(
                &mut buffer,
                Vec2::new(x + padding + swatch_size + 8.0, entry_y + 1.0),
            );
        }
    }

    /// Render all queued text.
    pub fn render(&mut self, pass: &mut wgpu::RenderPass) {
        self.font_renderer.render(pass);
    }

    /// Get the underlying font renderer for advanced use.
    pub fn font_renderer(&self) -> &FontRenderer {
        &self.font_renderer
    }

    /// Get mutable access to the underlying font renderer.
    pub fn font_renderer_mut(&mut self) -> &mut FontRenderer {
        &mut self.font_renderer
    }
}

/// Format a tick value for display.
///
/// Uses appropriate formatting based on the magnitude:
/// - Values >= 1M: "1.2M"
/// - Values >= 1K: "1.2K"
/// - Integer values: "42"
/// - Fractional values: "3.14" (trimmed trailing zeros)
pub fn format_tick_value(value: f64) -> String {
    let abs_value = value.abs();

    if abs_value >= 1_000_000.0 {
        format!("{:.1}M", value / 1_000_000.0)
    } else if abs_value >= 1_000.0 {
        format!("{:.1}K", value / 1_000.0)
    } else if value == value.round() && abs_value < 10000.0 {
        format!("{:.0}", value)
    } else if abs_value >= 100.0 {
        format!("{:.1}", value)
    } else if abs_value >= 10.0 {
        format!("{:.2}", value)
    } else if abs_value >= 1.0 {
        format!("{:.2}", value)
    } else {
        // Small values - show more precision
        let formatted = format!("{:.3}", value);
        // Trim trailing zeros after decimal point
        let trimmed = formatted.trim_end_matches('0');
        let trimmed = trimmed.trim_end_matches('.');
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_tick_value_millions() {
        assert_eq!(format_tick_value(1_500_000.0), "1.5M");
        assert_eq!(format_tick_value(-2_300_000.0), "-2.3M");
    }

    #[test]
    fn test_format_tick_value_thousands() {
        assert_eq!(format_tick_value(1_500.0), "1.5K");
        assert_eq!(format_tick_value(-2_300.0), "-2.3K");
    }

    #[test]
    fn test_format_tick_value_integers() {
        assert_eq!(format_tick_value(42.0), "42");
        assert_eq!(format_tick_value(0.0), "0");
        assert_eq!(format_tick_value(-10.0), "-10");
    }

    #[test]
    fn test_format_tick_value_decimals() {
        assert_eq!(format_tick_value(std::f64::consts::PI), "3.14");
        assert_eq!(format_tick_value(0.5), "0.5");
        assert_eq!(format_tick_value(0.123), "0.123");
    }

    #[test]
    fn test_chart_margins_uniform() {
        let margins = ChartMargins::uniform(10.0);
        assert_eq!(margins.top, 10.0);
        assert_eq!(margins.bottom, 10.0);
        assert_eq!(margins.left, 10.0);
        assert_eq!(margins.right, 10.0);
    }

    #[test]
    fn test_chart_margins_add() {
        let m1 = ChartMargins {
            top: 10.0,
            bottom: 20.0,
            left: 30.0,
            right: 40.0,
        };
        let m2 = ChartMargins {
            top: 5.0,
            bottom: 5.0,
            left: 5.0,
            right: 5.0,
        };
        let result = m1.add(&m2);
        assert_eq!(result.top, 15.0);
        assert_eq!(result.bottom, 25.0);
        assert_eq!(result.left, 35.0);
        assert_eq!(result.right, 45.0);
    }
}
