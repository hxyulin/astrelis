//! Bezier curve primitives.
//!
//! Provides quadratic and cubic Bezier curves for path construction.

use glam::Vec2;

/// A quadratic Bezier curve (one control point).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuadraticBezier {
    /// Start point
    pub from: Vec2,
    /// Control point
    pub control: Vec2,
    /// End point
    pub to: Vec2,
}

impl QuadraticBezier {
    /// Create a new quadratic Bezier curve.
    pub fn new(from: Vec2, control: Vec2, to: Vec2) -> Self {
        Self { from, control, to }
    }

    /// Evaluate the curve at parameter t (0.0 to 1.0).
    pub fn eval(&self, t: f32) -> Vec2 {
        let t2 = t * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;

        self.from * mt2 + self.control * (2.0 * mt * t) + self.to * t2
    }

    /// Get the derivative at parameter t.
    pub fn derivative(&self, t: f32) -> Vec2 {
        let mt = 1.0 - t;
        (self.control - self.from) * (2.0 * mt) + (self.to - self.control) * (2.0 * t)
    }

    /// Get the tangent (normalized derivative) at parameter t.
    pub fn tangent(&self, t: f32) -> Vec2 {
        self.derivative(t).normalize_or_zero()
    }

    /// Get the normal (perpendicular to tangent) at parameter t.
    pub fn normal(&self, t: f32) -> Vec2 {
        let tangent = self.tangent(t);
        Vec2::new(-tangent.y, tangent.x)
    }

    /// Split the curve at parameter t, returning two curves.
    pub fn split(&self, t: f32) -> (Self, Self) {
        let p01 = self.from.lerp(self.control, t);
        let p12 = self.control.lerp(self.to, t);
        let p012 = p01.lerp(p12, t);

        (
            Self::new(self.from, p01, p012),
            Self::new(p012, p12, self.to),
        )
    }

    /// Approximate the arc length of the curve.
    pub fn arc_length(&self, subdivisions: usize) -> f32 {
        let mut length = 0.0;
        let mut prev = self.from;

        for i in 1..=subdivisions {
            let t = i as f32 / subdivisions as f32;
            let point = self.eval(t);
            length += prev.distance(point);
            prev = point;
        }

        length
    }
}

/// A cubic Bezier curve (two control points).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicBezier {
    /// Start point
    pub from: Vec2,
    /// First control point
    pub control1: Vec2,
    /// Second control point
    pub control2: Vec2,
    /// End point
    pub to: Vec2,
}

impl CubicBezier {
    /// Create a new cubic Bezier curve.
    pub fn new(from: Vec2, control1: Vec2, control2: Vec2, to: Vec2) -> Self {
        Self {
            from,
            control1,
            control2,
            to,
        }
    }

    /// Evaluate the curve at parameter t (0.0 to 1.0).
    pub fn eval(&self, t: f32) -> Vec2 {
        let t2 = t * t;
        let t3 = t2 * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;
        let mt3 = mt2 * mt;

        self.from * mt3
            + self.control1 * (3.0 * mt2 * t)
            + self.control2 * (3.0 * mt * t2)
            + self.to * t3
    }

    /// Get the derivative at parameter t.
    pub fn derivative(&self, t: f32) -> Vec2 {
        let t2 = t * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;

        (self.control1 - self.from) * (3.0 * mt2)
            + (self.control2 - self.control1) * (6.0 * mt * t)
            + (self.to - self.control2) * (3.0 * t2)
    }

    /// Get the tangent (normalized derivative) at parameter t.
    pub fn tangent(&self, t: f32) -> Vec2 {
        self.derivative(t).normalize_or_zero()
    }

    /// Get the normal (perpendicular to tangent) at parameter t.
    pub fn normal(&self, t: f32) -> Vec2 {
        let tangent = self.tangent(t);
        Vec2::new(-tangent.y, tangent.x)
    }

    /// Split the curve at parameter t, returning two curves.
    pub fn split(&self, t: f32) -> (Self, Self) {
        let p01 = self.from.lerp(self.control1, t);
        let p12 = self.control1.lerp(self.control2, t);
        let p23 = self.control2.lerp(self.to, t);
        let p012 = p01.lerp(p12, t);
        let p123 = p12.lerp(p23, t);
        let p0123 = p012.lerp(p123, t);

        (
            Self::new(self.from, p01, p012, p0123),
            Self::new(p0123, p123, p23, self.to),
        )
    }

    /// Approximate the arc length of the curve.
    pub fn arc_length(&self, subdivisions: usize) -> f32 {
        let mut length = 0.0;
        let mut prev = self.from;

        for i in 1..=subdivisions {
            let t = i as f32 / subdivisions as f32;
            let point = self.eval(t);
            length += prev.distance(point);
            prev = point;
        }

        length
    }

    /// Convert to a quadratic approximation (loses accuracy).
    pub fn to_quadratic(&self) -> QuadraticBezier {
        // Use the midpoint of the control points
        let control = (self.control1 + self.control2) * 0.5;
        QuadraticBezier::new(self.from, control, self.to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quadratic_endpoints() {
        let curve = QuadraticBezier::new(
            Vec2::new(0.0, 0.0),
            Vec2::new(50.0, 100.0),
            Vec2::new(100.0, 0.0),
        );

        assert_eq!(curve.eval(0.0), curve.from);
        assert_eq!(curve.eval(1.0), curve.to);
    }

    #[test]
    fn test_cubic_endpoints() {
        let curve = CubicBezier::new(
            Vec2::new(0.0, 0.0),
            Vec2::new(25.0, 100.0),
            Vec2::new(75.0, 100.0),
            Vec2::new(100.0, 0.0),
        );

        assert_eq!(curve.eval(0.0), curve.from);
        assert_eq!(curve.eval(1.0), curve.to);
    }

    #[test]
    fn test_quadratic_split() {
        let curve = QuadraticBezier::new(
            Vec2::new(0.0, 0.0),
            Vec2::new(50.0, 100.0),
            Vec2::new(100.0, 0.0),
        );

        let (left, right) = curve.split(0.5);
        let midpoint = curve.eval(0.5);

        assert!((left.to - midpoint).length() < 0.001);
        assert!((right.from - midpoint).length() < 0.001);
    }
}
