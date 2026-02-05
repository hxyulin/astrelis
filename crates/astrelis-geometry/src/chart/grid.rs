//! Grid configuration and rendering for charts.
//!
//! Provides configurable grid lines with:
//! - Multiple grid levels (major, minor, tertiary)
//! - Dash patterns
//! - Automatic or custom spacing

use astrelis_render::Color;

/// Line dash pattern.
///
/// Defines how a line is rendered with alternating on/off segments.
/// An empty segments array indicates a solid line.
#[derive(Debug, Clone, PartialEq)]
pub struct DashPattern {
    /// Alternating lengths: [on, off, on, off, ...]
    ///
    /// Empty = solid line.
    pub segments: Vec<f32>,

    /// Phase offset (starting position in the pattern).
    pub phase: f32,
}

impl Default for DashPattern {
    fn default() -> Self {
        Self::SOLID
    }
}

impl DashPattern {
    /// Solid line (no dashes).
    pub const SOLID: DashPattern = DashPattern {
        segments: Vec::new(),
        phase: 0.0,
    };

    /// Create a dashed line pattern.
    ///
    /// # Arguments
    ///
    /// * `dash` - Length of the dash (on segment)
    /// * `gap` - Length of the gap (off segment)
    pub fn dashed(dash: f32, gap: f32) -> Self {
        Self {
            segments: vec![dash, gap],
            phase: 0.0,
        }
    }

    /// Create a dotted line pattern.
    ///
    /// # Arguments
    ///
    /// * `size` - Dot size and gap size
    pub fn dotted(size: f32) -> Self {
        Self {
            segments: vec![size, size],
            phase: 0.0,
        }
    }

    /// Create a dash-dot pattern.
    ///
    /// # Arguments
    ///
    /// * `dash` - Length of the dash
    /// * `dot` - Length of the dot
    /// * `gap` - Length of gaps
    pub fn dash_dot(dash: f32, dot: f32, gap: f32) -> Self {
        Self {
            segments: vec![dash, gap, dot, gap],
            phase: 0.0,
        }
    }

    /// Create a dash-dot-dot pattern.
    pub fn dash_dot_dot(dash: f32, dot: f32, gap: f32) -> Self {
        Self {
            segments: vec![dash, gap, dot, gap, dot, gap],
            phase: 0.0,
        }
    }

    /// Create a pattern from explicit segments.
    pub fn custom(segments: Vec<f32>) -> Self {
        Self {
            segments,
            phase: 0.0,
        }
    }

    /// Set the phase offset.
    pub fn with_phase(mut self, phase: f32) -> Self {
        self.phase = phase;
        self
    }

    /// Check if this is a solid line (no pattern).
    pub fn is_solid(&self) -> bool {
        self.segments.is_empty()
    }

    /// Get the total length of one pattern cycle.
    pub fn cycle_length(&self) -> f32 {
        self.segments.iter().sum()
    }

    /// Standard presets
    pub fn short_dash() -> Self {
        Self::dashed(4.0, 2.0)
    }

    pub fn medium_dash() -> Self {
        Self::dashed(8.0, 4.0)
    }

    pub fn long_dash() -> Self {
        Self::dashed(12.0, 6.0)
    }

    pub fn fine_dot() -> Self {
        Self::dotted(1.0)
    }
}

/// Configuration for a single grid level (major, minor, or tertiary).
#[derive(Debug, Clone, PartialEq)]
pub struct GridLevel {
    /// Whether this grid level is enabled.
    pub enabled: bool,

    /// Line thickness in pixels.
    pub thickness: f32,

    /// Line color.
    pub color: Color,

    /// Dash pattern (solid by default).
    pub dash: DashPattern,

    /// Z-order for rendering (higher = on top).
    pub z_order: i32,
}

impl Default for GridLevel {
    fn default() -> Self {
        Self {
            enabled: true,
            thickness: 1.0,
            color: Color::rgba(0.25, 0.25, 0.28, 1.0),
            dash: DashPattern::SOLID,
            z_order: 0,
        }
    }
}

impl GridLevel {
    /// Create a new grid level.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a disabled grid level.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Set whether this level is enabled.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Set the line thickness.
    pub fn with_thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness;
        self
    }

    /// Set the line color.
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Set the dash pattern.
    pub fn with_dash(mut self, dash: DashPattern) -> Self {
        self.dash = dash;
        self
    }

    /// Set the z-order.
    pub fn with_z_order(mut self, z_order: i32) -> Self {
        self.z_order = z_order;
        self
    }

    /// Make this a dotted line.
    pub fn dotted(mut self) -> Self {
        self.dash = DashPattern::dotted(2.0);
        self
    }

    /// Make this a dashed line.
    pub fn dashed(mut self) -> Self {
        self.dash = DashPattern::medium_dash();
        self
    }

    /// Create a major grid level preset.
    pub fn major() -> Self {
        Self {
            enabled: true,
            thickness: 1.0,
            color: Color::rgba(0.3, 0.3, 0.33, 1.0),
            dash: DashPattern::SOLID,
            z_order: 0,
        }
    }

    /// Create a minor grid level preset.
    pub fn minor() -> Self {
        Self {
            enabled: true,
            thickness: 0.5,
            color: Color::rgba(0.2, 0.2, 0.22, 0.8),
            dash: DashPattern::SOLID,
            z_order: -1,
        }
    }

    /// Create a tertiary (very fine) grid level preset.
    pub fn tertiary() -> Self {
        Self {
            enabled: false, // Disabled by default
            thickness: 0.25,
            color: Color::rgba(0.15, 0.15, 0.17, 0.5),
            dash: DashPattern::dotted(1.0),
            z_order: -2,
        }
    }
}

/// Grid spacing strategy.
///
/// Determines how grid lines are positioned.
#[derive(Debug, Clone, PartialEq)]
pub enum GridSpacing {
    /// Automatic spacing targeting a specific number of lines.
    Auto {
        /// Target number of major grid lines.
        target_count: usize,
    },

    /// Fixed interval between grid lines.
    Fixed {
        /// Interval in data units.
        interval: f64,
    },

    /// Custom grid line positions.
    Custom {
        /// Explicit data values where grid lines should appear.
        values: Vec<f64>,
    },

    /// Logarithmic decades (for log scales).
    LogDecades {
        /// Number of subdivisions per decade (1, 2, 5, or 10 are common).
        subdivisions: usize,
    },

    /// Time-aware spacing (smart intervals for time data).
    ///
    /// Automatically chooses intervals like seconds, minutes, hours, etc.
    TimeAware,
}

impl Default for GridSpacing {
    fn default() -> Self {
        Self::Auto { target_count: 5 }
    }
}

impl GridSpacing {
    /// Create auto spacing with the given target count.
    pub fn auto(count: usize) -> Self {
        Self::Auto {
            target_count: count,
        }
    }

    /// Create fixed spacing with the given interval.
    pub fn fixed(interval: f64) -> Self {
        Self::Fixed { interval }
    }

    /// Create custom spacing with explicit values.
    pub fn custom(values: Vec<f64>) -> Self {
        Self::Custom { values }
    }

    /// Create log decade spacing.
    pub fn log_decades(subdivisions: usize) -> Self {
        Self::LogDecades { subdivisions }
    }

    /// Calculate grid line positions for the given range.
    ///
    /// Returns (major_positions, minor_positions).
    pub fn calculate_positions(
        &self,
        min: f64,
        max: f64,
        minor_divisions: usize,
    ) -> (Vec<f64>, Vec<f64>) {
        let range = max - min;
        if range.abs() < f64::EPSILON {
            return (vec![], vec![]);
        }

        let (major, minor) = match self {
            Self::Auto { target_count } => {
                self.calculate_auto(min, max, *target_count, minor_divisions)
            }

            Self::Fixed { interval } => {
                let major = self.calculate_fixed(min, max, *interval);
                let minor = if minor_divisions > 1 {
                    self.calculate_fixed(min, max, interval / minor_divisions as f64)
                        .into_iter()
                        .filter(|v| !major.iter().any(|m| (v - m).abs() < interval * 0.01))
                        .collect()
                } else {
                    vec![]
                };
                (major, minor)
            }

            Self::Custom { values } => {
                let major: Vec<f64> = values
                    .iter()
                    .filter(|&&v| v >= min && v <= max)
                    .copied()
                    .collect();
                (major, vec![])
            }

            Self::LogDecades { subdivisions } => {
                self.calculate_log_decades(min, max, *subdivisions)
            }

            Self::TimeAware => self.calculate_time_aware(min, max, minor_divisions),
        };

        (major, minor)
    }

    fn calculate_auto(
        &self,
        min: f64,
        max: f64,
        target_count: usize,
        minor_divisions: usize,
    ) -> (Vec<f64>, Vec<f64>) {
        let range = max - min;

        // Calculate a "nice" interval
        let rough_interval = range / target_count as f64;
        let magnitude = 10f64.powf(rough_interval.log10().floor());
        let normalized = rough_interval / magnitude;

        let nice_interval = if normalized < 1.5 {
            magnitude
        } else if normalized < 3.0 {
            2.0 * magnitude
        } else if normalized < 7.0 {
            5.0 * magnitude
        } else {
            10.0 * magnitude
        };

        let major = self.calculate_fixed(min, max, nice_interval);

        let minor = if minor_divisions > 1 {
            let minor_interval = nice_interval / minor_divisions as f64;
            self.calculate_fixed(min, max, minor_interval)
                .into_iter()
                .filter(|v| !major.iter().any(|m| (v - m).abs() < nice_interval * 0.01))
                .collect()
        } else {
            vec![]
        };

        (major, minor)
    }

    fn calculate_fixed(&self, min: f64, max: f64, interval: f64) -> Vec<f64> {
        if interval <= 0.0 {
            return vec![];
        }

        let start = (min / interval).ceil() * interval;
        let mut positions = Vec::new();
        let mut current = start;

        while current <= max {
            positions.push(current);
            current += interval;
        }

        positions
    }

    fn calculate_log_decades(
        &self,
        min: f64,
        max: f64,
        subdivisions: usize,
    ) -> (Vec<f64>, Vec<f64>) {
        if min <= 0.0 || max <= 0.0 {
            return (vec![], vec![]);
        }

        let log_min = min.log10().floor() as i32;
        let log_max = max.log10().ceil() as i32;

        let mut major = Vec::new();
        let mut minor = Vec::new();

        for exp in log_min..=log_max {
            let decade = 10f64.powi(exp);
            if decade >= min && decade <= max {
                major.push(decade);
            }

            if subdivisions > 1 {
                let subdivision_values: Vec<f64> = match subdivisions {
                    2 => vec![2.0, 5.0],
                    3 => vec![2.0, 4.0, 6.0, 8.0],
                    5 | _ => vec![2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0],
                };

                for &mult in &subdivision_values {
                    let value = decade * mult;
                    if value >= min && value <= max {
                        minor.push(value);
                    }
                }
            }
        }

        (major, minor)
    }

    fn calculate_time_aware(
        &self,
        min: f64,
        max: f64,
        minor_divisions: usize,
    ) -> (Vec<f64>, Vec<f64>) {
        let range = max - min;

        // Choose appropriate interval based on range
        let interval = if range < 60.0 {
            // Less than a minute: use seconds
            self.nice_time_interval(range, &[1.0, 2.0, 5.0, 10.0, 15.0, 30.0])
        } else if range < 3600.0 {
            // Less than an hour: use minutes
            self.nice_time_interval(range, &[60.0, 120.0, 300.0, 600.0, 900.0, 1800.0])
        } else if range < 86400.0 {
            // Less than a day: use hours
            self.nice_time_interval(range, &[3600.0, 7200.0, 10800.0, 21600.0, 43200.0])
        } else if range < 604800.0 {
            // Less than a week: use days
            self.nice_time_interval(range, &[86400.0, 172800.0])
        } else {
            // Use weeks or months
            self.nice_time_interval(range, &[604800.0, 2592000.0])
        };

        let major = self.calculate_fixed(min, max, interval);

        let minor = if minor_divisions > 1 {
            let minor_interval = interval / minor_divisions as f64;
            self.calculate_fixed(min, max, minor_interval)
                .into_iter()
                .filter(|v| !major.iter().any(|m| (v - m).abs() < interval * 0.01))
                .collect()
        } else {
            vec![]
        };

        (major, minor)
    }

    fn nice_time_interval(&self, range: f64, candidates: &[f64]) -> f64 {
        let target_count = 5;
        let ideal_interval = range / target_count as f64;

        candidates
            .iter()
            .copied()
            .min_by(|&a, &b| {
                let a_diff = (a - ideal_interval).abs();
                let b_diff = (b - ideal_interval).abs();
                a_diff.partial_cmp(&b_diff).unwrap()
            })
            .unwrap_or(ideal_interval)
    }
}

/// Complete grid configuration for an axis.
#[derive(Debug, Clone)]
pub struct GridConfig {
    /// Major grid lines.
    pub major: GridLevel,

    /// Minor grid lines (between major lines).
    pub minor: Option<GridLevel>,

    /// Tertiary grid lines (finest level).
    pub tertiary: Option<GridLevel>,

    /// Spacing strategy for grid lines.
    pub spacing: GridSpacing,

    /// Number of minor divisions between major grid lines.
    pub minor_divisions: usize,

    /// Whether grid lines extend beyond the plot area.
    pub extend_beyond_plot: bool,
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            major: GridLevel::major(),
            minor: None,
            tertiary: None,
            spacing: GridSpacing::default(),
            minor_divisions: 4,
            extend_beyond_plot: false,
        }
    }
}

impl GridConfig {
    /// Create a new grid configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a configuration with no grid.
    pub fn none() -> Self {
        Self {
            major: GridLevel::disabled(),
            minor: None,
            tertiary: None,
            ..Default::default()
        }
    }

    /// Create a minimal grid (major lines only).
    pub fn minimal() -> Self {
        Self {
            major: GridLevel::major(),
            minor: None,
            tertiary: None,
            ..Default::default()
        }
    }

    /// Create a detailed grid (major + minor).
    pub fn detailed() -> Self {
        Self {
            major: GridLevel::major(),
            minor: Some(GridLevel::minor()),
            tertiary: None,
            minor_divisions: 5,
            ..Default::default()
        }
    }

    /// Create a very detailed grid (major + minor + tertiary).
    pub fn fine() -> Self {
        Self {
            major: GridLevel::major(),
            minor: Some(GridLevel::minor()),
            tertiary: Some(GridLevel::tertiary().with_enabled(true)),
            minor_divisions: 5,
            ..Default::default()
        }
    }

    /// Set the major grid level.
    pub fn with_major(mut self, major: GridLevel) -> Self {
        self.major = major;
        self
    }

    /// Set the minor grid level.
    pub fn with_minor(mut self, minor: GridLevel) -> Self {
        self.minor = Some(minor);
        self
    }

    /// Set the tertiary grid level.
    pub fn with_tertiary(mut self, tertiary: GridLevel) -> Self {
        self.tertiary = Some(tertiary);
        self
    }

    /// Set the spacing strategy.
    pub fn with_spacing(mut self, spacing: GridSpacing) -> Self {
        self.spacing = spacing;
        self
    }

    /// Set the number of minor divisions.
    pub fn with_minor_divisions(mut self, divisions: usize) -> Self {
        self.minor_divisions = divisions;
        self
    }

    /// Enable extension beyond the plot area.
    pub fn extend_beyond(mut self) -> Self {
        self.extend_beyond_plot = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dash_pattern_cycle_length() {
        let dashed = DashPattern::dashed(10.0, 5.0);
        assert_eq!(dashed.cycle_length(), 15.0);

        let solid = DashPattern::SOLID;
        assert_eq!(solid.cycle_length(), 0.0);
        assert!(solid.is_solid());
    }

    #[test]
    fn test_grid_spacing_auto() {
        let spacing = GridSpacing::auto(5);
        let (major, _minor) = spacing.calculate_positions(0.0, 100.0, 2);

        assert!(!major.is_empty());
        // Should have "nice" intervals
        for &pos in &major {
            // Should be at nice positions like 0, 20, 40, 60, 80, 100
            assert!(pos >= 0.0 && pos <= 100.0);
        }
    }

    #[test]
    fn test_grid_spacing_fixed() {
        let spacing = GridSpacing::fixed(10.0);
        let (major, _) = spacing.calculate_positions(0.0, 50.0, 1);

        assert_eq!(major, vec![0.0, 10.0, 20.0, 30.0, 40.0, 50.0]);
    }

    #[test]
    fn test_grid_spacing_log_decades() {
        let spacing = GridSpacing::log_decades(2);
        let (major, minor) = spacing.calculate_positions(1.0, 1000.0, 1);

        assert!(major.contains(&1.0));
        assert!(major.contains(&10.0));
        assert!(major.contains(&100.0));
        assert!(major.contains(&1000.0));
        assert!(!minor.is_empty());
    }

    #[test]
    fn test_grid_config_presets() {
        let minimal = GridConfig::minimal();
        assert!(minimal.major.enabled);
        assert!(minimal.minor.is_none());

        let detailed = GridConfig::detailed();
        assert!(detailed.major.enabled);
        assert!(detailed.minor.is_some());
        assert!(detailed.minor.as_ref().unwrap().enabled);
    }
}
