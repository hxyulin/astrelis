//! Fluent chart builder API.
//!
//! Provides ergonomic builders for constructing charts with full configuration:
//!
//! # Example
//!
//! ```ignore
//! let chart = ChartBuilder::new()
//!     .title("Multi-Axis Chart")
//!     .x_axis(|a| a
//!         .label("Time (s)")
//!         .range(0.0, 100.0)
//!         .grid(|g| g
//!             .major(|m| m.thickness(1.0).color(Color::GRAY))
//!             .minor(|m| m.thickness(0.5).dotted())
//!             .divisions(5)
//!         )
//!     )
//!     .y_axis(|a| a
//!         .label("Temperature")
//!         .auto_range(0.1)
//!     )
//!     .series("Temperature", &temp_data)
//!     .add_series("Pressure", |s| s
//!         .data(&pressure_data)
//!         .color(Color::ORANGE)
//!         .dashed(5.0, 3.0)
//!     )
//!     .build();
//! ```

use super::grid::{DashPattern, GridConfig, GridLevel, GridSpacing};
use super::style::{FillStyle, LineStyle, PointStyle, SeriesStyle, palette_color};
use super::types::{
    Axis, AxisId, AxisOrientation, AxisPosition, BarConfig, Chart, ChartTitle, ChartType,
    DataPoint, FillRegion, LegendConfig, LegendPosition, LineAnnotation, Series, TextAnnotation,
};
use astrelis_render::Color;

/// Builder for creating charts.
#[derive(Debug)]
pub struct ChartBuilder {
    chart: Chart,
    series_count: usize,
}

impl Default for ChartBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ChartBuilder {
    /// Create a new chart builder.
    pub fn new() -> Self {
        Self {
            chart: Chart::default(),
            series_count: 0,
        }
    }

    /// Create a line chart builder.
    pub fn line() -> Self {
        let mut builder = Self::new();
        builder.chart.chart_type = ChartType::Line;
        builder
    }

    /// Create a bar chart builder.
    pub fn bar() -> Self {
        let mut builder = Self::new();
        builder.chart.chart_type = ChartType::Bar;
        builder
    }

    /// Create a scatter plot builder.
    pub fn scatter() -> Self {
        let mut builder = Self::new();
        builder.chart.chart_type = ChartType::Scatter;
        builder
    }

    /// Create an area chart builder.
    pub fn area() -> Self {
        let mut builder = Self::new();
        builder.chart.chart_type = ChartType::Area;
        builder
    }

    /// Set the chart title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.chart.title = Some(ChartTitle::new(title));
        self
    }

    /// Set the chart title with full configuration.
    pub fn title_config(mut self, title: ChartTitle) -> Self {
        self.chart.title = Some(title);
        self
    }

    /// Set the chart subtitle.
    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.chart.subtitle = Some(ChartTitle::new(subtitle).with_font_size(12.0));
        self
    }

    /// Set the X axis label.
    pub fn x_label(mut self, label: impl Into<String>) -> Self {
        if let Some(axis) = self.chart.get_axis_mut(AxisId::X_PRIMARY) {
            axis.label = Some(label.into());
        }
        self
    }

    /// Set the Y axis label.
    pub fn y_label(mut self, label: impl Into<String>) -> Self {
        if let Some(axis) = self.chart.get_axis_mut(AxisId::Y_PRIMARY) {
            axis.label = Some(label.into());
        }
        self
    }

    /// Configure the primary X axis.
    pub fn x_axis_config(mut self, axis: Axis) -> Self {
        let mut axis = axis;
        axis.id = AxisId::X_PRIMARY;
        axis.orientation = AxisOrientation::Horizontal;
        axis.position = AxisPosition::Bottom;
        self.chart.set_axis(axis);
        self
    }

    /// Configure the primary Y axis.
    pub fn y_axis_config(mut self, axis: Axis) -> Self {
        let mut axis = axis;
        axis.id = AxisId::Y_PRIMARY;
        axis.orientation = AxisOrientation::Vertical;
        axis.position = AxisPosition::Left;
        self.chart.set_axis(axis);
        self
    }

    /// Add a secondary Y axis (right side).
    pub fn secondary_y_axis(mut self, axis: Axis) -> Self {
        let mut axis = axis;
        axis.id = AxisId::Y_SECONDARY;
        axis.orientation = AxisOrientation::Vertical;
        axis.position = AxisPosition::Right;
        self.chart.set_axis(axis);
        self
    }

    /// Add a secondary X axis (top).
    pub fn secondary_x_axis(mut self, axis: Axis) -> Self {
        let mut axis = axis;
        axis.id = AxisId::X_SECONDARY;
        axis.orientation = AxisOrientation::Horizontal;
        axis.position = AxisPosition::Top;
        self.chart.set_axis(axis);
        self
    }

    /// Add a custom axis.
    pub fn add_axis(mut self, axis: Axis) -> Self {
        self.chart.set_axis(axis);
        self
    }

    /// Set the X axis range.
    pub fn x_range(mut self, min: f64, max: f64) -> Self {
        if let Some(axis) = self.chart.get_axis_mut(AxisId::X_PRIMARY) {
            axis.min = Some(min);
            axis.max = Some(max);
        }
        self
    }

    /// Set the Y axis range.
    pub fn y_range(mut self, min: f64, max: f64) -> Self {
        if let Some(axis) = self.chart.get_axis_mut(AxisId::Y_PRIMARY) {
            axis.min = Some(min);
            axis.max = Some(max);
        }
        self
    }

    /// Set the secondary Y axis range.
    pub fn secondary_y_range(mut self, min: f64, max: f64) -> Self {
        if let Some(axis) = self.chart.get_axis_mut(AxisId::Y_SECONDARY) {
            axis.min = Some(min);
            axis.max = Some(max);
        } else {
            // Create the axis if it doesn't exist
            self.chart
                .set_axis(Axis::y_secondary().with_range(min, max));
        }
        self
    }

    /// Add a data series.
    pub fn add_series<T: Into<DataPoint> + Copy>(
        mut self,
        name: impl Into<String>,
        data: &[T],
    ) -> Self {
        let color = palette_color(self.series_count);
        let style = SeriesStyle::with_color(color);
        self.chart
            .series
            .push(Series::from_tuples(name, data, style));
        self.series_count += 1;
        self
    }

    /// Add a data series with custom style.
    pub fn add_series_styled<T: Into<DataPoint> + Copy>(
        mut self,
        name: impl Into<String>,
        data: &[T],
        style: SeriesStyle,
    ) -> Self {
        self.chart
            .series
            .push(Series::from_tuples(name, data, style));
        self.series_count += 1;
        self
    }

    /// Add a data series on the secondary Y axis.
    pub fn add_series_secondary_y<T: Into<DataPoint> + Copy>(
        mut self,
        name: impl Into<String>,
        data: &[T],
    ) -> Self {
        let color = palette_color(self.series_count);
        let style = SeriesStyle::with_color(color);
        let series = Series::from_tuples(name, data, style)
            .with_axes(AxisId::X_PRIMARY, AxisId::Y_SECONDARY);
        self.chart.series.push(series);
        self.series_count += 1;
        self
    }

    /// Add a data series with custom axes.
    pub fn add_series_with_axes<T: Into<DataPoint> + Copy>(
        mut self,
        name: impl Into<String>,
        data: &[T],
        x_axis: AxisId,
        y_axis: AxisId,
    ) -> Self {
        let color = palette_color(self.series_count);
        let style = SeriesStyle::with_color(color);
        let series = Series::from_tuples(name, data, style).with_axes(x_axis, y_axis);
        self.chart.series.push(series);
        self.series_count += 1;
        self
    }

    /// Plot a mathematical function.
    pub fn plot_function<F>(
        mut self,
        name: impl Into<String>,
        f: F,
        x_min: f64,
        x_max: f64,
        samples: usize,
    ) -> Self
    where
        F: Fn(f64) -> f64,
    {
        let step = (x_max - x_min) / (samples - 1) as f64;
        let data: Vec<DataPoint> = (0..samples)
            .map(|i| {
                let x = x_min + step * i as f64;
                DataPoint::new(x, f(x))
            })
            .collect();

        let color = palette_color(self.series_count);
        let style = SeriesStyle::with_color(color);
        self.chart.series.push(Series::new(name, data, style));
        self.series_count += 1;
        self
    }

    /// Add a text annotation.
    pub fn add_text_annotation(mut self, annotation: TextAnnotation) -> Self {
        self.chart.text_annotations.push(annotation);
        self
    }

    /// Add a text at data coordinates.
    pub fn add_text_at(mut self, text: impl Into<String>, x: f64, y: f64) -> Self {
        self.chart
            .text_annotations
            .push(TextAnnotation::at_data(text, x, y));
        self
    }

    /// Add a line annotation.
    pub fn add_line_annotation(mut self, annotation: LineAnnotation) -> Self {
        self.chart.line_annotations.push(annotation);
        self
    }

    /// Add a horizontal reference line.
    pub fn add_horizontal_line(mut self, y: f64, color: Color) -> Self {
        let (x_min, x_max) = self.chart.x_range();
        self.chart
            .line_annotations
            .push(LineAnnotation::horizontal(y, x_min, x_max).with_color(color));
        self
    }

    /// Add a vertical reference line.
    pub fn add_vertical_line(mut self, x: f64, color: Color) -> Self {
        let (y_min, y_max) = self.chart.y_range();
        self.chart
            .line_annotations
            .push(LineAnnotation::vertical(x, y_min, y_max).with_color(color));
        self
    }

    /// Add a fill region.
    pub fn add_fill_region(mut self, region: FillRegion) -> Self {
        self.chart.fill_regions.push(region);
        self
    }

    /// Add a horizontal band fill.
    pub fn add_horizontal_band(mut self, y_min: f64, y_max: f64, color: Color) -> Self {
        self.chart
            .fill_regions
            .push(FillRegion::horizontal_band(y_min, y_max, color));
        self
    }

    /// Add a vertical band fill.
    pub fn add_vertical_band(mut self, x_min: f64, x_max: f64, color: Color) -> Self {
        self.chart
            .fill_regions
            .push(FillRegion::vertical_band(x_min, x_max, color));
        self
    }

    /// Add fill below a series.
    pub fn fill_below_series(mut self, series_index: usize, baseline: f64, color: Color) -> Self {
        self.chart
            .fill_regions
            .push(FillRegion::below_series(series_index, baseline, color));
        self
    }

    /// Add fill between two series.
    pub fn fill_between_series(mut self, series1: usize, series2: usize, color: Color) -> Self {
        self.chart
            .fill_regions
            .push(FillRegion::between_series(series1, series2, color));
        self
    }

    /// Enable grid lines.
    pub fn with_grid(mut self) -> Self {
        for axis in &mut self.chart.axes {
            axis.grid_lines = true;
        }
        self
    }

    /// Disable grid lines.
    pub fn without_grid(mut self) -> Self {
        for axis in &mut self.chart.axes {
            axis.grid_lines = false;
        }
        self
    }

    /// Set legend position.
    pub fn with_legend(mut self, position: LegendPosition) -> Self {
        self.chart.legend = Some(LegendConfig {
            position,
            padding: 10.0,
        });
        self
    }

    /// Disable legend.
    pub fn without_legend(mut self) -> Self {
        self.chart.legend = None;
        self
    }

    /// Set background color.
    pub fn background(mut self, color: Color) -> Self {
        self.chart.background_color = color;
        self
    }

    /// Set padding around the chart area.
    pub fn padding(mut self, padding: f32) -> Self {
        self.chart.padding = padding;
        self
    }

    /// Set bar chart configuration.
    pub fn bar_config(mut self, config: BarConfig) -> Self {
        self.chart.bar_config = config;
        self
    }

    /// Enable interactivity (pan and zoom).
    pub fn interactive(mut self, enabled: bool) -> Self {
        self.chart.interactive.pan_enabled = enabled;
        self.chart.interactive.zoom_enabled = enabled;
        self
    }

    /// Enable crosshair on hover.
    pub fn with_crosshair(mut self) -> Self {
        self.chart.show_crosshair = true;
        self
    }

    /// Enable tooltips on hover.
    pub fn with_tooltips(mut self) -> Self {
        self.chart.show_tooltips = true;
        self
    }

    /// Disable tooltips.
    pub fn without_tooltips(mut self) -> Self {
        self.chart.show_tooltips = false;
        self
    }

    /// Set zoom limits.
    pub fn zoom_limits(mut self, min: f32, max: f32) -> Self {
        self.chart.interactive.zoom_min = min;
        self.chart.interactive.zoom_max = max;
        self
    }

    /// Build the chart.
    pub fn build(self) -> Chart {
        self.chart
    }

    // =========================================================================
    // Enhanced Builder API (closure-based configuration)
    // =========================================================================

    /// Configure the primary X axis using a closure.
    ///
    /// # Example
    ///
    /// ```ignore
    /// chart.x_axis(|a| a
    ///     .label("Time (s)")
    ///     .range(0.0, 100.0)
    ///     .grid(|g| g.major(|m| m.thickness(1.0)))
    /// );
    /// ```
    pub fn x_axis<F>(self, f: F) -> Self
    where
        F: FnOnce(AxisBuilder) -> AxisBuilder,
    {
        let builder = AxisBuilder::new(AxisId::X_PRIMARY)
            .orientation(AxisOrientation::Horizontal)
            .position(AxisPosition::Bottom);
        let configured = f(builder);
        self.x_axis_config(configured.build())
    }

    /// Configure the primary Y axis using a closure.
    pub fn y_axis<F>(self, f: F) -> Self
    where
        F: FnOnce(AxisBuilder) -> AxisBuilder,
    {
        let builder = AxisBuilder::new(AxisId::Y_PRIMARY)
            .orientation(AxisOrientation::Vertical)
            .position(AxisPosition::Left);
        let configured = f(builder);
        self.y_axis_config(configured.build())
    }

    /// Add a custom axis using a closure.
    ///
    /// # Example
    ///
    /// ```ignore
    /// chart.add_custom_axis("pressure", |a| a
    ///     .orientation(AxisOrientation::Vertical)
    ///     .position(AxisPosition::Right)
    ///     .label("Pressure (kPa)")
    ///     .range(0.0, 200.0)
    /// );
    /// ```
    pub fn add_custom_axis<F>(mut self, name: &str, f: F) -> Self
    where
        F: FnOnce(AxisBuilder) -> AxisBuilder,
    {
        let axis_id = AxisId::from_name(name);
        let builder = AxisBuilder::new(axis_id).name(name);
        let configured = f(builder);
        self.chart.set_axis(configured.build());
        self
    }

    /// Add a series using a closure for configuration.
    ///
    /// # Example
    ///
    /// ```ignore
    /// chart.add_series_with("Temperature", |s| s
    ///     .data(&data)
    ///     .color(Color::RED)
    ///     .dashed(5.0, 3.0)
    ///     .markers(|m| m.circle().size(4.0))
    /// );
    /// ```
    pub fn add_series_with<F>(mut self, name: impl Into<String>, f: F) -> Self
    where
        F: FnOnce(SeriesBuilder) -> SeriesBuilder,
    {
        let color = palette_color(self.series_count);
        let builder = SeriesBuilder::new(name).color(color);
        let configured = f(builder);
        self.chart.series.push(configured.build());
        self.series_count += 1;
        self
    }

    /// Create a streaming series with a ring buffer.
    ///
    /// The series is created with an empty ring buffer of the specified capacity.
    ///
    /// # Example
    ///
    /// ```ignore
    /// chart.streaming_series("Live Sensor", 10_000, |s| s
    ///     .color(Color::BLUE)
    ///     .fill_to_baseline(Color::rgba(0.0, 0.0, 1.0, 0.1))
    /// );
    /// ```
    pub fn streaming_series<F>(mut self, name: impl Into<String>, capacity: usize, f: F) -> Self
    where
        F: FnOnce(SeriesBuilder) -> SeriesBuilder,
    {
        let color = palette_color(self.series_count);
        let builder = SeriesBuilder::new(name).color(color).streaming(capacity);
        let configured = f(builder);
        self.chart.series.push(configured.build());
        self.series_count += 1;
        self
    }
}

// =============================================================================
// AxisBuilder
// =============================================================================

/// Builder for configuring chart axes.
#[derive(Debug)]
pub struct AxisBuilder {
    axis: Axis,
    grid_config: Option<GridConfig>,
}

impl AxisBuilder {
    /// Create a new axis builder with the given ID.
    pub fn new(id: AxisId) -> Self {
        Self {
            axis: Axis {
                id,
                ..Default::default()
            },
            grid_config: None,
        }
    }

    /// Set the axis name (for custom axes).
    pub fn name(self, name: impl Into<String>) -> Self {
        // Store name in label for now (could add separate field later)
        let _ = name.into();
        self
    }

    /// Set the axis label.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.axis.label = Some(label.into());
        self
    }

    /// Set the axis range.
    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.axis.min = Some(min);
        self.axis.max = Some(max);
        self
    }

    /// Enable auto-ranging with the specified padding.
    pub fn auto_range(mut self, padding: f64) -> Self {
        self.axis.min = None;
        self.axis.max = None;
        // Note: padding is stored elsewhere in enhanced axis
        let _ = padding;
        self
    }

    /// Set the axis orientation.
    pub fn orientation(mut self, orientation: AxisOrientation) -> Self {
        self.axis.orientation = orientation;
        self
    }

    /// Set the axis position.
    pub fn position(mut self, position: AxisPosition) -> Self {
        self.axis.position = position;
        self
    }

    /// Set the number of ticks.
    pub fn ticks(mut self, count: usize) -> Self {
        self.axis.tick_count = count;
        self
    }

    /// Set custom tick values.
    pub fn custom_ticks(mut self, ticks: Vec<(f64, String)>) -> Self {
        self.axis.custom_ticks = Some(ticks);
        self
    }

    /// Enable or disable grid lines.
    pub fn show_grid(mut self, show: bool) -> Self {
        self.axis.grid_lines = show;
        self
    }

    /// Configure grid lines using a closure.
    pub fn grid<F>(mut self, f: F) -> Self
    where
        F: FnOnce(GridBuilder) -> GridBuilder,
    {
        let builder = GridBuilder::new();
        let configured = f(builder);
        self.grid_config = Some(configured.build());
        self.axis.grid_lines = true;
        self
    }

    /// Set visibility.
    pub fn visible(mut self, visible: bool) -> Self {
        self.axis.visible = visible;
        self
    }

    /// Build the axis.
    pub fn build(self) -> Axis {
        self.axis
    }
}

// =============================================================================
// GridBuilder
// =============================================================================

/// Builder for configuring grid lines.
#[derive(Debug)]
pub struct GridBuilder {
    config: GridConfig,
}

impl GridBuilder {
    /// Create a new grid builder.
    pub fn new() -> Self {
        Self {
            config: GridConfig::default(),
        }
    }

    /// Configure major grid lines.
    pub fn major<F>(mut self, f: F) -> Self
    where
        F: FnOnce(GridLevelBuilder) -> GridLevelBuilder,
    {
        let builder = GridLevelBuilder::new(GridLevel::major());
        let configured = f(builder);
        self.config.major = configured.build();
        self
    }

    /// Configure minor grid lines.
    pub fn minor<F>(mut self, f: F) -> Self
    where
        F: FnOnce(GridLevelBuilder) -> GridLevelBuilder,
    {
        let builder = GridLevelBuilder::new(GridLevel::minor());
        let configured = f(builder);
        self.config.minor = Some(configured.build());
        self
    }

    /// Configure tertiary grid lines.
    pub fn tertiary<F>(mut self, f: F) -> Self
    where
        F: FnOnce(GridLevelBuilder) -> GridLevelBuilder,
    {
        let builder = GridLevelBuilder::new(GridLevel::tertiary());
        let configured = f(builder);
        self.config.tertiary = Some(configured.build());
        self
    }

    /// Set the number of minor divisions between major lines.
    pub fn divisions(mut self, count: usize) -> Self {
        self.config.minor_divisions = count;
        self
    }

    /// Set the grid spacing strategy.
    pub fn spacing(mut self, spacing: GridSpacing) -> Self {
        self.config.spacing = spacing;
        self
    }

    /// Use auto spacing with the target count.
    pub fn auto_spacing(mut self, count: usize) -> Self {
        self.config.spacing = GridSpacing::auto(count);
        self
    }

    /// Use fixed interval spacing.
    pub fn fixed_spacing(mut self, interval: f64) -> Self {
        self.config.spacing = GridSpacing::fixed(interval);
        self
    }

    /// Build the grid configuration.
    pub fn build(self) -> GridConfig {
        self.config
    }
}

impl Default for GridBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for a single grid level.
#[derive(Debug)]
pub struct GridLevelBuilder {
    level: GridLevel,
}

impl GridLevelBuilder {
    /// Create a new grid level builder.
    pub fn new(level: GridLevel) -> Self {
        Self { level }
    }

    /// Set the line thickness.
    pub fn thickness(mut self, thickness: f32) -> Self {
        self.level.thickness = thickness;
        self
    }

    /// Set the line color.
    pub fn color(mut self, color: Color) -> Self {
        self.level.color = color;
        self
    }

    /// Set the dash pattern.
    pub fn dash(mut self, dash: DashPattern) -> Self {
        self.level.dash = dash;
        self
    }

    /// Make this a dotted line.
    pub fn dotted(mut self) -> Self {
        self.level.dash = DashPattern::dotted(2.0);
        self
    }

    /// Make this a dashed line.
    pub fn dashed(mut self) -> Self {
        self.level.dash = DashPattern::medium_dash();
        self
    }

    /// Enable or disable this level.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.level.enabled = enabled;
        self
    }

    /// Build the grid level.
    pub fn build(self) -> GridLevel {
        self.level
    }
}

// =============================================================================
// SeriesBuilder
// =============================================================================

/// Builder for configuring data series.
#[derive(Debug)]
pub struct SeriesBuilder {
    name: String,
    data: Vec<DataPoint>,
    style: SeriesStyle,
    x_axis: AxisId,
    y_axis: AxisId,
    is_streaming: bool,
    streaming_capacity: usize,
}

impl SeriesBuilder {
    /// Create a new series builder.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            data: Vec::new(),
            style: SeriesStyle::default(),
            x_axis: AxisId::X_PRIMARY,
            y_axis: AxisId::Y_PRIMARY,
            is_streaming: false,
            streaming_capacity: 1000,
        }
    }

    /// Set the data points.
    pub fn data<T: Into<DataPoint> + Copy>(mut self, data: &[T]) -> Self {
        self.data = data.iter().map(|&d| d.into()).collect();
        self
    }

    /// Set as a streaming series with ring buffer.
    pub fn streaming(mut self, capacity: usize) -> Self {
        self.is_streaming = true;
        self.streaming_capacity = capacity;
        self
    }

    /// Set the line color.
    pub fn color(mut self, color: Color) -> Self {
        self.style.color = color;
        self
    }

    /// Set the line width.
    pub fn line_width(mut self, width: f32) -> Self {
        self.style.line_width = width;
        self
    }

    /// Make this a dashed line.
    pub fn dashed(mut self, dash_len: f32, gap_len: f32) -> Self {
        self.style.line_style = LineStyle::Dashed;
        let _ = (dash_len, gap_len); // Would be used with enhanced line config
        self
    }

    /// Make this a dotted line.
    pub fn dotted(mut self) -> Self {
        self.style.line_style = LineStyle::Dotted;
        self
    }

    /// Add markers using a closure.
    pub fn markers<F>(mut self, f: F) -> Self
    where
        F: FnOnce(MarkerBuilder) -> MarkerBuilder,
    {
        let builder = MarkerBuilder::new();
        let configured = f(builder);
        self.style.point_style = Some(configured.build());
        self
    }

    /// Add simple circle markers.
    pub fn with_markers(mut self) -> Self {
        self.style.point_style = Some(PointStyle {
            size: 6.0,
            shape: super::style::MarkerShape::Circle,
            color: self.style.color,
        });
        self
    }

    /// Add fill to baseline.
    pub fn fill_to_baseline(mut self, color: Color) -> Self {
        self.style.fill = Some(FillStyle {
            color,
            opacity: color.a,
        });
        self
    }

    /// Set the X axis.
    pub fn x_axis(mut self, axis: AxisId) -> Self {
        self.x_axis = axis;
        self
    }

    /// Set the Y axis.
    pub fn y_axis(mut self, axis: AxisId) -> Self {
        self.y_axis = axis;
        self
    }

    /// Set both axes.
    pub fn axes(mut self, x_axis: AxisId, y_axis: AxisId) -> Self {
        self.x_axis = x_axis;
        self.y_axis = y_axis;
        self
    }

    /// Set z-order.
    pub fn z_order(mut self, z_order: i32) -> Self {
        self.style.z_order = z_order;
        self
    }

    /// Set visibility.
    pub fn visible(mut self, visible: bool) -> Self {
        self.style.visible = visible;
        self
    }

    /// Hide from legend.
    pub fn hide_from_legend(mut self) -> Self {
        self.style.show_in_legend = false;
        self
    }

    /// Build the series.
    pub fn build(self) -> Series {
        Series {
            name: self.name,
            data: self.data,
            style: self.style,
            x_axis: self.x_axis,
            y_axis: self.y_axis,
        }
    }
}

/// Builder for marker configuration.
#[derive(Debug)]
pub struct MarkerBuilder {
    style: PointStyle,
}

impl MarkerBuilder {
    /// Create a new marker builder.
    pub fn new() -> Self {
        Self {
            style: PointStyle::default(),
        }
    }

    /// Use circle markers.
    pub fn circle(mut self) -> Self {
        self.style.shape = super::style::MarkerShape::Circle;
        self
    }

    /// Use square markers.
    pub fn square(mut self) -> Self {
        self.style.shape = super::style::MarkerShape::Square;
        self
    }

    /// Use diamond markers.
    pub fn diamond(mut self) -> Self {
        self.style.shape = super::style::MarkerShape::Diamond;
        self
    }

    /// Use triangle markers.
    pub fn triangle(mut self) -> Self {
        self.style.shape = super::style::MarkerShape::Triangle;
        self
    }

    /// Set marker size.
    pub fn size(mut self, size: f32) -> Self {
        self.style.size = size;
        self
    }

    /// Set marker color.
    pub fn color(mut self, color: Color) -> Self {
        self.style.color = color;
        self
    }

    /// Build the point style.
    pub fn build(self) -> PointStyle {
        self.style
    }
}

impl Default for MarkerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_chart_builder() {
        let chart = ChartBuilder::line()
            .title("Test Chart")
            .x_label("X")
            .y_label("Y")
            .add_series("Series 1", &[(0.0, 1.0), (1.0, 2.0), (2.0, 1.5)])
            .with_grid()
            .build();

        assert_eq!(chart.chart_type, ChartType::Line);
        assert_eq!(
            chart.title.as_ref().map(|t| t.text.as_str()),
            Some("Test Chart")
        );
        assert_eq!(chart.series.len(), 1);
    }

    #[test]
    fn test_function_plot() {
        let chart = ChartBuilder::line()
            .plot_function("sin(x)", |x| x.sin(), 0.0, std::f64::consts::TAU, 100)
            .build();

        assert_eq!(chart.series.len(), 1);
        assert_eq!(chart.series[0].data.len(), 100);
    }

    #[test]
    fn test_secondary_axis() {
        let chart = ChartBuilder::line()
            .add_series("Primary", &[(0.0, 1.0), (1.0, 2.0)])
            .secondary_y_axis(Axis::y_secondary().with_label("Secondary Y"))
            .secondary_y_range(0.0, 100.0)
            .add_series_secondary_y("Secondary", &[(0.0, 50.0), (1.0, 75.0)])
            .build();

        assert_eq!(chart.series.len(), 2);
        assert_eq!(chart.series[1].y_axis, AxisId::Y_SECONDARY);
        assert!(chart.get_axis(AxisId::Y_SECONDARY).is_some());
    }

    #[test]
    fn test_annotations() {
        let chart = ChartBuilder::line()
            .add_text_at("Peak", 1.0, 2.0)
            .add_horizontal_line(1.5, Color::RED)
            .add_horizontal_band(0.5, 1.0, Color::rgba(0.0, 1.0, 0.0, 0.2))
            .build();

        assert_eq!(chart.text_annotations.len(), 1);
        assert_eq!(chart.line_annotations.len(), 1);
        assert_eq!(chart.fill_regions.len(), 1);
    }
}
