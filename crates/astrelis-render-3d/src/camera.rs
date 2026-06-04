//! 3D perspective camera.

use astrelis_core::math::{Mat3, Mat4, Quat, Vec3};

/// A perspective 3D camera.
///
/// Convention: right-handed, +Y up; identity rotation looks down −Z.
/// The projection is reverse-Z with an infinite far plane: the near
/// plane maps to depth 1 and infinity to depth 0, which gives
/// near-uniform float precision over the whole range. Pair with a
/// `GreaterEqual` depth compare and a clear value of 0.0.
pub struct Camera3D {
    /// Camera position in world space.
    pub position: Vec3,
    /// Camera orientation. Identity looks down −Z with +Y up.
    pub rotation: Quat,
    /// Vertical field of view in radians.
    pub fov_y: f32,
    /// Viewport aspect ratio (width / height). Update on resize.
    ///
    /// Deliberately a bare ratio, not a viewport size: the projection
    /// only needs the ratio, and a size would re-import the
    /// physical/logical-pixel ambiguity.
    pub aspect: f32,
    /// Near plane distance. There is no far plane (infinite reverse-Z).
    pub near: f32,
}

impl Camera3D {
    /// Creates a camera at the origin looking down −Z.
    ///
    /// Defaults: 60° vertical FOV, near plane at 0.1.
    pub fn new(aspect: f32) -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            fov_y: 60f32.to_radians(),
            aspect,
            near: 0.1,
        }
    }

    /// Rotates the camera in place to look at `target`.
    pub fn look_at(&mut self, target: Vec3, up: Vec3) {
        let view = Mat4::look_at_rh(self.position, target, up);
        // The view matrix is the inverse of the camera's world
        // transform; its inverse's rotation part is the camera pose.
        self.rotation = Quat::from_mat3(&Mat3::from_mat4(view.inverse())).normalize();
    }

    /// Computes the combined view-projection matrix.
    pub fn view_projection(&self) -> Mat4 {
        let view = Mat4::from_rotation_translation(self.rotation, self.position).inverse();
        let proj = Mat4::perspective_infinite_reverse_rh(self.fov_y, self.aspect, self.near);
        proj * view
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Projects a world point to NDC (perspective divide included).
    fn ndc(camera: &Camera3D, p: Vec3) -> Vec3 {
        let clip = camera.view_projection() * p.extend(1.0);
        Vec3::new(clip.x / clip.w, clip.y / clip.w, clip.z / clip.w)
    }

    #[test]
    fn defaults_are_sane() {
        let cam = Camera3D::new(16.0 / 9.0);
        assert_eq!(cam.position, Vec3::ZERO);
        assert_eq!(cam.rotation, Quat::IDENTITY);
        assert!((cam.fov_y - 60f32.to_radians()).abs() < 1e-6);
        assert!((cam.near - 0.1).abs() < 1e-6);
    }

    #[test]
    fn point_straight_ahead_maps_to_ndc_center() {
        // Identity rotation looks down −Z (right-handed, +Y up).
        let cam = Camera3D::new(1.0);
        let n = ndc(&cam, Vec3::new(0.0, 0.0, -10.0));
        assert!(n.x.abs() < 1e-5 && n.y.abs() < 1e-5, "got {n:?}");
        assert!(n.z > 0.0 && n.z < 1.0, "depth must be inside (0,1), got {}", n.z);
    }

    #[test]
    fn reverse_z_near_is_one_far_is_zero() {
        let cam = Camera3D::new(1.0);
        let near = ndc(&cam, Vec3::new(0.0, 0.0, -cam.near));
        let far = ndc(&cam, Vec3::new(0.0, 0.0, -1.0e6));
        assert!((near.z - 1.0).abs() < 1e-4, "near depth ≈ 1, got {}", near.z);
        assert!(far.z < 1e-4, "distant depth → 0, got {}", far.z);
    }

    #[test]
    fn aspect_scales_x_only() {
        let narrow = Camera3D::new(1.0);
        let wide = Camera3D::new(2.0);
        let p = Vec3::new(1.0, 1.0, -10.0);
        let n1 = ndc(&narrow, p);
        let n2 = ndc(&wide, p);
        assert!((n2.x - n1.x / 2.0).abs() < 1e-5, "x halves when aspect doubles");
        assert!((n2.y - n1.y).abs() < 1e-6, "y unchanged by aspect");
    }

    #[test]
    fn off_axis_point_keeps_ndc_signs() {
        // A y-axis sign flip in the projection (the Camera2D bug
        // class) is invisible to on-axis tests: world-up must land at
        // positive NDC y, world-right at positive NDC x.
        let cam = Camera3D::new(1.0);
        let n = ndc(&cam, Vec3::new(2.0, 3.0, -10.0));
        assert!(n.x > 0.0, "world +X is NDC right, got {n:?}");
        assert!(n.y > 0.0, "world +Y is NDC up, got {n:?}");
    }

    #[test]
    fn look_at_points_forward_axis_at_target() {
        let mut cam = Camera3D::new(1.0);
        cam.position = Vec3::new(5.0, 0.0, 0.0);
        cam.look_at(Vec3::ZERO, Vec3::Y);
        // Camera forward is rotation * −Z; it must point toward −X.
        let forward = cam.rotation * Vec3::NEG_Z;
        assert!((forward - Vec3::NEG_X).length() < 1e-5, "got forward {forward:?}");
        // And the target must project to NDC center.
        let n = ndc(&cam, Vec3::ZERO);
        assert!(n.x.abs() < 1e-5 && n.y.abs() < 1e-5, "got {n:?}");
    }
}
