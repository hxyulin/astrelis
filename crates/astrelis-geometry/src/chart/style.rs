//! Chart styling types.
//!
//! Provides comprehensive styling options for chart elements:
//! - Line configuration with dash patterns and caps/joins
//! - Marker configuration with various shapes
//! - Fill configuration with gradients and targets
//! - Axis styling

use super::grid::DashPattern;
use super::types::SeriesId;
use astrelis_render::Color;

/// Line cap style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineCap {
    /// Flat cap (line ends at endpoint).
    #[default]
    Butt,
    /// Rounded cap (semicircle at endpoint).
    Round,
    /// Square cap (extends beyond endpoint by half line width).
    Square,
}

/// Line join style for connecting line segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineJoin {
    /// Sharp corner (miter).
    #[default]
    Miter,
    /// Rounded corner.
    Round,
    /// Beveled corner.
    Bevel,
}

/// Line style for series (legacy enum, kept for backward compatibility).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineStyle {
    /// Solid line
    #[default]
    Solid,
    /// Dashed line
    Dashed,
    /// Dotted line
    Dotted,
}

impl LineStyle {
    /// Convert to a DashPattern.
    pub fn to_dash_pattern(&self) -> DashPattern {
        match self {
            Self::Solid => DashPattern::SOLID,
            Self::Dashed => DashPattern::medium_dash(),
            Self::Dotted => DashPattern::dotted(2.0),
        }
    }
}

/// Enhanced line configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct LineConfig {
    /// Line color.
    pub color: Color,
    /// Line thickness in pixels.
    pub thickness: f32,
    /// Dash pattern.
    pub dash: DashPattern,
    /// Line cap style.
    pub cap: LineCap,
    /// Line join style.
    pub join: LineJoin,
}

impl Default for LineConfig {
    fn default() -> Self {
        Self {
            color: Color::BLUE,
            thickness: 1.5,
            dash: DashPattern::SOLID,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
        }
    }
}

impl LineConfig {
    /// Create a line config with the specified color.
    pub fn with_color(color: Color) -> Self {
        Self {
            color,
            ..Default::default()
        }
    }

    /// Set the line thickness.
    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness;
        self
    }

    /// Set the dash pattern.
    pub fn dash(mut self, dash: DashPattern) -> Self {
        self.dash = dash;
        self
    }

    /// Make this a dashed line.
    pub fn dashed(mut self, dash_len: f32, gap_len: f32) -> Self {
        self.dash = DashPattern::dashed(dash_len, gap_len);
        self
    }

    /// Make this a dotted line.
    pub fn dotted(mut self, dot_size: f32) -> Self {
        self.dash = DashPattern::dotted(dot_size);
        self
    }

    /// Set the line cap.
    pub fn cap(mut self, cap: LineCap) -> Self {
        self.cap = cap;
        self
    }

    /// Set the line join.
    pub fn join(mut self, join: LineJoin) -> Self {
        self.join = join;
        self
    }
}

/// Marker shape for data points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MarkerShape {
    /// Circle
    #[default]
    Circle,
    /// Square
    Square,
    /// Triangle pointing up
    Triangle,
    /// Triangle pointing down
    TriangleDown,
    /// Diamond (rotated square)
    Diamond,
    /// Cross (+)
    Cross,
    /// X shape
    X,
    /// Star
    Star,
    /// No marker (invisible)
    None,
}

/// Point shape (alias for backward compatibility).
pub type PointShape = MarkerShape;

/// Enhanced marker configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkerConfig {
    /// Marker shape.
    pub shape: MarkerShape,
    /// Marker size in pixels.
    pub size: f32,
    /// Fill color (None = no fill).
    pub fill: Option<Color>,
    /// Stroke color (None = no stroke).
    pub stroke: Option<Color>,
    /// Stroke thickness.
    pub stroke_thickness: f32,
    /// Show marker every Nth point (1 = all points).
    pub interval: usize,
    /// Only show markers on hover.
    pub hover_only: bool,
}

impl Default for MarkerConfig {
    fn default() -> Self {
        Self {
            shape: MarkerShape::Circle,
            size: 6.0,
            fill: Some(Color::WHITE),
            stroke: None,
            stroke_thickness: 1.0,
            interval: 1,
            hover_only: false,
        }
    }
}

impl MarkerConfig {
    /// Create a new marker config with the specified shape.
    pub fn new(shape: MarkerShape) -> Self {
        Self {
            shape,
            ..Default::default()
        }
    }

    /// Create a circle marker.
    pub fn circle() -> Self {
        Self::new(MarkerShape::Circle)
    }

    /// Create a square marker.
    pub fn square() -> Self {
        Self::new(MarkerShape::Square)
    }

    /// Create a diamond marker.
    pub fn diamond() -> Self {
        Self::new(MarkerShape::Diamond)
    }

    /// Set the marker size.
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// Set the fill color.
    pub fn fill(mut self, color: Color) -> Self {
        self.fill = Some(color);
        self
    }

    /// Set no fill (outline only).
    pub fn no_fill(mut self) -> Self {
        self.fill = None;
        self
    }

    /// Set the stroke color.
    pub fn stroke(mut self, color: Color) -> Self {
        self.stroke = Some(color);
        self
    }

    /// Set the stroke thickness.
    pub fn stroke_thickness(mut self, thickness: f32) -> Self {
        self.stroke_thickness = thickness;
        self
    }

    /// Show markers at intervals (every Nth point).
    pub fn every(mut self, n: usize) -> Self {
        self.interval = n.max(1);
        self
    }

    /// Only show markers on hover.
    pub fn on_hover_only(mut self) -> Self {
        self.hover_only = true;
        self
    }
}

/// Fill target for area fills.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum FillTarget {
    /// Fill to a constant Y value.
    ToValue(f64),
    /// Fill to the X axis baseline (Y = 0 or axis minimum).
    #[default]
    ToBaseline,
    /// Fill to another series.
    ToSeries { series_id: SeriesId },
    /// Fill a band between two series.
    Band { lower: SeriesId, upper: SeriesId },
}

/// Gradient definition.
#[derive(Debug, Clone, PartialEq)]
pub struct Gradient {
    /// Gradient stops (position 0.0-1.0, color).
    pub stops: Vec<(f32, Color)>,
    /// Whether the gradient is vertical (true) or horizontal (false).
    pub vertical: bool,
}

impl Default for Gradient {
    fn default() -> Self {
        Self {
            stops: vec![(0.0, Color::WHITE), (1.0, Color::BLACK)],
            vertical: true,
        }
    }
}

impl Gradient {
    /// Create a two-color vertical gradient.
    pub fn vertical(top: Color, bottom: Color) -> Self {
        Self {
            stops: vec![(0.0, top), (1.0, bottom)],
            vertical: true,
        }
    }

    /// Create a two-color horizontal gradient.
    pub fn horizontal(left: Color, right: Color) -> Self {
        Self {
            stops: vec![(0.0, left), (1.0, right)],
            vertical: false,
        }
    }

    /// Add a gradient stop.
    pub fn with_stop(mut self, position: f32, color: Color) -> Self {
        self.stops.push((position.clamp(0.0, 1.0), color));
        self.stops.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        self
    }

    /// Get the color at a position (0.0-1.0).
    pub fn color_at(&self, position: f32) -> Color {
        if self.stops.is_empty() {
            return Color::WHITE;
        }
        if self.stops.len() == 1 {
            return self.stops[0].1;
        }

        let pos = position.clamp(0.0, 1.0);

        // Find the two stops to interpolate between
        let mut prev = &self.stops[0];
        for stop in &self.stops {
            if stop.0 >= pos {
                if (stop.0 - prev.0).abs() < f32::EPSILON {
                    return stop.1;
                }
                let t = (pos - prev.0) / (stop.0 - prev.0);
                return Color::rgba(
                    prev.1.r + (stop.1.r - prev.1.r) * t,
                    prev.1.g + (stop.1.g - prev.1.g) * t,
                    prev.1.b + (stop.1.b - prev.1.b) * t,
                    prev.1.a + (stop.1.a - prev.1.a) * t,
                );
            }
            prev = stop;
        }

        self.stops.last().unwrap().1
    }
}

/// Enhanced fill configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct FillConfig {
    /// Fill target.
    pub target: FillTarget,
    /// Solid fill color.
    pub color: Color,
    /// Optional gradient (overrides solid color if present).
    pub gradient: Option<Gradient>,
}

impl Default for FillConfig {
    fn default() -> Self {
        Self {
            target: FillTarget::ToBaseline,
            color: Color::rgba(0.0, 0.5, 1.0, 0.3),
            gradient: None,
        }
    }
}

impl FillConfig {
    /// Create a fill to baseline with the specified color.
    pub fn to_baseline(color: Color) -> Self {
        Self {
            target: FillTarget::ToBaseline,
            color,
            gradient: None,
        }
    }

    /// Create a fill to a constant value.
    pub fn to_value(value: f64, color: Color) -> Self {
        Self {
            target: FillTarget::ToValue(value),
            color,
            gradient: None,
        }
    }

    /// Create a fill to another series.
    pub fn to_series(series_id: SeriesId, color: Color) -> Self {
        Self {
            target: FillTarget::ToSeries { series_id },
            color,
            gradient: None,
        }
    }

    /// Set a gradient fill.
    pub fn with_gradient(mut self, gradient: Gradient) -> Self {
        self.gradient = Some(gradient);
        self
    }

    /// Get the effective fill color at a position (considering gradient).
    pub fn color_at(&self, position: f32) -> Color {
        if let Some(gradient) = &self.gradient {
            gradient.color_at(position)
        } else {
            self.color
        }
    }
}

/// Point style for scatter/line charts (legacy, kept for compatibility).
#[derive(Debug, Clone, Copy)]
pub struct PointStyle {
    /// Point size
    pub size: f32,
    /// Point shape
    pub shape: PointShape,
    /// Fill color
    pub color: Color,
}

impl Default for PointStyle {
    fn default() -> Self {
        Self {
            size: 6.0,
            shape: PointShape::Circle,
            color: Color::WHITE,
        }
    }
}

/// Fill style for area charts (legacy, kept for compatibility).
#[derive(Debug, Clone, Copy)]
pub struct FillStyle {
    /// Fill color
    pub color: Color,
    /// Opacity (0.0 to 1.0)
    pub opacity: f32,
}

impl Default for FillStyle {
    fn default() -> Self {
        Self {
            color: Color::BLUE,
            opacity: 0.3,
        }
    }
}

/// Series visual style.
#[derive(Debug, Clone)]
pub struct SeriesStyle {
    /// Line color
    pub color: Color,
    /// Line width
    pub line_width: f32,
    /// Line style (legacy)
    pub line_style: LineStyle,
    /// Point style (None = no points) - legacy
    pub point_style: Option<PointStyle>,
    /// Fill style (for area charts) - legacy
    pub fill: Option<FillStyle>,
    /// Z-order for rendering (higher = on top)
    pub z_order: i32,
    /// Whether this series is visible
    pub visible: bool,
    /// Whether to show in legend
    pub show_in_legend: bool,
}

impl Default for SeriesStyle {
    fn default() -> Self {
        Self {
            color: Color::BLUE,
            line_width: 1.0, // Thinner lines for better visibility with dense data
            line_style: LineStyle::Solid,
            point_style: None,
            fill: None,
            z_order: 0,
            visible: true,
            show_in_legend: true,
        }
    }
}

impl SeriesStyle {
    /// Create a style with a specific color.
    pub fn with_color(color: Color) -> Self {
        Self {
            color,
            ..Default::default()
        }
    }

    /// Set line width.
    pub fn line_width(mut self, width: f32) -> Self {
        self.line_width = width;
        self
    }

    /// Set line style.
    pub fn line_style(mut self, style: LineStyle) -> Self {
        self.line_style = style;
        self
    }

    /// Add points.
    pub fn with_points(mut self) -> Self {
        self.point_style = Some(PointStyle {
            color: self.color,
            ..Default::default()
        });
        self
    }

    /// Add points with custom style.
    pub fn with_point_style(mut self, style: PointStyle) -> Self {
        self.point_style = Some(style);
        self
    }

    /// Add area fill.
    pub fn with_fill(mut self) -> Self {
        self.fill = Some(FillStyle {
            color: self.color,
            opacity: 0.3,
        });
        self
    }

    /// Add area fill with custom style.
    pub fn with_fill_style(mut self, style: FillStyle) -> Self {
        self.fill = Some(style);
        self
    }

    /// Set z-order (higher = rendered on top).
    pub fn z_order(mut self, z_order: i32) -> Self {
        self.z_order = z_order;
        self
    }

    /// Set visibility.
    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// Hide from legend.
    pub fn hide_from_legend(mut self) -> Self {
        self.show_in_legend = false;
        self
    }

    /// Make this a dashed line.
    pub fn dashed(mut self) -> Self {
        self.line_style = LineStyle::Dashed;
        self
    }

    /// Make this a dotted line.
    pub fn dotted(mut self) -> Self {
        self.line_style = LineStyle::Dotted;
        self
    }

    /// Get the line configuration.
    pub fn to_line_config(&self) -> LineConfig {
        LineConfig {
            color: self.color,
            thickness: self.line_width,
            dash: self.line_style.to_dash_pattern(),
            cap: LineCap::default(),
            join: LineJoin::default(),
        }
    }

    /// Get the marker configuration.
    pub fn to_marker_config(&self) -> Option<MarkerConfig> {
        self.point_style.as_ref().map(|ps| MarkerConfig {
            shape: ps.shape,
            size: ps.size,
            fill: Some(ps.color),
            stroke: None,
            stroke_thickness: 1.0,
            interval: 1,
            hover_only: false,
        })
    }

    /// Get the fill configuration.
    pub fn to_fill_config(&self) -> Option<FillConfig> {
        self.fill.as_ref().map(|fs| FillConfig {
            target: FillTarget::ToBaseline,
            color: Color::rgba(fs.color.r, fs.color.g, fs.color.b, fs.opacity),
            gradient: None,
        })
    }
}

/// Enhanced series style with full configuration.
#[derive(Debug, Clone)]
pub struct EnhancedSeriesStyle {
    /// Line configuration.
    pub line: LineConfig,
    /// Marker configuration (None = no markers).
    pub markers: Option<MarkerConfig>,
    /// Fill configuration (None = no fill).
    pub fill: Option<FillConfig>,
    /// Z-order for rendering.
    pub z_order: i32,
    /// Whether this series is visible.
    pub visible: bool,
    /// Whether to show in legend.
    pub show_in_legend: bool,
}

impl Default for EnhancedSeriesStyle {
    fn default() -> Self {
        Self {
            line: LineConfig::default(),
            markers: None,
            fill: None,
            z_order: 0,
            visible: true,
            show_in_legend: true,
        }
    }
}

impl EnhancedSeriesStyle {
    /// Create a style with the specified line color.
    pub fn with_color(color: Color) -> Self {
        Self {
            line: LineConfig::with_color(color),
            ..Default::default()
        }
    }

    /// Set the line configuration.
    pub fn line(mut self, line: LineConfig) -> Self {
        self.line = line;
        self
    }

    /// Set the marker configuration.
    pub fn markers(mut self, markers: MarkerConfig) -> Self {
        self.markers = Some(markers);
        self
    }

    /// Set the fill configuration.
    pub fn fill(mut self, fill: FillConfig) -> Self {
        self.fill = Some(fill);
        self
    }

    /// Set z-order.
    pub fn z_order(mut self, z_order: i32) -> Self {
        self.z_order = z_order;
        self
    }

    /// Set visibility.
    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// Hide from legend.
    pub fn hide_from_legend(mut self) -> Self {
        self.show_in_legend = false;
        self
    }

    /// Convert to legacy SeriesStyle.
    pub fn to_legacy(&self) -> SeriesStyle {
        SeriesStyle {
            color: self.line.color,
            line_width: self.line.thickness,
            line_style: if self.line.dash.is_solid() {
                LineStyle::Solid
            } else if self.line.dash.segments.len() == 2
                && self.line.dash.segments[0] == self.line.dash.segments[1]
            {
                LineStyle::Dotted
            } else {
                LineStyle::Dashed
            },
            point_style: self.markers.as_ref().map(|m| PointStyle {
                size: m.size,
                shape: m.shape,
                color: m.fill.unwrap_or(self.line.color),
            }),
            fill: self.fill.as_ref().map(|f| FillStyle {
                color: f.color,
                opacity: f.color.a,
            }),
            z_order: self.z_order,
            visible: self.visible,
            show_in_legend: self.show_in_legend,
        }
    }
}

/// Axis visual style.
#[derive(Debug, Clone)]
pub struct AxisStyle {
    /// Axis line color
    pub line_color: Color,
    /// Axis line width
    pub line_width: f32,
    /// Tick color
    pub tick_color: Color,
    /// Tick length
    pub tick_length: f32,
    /// Grid line color
    pub grid_color: Color,
    /// Grid line width
    pub grid_width: f32,
    /// Label color
    pub label_color: Color,
    /// Label font size
    pub label_size: f32,
}

impl Default for AxisStyle {
    fn default() -> Self {
        Self {
            line_color: Color::rgba(0.4, 0.4, 0.45, 1.0),
            line_width: 1.0,
            tick_color: Color::rgba(0.4, 0.4, 0.45, 1.0),
            tick_length: 4.0,
            grid_color: Color::rgba(0.25, 0.25, 0.28, 1.0),
            grid_width: 0.5,
            label_color: Color::rgba(0.6, 0.6, 0.65, 1.0),
            label_size: 11.0,
        }
    }
}

/// Modern, minimal color palette for series.
/// Inspired by contemporary data visualization tools.
pub const SERIES_COLORS: [Color; 8] = [
    Color::rgba(0.36, 0.67, 0.93, 1.0), // Soft blue
    Color::rgba(0.95, 0.55, 0.38, 1.0), // Coral
    Color::rgba(0.45, 0.80, 0.69, 1.0), // Mint/teal
    Color::rgba(0.91, 0.70, 0.41, 1.0), // Warm gold
    Color::rgba(0.70, 0.55, 0.85, 1.0), // Soft purple
    Color::rgba(0.95, 0.60, 0.60, 1.0), // Soft red/pink
    Color::rgba(0.55, 0.75, 0.50, 1.0), // Sage green
    Color::rgba(0.60, 0.60, 0.65, 1.0), // Neutral gray
];

/// Get a color from the palette by index.
pub fn palette_color(index: usize) -> Color {
    SERIES_COLORS[index % SERIES_COLORS.len()]
}
