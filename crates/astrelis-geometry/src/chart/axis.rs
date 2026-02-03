//! Enhanced axis system with multiple scale types and axis linking.
//!
//! This module provides:
//! - `ScaleType` - Linear, logarithmic, symmetric log, and time scales
//! - `AxisLink` - Linking axes for synchronized pan/zoom
//! - Extended `AxisPosition` with custom positioning
//! - `TickConfig` - Fine-grained tick configuration

use super::style::AxisStyle;
use super::types::{AxisId, AxisOrientation, AxisPosition};

/// Time epoch for time-based scales.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimeEpoch {
    /// Unix epoch (1970-01-01 00:00:00 UTC)
    #[default]
    Unix,
    /// J2000 epoch (2000-01-01 12:00:00 TT)
    J2000,
    /// Custom epoch (offset in seconds from Unix epoch)
    Custom(i64),
}

/// Scale type for axis transformation.
///
/// Determines how data values are mapped to pixel coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ScaleType {
    /// Linear scale (default).
    ///
    /// Data values are mapped linearly to pixel coordinates.
    #[default]
    Linear,

    /// Logarithmic scale.
    ///
    /// Useful for data spanning multiple orders of magnitude.
    /// Data must be > 0.
    Logarithmic {
        /// Log base (typically 10 or e)
        base: f64,
    },

    /// Symmetric logarithmic scale.
    ///
    /// Like log scale but handles negative values and zero.
    /// Uses linear scale near zero and log scale for larger values.
    Symlog {
        /// Threshold below which linear scaling is used
        lin_threshold: f64,
    },

    /// Time-based scale.
    ///
    /// Data values are interpreted as timestamps.
    Time {
        /// Time epoch for interpretation
        epoch: TimeEpoch,
    },
}

impl ScaleType {
    /// Create a base-10 logarithmic scale.
    pub fn log10() -> Self {
        Self::Logarithmic { base: 10.0 }
    }

    /// Create a natural logarithmic scale.
    pub fn ln() -> Self {
        Self::Logarithmic { base: std::f64::consts::E }
    }

    /// Create a symmetric log scale with the given threshold.
    pub fn symlog(threshold: f64) -> Self {
        Self::Symlog {
            lin_threshold: threshold,
        }
    }

    /// Create a time scale with Unix epoch.
    pub fn time() -> Self {
        Self::Time {
            epoch: TimeEpoch::Unix,
        }
    }

    /// Transform a data value to normalized coordinates [0, 1].
    ///
    /// Given a value in the range [min, max], returns a normalized value.
    pub fn normalize(&self, value: f64, min: f64, max: f64) -> f64 {
        if (max - min).abs() < f64::EPSILON {
            return 0.5;
        }

        match self {
            Self::Linear | Self::Time { .. } => (value - min) / (max - min),

            Self::Logarithmic { base } => {
                if value <= 0.0 || min <= 0.0 || max <= 0.0 {
                    // Fall back to linear for invalid log values
                    return (value - min) / (max - min);
                }
                let log_value = value.log(*base);
                let log_min = min.log(*base);
                let log_max = max.log(*base);
                (log_value - log_min) / (log_max - log_min)
            }

            Self::Symlog { lin_threshold } => {
                let symlog = |x: f64| -> f64 {
                    let thresh = *lin_threshold;
                    if x.abs() < thresh {
                        x / thresh
                    } else {
                        x.signum() * (1.0 + (x.abs() / thresh).ln())
                    }
                };

                let sym_value = symlog(value);
                let sym_min = symlog(min);
                let sym_max = symlog(max);
                (sym_value - sym_min) / (sym_max - sym_min)
            }
        }
    }

    /// Transform a normalized coordinate back to data value.
    ///
    /// Inverse of `normalize`.
    pub fn denormalize(&self, normalized: f64, min: f64, max: f64) -> f64 {
        match self {
            Self::Linear | Self::Time { .. } => min + normalized * (max - min),

            Self::Logarithmic { base } => {
                if min <= 0.0 || max <= 0.0 {
                    return min + normalized * (max - min);
                }
                let log_min = min.log(*base);
                let log_max = max.log(*base);
                let log_value = log_min + normalized * (log_max - log_min);
                base.powf(log_value)
            }

            Self::Symlog { lin_threshold } => {
                let thresh = *lin_threshold;

                let symlog = |x: f64| -> f64 {
                    if x.abs() < thresh {
                        x / thresh
                    } else {
                        x.signum() * (1.0 + (x.abs() / thresh).ln())
                    }
                };

                let inv_symlog = |y: f64| -> f64 {
                    if y.abs() < 1.0 {
                        y * thresh
                    } else {
                        y.signum() * thresh * (y.abs() - 1.0).exp()
                    }
                };

                let sym_min = symlog(min);
                let sym_max = symlog(max);
                let sym_value = sym_min + normalized * (sym_max - sym_min);
                inv_symlog(sym_value)
            }
        }
    }
}

/// Axis linking configuration for synchronized pan/zoom.
///
/// Multiple axes can be linked together so that panning or zooming
/// one axis affects all linked axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AxisLink {
    /// Pan synchronization group.
    ///
    /// Axes with the same pan group will pan together.
    /// `None` means no pan linking.
    pub pan_group: Option<u32>,

    /// Zoom synchronization group.
    ///
    /// Axes with the same zoom group will zoom together.
    /// `None` means no zoom linking.
    pub zoom_group: Option<u32>,

    /// Whether this axis is inverted relative to its link group.
    ///
    /// When `true`, pan/zoom operations are applied in reverse.
    pub inverted: bool,
}

impl AxisLink {
    /// Create an unlinked axis.
    pub fn none() -> Self {
        Self::default()
    }

    /// Link both pan and zoom to the specified group.
    pub fn linked(group: u32) -> Self {
        Self {
            pan_group: Some(group),
            zoom_group: Some(group),
            inverted: false,
        }
    }

    /// Link only pan to the specified group.
    pub fn pan_only(group: u32) -> Self {
        Self {
            pan_group: Some(group),
            zoom_group: None,
            inverted: false,
        }
    }

    /// Link only zoom to the specified group.
    pub fn zoom_only(group: u32) -> Self {
        Self {
            pan_group: None,
            zoom_group: Some(group),
            inverted: false,
        }
    }

    /// Set this axis as inverted relative to its link group.
    pub fn inverted(mut self) -> Self {
        self.inverted = true;
        self
    }
}

/// Extended axis position supporting custom positioning.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExtendedAxisPosition {
    /// Standard position (left, right, top, bottom).
    Standard(AxisPosition),

    /// Position at a specific data value.
    ///
    /// The axis line will be drawn at this data coordinate.
    AtValue(f64),

    /// Position at a percentage of the plot area.
    ///
    /// 0.0 = left/top edge, 1.0 = right/bottom edge.
    AtPercent(f32),
}

impl Default for ExtendedAxisPosition {
    fn default() -> Self {
        Self::Standard(AxisPosition::Left)
    }
}

impl From<AxisPosition> for ExtendedAxisPosition {
    fn from(pos: AxisPosition) -> Self {
        Self::Standard(pos)
    }
}

impl ExtendedAxisPosition {
    /// Check if this is a standard position.
    pub fn is_standard(&self) -> bool {
        matches!(self, Self::Standard(_))
    }

    /// Get the standard position if this is one.
    pub fn standard(&self) -> Option<AxisPosition> {
        match self {
            Self::Standard(pos) => Some(*pos),
            _ => None,
        }
    }
}

/// Tick configuration for axis labels.
#[derive(Debug, Clone)]
pub struct TickConfig {
    /// Target number of major ticks (auto-calculated if None).
    pub major_count: Option<usize>,

    /// Number of minor ticks between major ticks.
    pub minor_count: usize,

    /// Whether to show tick labels.
    pub show_labels: bool,

    /// Tick label format string (printf-style for numbers).
    /// `None` uses automatic formatting.
    pub label_format: Option<String>,

    /// Custom tick values and labels (overrides auto ticks).
    pub custom_ticks: Option<Vec<(f64, String)>>,

    /// Rotation angle for tick labels (in degrees).
    pub label_rotation: f32,

    /// Whether ticks point inward (into the plot area).
    pub ticks_inward: bool,
}

impl Default for TickConfig {
    fn default() -> Self {
        Self {
            major_count: Some(5),
            minor_count: 0,
            show_labels: true,
            label_format: None,
            custom_ticks: None,
            label_rotation: 0.0,
            ticks_inward: false,
        }
    }
}

impl TickConfig {
    /// Create tick config with the specified major tick count.
    pub fn with_count(count: usize) -> Self {
        Self {
            major_count: Some(count),
            ..Default::default()
        }
    }

    /// Create tick config with custom ticks.
    pub fn custom(ticks: Vec<(f64, String)>) -> Self {
        Self {
            custom_ticks: Some(ticks),
            ..Default::default()
        }
    }

    /// Set minor tick count.
    pub fn minor(mut self, count: usize) -> Self {
        self.minor_count = count;
        self
    }

    /// Hide tick labels.
    pub fn no_labels(mut self) -> Self {
        self.show_labels = false;
        self
    }

    /// Set label format string.
    pub fn format(mut self, fmt: impl Into<String>) -> Self {
        self.label_format = Some(fmt.into());
        self
    }

    /// Set label rotation.
    pub fn rotated(mut self, degrees: f32) -> Self {
        self.label_rotation = degrees;
        self
    }

    /// Set ticks to point inward.
    pub fn inward(mut self) -> Self {
        self.ticks_inward = true;
        self
    }
}

/// Enhanced axis configuration with all options.
#[derive(Debug, Clone)]
pub struct EnhancedAxis {
    /// Unique identifier.
    pub id: AxisId,

    /// Optional name for the axis (used for lookup by name).
    pub name: Option<String>,

    /// Axis label displayed alongside the axis.
    pub label: Option<String>,

    /// Axis orientation (horizontal or vertical).
    pub orientation: AxisOrientation,

    /// Position of the axis.
    pub position: ExtendedAxisPosition,

    /// Offset from the standard position (for stacked axes).
    pub position_offset: f32,

    /// Minimum value (None = auto from data).
    pub min: Option<f64>,

    /// Maximum value (None = auto from data).
    pub max: Option<f64>,

    /// Scale type for value transformation.
    pub scale: ScaleType,

    /// Tick configuration.
    pub ticks: TickConfig,

    /// Visual style.
    pub style: AxisStyle,

    /// Whether the axis is visible.
    pub visible: bool,

    /// Axis linking configuration.
    pub link: AxisLink,

    /// Whether to auto-range based on data.
    pub auto_range: bool,

    /// Padding to add around auto-ranged data (as a fraction).
    pub range_padding: f64,
}

impl Default for EnhancedAxis {
    fn default() -> Self {
        Self {
            id: AxisId::default(),
            name: None,
            label: None,
            orientation: AxisOrientation::Vertical,
            position: ExtendedAxisPosition::Standard(AxisPosition::Left),
            position_offset: 0.0,
            min: None,
            max: None,
            scale: ScaleType::Linear,
            ticks: TickConfig::default(),
            style: AxisStyle::default(),
            visible: true,
            link: AxisLink::none(),
            auto_range: true,
            range_padding: 0.05,
        }
    }
}

impl EnhancedAxis {
    /// Create a new primary X axis.
    pub fn x() -> Self {
        Self {
            id: AxisId::X_PRIMARY,
            orientation: AxisOrientation::Horizontal,
            position: ExtendedAxisPosition::Standard(AxisPosition::Bottom),
            ..Default::default()
        }
    }

    /// Create a new primary Y axis.
    pub fn y() -> Self {
        Self {
            id: AxisId::Y_PRIMARY,
            orientation: AxisOrientation::Vertical,
            position: ExtendedAxisPosition::Standard(AxisPosition::Left),
            ..Default::default()
        }
    }

    /// Create a secondary X axis (top).
    pub fn x_secondary() -> Self {
        Self {
            id: AxisId::X_SECONDARY,
            orientation: AxisOrientation::Horizontal,
            position: ExtendedAxisPosition::Standard(AxisPosition::Top),
            ..Default::default()
        }
    }

    /// Create a secondary Y axis (right).
    pub fn y_secondary() -> Self {
        Self {
            id: AxisId::Y_SECONDARY,
            orientation: AxisOrientation::Vertical,
            position: ExtendedAxisPosition::Standard(AxisPosition::Right),
            ..Default::default()
        }
    }

    /// Create a custom axis with the specified ID and name.
    pub fn custom(id: u32, name: impl Into<String>) -> Self {
        Self {
            id: AxisId::custom(id),
            name: Some(name.into()),
            ..Default::default()
        }
    }

    /// Set the axis label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the axis range.
    pub fn with_range(mut self, min: f64, max: f64) -> Self {
        self.min = Some(min);
        self.max = Some(max);
        self.auto_range = false;
        self
    }

    /// Set the scale type.
    pub fn with_scale(mut self, scale: ScaleType) -> Self {
        self.scale = scale;
        self
    }

    /// Set the position.
    pub fn with_position(mut self, position: impl Into<ExtendedAxisPosition>) -> Self {
        self.position = position.into();
        self
    }

    /// Set the position offset.
    pub fn with_offset(mut self, offset: f32) -> Self {
        self.position_offset = offset;
        self
    }

    /// Set the tick configuration.
    pub fn with_ticks(mut self, ticks: TickConfig) -> Self {
        self.ticks = ticks;
        self
    }

    /// Set tick count.
    pub fn with_tick_count(mut self, count: usize) -> Self {
        self.ticks.major_count = Some(count);
        self
    }

    /// Set the style.
    pub fn with_style(mut self, style: AxisStyle) -> Self {
        self.style = style;
        self
    }

    /// Set visibility.
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// Set axis linking.
    pub fn with_link(mut self, link: AxisLink) -> Self {
        self.link = link;
        self
    }

    /// Enable auto-ranging with the specified padding.
    pub fn auto_ranged(mut self, padding: f64) -> Self {
        self.auto_range = true;
        self.range_padding = padding;
        self
    }

    /// Get the effective range for this axis.
    ///
    /// If auto_range is enabled and data bounds are provided,
    /// returns the data bounds with padding applied.
    pub fn effective_range(&self, data_bounds: Option<(f64, f64)>) -> (f64, f64) {
        match (self.min, self.max) {
            (Some(min), Some(max)) => (min, max),
            (Some(min), None) => {
                let max = data_bounds.map(|(_, max)| max).unwrap_or(1.0);
                let padded_max = if self.auto_range {
                    max + (max - min).abs() * self.range_padding
                } else {
                    max
                };
                (min, padded_max)
            }
            (None, Some(max)) => {
                let min = data_bounds.map(|(min, _)| min).unwrap_or(0.0);
                let padded_min = if self.auto_range {
                    min - (max - min).abs() * self.range_padding
                } else {
                    min
                };
                (padded_min, max)
            }
            (None, None) => {
                let (min, max) = data_bounds.unwrap_or((0.0, 1.0));
                if self.auto_range {
                    let range = (max - min).abs();
                    let padding = if range < f64::EPSILON {
                        0.5 // Default padding for zero range
                    } else {
                        range * self.range_padding
                    };
                    (min - padding, max + padding)
                } else {
                    (min, max)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_scale() {
        let scale = ScaleType::Linear;

        assert!((scale.normalize(50.0, 0.0, 100.0) - 0.5).abs() < 0.001);
        assert!((scale.denormalize(0.5, 0.0, 100.0) - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_log_scale() {
        let scale = ScaleType::log10();

        // 10 is at 50% between 1 and 100 on a log scale
        let normalized = scale.normalize(10.0, 1.0, 100.0);
        assert!((normalized - 0.5).abs() < 0.001);

        let denormalized = scale.denormalize(0.5, 1.0, 100.0);
        assert!((denormalized - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_symlog_scale() {
        let scale = ScaleType::symlog(1.0);

        // Should handle zero and negative values
        let norm_zero = scale.normalize(0.0, -10.0, 10.0);
        assert!((norm_zero - 0.5).abs() < 0.001);

        // Round trip
        let value = -5.0;
        let normalized = scale.normalize(value, -10.0, 10.0);
        let denormalized = scale.denormalize(normalized, -10.0, 10.0);
        assert!((denormalized - value).abs() < 0.001);
    }

    #[test]
    fn test_axis_effective_range() {
        let axis = EnhancedAxis::y().auto_ranged(0.1);

        let range = axis.effective_range(Some((0.0, 100.0)));
        assert!(range.0 < 0.0); // Should have negative padding
        assert!(range.1 > 100.0); // Should have positive padding
    }

    #[test]
    fn test_axis_link() {
        let link = AxisLink::linked(1).inverted();

        assert_eq!(link.pan_group, Some(1));
        assert_eq!(link.zoom_group, Some(1));
        assert!(link.inverted);
    }
}
