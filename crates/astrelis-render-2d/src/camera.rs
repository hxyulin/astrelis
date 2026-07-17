//! Orthographic two-dimensional camera.

use astrelis_core::math::{Affine2, Mat4, Vec2, Vec3};

/// A logical-pixel-oriented, Y-down orthographic camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera2D {
    /// World-space point shown at the target center.
    pub center: Vec2,
    /// Clockwise camera rotation in radians in the Y-down world.
    pub rotation: f32,
    /// Magnification, where `1` maps one world unit to one logical pixel.
    pub zoom: f32,
}

impl Default for Camera2D {
    fn default() -> Self {
        Self {
            center: Vec2::ZERO,
            rotation: 0.0,
            zoom: 1.0,
        }
    }
}

impl Camera2D {
    /// Returns the world-to-clip transform for a logical viewport size.
    pub fn view_projection(self, logical_size: Vec2) -> Option<Mat4> {
        if !logical_size.is_finite()
            || logical_size.min_element() <= 0.0
            || !self.zoom.is_finite()
            || self.zoom <= 0.0
            || !self.center.is_finite()
            || !self.rotation.is_finite()
        {
            return None;
        }
        Some(
            Mat4::from_scale(Vec3::new(
                2.0 * self.zoom / logical_size.x,
                -2.0 * self.zoom / logical_size.y,
                1.0,
            )) * Mat4::from_rotation_z(-self.rotation)
                * Mat4::from_translation((-self.center).extend(0.0)),
        )
    }

    /// Returns a conservative world-space AABB for the rotated viewport.
    pub fn visible_bounds(self, logical_size: Vec2) -> Option<(Vec2, Vec2)> {
        self.view_projection(logical_size)?;
        let half = logical_size * (0.5 / self.zoom);
        let transform = Affine2::from_translation(self.center) * Affine2::from_angle(self.rotation);
        let corners = [
            transform.transform_point2(Vec2::new(-half.x, -half.y)),
            transform.transform_point2(Vec2::new(half.x, -half.y)),
            transform.transform_point2(Vec2::new(half.x, half.y)),
            transform.transform_point2(Vec2::new(-half.x, half.y)),
        ];
        let mut min = corners[0];
        let mut max = corners[0];
        for corner in &corners[1..] {
            min = min.min(*corner);
            max = max.max(*corner);
        }
        Some((min, max))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_maps_to_clip_center_and_y_is_down() {
        let camera = Camera2D {
            center: Vec2::new(10.0, 20.0),
            ..Default::default()
        };
        let matrix = camera.view_projection(Vec2::new(200.0, 100.0)).unwrap();
        assert!(
            matrix
                .transform_point3(Vec3::new(10.0, 20.0, 0.0))
                .truncate()
                .length()
                < 1e-6
        );
        assert!(matrix.transform_point3(Vec3::new(10.0, 30.0, 0.0)).y < 0.0);
    }
}
