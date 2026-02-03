//! Fill rules and fill options.
//!
//! Fill rules determine how to decide which areas are "inside" a path.

use crate::Paint;
use astrelis_render::Color;

/// Fill rule for determining interior of a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FillRule {
    /// Non-zero winding rule (default).
    ///
    /// A point is inside if the winding number is non-zero.
    #[default]
    NonZero,
    /// Even-odd (parity) rule.
    ///
    /// A point is inside if the number of crossings is odd.
    EvenOdd,
}

/// Fill properties for geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct Fill {
    /// The paint to use for filling
    pub paint: Paint,
    /// Fill rule for determining interior
    pub rule: FillRule,
    /// Opacity multiplier (0.0 to 1.0)
    pub opacity: f32,
}

impl Fill {
    /// Create a solid color fill.
    pub fn solid(color: Color) -> Self {
        Self {
            paint: Paint::Solid(color),
            rule: FillRule::NonZero,
            opacity: 1.0,
        }
    }

    /// Create a fill from a paint.
    pub fn from_paint(paint: Paint) -> Self {
        Self {
            paint,
            rule: FillRule::NonZero,
            opacity: 1.0,
        }
    }

    /// Set the fill rule.
    pub fn with_rule(mut self, rule: FillRule) -> Self {
        self.rule = rule;
        self
    }

    /// Set the opacity.
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// Get the effective color (for solid fills).
    pub fn effective_color(&self) -> Option<Color> {
        match &self.paint {
            Paint::Solid(color) => Some(Color::rgba(color.r, color.g, color.b, color.a * self.opacity)),
            _ => None,
        }
    }
}

impl Default for Fill {
    fn default() -> Self {
        Self::solid(Color::BLACK)
    }
}

impl From<Color> for Fill {
    fn from(color: Color) -> Self {
        Self::solid(color)
    }
}

impl From<Paint> for Fill {
    fn from(paint: Paint) -> Self {
        Self::from_paint(paint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solid_fill() {
        let fill = Fill::solid(Color::RED);
        assert_eq!(fill.opacity, 1.0);
        assert!(matches!(fill.paint, Paint::Solid(_)));
    }

    #[test]
    fn test_fill_opacity() {
        let fill = Fill::solid(Color::RED).with_opacity(0.5);
        let effective = fill.effective_color().unwrap();
        assert!((effective.a - 0.5).abs() < 0.01);
    }
}
