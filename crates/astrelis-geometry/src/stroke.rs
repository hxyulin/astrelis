//! Stroke properties for geometry outlines.
//!
//! Defines how paths are stroked: width, caps, joins, and dash patterns.

use crate::Paint;
use astrelis_render::Color;

/// Line cap style for stroke endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineCap {
    /// Flat cap ending at the endpoint.
    #[default]
    Butt,
    /// Round cap extending beyond the endpoint.
    Round,
    /// Square cap extending beyond the endpoint.
    Square,
}

/// Line join style for stroke corners.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineJoin {
    /// Miter join (sharp corner).
    #[default]
    Miter,
    /// Round join (rounded corner).
    Round,
    /// Bevel join (flat corner).
    Bevel,
}

/// Dash pattern for stroked lines.
#[derive(Debug, Clone, PartialEq)]
pub struct DashPattern {
    /// Alternating on/off lengths.
    pub pattern: Vec<f32>,
    /// Offset into the pattern to start.
    pub offset: f32,
}

impl DashPattern {
    /// Create a new dash pattern.
    pub fn new(pattern: Vec<f32>, offset: f32) -> Self {
        Self { pattern, offset }
    }

    /// Create a simple dashed line.
    pub fn dashed(dash: f32, gap: f32) -> Self {
        Self {
            pattern: vec![dash, gap],
            offset: 0.0,
        }
    }

    /// Create a dotted line.
    pub fn dotted(gap: f32) -> Self {
        Self {
            pattern: vec![0.0, gap],
            offset: 0.0,
        }
    }

    /// Create a dash-dot pattern.
    pub fn dash_dot(dash: f32, gap: f32, dot: f32) -> Self {
        Self {
            pattern: vec![dash, gap, dot, gap],
            offset: 0.0,
        }
    }
}

/// Stroke properties for geometry outlines.
#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    /// Stroke width in logical pixels
    pub width: f32,
    /// Paint for the stroke color/gradient
    pub paint: Paint,
    /// Line cap style
    pub line_cap: LineCap,
    /// Line join style
    pub line_join: LineJoin,
    /// Miter limit for miter joins
    pub miter_limit: f32,
    /// Optional dash pattern
    pub dash: Option<DashPattern>,
    /// Opacity multiplier (0.0 to 1.0)
    pub opacity: f32,
}

impl Stroke {
    /// Create a solid color stroke.
    pub fn solid(color: Color, width: f32) -> Self {
        Self {
            width,
            paint: Paint::Solid(color),
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            miter_limit: 4.0,
            dash: None,
            opacity: 1.0,
        }
    }

    /// Create a stroke from a paint.
    pub fn from_paint(paint: Paint, width: f32) -> Self {
        Self {
            width,
            paint,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            miter_limit: 4.0,
            dash: None,
            opacity: 1.0,
        }
    }

    /// Set the line cap style.
    pub fn with_line_cap(mut self, cap: LineCap) -> Self {
        self.line_cap = cap;
        self
    }

    /// Set the line join style.
    pub fn with_line_join(mut self, join: LineJoin) -> Self {
        self.line_join = join;
        self
    }

    /// Set the miter limit.
    pub fn with_miter_limit(mut self, limit: f32) -> Self {
        self.miter_limit = limit.max(1.0);
        self
    }

    /// Set a dash pattern.
    pub fn with_dash(mut self, pattern: DashPattern) -> Self {
        self.dash = Some(pattern);
        self
    }

    /// Set a simple dashed pattern.
    pub fn dashed(mut self, dash: f32, gap: f32) -> Self {
        self.dash = Some(DashPattern::dashed(dash, gap));
        self
    }

    /// Set the opacity.
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// Get the effective color (for solid strokes).
    pub fn effective_color(&self) -> Option<Color> {
        match &self.paint {
            Paint::Solid(color) => Some(Color::rgba(
                color.r,
                color.g,
                color.b,
                color.a * self.opacity,
            )),
            _ => None,
        }
    }

    /// Check if the stroke is visible.
    pub fn is_visible(&self) -> bool {
        self.width > 0.0 && self.opacity > 0.0
    }
}

impl Default for Stroke {
    fn default() -> Self {
        Self::solid(Color::BLACK, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solid_stroke() {
        let stroke = Stroke::solid(Color::RED, 2.0);
        assert_eq!(stroke.width, 2.0);
        assert!(stroke.is_visible());
    }

    #[test]
    fn test_dashed_stroke() {
        let stroke = Stroke::solid(Color::BLUE, 1.0).dashed(5.0, 3.0);
        assert!(stroke.dash.is_some());
    }

    #[test]
    fn test_stroke_visibility() {
        let stroke = Stroke::solid(Color::RED, 0.0);
        assert!(!stroke.is_visible());

        let stroke = Stroke::solid(Color::RED, 1.0).with_opacity(0.0);
        assert!(!stroke.is_visible());
    }
}
