//! Core chart types.

use super::style::{AxisStyle, SeriesStyle};
use astrelis_render::Color;
use glam::Vec2;

/// A unique identifier for an axis.
///
/// Supports both standard axes (X/Y primary/secondary) and unlimited custom axes.
/// Custom axes can be created by ID or by name using hash-based ID generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct AxisId(pub u32);

impl AxisId {
    /// Primary X axis (bottom).
    pub const X_PRIMARY: AxisId = AxisId(0);
    /// Primary Y axis (left).
    pub const Y_PRIMARY: AxisId = AxisId(1);
    /// Secondary X axis (top).
    pub const X_SECONDARY: AxisId = AxisId(2);
    /// Secondary Y axis (right).
    pub const Y_SECONDARY: AxisId = AxisId(3);

    /// Create a custom axis ID.
    ///
    /// IDs 0-3 are reserved for standard axes.
    pub fn custom(id: u32) -> Self {
        Self(id + 4) // Reserve 0-3 for standard axes
    }

    /// Create an axis ID from a name using FNV-1a hash.
    ///
    /// This allows referencing axes by name in a consistent way.
    /// The same name will always produce the same ID.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let pressure_axis = AxisId::from_name("pressure");
    /// // Use the same name to reference the axis later
    /// series.y_axis = AxisId::from_name("pressure");
    /// ```
    pub fn from_name(name: &str) -> Self {
        // FNV-1a hash
        const FNV_OFFSET_BASIS: u32 = 2166136261;
        const FNV_PRIME: u32 = 16777619;

        let mut hash = FNV_OFFSET_BASIS;
        for byte in name.bytes() {
            hash ^= u32::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }

        // Ensure we don't collide with reserved IDs
        Self(hash | 0x8000_0000)
    }

    /// Check if this is a standard (built-in) axis.
    pub fn is_standard(&self) -> bool {
        self.0 < 4
    }

    /// Check if this is a custom axis.
    pub fn is_custom(&self) -> bool {
        !self.is_standard()
    }

    /// Get the raw ID value.
    pub fn raw(&self) -> u32 {
        self.0
    }
}

/// Position of an axis on the chart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AxisPosition {
    /// Left side (for Y axes)
    #[default]
    Left,
    /// Right side (for Y axes)
    Right,
    /// Top (for X axes)
    Top,
    /// Bottom (for X axes)
    Bottom,
}

/// Axis orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AxisOrientation {
    /// Horizontal axis (X)
    #[default]
    Horizontal,
    /// Vertical axis (Y)
    Vertical,
}

/// A unique identifier for a data series.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SeriesId(pub u32);

impl SeriesId {
    /// Create a series ID from an index.
    pub fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    /// Create a series ID from a name using hash.
    pub fn from_name(name: &str) -> Self {
        // FNV-1a hash
        const FNV_OFFSET_BASIS: u32 = 2166136261;
        const FNV_PRIME: u32 = 16777619;

        let mut hash = FNV_OFFSET_BASIS;
        for byte in name.bytes() {
            hash ^= u32::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }

        Self(hash)
    }

    /// Get the raw ID value.
    pub fn raw(&self) -> u32 {
        self.0
    }
}

/// A data point in a chart.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DataPoint {
    /// X coordinate
    pub x: f64,
    /// Y coordinate
    pub y: f64,
}

impl DataPoint {
    /// Create a new data point.
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

impl From<(f64, f64)> for DataPoint {
    fn from((x, y): (f64, f64)) -> Self {
        Self { x, y }
    }
}

impl From<(f32, f32)> for DataPoint {
    fn from((x, y): (f32, f32)) -> Self {
        Self {
            x: x as f64,
            y: y as f64,
        }
    }
}

/// A data series in a chart.
#[derive(Debug, Clone)]
pub struct Series {
    /// Series name (for legend)
    pub name: String,
    /// Data points
    pub data: Vec<DataPoint>,
    /// Visual style
    pub style: SeriesStyle,
    /// Which X axis this series uses
    pub x_axis: AxisId,
    /// Which Y axis this series uses
    pub y_axis: AxisId,
}

impl Series {
    /// Create a new series.
    pub fn new(name: impl Into<String>, data: Vec<DataPoint>, style: SeriesStyle) -> Self {
        Self {
            name: name.into(),
            data,
            style,
            x_axis: AxisId::X_PRIMARY,
            y_axis: AxisId::Y_PRIMARY,
        }
    }

    /// Create a series from tuples.
    pub fn from_tuples<T: Into<DataPoint> + Copy>(
        name: impl Into<String>,
        data: &[T],
        style: SeriesStyle,
    ) -> Self {
        Self {
            name: name.into(),
            data: data.iter().map(|&d| d.into()).collect(),
            style,
            x_axis: AxisId::X_PRIMARY,
            y_axis: AxisId::Y_PRIMARY,
        }
    }

    /// Set which axes this series uses.
    pub fn with_axes(mut self, x_axis: AxisId, y_axis: AxisId) -> Self {
        self.x_axis = x_axis;
        self.y_axis = y_axis;
        self
    }

    /// Get the min/max bounds of this series.
    pub fn bounds(&self) -> Option<(DataPoint, DataPoint)> {
        if self.data.is_empty() {
            return None;
        }

        let mut min = DataPoint::new(f64::INFINITY, f64::INFINITY);
        let mut max = DataPoint::new(f64::NEG_INFINITY, f64::NEG_INFINITY);

        for p in &self.data {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
        }

        Some((min, max))
    }
}

/// Chart type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChartType {
    /// Line chart
    #[default]
    Line,
    /// Bar chart
    Bar,
    /// Scatter plot
    Scatter,
    /// Area chart (filled line)
    Area,
}

/// Axis configuration.
#[derive(Debug, Clone)]
pub struct Axis {
    /// Unique identifier
    pub id: AxisId,
    /// Axis label
    pub label: Option<String>,
    /// Minimum value (None = auto)
    pub min: Option<f64>,
    /// Maximum value (None = auto)
    pub max: Option<f64>,
    /// Number of tick marks
    pub tick_count: usize,
    /// Show grid lines
    pub grid_lines: bool,
    /// Visual style
    pub style: AxisStyle,
    /// Position on the chart
    pub position: AxisPosition,
    /// Orientation
    pub orientation: AxisOrientation,
    /// Whether this axis is visible
    pub visible: bool,
    /// Custom tick values (if provided, overrides auto ticks)
    pub custom_ticks: Option<Vec<(f64, String)>>,
}

impl Default for Axis {
    fn default() -> Self {
        Self {
            id: AxisId::default(),
            label: None,
            min: None,
            max: None,
            tick_count: 5,
            grid_lines: true,
            style: AxisStyle::default(),
            position: AxisPosition::Left,
            orientation: AxisOrientation::Vertical,
            visible: true,
            custom_ticks: None,
        }
    }
}

impl Axis {
    /// Create a new X axis.
    pub fn x() -> Self {
        Self {
            id: AxisId::X_PRIMARY,
            orientation: AxisOrientation::Horizontal,
            position: AxisPosition::Bottom,
            ..Default::default()
        }
    }

    /// Create a new Y axis.
    pub fn y() -> Self {
        Self {
            id: AxisId::Y_PRIMARY,
            orientation: AxisOrientation::Vertical,
            position: AxisPosition::Left,
            ..Default::default()
        }
    }

    /// Create a secondary X axis (top).
    pub fn x_secondary() -> Self {
        Self {
            id: AxisId::X_SECONDARY,
            orientation: AxisOrientation::Horizontal,
            position: AxisPosition::Top,
            ..Default::default()
        }
    }

    /// Create a secondary Y axis (right).
    pub fn y_secondary() -> Self {
        Self {
            id: AxisId::Y_SECONDARY,
            orientation: AxisOrientation::Vertical,
            position: AxisPosition::Right,
            ..Default::default()
        }
    }

    /// Create a new axis with a label.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: Some(label.into()),
            ..Default::default()
        }
    }

    /// Set the axis ID.
    pub fn with_id(mut self, id: AxisId) -> Self {
        self.id = id;
        self
    }

    /// Set the min/max range.
    pub fn with_range(mut self, min: f64, max: f64) -> Self {
        self.min = Some(min);
        self.max = Some(max);
        self
    }

    /// Set tick count.
    pub fn with_ticks(mut self, count: usize) -> Self {
        self.tick_count = count;
        self
    }

    /// Set custom tick values.
    pub fn with_custom_ticks(mut self, ticks: Vec<(f64, String)>) -> Self {
        self.custom_ticks = Some(ticks);
        self
    }

    /// Enable/disable grid lines.
    pub fn with_grid(mut self, enabled: bool) -> Self {
        self.grid_lines = enabled;
        self
    }

    /// Set the axis position.
    pub fn with_position(mut self, position: AxisPosition) -> Self {
        self.position = position;
        self
    }

    /// Set visibility.
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// Set the axis label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// Legend position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LegendPosition {
    /// Top-left corner
    TopLeft,
    /// Top-right corner
    #[default]
    TopRight,
    /// Bottom-left corner
    BottomLeft,
    /// Bottom-right corner
    BottomRight,
    /// No legend
    None,
}

/// Legend configuration.
#[derive(Debug, Clone)]
pub struct LegendConfig {
    /// Position
    pub position: LegendPosition,
    /// Padding from edge
    pub padding: f32,
}

impl Default for LegendConfig {
    fn default() -> Self {
        Self {
            position: LegendPosition::TopRight,
            padding: 10.0,
        }
    }
}

/// Bar chart configuration.
#[derive(Debug, Clone, Copy)]
pub struct BarConfig {
    /// Width of each bar
    pub bar_width: f32,
    /// Gap between bars
    pub gap: f32,
}

impl Default for BarConfig {
    fn default() -> Self {
        Self {
            bar_width: 20.0,
            gap: 5.0,
        }
    }
}

/// Text annotation on the chart.
#[derive(Debug, Clone)]
pub struct TextAnnotation {
    /// Text content
    pub text: String,
    /// Position in data coordinates (None = pixel coordinates)
    pub data_position: Option<DataPoint>,
    /// Position in pixel coordinates (used if data_position is None)
    pub pixel_position: Vec2,
    /// Text color
    pub color: Color,
    /// Font size
    pub font_size: f32,
    /// Anchor point (0,0 = top-left, 0.5,0.5 = center, 1,1 = bottom-right)
    pub anchor: Vec2,
    /// Which axes to use for data coordinates
    pub x_axis: AxisId,
    pub y_axis: AxisId,
}

impl TextAnnotation {
    /// Create a text annotation at data coordinates.
    pub fn at_data(text: impl Into<String>, x: f64, y: f64) -> Self {
        Self {
            text: text.into(),
            data_position: Some(DataPoint::new(x, y)),
            pixel_position: Vec2::ZERO,
            color: Color::WHITE,
            font_size: 12.0,
            anchor: Vec2::new(0.5, 0.5),
            x_axis: AxisId::X_PRIMARY,
            y_axis: AxisId::Y_PRIMARY,
        }
    }

    /// Create a text annotation at pixel coordinates.
    pub fn at_pixel(text: impl Into<String>, x: f32, y: f32) -> Self {
        Self {
            text: text.into(),
            data_position: None,
            pixel_position: Vec2::new(x, y),
            color: Color::WHITE,
            font_size: 12.0,
            anchor: Vec2::new(0.5, 0.5),
            x_axis: AxisId::X_PRIMARY,
            y_axis: AxisId::Y_PRIMARY,
        }
    }

    /// Set the text color.
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Set the font size.
    pub fn with_font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Set the anchor point.
    pub fn with_anchor(mut self, anchor: Vec2) -> Self {
        self.anchor = anchor;
        self
    }
}

/// Line annotation on the chart.
#[derive(Debug, Clone)]
pub struct LineAnnotation {
    /// Start point in data coordinates
    pub start: DataPoint,
    /// End point in data coordinates
    pub end: DataPoint,
    /// Line color
    pub color: Color,
    /// Line width
    pub width: f32,
    /// Dash pattern (None = solid)
    pub dash: Option<f32>,
    /// Which axes to use
    pub x_axis: AxisId,
    pub y_axis: AxisId,
}

impl LineAnnotation {
    /// Create a horizontal line at a Y value.
    pub fn horizontal(y: f64, x_min: f64, x_max: f64) -> Self {
        Self {
            start: DataPoint::new(x_min, y),
            end: DataPoint::new(x_max, y),
            color: Color::rgba(0.5, 0.5, 0.5, 0.8),
            width: 1.0,
            dash: None,
            x_axis: AxisId::X_PRIMARY,
            y_axis: AxisId::Y_PRIMARY,
        }
    }

    /// Create a vertical line at an X value.
    pub fn vertical(x: f64, y_min: f64, y_max: f64) -> Self {
        Self {
            start: DataPoint::new(x, y_min),
            end: DataPoint::new(x, y_max),
            color: Color::rgba(0.5, 0.5, 0.5, 0.8),
            width: 1.0,
            dash: None,
            x_axis: AxisId::X_PRIMARY,
            y_axis: AxisId::Y_PRIMARY,
        }
    }

    /// Set the line color.
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Set the line width.
    pub fn with_width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Set dash pattern.
    pub fn with_dash(mut self, dash: f32) -> Self {
        self.dash = Some(dash);
        self
    }
}

/// A filled region on the chart.
#[derive(Debug, Clone)]
pub struct FillRegion {
    /// Region type
    pub kind: FillRegionKind,
    /// Fill color
    pub color: Color,
    /// Which axes to use
    pub x_axis: AxisId,
    pub y_axis: AxisId,
}

/// Types of fill regions.
#[derive(Debug, Clone)]
pub enum FillRegionKind {
    /// Fill between two Y values across the entire X range
    HorizontalBand { y_min: f64, y_max: f64 },
    /// Fill between two X values across the entire Y range
    VerticalBand { x_min: f64, x_max: f64 },
    /// Fill between a series and a constant Y value
    BelowSeries {
        series_index: usize,
        y_baseline: f64,
    },
    /// Fill between two series
    BetweenSeries {
        series_index_1: usize,
        series_index_2: usize,
    },
    /// Fill a rectangular region
    Rectangle {
        x_min: f64,
        y_min: f64,
        x_max: f64,
        y_max: f64,
    },
    /// Fill a custom polygon
    Polygon { points: Vec<DataPoint> },
}

impl FillRegion {
    /// Create a horizontal band fill.
    pub fn horizontal_band(y_min: f64, y_max: f64, color: Color) -> Self {
        Self {
            kind: FillRegionKind::HorizontalBand { y_min, y_max },
            color,
            x_axis: AxisId::X_PRIMARY,
            y_axis: AxisId::Y_PRIMARY,
        }
    }

    /// Create a vertical band fill.
    pub fn vertical_band(x_min: f64, x_max: f64, color: Color) -> Self {
        Self {
            kind: FillRegionKind::VerticalBand { x_min, x_max },
            color,
            x_axis: AxisId::X_PRIMARY,
            y_axis: AxisId::Y_PRIMARY,
        }
    }

    /// Create a fill below a series.
    pub fn below_series(series_index: usize, y_baseline: f64, color: Color) -> Self {
        Self {
            kind: FillRegionKind::BelowSeries {
                series_index,
                y_baseline,
            },
            color,
            x_axis: AxisId::X_PRIMARY,
            y_axis: AxisId::Y_PRIMARY,
        }
    }

    /// Create a fill between two series.
    pub fn between_series(series_index_1: usize, series_index_2: usize, color: Color) -> Self {
        Self {
            kind: FillRegionKind::BetweenSeries {
                series_index_1,
                series_index_2,
            },
            color,
            x_axis: AxisId::X_PRIMARY,
            y_axis: AxisId::Y_PRIMARY,
        }
    }

    /// Create a rectangular fill region.
    pub fn rectangle(x_min: f64, y_min: f64, x_max: f64, y_max: f64, color: Color) -> Self {
        Self {
            kind: FillRegionKind::Rectangle {
                x_min,
                y_min,
                x_max,
                y_max,
            },
            color,
            x_axis: AxisId::X_PRIMARY,
            y_axis: AxisId::Y_PRIMARY,
        }
    }

    /// Create a polygon fill.
    pub fn polygon(points: Vec<DataPoint>, color: Color) -> Self {
        Self {
            kind: FillRegionKind::Polygon { points },
            color,
            x_axis: AxisId::X_PRIMARY,
            y_axis: AxisId::Y_PRIMARY,
        }
    }

    /// Set which axes this region uses.
    pub fn with_axes(mut self, x_axis: AxisId, y_axis: AxisId) -> Self {
        self.x_axis = x_axis;
        self.y_axis = y_axis;
        self
    }
}

/// Chart title configuration.
#[derive(Debug, Clone)]
pub struct ChartTitle {
    /// Title text
    pub text: String,
    /// Font size
    pub font_size: f32,
    /// Text color
    pub color: Color,
    /// Position (relative to chart, 0-1 range)
    pub position: TitlePosition,
}

/// Position of a title.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TitlePosition {
    /// Centered at top
    #[default]
    Top,
    /// Centered at bottom
    Bottom,
    /// Left side (rotated)
    Left,
    /// Right side (rotated)
    Right,
}

impl ChartTitle {
    /// Create a new title.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            font_size: 16.0,
            color: Color::WHITE,
            position: TitlePosition::Top,
        }
    }

    /// Set font size.
    pub fn with_font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Set color.
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Set position.
    pub fn with_position(mut self, position: TitlePosition) -> Self {
        self.position = position;
        self
    }
}

/// Interactive state for a chart.
#[derive(Debug, Clone)]
pub struct InteractiveState {
    /// Pan offset in data coordinates
    pub pan_offset: Vec2,
    /// Zoom level (1.0 = default)
    pub zoom: Vec2,
    /// Whether panning is enabled
    pub pan_enabled: bool,
    /// Whether zooming is enabled
    pub zoom_enabled: bool,
    /// Minimum zoom level
    pub zoom_min: f32,
    /// Maximum zoom level
    pub zoom_max: f32,
    /// Currently hovered data point (series_index, point_index)
    pub hovered_point: Option<(usize, usize)>,
    /// Selected data points
    pub selected_points: Vec<(usize, usize)>,
    /// Whether the chart is being dragged
    pub is_dragging: bool,
    /// Last mouse position during drag
    pub drag_start: Option<Vec2>,
}

impl Default for InteractiveState {
    fn default() -> Self {
        Self {
            pan_offset: Vec2::ZERO,
            zoom: Vec2::ONE,
            pan_enabled: true,
            zoom_enabled: true,
            zoom_min: 0.1,
            zoom_max: 10.0,
            hovered_point: None,
            selected_points: Vec::new(),
            is_dragging: false,
            drag_start: None,
        }
    }
}

impl InteractiveState {
    /// Reset to default view.
    pub fn reset(&mut self) {
        self.pan_offset = Vec2::ZERO;
        self.zoom = Vec2::ONE;
    }

    /// Apply uniform zoom (centered on current view center).
    pub fn zoom_by(&mut self, factor: f32) {
        self.zoom =
            (self.zoom * factor).clamp(Vec2::splat(self.zoom_min), Vec2::splat(self.zoom_max));
    }

    /// Apply independent X and Y zoom factors.
    pub fn zoom_xy(&mut self, factor_x: f32, factor_y: f32) {
        self.zoom.x = (self.zoom.x * factor_x).clamp(self.zoom_min, self.zoom_max);
        self.zoom.y = (self.zoom.y * factor_y).clamp(self.zoom_min, self.zoom_max);
    }

    /// Apply zoom only on X axis.
    pub fn zoom_x(&mut self, factor: f32) {
        self.zoom.x = (self.zoom.x * factor).clamp(self.zoom_min, self.zoom_max);
    }

    /// Apply zoom only on Y axis.
    pub fn zoom_y(&mut self, factor: f32) {
        self.zoom.y = (self.zoom.y * factor).clamp(self.zoom_min, self.zoom_max);
    }

    /// Apply zoom centered on a point in normalized coordinates (0-1 range within plot area).
    ///
    /// This adjusts pan to keep the specified point visually fixed during zoom.
    /// `normalized_center` should be (0.5, 0.5) for center, (0, 0) for top-left, etc.
    pub fn zoom_at_normalized(&mut self, normalized_center: Vec2, factor: f32) {
        let old_zoom = self.zoom;
        let new_zoom =
            (self.zoom * factor).clamp(Vec2::splat(self.zoom_min), Vec2::splat(self.zoom_max));

        if new_zoom == old_zoom {
            return;
        }

        // The normalized center represents a position in the visible data range.
        // When we zoom, we want that position to stay at the same screen location.
        //
        // Before zoom: data_pos = center + (normalized - 0.5) * range / old_zoom
        // After zoom:  data_pos = new_center + (normalized - 0.5) * range / new_zoom
        //
        // For the same data_pos:
        // new_center = center + (normalized - 0.5) * range * (1/old_zoom - 1/new_zoom)
        //
        // Since pan_offset IS the center offset in data coordinates, we adjust it:
        let offset_from_center = normalized_center - Vec2::splat(0.5);
        let zoom_diff = Vec2::new(
            1.0 / old_zoom.x - 1.0 / new_zoom.x,
            1.0 / old_zoom.y - 1.0 / new_zoom.y,
        );

        // We don't know the actual data range here, so we scale by a reasonable factor
        // The pan_offset is in "data units", and the zoom change affects how much
        // of the data range is visible. This is a simplified approximation.
        self.pan_offset += offset_from_center * zoom_diff * 2.0;
        self.zoom = new_zoom;
    }

    /// Apply zoom centered on a pixel position.
    ///
    /// DEPRECATED: Use `zoom_at_normalized` with proper coordinate conversion instead.
    /// This function just applies uniform zoom without center adjustment.
    pub fn zoom_at(&mut self, _center: Vec2, factor: f32) {
        // For now, just do uniform zoom - the center adjustment was broken
        self.zoom_by(factor);
    }

    /// Pan by a delta amount (in data coordinates).
    pub fn pan(&mut self, delta: Vec2) {
        if self.pan_enabled {
            self.pan_offset += delta;
        }
    }
}

/// Complete chart data.
#[derive(Debug, Clone)]
pub struct Chart {
    /// Chart type
    pub chart_type: ChartType,
    /// Data series
    pub series: Vec<Series>,
    /// All axes (indexed by AxisId)
    pub axes: Vec<Axis>,
    /// Main title
    pub title: Option<ChartTitle>,
    /// Subtitle
    pub subtitle: Option<ChartTitle>,
    /// Legend configuration
    pub legend: Option<LegendConfig>,
    /// Background color
    pub background_color: Color,
    /// Bar configuration (if bar chart)
    pub bar_config: BarConfig,
    /// Padding around the chart area
    pub padding: f32,
    /// Text annotations
    pub text_annotations: Vec<TextAnnotation>,
    /// Line annotations
    pub line_annotations: Vec<LineAnnotation>,
    /// Fill regions
    pub fill_regions: Vec<FillRegion>,
    /// Interactive state
    pub interactive: InteractiveState,
    /// Whether to show crosshair on hover
    pub show_crosshair: bool,
    /// Whether to show tooltips on hover
    pub show_tooltips: bool,
}

impl Default for Chart {
    fn default() -> Self {
        Self {
            chart_type: ChartType::Line,
            series: Vec::new(),
            axes: vec![Axis::x(), Axis::y()],
            title: None,
            subtitle: None,
            legend: Some(LegendConfig::default()),
            background_color: Color::rgba(0.12, 0.12, 0.14, 1.0),
            bar_config: BarConfig::default(),
            padding: 50.0,
            text_annotations: Vec::new(),
            line_annotations: Vec::new(),
            fill_regions: Vec::new(),
            interactive: InteractiveState::default(),
            show_crosshair: false,
            show_tooltips: true,
        }
    }
}

impl Chart {
    // =========================================================================
    // Streaming/Live Data API
    // =========================================================================

    /// Append data points to a series efficiently.
    ///
    /// This is more efficient than replacing all data when you're only adding
    /// new points, as it allows caches to perform partial updates.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Add new sensor readings
    /// chart.append_data(0, &[DataPoint::new(10.0, 25.5), DataPoint::new(11.0, 26.0)]);
    /// ```
    pub fn append_data(&mut self, series_idx: usize, points: &[DataPoint]) {
        if let Some(series) = self.series.get_mut(series_idx) {
            series.data.extend_from_slice(points);
        }
    }

    /// Push a single data point to a series with an optional sliding window.
    ///
    /// If `max_points` is Some, the oldest points will be removed to keep
    /// the series at or below the specified size. This is useful for
    /// real-time charts that show a fixed time window.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Keep only the last 1000 points
    /// chart.push_point(0, DataPoint::new(timestamp, value), Some(1000));
    /// ```
    pub fn push_point(&mut self, series_idx: usize, point: DataPoint, max_points: Option<usize>) {
        if let Some(series) = self.series.get_mut(series_idx) {
            series.data.push(point);

            // Apply sliding window if specified
            if let Some(max) = max_points
                && series.data.len() > max
            {
                let excess = series.data.len() - max;
                series.data.drain(..excess);
            }
        }
    }

    /// Replace all data in a series.
    ///
    /// Use this when you need to completely replace the data, not just append.
    ///
    /// # Example
    ///
    /// ```ignore
    /// chart.set_data(0, new_data_points);
    /// ```
    pub fn set_data(&mut self, series_idx: usize, data: Vec<DataPoint>) {
        if let Some(series) = self.series.get_mut(series_idx) {
            series.data = data;
        }
    }

    /// Clear all data from a series.
    pub fn clear_data(&mut self, series_idx: usize) {
        if let Some(series) = self.series.get_mut(series_idx) {
            series.data.clear();
        }
    }

    /// Get mutable access to a series for direct manipulation.
    pub fn series_mut(&mut self, series_idx: usize) -> Option<&mut Series> {
        self.series.get_mut(series_idx)
    }

    /// Get the number of data points in a series.
    pub fn series_len(&self, series_idx: usize) -> usize {
        self.series
            .get(series_idx)
            .map(|s| s.data.len())
            .unwrap_or(0)
    }

    /// Get the total number of data points across all series.
    pub fn total_points(&self) -> usize {
        self.series.iter().map(|s| s.data.len()).sum()
    }

    // =========================================================================
    // Axis Management
    // =========================================================================

    /// Get an axis by ID.
    pub fn get_axis(&self, id: AxisId) -> Option<&Axis> {
        self.axes.iter().find(|a| a.id == id)
    }

    /// Get a mutable axis by ID.
    pub fn get_axis_mut(&mut self, id: AxisId) -> Option<&mut Axis> {
        self.axes.iter_mut().find(|a| a.id == id)
    }

    /// Add or replace an axis.
    pub fn set_axis(&mut self, axis: Axis) {
        if let Some(existing) = self.axes.iter_mut().find(|a| a.id == axis.id) {
            *existing = axis;
        } else {
            self.axes.push(axis);
        }
    }

    /// Get the X axis (primary).
    pub fn x_axis(&self) -> Option<&Axis> {
        self.get_axis(AxisId::X_PRIMARY)
    }

    /// Get the Y axis (primary).
    pub fn y_axis(&self) -> Option<&Axis> {
        self.get_axis(AxisId::Y_PRIMARY)
    }

    /// Get the combined bounds of all series for a specific axis.
    pub fn data_bounds_for_axis(&self, axis_id: AxisId) -> Option<(f64, f64)> {
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        let mut has_data = false;

        for series in &self.series {
            let is_x_axis = series.x_axis == axis_id;
            let is_y_axis = series.y_axis == axis_id;

            if !is_x_axis && !is_y_axis {
                continue;
            }

            if let Some((series_min, series_max)) = series.bounds() {
                has_data = true;
                if is_x_axis {
                    min = min.min(series_min.x);
                    max = max.max(series_max.x);
                } else {
                    min = min.min(series_min.y);
                    max = max.max(series_max.y);
                }
            }
        }

        if has_data { Some((min, max)) } else { None }
    }

    /// Get the combined bounds of all series.
    pub fn data_bounds(&self) -> Option<(DataPoint, DataPoint)> {
        let mut combined_min = DataPoint::new(f64::INFINITY, f64::INFINITY);
        let mut combined_max = DataPoint::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
        let mut has_data = false;

        for series in &self.series {
            if let Some((min, max)) = series.bounds() {
                has_data = true;
                combined_min.x = combined_min.x.min(min.x);
                combined_min.y = combined_min.y.min(min.y);
                combined_max.x = combined_max.x.max(max.x);
                combined_max.y = combined_max.y.max(max.y);
            }
        }

        if has_data {
            Some((combined_min, combined_max))
        } else {
            None
        }
    }

    /// Get the effective range for an axis.
    pub fn axis_range(&self, axis_id: AxisId) -> (f64, f64) {
        let axis = self.get_axis(axis_id);
        let bounds = self.data_bounds_for_axis(axis_id);

        let (data_min, data_max) = bounds.unwrap_or((0.0, 1.0));

        let min = axis.and_then(|a| a.min).unwrap_or(data_min);
        let max = axis.and_then(|a| a.max).unwrap_or(data_max);

        // Apply interactive zoom/pan
        let zoom = if axis.map(|a| a.orientation) == Some(AxisOrientation::Horizontal) {
            self.interactive.zoom.x
        } else {
            self.interactive.zoom.y
        };

        let pan = if axis.map(|a| a.orientation) == Some(AxisOrientation::Horizontal) {
            self.interactive.pan_offset.x as f64
        } else {
            self.interactive.pan_offset.y as f64
        };

        let range = max - min;
        let zoomed_range = range / zoom as f64;
        let center = (min + max) / 2.0 + pan;

        (center - zoomed_range / 2.0, center + zoomed_range / 2.0)
    }

    /// Get the effective X range (respecting axis min/max overrides).
    pub fn x_range(&self) -> (f64, f64) {
        self.axis_range(AxisId::X_PRIMARY)
    }

    /// Get the effective Y range (respecting axis min/max overrides).
    pub fn y_range(&self) -> (f64, f64) {
        self.axis_range(AxisId::Y_PRIMARY)
    }
}
