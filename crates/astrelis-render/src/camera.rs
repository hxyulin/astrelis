//! Camera system for view-projection matrix management.
//!
//! Provides both orthographic and perspective cameras with coordinate conversion utilities.
//!
//! # Example
//!
//! ```ignore
//! use astrelis_render::*;
//! use glam::Vec3;
//!
//! // Create an orthographic camera for 2D
//! let mut camera = Camera::orthographic(800.0, 600.0, 0.1, 100.0);
//! camera.look_at(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0), Vec3::Y);
//!
//! // Get matrices
//! let view_projection = camera.view_projection_matrix();
//!
//! // Create a perspective camera for 3D
//! let mut camera = Camera::perspective(60.0, 16.0 / 9.0, 0.1, 100.0);
//! camera.look_at(
//!     Vec3::new(0.0, 5.0, 10.0),
//!     Vec3::ZERO,
//!     Vec3::Y
//! );
//!
//! // Convert screen to world coordinates
//! let world_pos = camera.screen_to_world(Vec2::new(400.0, 300.0), Vec2::new(800.0, 600.0));
//! ```

use glam::{Mat4, Vec2, Vec3, Vec4};

/// Projection mode for a camera.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProjectionMode {
    /// Orthographic projection (typically for 2D).
    Orthographic {
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        near: f32,
        far: f32,
    },
    /// Perspective projection (typically for 3D).
    Perspective {
        fov_y_radians: f32,
        aspect_ratio: f32,
        near: f32,
        far: f32,
    },
}

/// A camera with view and projection matrices.
pub struct Camera {
    /// Camera position in world space
    position: Vec3,
    /// Target position to look at
    target: Vec3,
    /// Up vector (typically Vec3::Y)
    up: Vec3,
    /// Projection mode
    projection: ProjectionMode,
    /// Cached view matrix
    view_matrix: Mat4,
    /// Cached projection matrix
    projection_matrix: Mat4,
    /// Cached view-projection matrix
    view_projection_matrix: Mat4,
    /// Dirty flag - set to true when position/target/up changes
    dirty: bool,
}

impl Camera {
    /// Create an orthographic camera.
    ///
    /// # Arguments
    ///
    /// * `width` - Viewport width
    /// * `height` - Viewport height
    /// * `near` - Near clip plane
    /// * `far` - Far clip plane
    pub fn orthographic(width: f32, height: f32, near: f32, far: f32) -> Self {
        let half_width = width / 2.0;
        let half_height = height / 2.0;

        let projection = ProjectionMode::Orthographic {
            left: -half_width,
            right: half_width,
            bottom: -half_height,
            top: half_height,
            near,
            far,
        };

        let mut camera = Self {
            position: Vec3::new(0.0, 0.0, 1.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            projection,
            view_matrix: Mat4::IDENTITY,
            projection_matrix: Mat4::IDENTITY,
            view_projection_matrix: Mat4::IDENTITY,
            dirty: true,
        };

        camera.update_matrices();
        camera
    }

    /// Create an orthographic camera with custom bounds.
    pub fn orthographic_custom(
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        near: f32,
        far: f32,
    ) -> Self {
        let projection = ProjectionMode::Orthographic {
            left,
            right,
            bottom,
            top,
            near,
            far,
        };

        let mut camera = Self {
            position: Vec3::new(0.0, 0.0, 1.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            projection,
            view_matrix: Mat4::IDENTITY,
            projection_matrix: Mat4::IDENTITY,
            view_projection_matrix: Mat4::IDENTITY,
            dirty: true,
        };

        camera.update_matrices();
        camera
    }

    /// Create a perspective camera.
    ///
    /// # Arguments
    ///
    /// * `fov_y_degrees` - Vertical field of view in degrees
    /// * `aspect_ratio` - Aspect ratio (width / height)
    /// * `near` - Near clip plane
    /// * `far` - Far clip plane
    pub fn perspective(fov_y_degrees: f32, aspect_ratio: f32, near: f32, far: f32) -> Self {
        let projection = ProjectionMode::Perspective {
            fov_y_radians: fov_y_degrees.to_radians(),
            aspect_ratio,
            near,
            far,
        };

        let mut camera = Self {
            position: Vec3::new(0.0, 5.0, 10.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            projection,
            view_matrix: Mat4::IDENTITY,
            projection_matrix: Mat4::IDENTITY,
            view_projection_matrix: Mat4::IDENTITY,
            dirty: true,
        };

        camera.update_matrices();
        camera
    }

    /// Set the camera to look at a target from a position.
    pub fn look_at(&mut self, eye: Vec3, target: Vec3, up: Vec3) {
        self.position = eye;
        self.target = target;
        self.up = up;
        self.dirty = true;
    }

    /// Set the camera position.
    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
        self.dirty = true;
    }

    /// Get the camera position.
    pub fn position(&self) -> Vec3 {
        self.position
    }

    /// Set the camera target.
    pub fn set_target(&mut self, target: Vec3) {
        self.target = target;
        self.dirty = true;
    }

    /// Get the camera target.
    pub fn target(&self) -> Vec3 {
        self.target
    }

    /// Set the up vector.
    pub fn set_up(&mut self, up: Vec3) {
        self.up = up;
        self.dirty = true;
    }

    /// Get the up vector.
    pub fn up(&self) -> Vec3 {
        self.up
    }

    /// Get the forward direction (normalized vector from position to target).
    pub fn forward(&self) -> Vec3 {
        (self.target - self.position).normalize()
    }

    /// Get the right direction.
    pub fn right(&self) -> Vec3 {
        self.forward().cross(self.up).normalize()
    }

    /// Update the projection mode.
    pub fn set_projection(&mut self, projection: ProjectionMode) {
        self.projection = projection;
        self.dirty = true;
    }

    /// Get the projection mode.
    pub fn projection(&self) -> ProjectionMode {
        self.projection
    }

    /// Update the aspect ratio (only affects perspective cameras).
    pub fn set_aspect_ratio(&mut self, aspect_ratio: f32) {
        if let ProjectionMode::Perspective {
            fov_y_radians,
            near,
            far,
            ..
        } = self.projection
        {
            self.projection = ProjectionMode::Perspective {
                fov_y_radians,
                aspect_ratio,
                near,
                far,
            };
            self.dirty = true;
        }
    }

    /// Get the view matrix.
    pub fn view_matrix(&mut self) -> Mat4 {
        if self.dirty {
            self.update_matrices();
        }
        self.view_matrix
    }

    /// Get the projection matrix.
    pub fn projection_matrix(&mut self) -> Mat4 {
        if self.dirty {
            self.update_matrices();
        }
        self.projection_matrix
    }

    /// Get the combined view-projection matrix.
    pub fn view_projection_matrix(&mut self) -> Mat4 {
        if self.dirty {
            self.update_matrices();
        }
        self.view_projection_matrix
    }

    /// Convert screen coordinates to world coordinates.
    ///
    /// # Arguments
    ///
    /// * `screen_pos` - Screen position (pixels)
    /// * `viewport_size` - Viewport size (pixels)
    /// * `depth` - Depth in NDC space (-1.0 to 1.0, where -1.0 is near plane)
    ///
    /// # Returns
    ///
    /// World position as Vec3
    pub fn screen_to_world(&mut self, screen_pos: Vec2, viewport_size: Vec2, depth: f32) -> Vec3 {
        // Convert screen to NDC
        let ndc_x = (screen_pos.x / viewport_size.x) * 2.0 - 1.0;
        let ndc_y = 1.0 - (screen_pos.y / viewport_size.y) * 2.0; // Flip Y
        let ndc = Vec4::new(ndc_x, ndc_y, depth, 1.0);

        // Transform to world space
        let view_proj = self.view_projection_matrix();
        let inv_view_proj = view_proj.inverse();
        let world = inv_view_proj * ndc;

        // Perspective divide
        Vec3::new(world.x / world.w, world.y / world.w, world.z / world.w)
    }

    /// Convert world coordinates to screen coordinates.
    ///
    /// # Arguments
    ///
    /// * `world_pos` - World position
    /// * `viewport_size` - Viewport size (pixels)
    ///
    /// # Returns
    ///
    /// Screen position as Vec2 (pixels) and depth
    pub fn world_to_screen(&mut self, world_pos: Vec3, viewport_size: Vec2) -> (Vec2, f32) {
        let view_proj = self.view_projection_matrix();
        let clip = view_proj * Vec4::new(world_pos.x, world_pos.y, world_pos.z, 1.0);

        // Perspective divide
        let ndc = Vec3::new(clip.x / clip.w, clip.y / clip.w, clip.z / clip.w);

        // Convert NDC to screen
        let screen_x = (ndc.x + 1.0) * 0.5 * viewport_size.x;
        let screen_y = (1.0 - ndc.y) * 0.5 * viewport_size.y; // Flip Y

        (Vec2::new(screen_x, screen_y), ndc.z)
    }

    /// Update all matrices.
    fn update_matrices(&mut self) {
        // Update view matrix
        self.view_matrix = Mat4::look_at_rh(self.position, self.target, self.up);

        // Update projection matrix
        self.projection_matrix = match self.projection {
            ProjectionMode::Orthographic {
                left,
                right,
                bottom,
                top,
                near,
                far,
            } => Mat4::orthographic_rh(left, right, bottom, top, near, far),
            ProjectionMode::Perspective {
                fov_y_radians,
                aspect_ratio,
                near,
                far,
            } => Mat4::perspective_rh(fov_y_radians, aspect_ratio, near, far),
        };

        // Update combined matrix
        self.view_projection_matrix = self.projection_matrix * self.view_matrix;

        self.dirty = false;
    }
}

/// Camera uniform buffer data for shaders.
///
/// This is a standard layout that can be used in shaders:
///
/// ```wgsl
/// struct CameraUniform {
///     view_proj: mat4x4<f32>,
///     view: mat4x4<f32>,
///     projection: mat4x4<f32>,
///     position: vec3<f32>,
/// }
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    /// View-projection matrix
    pub view_proj: [[f32; 4]; 4],
    /// View matrix
    pub view: [[f32; 4]; 4],
    /// Projection matrix
    pub projection: [[f32; 4]; 4],
    /// Camera position in world space
    pub position: [f32; 3],
    /// Padding for alignment
    pub _padding: f32,
}

impl CameraUniform {
    /// Create camera uniform data from a camera.
    pub fn from_camera(camera: &mut Camera) -> Self {
        Self {
            view_proj: camera.view_projection_matrix().to_cols_array_2d(),
            view: camera.view_matrix().to_cols_array_2d(),
            projection: camera.projection_matrix().to_cols_array_2d(),
            position: camera.position().to_array(),
            _padding: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orthographic_camera() {
        let mut camera = Camera::orthographic(800.0, 600.0, 0.1, 100.0);
        camera.look_at(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0), Vec3::Y);

        let view_proj = camera.view_projection_matrix();
        assert!(!view_proj.is_nan());
    }

    #[test]
    fn test_perspective_camera() {
        let mut camera = Camera::perspective(60.0, 16.0 / 9.0, 0.1, 100.0);
        camera.look_at(Vec3::new(0.0, 5.0, 10.0), Vec3::ZERO, Vec3::Y);

        let view_proj = camera.view_projection_matrix();
        assert!(!view_proj.is_nan());
    }

    #[test]
    fn test_screen_to_world() {
        let mut camera = Camera::orthographic(800.0, 600.0, 0.1, 100.0);
        camera.look_at(Vec3::new(0.0, 0.0, 1.0), Vec3::ZERO, Vec3::Y);

        let world_pos = camera.screen_to_world(Vec2::new(400.0, 300.0), Vec2::new(800.0, 600.0), 0.0);

        // Center of screen should map to roughly (0, 0) in world space
        assert!((world_pos.x.abs()) < 0.1);
        assert!((world_pos.y.abs()) < 0.1);
    }

    #[test]
    fn test_world_to_screen() {
        let mut camera = Camera::orthographic(800.0, 600.0, 0.1, 100.0);
        camera.look_at(Vec3::new(0.0, 0.0, 1.0), Vec3::ZERO, Vec3::Y);

        let (screen_pos, _depth) = camera.world_to_screen(Vec3::ZERO, Vec2::new(800.0, 600.0));

        // World origin should map to center of screen
        assert!((screen_pos.x - 400.0).abs() < 1.0);
        assert!((screen_pos.y - 300.0).abs() < 1.0);
    }
}
