//! 2D orthographic camera.

use astrelis_core::geometry::{Physical, Size};
use astrelis_core::math::Vec2;

/// An orthographic 2D camera.
///
/// Produces a view-projection matrix that transforms world-space
/// coordinates into clip space for the vertex shader.
pub struct Camera2D {
    /// Camera center position in world space.
    pub position: Vec2,
    /// Zoom factor (1.0 = default, >1 = zoomed in).
    pub zoom: f32,
    /// Rotation in radians.
    pub rotation: f32,
    /// Viewport size in physical pixels (set from window size).
    pub viewport: Size<Physical>,
}

impl Camera2D {
    /// Creates a new camera centered at the origin.
    pub fn new(viewport_width: u32, viewport_height: u32) -> Self {
        Self {
            position: Vec2::ZERO,
            zoom: 1.0,
            rotation: 0.0,
            viewport: Size::<Physical>::new(viewport_width as f32, viewport_height as f32),
        }
    }

    /// Computes the combined view-projection matrix.
    ///
    /// The projection is orthographic with (0,0) at the top-left when
    /// the camera is at the origin with no rotation. Y increases downward,
    /// matching screen coordinates.
    ///
    /// Convention: `position` is the *top-left* of the visible world
    /// rect; with the camera at the origin, `[0, vw] x [0, vh]` is
    /// visible (divided by `zoom`, which shrinks the extent around the
    /// view center). Rotation also pivots on the view center.
    pub fn view_projection(&self) -> [[f32; 4]; 4] {
        let vw = self.viewport.width;
        let vh = self.viewport.height;

        if vw == 0.0 || vh == 0.0 {
            return [[0.0; 4]; 4];
        }

        let half_w = vw / (2.0 * self.zoom);
        let half_h = vh / (2.0 * self.zoom);

        // View center: zoom and rotation pivot here. At zoom 1 the
        // center is position + half the viewport.
        let cx = self.position.x + vw / 2.0;
        let cy = self.position.y + vh / 2.0;

        // NDC = S * R * (p - center): translate the view center to the
        // origin, rotate, then scale to [-1, 1] with y flipped (world
        // is y-down, NDC is y-up).
        let sx = 1.0 / half_w;
        let sy = -1.0 / half_h;
        let (sin, cos) = self.rotation.sin_cos();

        [
            [sx * cos, sy * sin, 0.0, 0.0],
            [sx * -sin, sy * cos, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [
                sx * (-cx * cos + cy * sin),
                sy * (-cx * sin - cy * cos),
                0.0,
                1.0,
            ],
        ]
    }
}

impl Default for Camera2D {
    fn default() -> Self {
        Self::new(1280, 720)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Applies the column-major view-projection to a world point.
    fn project(camera: &Camera2D, x: f32, y: f32) -> (f32, f32) {
        let m = camera.view_projection();
        (
            m[0][0] * x + m[1][0] * y + m[3][0],
            m[0][1] * x + m[1][1] * y + m[3][1],
        )
    }

    fn assert_ndc(actual: (f32, f32), expected: (f32, f32)) {
        assert!(
            (actual.0 - expected.0).abs() < 1e-5 && (actual.1 - expected.1).abs() < 1e-5,
            "got NDC {actual:?}, expected {expected:?}"
        );
    }

    #[test]
    fn origin_camera_maps_top_left_world_to_top_left_ndc() {
        // Documented contract: (0,0) is the top-left when the camera
        // is at the origin. NDC top-left is (-1, +1) (wgpu, y-up NDC).
        let camera = Camera2D::new(1280, 720);
        assert_ndc(project(&camera, 0.0, 0.0), (-1.0, 1.0));
        assert_ndc(project(&camera, 1280.0, 720.0), (1.0, -1.0));
        assert_ndc(project(&camera, 640.0, 360.0), (0.0, 0.0));
    }

    #[test]
    fn camera_position_pans_the_view() {
        // Moving the camera to (100, 50) means world (100, 50) is now
        // the top-left of the screen.
        let mut camera = Camera2D::new(1280, 720);
        camera.position = Vec2::new(100.0, 50.0);
        assert_ndc(project(&camera, 100.0, 50.0), (-1.0, 1.0));
        assert_ndc(project(&camera, 100.0 + 640.0, 50.0 + 360.0), (0.0, 0.0));
    }

    #[test]
    fn zoom_shrinks_visible_world_around_view_center() {
        // zoom = 2 halves the visible extent; the view center stays put.
        let mut camera = Camera2D::new(1280, 720);
        camera.zoom = 2.0;
        // Visible world: [320, 960] x [180, 540]; center unchanged.
        assert_ndc(project(&camera, 640.0, 360.0), (0.0, 0.0));
        assert_ndc(project(&camera, 320.0, 180.0), (-1.0, 1.0));
        assert_ndc(project(&camera, 960.0, 540.0), (1.0, -1.0));
    }

    #[test]
    fn rotation_pivots_on_view_center() {
        // Rotating the camera must keep the view-center fixed and move
        // a point right-of-center to below-center (90° CW camera turn
        // in y-down screen space).
        let mut camera = Camera2D::new(1280, 720);
        camera.rotation = std::f32::consts::FRAC_PI_2;
        assert_ndc(project(&camera, 640.0, 360.0), (0.0, 0.0));
        // World point 360 to the right of center lands a full half-height
        // below center: NDC (0, -1) given the y-down flip.
        assert_ndc(project(&camera, 1000.0, 360.0), (0.0, -1.0));
    }
}
