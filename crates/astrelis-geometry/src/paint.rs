//! Paint types for filling and stroking.
//!
//! Paints define how geometry is colored - solid colors or gradients.

use astrelis_render::Color;
use glam::Vec2;

/// A paint defines how to color geometry.
#[derive(Debug, Clone, PartialEq)]
pub enum Paint {
    /// Solid color.
    Solid(Color),
    /// Linear gradient.
    LinearGradient(LinearGradient),
    /// Radial gradient.
    RadialGradient(RadialGradient),
}

impl Paint {
    /// Create a solid color paint.
    pub fn solid(color: Color) -> Self {
        Self::Solid(color)
    }

    /// Create a linear gradient paint.
    pub fn linear_gradient(start: Vec2, end: Vec2, stops: Vec<GradientStop>) -> Self {
        Self::LinearGradient(LinearGradient { start, end, stops })
    }

    /// Create a radial gradient paint.
    pub fn radial_gradient(center: Vec2, radius: f32, stops: Vec<GradientStop>) -> Self {
        Self::RadialGradient(RadialGradient {
            center,
            radius,
            stops,
        })
    }

    /// Check if this is a solid color.
    pub fn is_solid(&self) -> bool {
        matches!(self, Self::Solid(_))
    }

    /// Get the solid color if this is a solid paint.
    pub fn as_solid(&self) -> Option<Color> {
        match self {
            Self::Solid(color) => Some(*color),
            _ => None,
        }
    }
}

impl Default for Paint {
    fn default() -> Self {
        Self::Solid(Color::BLACK)
    }
}

impl From<Color> for Paint {
    fn from(color: Color) -> Self {
        Self::Solid(color)
    }
}

/// A linear gradient.
#[derive(Debug, Clone, PartialEq)]
pub struct LinearGradient {
    /// Start point
    pub start: Vec2,
    /// End point
    pub end: Vec2,
    /// Color stops
    pub stops: Vec<GradientStop>,
}

impl LinearGradient {
    /// Create a new linear gradient.
    pub fn new(start: Vec2, end: Vec2, stops: Vec<GradientStop>) -> Self {
        Self { start, end, stops }
    }

    /// Create a horizontal gradient.
    pub fn horizontal(width: f32, stops: Vec<GradientStop>) -> Self {
        Self {
            start: Vec2::ZERO,
            end: Vec2::new(width, 0.0),
            stops,
        }
    }

    /// Create a vertical gradient.
    pub fn vertical(height: f32, stops: Vec<GradientStop>) -> Self {
        Self {
            start: Vec2::ZERO,
            end: Vec2::new(0.0, height),
            stops,
        }
    }

    /// Get the direction vector (normalized).
    pub fn direction(&self) -> Vec2 {
        (self.end - self.start).normalize_or_zero()
    }

    /// Interpolate color at a position.
    pub fn sample(&self, position: Vec2) -> Color {
        if self.stops.is_empty() {
            return Color::TRANSPARENT;
        }
        if self.stops.len() == 1 {
            return self.stops[0].color;
        }

        let dir = self.end - self.start;
        let len_sq = dir.length_squared();
        if len_sq < f32::EPSILON {
            return self.stops[0].color;
        }

        // Project position onto gradient line
        let t = ((position - self.start).dot(dir) / len_sq).clamp(0.0, 1.0);

        interpolate_gradient(&self.stops, t)
    }
}

/// A radial gradient.
#[derive(Debug, Clone, PartialEq)]
pub struct RadialGradient {
    /// Center point
    pub center: Vec2,
    /// Radius
    pub radius: f32,
    /// Color stops
    pub stops: Vec<GradientStop>,
}

impl RadialGradient {
    /// Create a new radial gradient.
    pub fn new(center: Vec2, radius: f32, stops: Vec<GradientStop>) -> Self {
        Self {
            center,
            radius,
            stops,
        }
    }

    /// Interpolate color at a position.
    pub fn sample(&self, position: Vec2) -> Color {
        if self.stops.is_empty() {
            return Color::TRANSPARENT;
        }
        if self.stops.len() == 1 {
            return self.stops[0].color;
        }

        let dist = (position - self.center).length();
        let t = (dist / self.radius).clamp(0.0, 1.0);

        interpolate_gradient(&self.stops, t)
    }
}

/// A color stop in a gradient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientStop {
    /// Position along the gradient (0.0 to 1.0)
    pub offset: f32,
    /// Color at this stop
    pub color: Color,
}

impl GradientStop {
    /// Create a new gradient stop.
    pub fn new(offset: f32, color: Color) -> Self {
        Self {
            offset: offset.clamp(0.0, 1.0),
            color,
        }
    }
}

/// Interpolate a gradient at a given t value.
fn interpolate_gradient(stops: &[GradientStop], t: f32) -> Color {
    if stops.is_empty() {
        return Color::TRANSPARENT;
    }
    if stops.len() == 1 {
        return stops[0].color;
    }

    // Find the two stops to interpolate between
    let mut prev = &stops[0];
    for stop in &stops[1..] {
        if t <= stop.offset {
            // Interpolate between prev and stop
            let range = stop.offset - prev.offset;
            if range < f32::EPSILON {
                return stop.color;
            }
            let local_t = (t - prev.offset) / range;
            return lerp_color(prev.color, stop.color, local_t);
        }
        prev = stop;
    }

    // Past the last stop
    stops.last().map(|s| s.color).unwrap_or(Color::TRANSPARENT)
}

/// Linearly interpolate between two colors.
fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    Color::rgba(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solid_paint() {
        let paint = Paint::solid(Color::RED);
        assert!(paint.is_solid());
        assert_eq!(paint.as_solid(), Some(Color::RED));
    }

    #[test]
    fn test_linear_gradient_sample() {
        let gradient = LinearGradient::horizontal(
            100.0,
            vec![
                GradientStop::new(0.0, Color::RED),
                GradientStop::new(1.0, Color::BLUE),
            ],
        );

        let at_start = gradient.sample(Vec2::new(0.0, 0.0));
        let at_end = gradient.sample(Vec2::new(100.0, 0.0));
        let at_mid = gradient.sample(Vec2::new(50.0, 0.0));

        assert!((at_start.r - 1.0).abs() < 0.01);
        assert!((at_end.b - 1.0).abs() < 0.01);
        // At midpoint, should be purple-ish
        assert!((at_mid.r - 0.5).abs() < 0.01);
        assert!((at_mid.b - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_gradient_stop() {
        let stop = GradientStop::new(0.5, Color::GREEN);
        assert_eq!(stop.offset, 0.5);
        assert_eq!(stop.color, Color::GREEN);
    }
}
