//! Local node transform (translation, rotation, scale).

use astrelis_core::math::{Mat4, Quat, Vec3};

/// A node's local transform, relative to its parent.
///
/// Composed as scale, then rotation, then translation (standard TRS).
/// 2D content uses `position.x`/`position.y` plus
/// [`set_rotation_2d`](Self::set_rotation_2d); `position.z` is
/// available for draw-order layering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// Translation relative to the parent.
    pub position: Vec3,
    /// Rotation relative to the parent.
    pub rotation: Quat,
    /// Scale relative to the parent.
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    /// The identity transform: zero translation, no rotation, unit scale.
    pub const IDENTITY: Self = Self {
        position: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    /// Creates a transform with the given translation.
    pub fn from_position(position: Vec3) -> Self {
        Self {
            position,
            ..Self::IDENTITY
        }
    }

    /// Creates a 2D transform at `(x, y)` with `z = 0`.
    pub fn from_xy(x: f32, y: f32) -> Self {
        Self::from_position(Vec3::new(x, y, 0.0))
    }

    /// Sets the rotation to `angle` radians around +Z (the 2D rotation axis).
    pub fn set_rotation_2d(&mut self, angle: f32) {
        self.rotation = Quat::from_rotation_z(angle);
    }

    /// Computes the local transformation matrix.
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use astrelis_core::math::{Mat4, Quat, Vec3};

    #[test]
    fn default_is_identity() {
        assert_eq!(Transform::default(), Transform::IDENTITY);
        assert_eq!(Transform::IDENTITY.matrix(), Mat4::IDENTITY);
    }

    #[test]
    fn matrix_applies_scale_then_rotation_then_translation() {
        let t = Transform {
            position: Vec3::new(1.0, 0.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(2.0),
        };
        // (1,1,1) scaled by 2 -> (2,2,2), translated by (1,0,0) -> (3,2,2)
        let p = t.matrix().transform_point3(Vec3::ONE);
        assert!(p.abs_diff_eq(Vec3::new(3.0, 2.0, 2.0), 1e-6));
    }

    #[test]
    fn from_xy_sets_z_zero() {
        let t = Transform::from_xy(3.0, 4.0);
        assert_eq!(t.position, Vec3::new(3.0, 4.0, 0.0));
        assert_eq!(t.rotation, Quat::IDENTITY);
        assert_eq!(t.scale, Vec3::ONE);
    }

    #[test]
    fn rotation_2d_rotates_around_z() {
        let mut t = Transform::IDENTITY;
        t.set_rotation_2d(std::f32::consts::FRAC_PI_2);
        // +X rotated 90 degrees around +Z lands on +Y.
        let p = t.matrix().transform_point3(Vec3::X);
        assert!(p.abs_diff_eq(Vec3::Y, 1e-6));
    }
}
