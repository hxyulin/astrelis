//! 2D affine transformations.
//!
//! Provides a 2D transform matrix for translation, rotation, scaling, and skewing.

use glam::{Mat3, Vec2};

/// A 2D affine transformation matrix.
///
/// Internally uses a 3x3 matrix for affine transforms.
/// The last row is always [0, 0, 1].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform2D {
    matrix: Mat3,
}

impl Default for Transform2D {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform2D {
    /// Identity transform (no transformation).
    pub const IDENTITY: Self = Self {
        matrix: Mat3::IDENTITY,
    };

    /// Create from a 3x3 matrix.
    pub fn from_mat3(matrix: Mat3) -> Self {
        Self { matrix }
    }

    /// Create a translation transform.
    pub fn translate(offset: Vec2) -> Self {
        Self {
            matrix: Mat3::from_translation(offset),
        }
    }

    /// Create a rotation transform (angle in radians).
    pub fn rotate(angle: f32) -> Self {
        Self {
            matrix: Mat3::from_angle(angle),
        }
    }

    /// Create a uniform scale transform.
    pub fn scale(factor: f32) -> Self {
        Self {
            matrix: Mat3::from_scale(Vec2::splat(factor)),
        }
    }

    /// Create a non-uniform scale transform.
    pub fn scale_xy(scale: Vec2) -> Self {
        Self {
            matrix: Mat3::from_scale(scale),
        }
    }

    /// Create a skew transform.
    ///
    /// `skew_x` is the horizontal skew angle in radians.
    /// `skew_y` is the vertical skew angle in radians.
    pub fn skew(skew_x: f32, skew_y: f32) -> Self {
        Self {
            matrix: Mat3::from_cols(
                glam::Vec3::new(1.0, skew_y.tan(), 0.0),
                glam::Vec3::new(skew_x.tan(), 1.0, 0.0),
                glam::Vec3::new(0.0, 0.0, 1.0),
            ),
        }
    }

    /// Combine two transforms (self then other).
    pub fn then(&self, other: &Transform2D) -> Self {
        Self {
            matrix: other.matrix * self.matrix,
        }
    }

    /// Add a translation after this transform.
    pub fn then_translate(&self, offset: Vec2) -> Self {
        self.then(&Transform2D::translate(offset))
    }

    /// Add a rotation after this transform.
    pub fn then_rotate(&self, angle: f32) -> Self {
        self.then(&Transform2D::rotate(angle))
    }

    /// Add a scale after this transform.
    pub fn then_scale(&self, factor: f32) -> Self {
        self.then(&Transform2D::scale(factor))
    }

    /// Add a non-uniform scale after this transform.
    pub fn then_scale_xy(&self, scale: Vec2) -> Self {
        self.then(&Transform2D::scale_xy(scale))
    }

    /// Transform a point.
    pub fn transform_point(&self, point: Vec2) -> Vec2 {
        self.matrix.transform_point2(point)
    }

    /// Transform a vector (ignores translation).
    pub fn transform_vector(&self, vector: Vec2) -> Vec2 {
        self.matrix.transform_vector2(vector)
    }

    /// Get the inverse transform, if it exists.
    pub fn inverse(&self) -> Option<Self> {
        let det = self.matrix.determinant();
        if det.abs() < f32::EPSILON {
            None
        } else {
            Some(Self {
                matrix: self.matrix.inverse(),
            })
        }
    }

    /// Get the underlying 3x3 matrix.
    pub fn as_mat3(&self) -> &Mat3 {
        &self.matrix
    }

    /// Get the translation component.
    pub fn translation(&self) -> Vec2 {
        Vec2::new(self.matrix.z_axis.x, self.matrix.z_axis.y)
    }

    /// Get the scale component (approximate for non-uniform transforms).
    pub fn scale_factor(&self) -> Vec2 {
        Vec2::new(
            Vec2::new(self.matrix.x_axis.x, self.matrix.x_axis.y).length(),
            Vec2::new(self.matrix.y_axis.x, self.matrix.y_axis.y).length(),
        )
    }

    /// Get the rotation angle in radians (approximate for skewed transforms).
    pub fn rotation(&self) -> f32 {
        self.matrix.x_axis.y.atan2(self.matrix.x_axis.x)
    }
}

impl std::ops::Mul<Transform2D> for Transform2D {
    type Output = Transform2D;

    fn mul(self, rhs: Transform2D) -> Transform2D {
        self.then(&rhs)
    }
}

impl std::ops::Mul<Vec2> for Transform2D {
    type Output = Vec2;

    fn mul(self, rhs: Vec2) -> Vec2 {
        self.transform_point(rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_identity() {
        let t = Transform2D::IDENTITY;
        let point = Vec2::new(10.0, 20.0);
        assert_eq!(t.transform_point(point), point);
    }

    #[test]
    fn test_translate() {
        let t = Transform2D::translate(Vec2::new(5.0, 10.0));
        let point = Vec2::new(10.0, 20.0);
        assert_eq!(t.transform_point(point), Vec2::new(15.0, 30.0));
    }

    #[test]
    fn test_scale() {
        let t = Transform2D::scale(2.0);
        let point = Vec2::new(10.0, 20.0);
        assert_eq!(t.transform_point(point), Vec2::new(20.0, 40.0));
    }

    #[test]
    fn test_rotate_90() {
        let t = Transform2D::rotate(PI / 2.0);
        let point = Vec2::new(1.0, 0.0);
        let result = t.transform_point(point);
        assert!((result.x - 0.0).abs() < 0.001);
        assert!((result.y - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_chain_transforms() {
        let t = Transform2D::translate(Vec2::new(10.0, 0.0)).then_scale(2.0);
        let point = Vec2::new(5.0, 5.0);
        // First translate: (15, 5), then scale: (30, 10)
        let result = t.transform_point(point);
        assert_eq!(result, Vec2::new(30.0, 10.0));
    }

    #[test]
    fn test_inverse() {
        let t = Transform2D::translate(Vec2::new(10.0, 20.0)).then_scale(2.0);
        let inv = t.inverse().unwrap();
        let point = Vec2::new(5.0, 5.0);
        let transformed = t.transform_point(point);
        let restored = inv.transform_point(transformed);
        assert!((restored - point).length() < 0.001);
    }
}
