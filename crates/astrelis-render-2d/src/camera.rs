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
    pub fn view_projection(&self) -> [[f32; 4]; 4] {
        let vw = self.viewport.width;
        let vh = self.viewport.height;

        if vw == 0.0 || vh == 0.0 {
            return [[0.0; 4]; 4];
        }

        // Orthographic projection: maps [0, width] x [0, height] → [-1, 1]
        // with y-down (top-left origin).
        let half_w = vw / (2.0 * self.zoom);
        let half_h = vh / (2.0 * self.zoom);

        let left = self.position.x - half_w;
        let right = self.position.x + half_w;
        let top = self.position.y - half_h;
        let bottom = self.position.y + half_h;

        // Rotation around camera center.
        let (sin, cos) = self.rotation.sin_cos();

        // View matrix: translate to camera center, rotate, then project.
        // Combined into a single 4x4 for efficiency.
        let sx = 2.0 / (right - left);
        let sy = 2.0 / (top - bottom); // flipped: top < bottom for y-down
        let tx = -(right + left) / (right - left);
        let ty = -(top + bottom) / (top - bottom);

        // Apply rotation: P * R where R rotates around screen center.
        [
            [sx * cos, sy * sin, 0.0, 0.0],
            [sx * -sin, sy * cos, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [sx * (tx * cos - ty * sin), sy * (tx * sin + ty * cos), 0.0, 1.0],
        ]
    }
}

impl Default for Camera2D {
    fn default() -> Self {
        Self::new(1280, 720)
    }
}
