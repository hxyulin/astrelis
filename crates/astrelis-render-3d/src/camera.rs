//! Right-handed perspective camera.

use astrelis_core::math::{Mat4, Quat, Vec3};

/// Infinite-far reverse-Z perspective camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera3D {
    /// Camera position in world space.
    pub position: Vec3,
    /// Orientation; identity looks down negative Z with positive Y up.
    pub rotation: Quat,
    /// Vertical field of view in radians.
    pub fov_y: f32,
    /// Positive near-plane distance.
    pub near: f32,
}

impl Default for Camera3D {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            fov_y: 60.0_f32.to_radians(),
            near: 0.1,
        }
    }
}

impl Camera3D {
    /// Points the camera's negative-Z axis at a target.
    pub fn look_at(&mut self, target: Vec3, up: Vec3) {
        self.rotation = Quat::from_mat4(&Mat4::look_at_rh(self.position, target, up).inverse());
    }

    /// Returns the view matrix when all camera values are valid.
    pub fn view(self) -> Option<Mat4> {
        if !self.position.is_finite()
            || !self.rotation.is_finite()
            || !self.fov_y.is_finite()
            || !(0.0..std::f32::consts::PI).contains(&self.fov_y)
            || !self.near.is_finite()
            || self.near <= 0.0
        {
            return None;
        }
        Some(Mat4::from_rotation_translation(self.rotation, self.position).inverse())
    }

    /// Returns the reverse-Z view-projection matrix.
    pub fn view_projection(self, aspect: f32) -> Option<Mat4> {
        if !aspect.is_finite() || aspect <= 0.0 {
            return None;
        }
        Some(Mat4::perspective_infinite_reverse_rh(self.fov_y, aspect, self.near) * self.view()?)
    }

    pub(crate) fn sphere_visible(self, center: Vec3, radius: f32, aspect: f32) -> bool {
        let Some(view) = self.view() else {
            return false;
        };
        let point = view.transform_point3(center);
        let distance = -point.z;
        if distance + radius < self.near || distance + radius <= 0.0 {
            return false;
        }
        let half_y = distance.max(0.0) * (self.fov_y * 0.5).tan();
        let half_x = half_y * aspect;
        point.x.abs() <= half_x + radius && point.y.abs() <= half_y + radius
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ndc(camera: Camera3D, point: Vec3) -> Vec3 {
        let clip = camera.view_projection(1.0).unwrap() * point.extend(1.0);
        clip.truncate() / clip.w
    }

    #[test]
    fn reverse_z_maps_near_to_one_and_distance_to_zero() {
        let camera = Camera3D::default();
        assert!((ndc(camera, Vec3::new(0.0, 0.0, -camera.near)).z - 1.0).abs() < 1e-5);
        assert!(ndc(camera, Vec3::new(0.0, 0.0, -1.0e6)).z < 1e-5);
    }

    #[test]
    fn sphere_culling_rejects_objects_behind_camera() {
        let camera = Camera3D::default();
        assert!(camera.sphere_visible(Vec3::new(0.0, 0.0, -5.0), 1.0, 1.0));
        assert!(!camera.sphere_visible(Vec3::new(0.0, 0.0, 5.0), 1.0, 1.0));
    }
}
