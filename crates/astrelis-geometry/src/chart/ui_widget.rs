//! astrelis-ui integration for interactive charts.
//!
//! Provides helpers for using charts with the astrelis-ui system and handling
//! user interactions like pan, zoom, and hover.
//!
//! # Text Rendering
//!
//! When the `chart-text` feature is enabled, you can use `ChartTextRenderer` to
//! render chart titles, axis labels, tick labels, and legends. See the
//! [`text`](super::text) module for details.
//!
//! # Example
//!
//! ```ignore
//! use astrelis_geometry::{GeometryRenderer, chart::*};
//!
//! // Create chart and controller
//! let chart = ChartBuilder::line()
//!     .title("My Chart")
//!     .x_label("Time")
//!     .y_label("Value")
//!     .add_series("Data", &[(0.0, 1.0), (1.0, 2.0)])
//!     .with_legend(LegendPosition::TopRight)
//!     .interactive(true)
//!     .build();
//!
//! let mut controller = InteractiveChartController::new();
//!
//! // With text rendering (requires chart-text feature):
//! #[cfg(feature = "chart-text")]
//! let mut text_renderer = ChartTextRenderer::new(context.clone(), font_system);
//!
//! // In your render loop:
//! controller.set_bounds(chart_bounds);
//! controller.handle_events(&mut chart, events);
//!
//! // Draw the chart geometry
//! let mut chart_renderer = ChartRenderer::new(&mut geometry);
//! chart_renderer.draw(&chart, chart_bounds);
//!
//! // Draw text elements (requires chart-text feature)
//! #[cfg(feature = "chart-text")]
//! {
//!     text_renderer.set_viewport(viewport);
//!     text_renderer.draw_title(&chart, &chart_bounds);
//!     text_renderer.draw_tick_labels(&chart, &plot_area);
//!     text_renderer.draw_axis_labels(&chart, &plot_area);
//!     text_renderer.draw_legend(&chart, &plot_area, &mut geometry);
//!     text_renderer.render(&mut pass);
//! }
//! ```

use super::rect::Rect;
use super::renderer::{ChartRenderer, HitTestResult};
use super::types::{Chart, DataPoint};
use astrelis_winit::event::{
    ElementState, Event, MouseButton, MouseScrollDelta, PanGesture, PinchGesture, TouchEvent,
    TouchPhase,
};
use glam::Vec2;

/// Interactive chart controller for use with astrelis-ui.
///
/// This struct manages the interactive state of a chart and handles input events
/// from the window system. Use it alongside a GeometryRenderer to create
/// interactive charts within an astrelis-ui application.
///
/// # Example
///
/// ```ignore
/// use astrelis_geometry::{GeometryRenderer, chart::*};
///
/// // Create chart and controller
/// let chart = ChartBuilder::line()
///     .add_series("Data", &[(0.0, 1.0), (1.0, 2.0)])
///     .interactive(true)
///     .build();
///
/// let mut controller = InteractiveChartController::new();
///
/// // In your render loop:
/// controller.set_bounds(chart_bounds);
/// controller.handle_events(&mut chart, events);
///
/// // Draw the chart
/// let mut chart_renderer = ChartRenderer::new(&mut geometry);
/// chart_renderer.draw(&chart, chart_bounds);
/// ```
pub struct InteractiveChartController {
    /// Bounds of the chart in screen coordinates
    bounds: Rect,
    /// Current mouse position
    mouse_pos: Vec2,
    /// Whether the mouse is over the chart
    is_hovered: bool,
    /// Hit test distance for point selection
    hit_test_distance: f32,
    /// Zoom sensitivity
    zoom_sensitivity: f32,
    /// Pan sensitivity
    pan_sensitivity: f32,
    /// Last drag position for calculating delta
    last_drag_pos: Option<Vec2>,
    /// Whether left mouse button is pressed
    left_mouse_down: bool,
}

impl Default for InteractiveChartController {
    fn default() -> Self {
        Self::new()
    }
}

impl InteractiveChartController {
    /// Create a new interactive chart controller.
    pub fn new() -> Self {
        Self {
            bounds: Rect::new(0.0, 0.0, 100.0, 100.0),
            mouse_pos: Vec2::ZERO,
            is_hovered: false,
            hit_test_distance: 15.0,
            zoom_sensitivity: 0.1,
            pan_sensitivity: 1.0,
            last_drag_pos: None,
            left_mouse_down: false,
        }
    }

    /// Set the chart bounds in screen coordinates.
    pub fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    /// Get the current bounds.
    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    /// Set hit test distance for point selection.
    pub fn set_hit_test_distance(&mut self, distance: f32) {
        self.hit_test_distance = distance;
    }

    /// Set zoom sensitivity.
    pub fn set_zoom_sensitivity(&mut self, sensitivity: f32) {
        self.zoom_sensitivity = sensitivity;
    }

    /// Set pan sensitivity.
    pub fn set_pan_sensitivity(&mut self, sensitivity: f32) {
        self.pan_sensitivity = sensitivity;
    }

    /// Check if the mouse is currently over the chart.
    pub fn is_hovered(&self) -> bool {
        self.is_hovered
    }

    /// Get the current mouse position.
    pub fn mouse_position(&self) -> Vec2 {
        self.mouse_pos
    }

    /// Get the plot area (bounds with padding applied).
    pub fn plot_area(&self, chart: &Chart) -> Rect {
        self.bounds.inset(chart.padding)
    }

    /// Handle a window event and update the chart's interactive state.
    ///
    /// Returns true if the event was consumed by the chart.
    pub fn handle_event(&mut self, chart: &mut Chart, event: &Event) -> bool {
        match event {
            Event::MouseMoved(pos) => {
                self.mouse_pos = Vec2::new(pos.x as f32, pos.y as f32);
                self.is_hovered = self.bounds.contains(self.mouse_pos);

                // Handle drag panning
                if chart.interactive.is_dragging && chart.interactive.pan_enabled {
                    if let Some(last_pos) = self.last_drag_pos {
                        let delta = self.mouse_pos - last_pos;
                        self.apply_pan(chart, delta);
                    }
                    self.last_drag_pos = Some(self.mouse_pos);
                    return true;
                }

                // Update hovered point
                if self.is_hovered {
                    let plot_area = self.plot_area(chart);
                    if let Some(hit) = self.hit_test(chart, &plot_area, self.mouse_pos) {
                        chart.interactive.hovered_point = Some((hit.series_index, hit.point_index));
                    } else {
                        chart.interactive.hovered_point = None;
                    }
                } else {
                    chart.interactive.hovered_point = None;
                }

                self.is_hovered
            }
            Event::MouseButtonDown(button) => {
                if *button == MouseButton::Left {
                    self.left_mouse_down = true;

                    if !self.is_hovered {
                        return false;
                    }

                    // Start dragging
                    if chart.interactive.pan_enabled {
                        chart.interactive.is_dragging = true;
                        chart.interactive.drag_start = Some(self.mouse_pos);
                        self.last_drag_pos = Some(self.mouse_pos);
                    }

                    // Handle point selection
                    let plot_area = self.plot_area(chart);
                    if let Some(hit) = self.hit_test(chart, &plot_area, self.mouse_pos) {
                        let point = (hit.series_index, hit.point_index);
                        if !chart.interactive.selected_points.contains(&point) {
                            chart.interactive.selected_points.push(point);
                        }
                    }
                    true
                } else {
                    false
                }
            }
            Event::MouseButtonUp(button) => {
                if *button == MouseButton::Left {
                    self.left_mouse_down = false;

                    // Stop dragging
                    chart.interactive.is_dragging = false;
                    chart.interactive.drag_start = None;
                    self.last_drag_pos = None;
                    true
                } else {
                    false
                }
            }
            Event::MouseScrolled(delta) => {
                if !self.is_hovered || !chart.interactive.zoom_enabled {
                    return false;
                }

                // Extract scroll amounts from delta (X and Y for independent axis zoom)
                let (scroll_x, scroll_y) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (*x, *y),
                    MouseScrollDelta::PixelDelta(pos) => (pos.x as f32 / 100.0, pos.y as f32 / 100.0),
                };

                // Use Y scroll for Y-axis zoom, X scroll for X-axis zoom
                // This allows independent axis zooming on trackpads with 2D scroll
                let zoom_factor_x = 1.0 + scroll_x * self.zoom_sensitivity;
                let zoom_factor_y = 1.0 + scroll_y * self.zoom_sensitivity;

                // If both are significant, apply both. If only one, apply that one.
                let x_significant = scroll_x.abs() > 0.001;
                let y_significant = scroll_y.abs() > 0.001;

                if x_significant && y_significant {
                    // Both axes - apply independent zoom
                    chart.interactive.zoom_xy(zoom_factor_x, zoom_factor_y);
                } else if y_significant {
                    // Vertical scroll only - zoom both axes uniformly (traditional behavior)
                    chart.interactive.zoom_by(zoom_factor_y);
                } else if x_significant {
                    // Horizontal scroll only - zoom X axis only
                    chart.interactive.zoom_x(zoom_factor_x);
                }

                true
            }
            Event::KeyInput(key_event) => {
                if !self.is_hovered {
                    return false;
                }

                // Handle keyboard shortcuts
                if key_event.state == ElementState::Pressed {
                    use astrelis_winit::event::{Key, NamedKey};

                    match &key_event.logical_key {
                        Key::Named(NamedKey::Home) => {
                            // Reset view
                            chart.interactive.reset();
                            true
                        }
                        Key::Character(c) if c == "r" || c == "R" => {
                            // Reset view
                            chart.interactive.reset();
                            true
                        }
                        Key::Character(c) if c == "+" || c == "=" => {
                            // Zoom in
                            if chart.interactive.zoom_enabled {
                                let center = self.bounds.center();
                                chart.interactive.zoom_at(center, 1.2);
                            }
                            true
                        }
                        Key::Character(c) if c == "-" || c == "_" => {
                            // Zoom out
                            if chart.interactive.zoom_enabled {
                                let center = self.bounds.center();
                                chart.interactive.zoom_at(center, 0.8);
                            }
                            true
                        }
                        _ => false,
                    }
                } else {
                    false
                }
            }
            Event::PinchGesture(PinchGesture { delta, phase }) => {
                if !self.is_hovered || !chart.interactive.zoom_enabled {
                    return false;
                }

                // Pinch gesture: delta > 0 = magnify (zoom in), delta < 0 = shrink (zoom out)
                // The delta from the OS is the scale change (e.g., 0.02 = 2% change)
                // Use it directly for a natural feel
                let zoom_factor = 1.0 + (*delta as f32);
                chart.interactive.zoom_by(zoom_factor);

                // Mark as dragging during gesture for UI feedback
                match phase {
                    TouchPhase::Started => {
                        chart.interactive.is_dragging = true;
                    }
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        chart.interactive.is_dragging = false;
                    }
                    TouchPhase::Moved => {}
                }

                true
            }
            Event::PanGesture(PanGesture { delta, phase }) => {
                if !self.is_hovered || !chart.interactive.pan_enabled {
                    return false;
                }

                // Two-finger pan gesture
                let pixel_delta = Vec2::new(delta.x as f32, delta.y as f32);
                self.apply_pan(chart, pixel_delta);

                // Mark as dragging during gesture for UI feedback
                match phase {
                    TouchPhase::Started => {
                        chart.interactive.is_dragging = true;
                    }
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        chart.interactive.is_dragging = false;
                    }
                    TouchPhase::Moved => {}
                }

                true
            }
            Event::Touch(TouchEvent { id, position, phase, .. }) => {
                // Basic touch handling - update mouse position for single touch
                if *id == 0 {
                    self.mouse_pos = Vec2::new(position.x as f32, position.y as f32);
                    self.is_hovered = self.bounds.contains(self.mouse_pos);

                    match phase {
                        TouchPhase::Started => {
                            if self.is_hovered && chart.interactive.pan_enabled {
                                chart.interactive.is_dragging = true;
                                self.last_drag_pos = Some(self.mouse_pos);
                            }
                        }
                        TouchPhase::Moved => {
                            if chart.interactive.is_dragging {
                                if let Some(last_pos) = self.last_drag_pos {
                                    let delta = self.mouse_pos - last_pos;
                                    self.apply_pan(chart, delta);
                                }
                                self.last_drag_pos = Some(self.mouse_pos);
                            }
                        }
                        TouchPhase::Ended | TouchPhase::Cancelled => {
                            chart.interactive.is_dragging = false;
                            self.last_drag_pos = None;
                        }
                    }
                    self.is_hovered
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Apply pan offset from a pixel delta.
    fn apply_pan(&self, chart: &mut Chart, pixel_delta: Vec2) {
        let plot_area = self.plot_area(chart);
        let (x_min, x_max) = chart.x_range();
        let (y_min, y_max) = chart.y_range();

        // Convert pixel delta to data delta
        let data_dx = -(pixel_delta.x / plot_area.width) as f64 * (x_max - x_min);
        let data_dy = (pixel_delta.y / plot_area.height) as f64 * (y_max - y_min);

        chart.interactive.pan_offset.x += data_dx as f32 * self.pan_sensitivity;
        chart.interactive.pan_offset.y += data_dy as f32 * self.pan_sensitivity;
    }

    /// Perform hit testing to find the nearest data point.
    ///
    /// Uses binary search to find nearby points in X, avoiding O(n) iteration.
    fn hit_test(&self, chart: &Chart, plot_area: &Rect, pixel: Vec2) -> Option<HitTestResult> {
        if !plot_area.contains(pixel) {
            return None;
        }

        let mut best: Option<HitTestResult> = None;

        for (series_idx, series) in chart.series.iter().enumerate() {
            let (x_min, x_max) = chart.axis_range(series.x_axis);
            let (y_min, y_max) = chart.axis_range(series.y_axis);

            // Convert pixel to data coordinates
            let data_x = x_min + ((pixel.x - plot_area.x) / plot_area.width) as f64 * (x_max - x_min);

            // Calculate hit test radius in data coordinates
            let data_radius = (self.hit_test_distance / plot_area.width) as f64 * (x_max - x_min);

            // Use binary search to find points near data_x
            let search_min = data_x - data_radius;
            let search_max = data_x + data_radius;

            // Find start index using binary search
            let start_idx = series.data
                .binary_search_by(|p| p.x.partial_cmp(&search_min).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or_else(|i| i);

            // Only iterate over points in the X range
            for (point_idx, point) in series.data[start_idx..].iter().enumerate() {
                if point.x > search_max {
                    break; // Past the search range
                }

                let actual_idx = start_idx + point_idx;

                let px = plot_area.x + ((point.x - x_min) / (x_max - x_min)) as f32 * plot_area.width;
                let py = plot_area.y + plot_area.height
                    - ((point.y - y_min) / (y_max - y_min)) as f32 * plot_area.height;

                let point_pixel = Vec2::new(px, py);
                let dist = pixel.distance(point_pixel);

                if dist <= self.hit_test_distance {
                    if best.as_ref().map_or(true, |b| dist < b.distance) {
                        best = Some(HitTestResult {
                            series_index: series_idx,
                            point_index: actual_idx,
                            distance: dist,
                            data_point: *point,
                            pixel_position: point_pixel,
                        });
                    }
                }
            }
        }

        best
    }

    /// Convert a screen position to data coordinates.
    pub fn screen_to_data(&self, chart: &Chart, screen_pos: Vec2) -> DataPoint {
        let plot_area = self.plot_area(chart);
        let (x_min, x_max) = chart.x_range();
        let (y_min, y_max) = chart.y_range();

        let x = x_min + ((screen_pos.x - plot_area.x) / plot_area.width) as f64 * (x_max - x_min);
        let y = y_max - ((screen_pos.y - plot_area.y) / plot_area.height) as f64 * (y_max - y_min);

        DataPoint::new(x, y)
    }

    /// Convert data coordinates to screen position.
    pub fn data_to_screen(&self, chart: &Chart, data: DataPoint) -> Vec2 {
        let plot_area = self.plot_area(chart);
        let (x_min, x_max) = chart.x_range();
        let (y_min, y_max) = chart.y_range();

        let x = plot_area.x + ((data.x - x_min) / (x_max - x_min)) as f32 * plot_area.width;
        let y = plot_area.y + plot_area.height
            - ((data.y - y_min) / (y_max - y_min)) as f32 * plot_area.height;

        Vec2::new(x, y)
    }

    /// Get tooltip text for the currently hovered point.
    pub fn tooltip_text(&self, chart: &Chart) -> Option<String> {
        if let Some((series_idx, point_idx)) = chart.interactive.hovered_point {
            if let Some(series) = chart.series.get(series_idx) {
                if let Some(point) = series.data.get(point_idx) {
                    return Some(format!(
                        "{}\nx: {:.4}\ny: {:.4}",
                        series.name, point.x, point.y
                    ));
                }
            }
        }
        None
    }

    /// Clear all selected points.
    pub fn clear_selection(&self, chart: &mut Chart) {
        chart.interactive.selected_points.clear();
    }
}

/// Extension trait for drawing interactive charts.
pub trait InteractiveChartExt {
    /// Draw an interactive chart and return information about hover state.
    fn draw_interactive(
        &mut self,
        chart: &Chart,
        bounds: Rect,
        controller: &InteractiveChartController,
    ) -> ChartDrawResult;
}

/// Result of drawing an interactive chart.
#[derive(Debug, Clone)]
pub struct ChartDrawResult {
    /// Whether the chart is being hovered
    pub is_hovered: bool,
    /// The currently hovered data point, if any
    pub hovered_point: Option<HitTestResult>,
    /// The plot area bounds
    pub plot_area: Rect,
}

impl InteractiveChartExt for ChartRenderer<'_> {
    fn draw_interactive(
        &mut self,
        chart: &Chart,
        bounds: Rect,
        controller: &InteractiveChartController,
    ) -> ChartDrawResult {
        // Draw the chart normally
        self.draw(chart, bounds);

        let plot_area = bounds.inset(chart.padding);

        // Get hover information
        let hovered_point = if controller.is_hovered() {
            self.hit_test(chart, &plot_area, controller.mouse_position(), 15.0)
        } else {
            None
        };

        ChartDrawResult {
            is_hovered: controller.is_hovered(),
            hovered_point,
            plot_area,
        }
    }
}

// =============================================================================
// Chart Text Rendering Integration (requires chart-text feature)
// =============================================================================

/// Extension trait for rendering chart text elements.
///
/// This trait provides convenient methods for drawing chart text (titles, labels,
/// legend) alongside the geometry rendering.
///
/// Requires the `chart-text` feature.
///
/// # Coordinate System
///
/// When rendering charts with text, there are two important areas:
///
/// 1. **Bounds**: The overall chart area including space for titles and labels
/// 2. **Plot area**: The inner area where chart data (lines, bars, etc.) is drawn
///
/// The `calculate_adjusted_bounds` method returns bounds that, when passed to
/// `ChartRenderer::draw()`, produce a plot area matching what the text renderer expects.
/// This ensures tick labels, axis labels, and legends align with the chart data.
///
/// # Example
///
/// ```ignore
/// // Calculate adjusted bounds that account for text margins
/// let adjusted_bounds = text_renderer.calculate_adjusted_bounds(&chart, &bounds);
///
/// // ChartRenderer will use plot_area = adjusted_bounds.inset(padding)
/// chart_renderer.draw(&chart, adjusted_bounds);
///
/// // Text uses the same plot_area
/// let plot_area = adjusted_bounds.inset(chart.padding);
/// text_renderer.draw_all_text(&chart, &bounds, &plot_area, &mut geometry);
/// ```
#[cfg(feature = "chart-text")]
pub trait ChartTextExt {
    /// Draw all text elements for a chart.
    ///
    /// This is a convenience method that draws title, subtitle, tick labels,
    /// axis labels, and legend in the correct order.
    ///
    /// # Arguments
    ///
    /// * `chart` - The chart to render text for
    /// * `bounds` - The overall chart bounds (used for title positioning)
    /// * `plot_area` - The plot area (where chart data is drawn)
    /// * `geometry` - Geometry renderer for legend background
    fn draw_all_text(
        &mut self,
        chart: &Chart,
        bounds: &Rect,
        plot_area: &Rect,
        geometry: &mut crate::GeometryRenderer,
    );

    /// Calculate adjusted bounds to pass to `ChartRenderer::draw()`.
    ///
    /// This returns bounds that, when `ChartRenderer` applies its internal
    /// `bounds.inset(padding)` calculation, produce a plot area that matches
    /// what the text renderer expects. This ensures chart data aligns with
    /// tick labels, axis labels, and legends.
    ///
    /// # Arguments
    ///
    /// * `chart` - The chart (for padding and axis configuration)
    /// * `bounds` - The original chart bounds
    ///
    /// # Returns
    ///
    /// Adjusted bounds suitable for passing to `ChartRenderer::draw()`.
    fn calculate_adjusted_bounds(&self, chart: &Chart, bounds: &Rect) -> Rect;

    /// Calculate the plot area after accounting for text margins.
    ///
    /// This computes the area where chart data is drawn, accounting for
    /// space needed for titles, labels, and legends.
    ///
    /// Note: This returns the same plot area that `ChartRenderer` will use
    /// when given bounds from `calculate_adjusted_bounds()`.
    fn calculate_plot_area(&self, chart: &Chart, bounds: &Rect) -> Rect;
}

#[cfg(feature = "chart-text")]
impl ChartTextExt for super::text::ChartTextRenderer {
    fn draw_all_text(
        &mut self,
        chart: &Chart,
        bounds: &Rect,
        plot_area: &Rect,
        geometry: &mut crate::GeometryRenderer,
    ) {
        // Draw in the correct order for proper layering
        self.draw_title(chart, bounds);
        self.draw_tick_labels(chart, plot_area);
        self.draw_axis_labels(chart, plot_area);
        self.draw_legend(chart, plot_area, geometry);
    }

    fn calculate_adjusted_bounds(&self, chart: &Chart, bounds: &Rect) -> Rect {
        let margins = self.calculate_margins(chart);

        // Return bounds adjusted by text margins.
        // When ChartRenderer applies bounds.inset(padding), the result will be
        // a plot area that accounts for text margins.
        Rect::new(
            bounds.x + margins.left,
            bounds.y + margins.top,
            (bounds.width - margins.left - margins.right).max(1.0),
            (bounds.height - margins.top - margins.bottom).max(1.0),
        )
    }

    fn calculate_plot_area(&self, chart: &Chart, bounds: &Rect) -> Rect {
        // Plot area = adjusted_bounds.inset(padding)
        // This is equivalent to: bounds.inset(margins).inset(padding)
        let adjusted = self.calculate_adjusted_bounds(chart, bounds);
        adjusted.inset(chart.padding)
    }
}
