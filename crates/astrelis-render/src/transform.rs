//! Shared data-to-screen coordinate transformation for instanced renderers.
//!
//! This module provides [`DataTransform`] used by
//! [`LineRenderer`](crate::LineRenderer), [`PointRenderer`](crate::PointRenderer),
//! and [`QuadRenderer`](crate::QuadRenderer) to map data coordinates to screen
//! pixels on the GPU.
//!
//! # How it works
//!
//! Data points are stored in their original coordinate space. The GPU applies:
//! ```text
//! screen_pos = data_pos * scale + offset
//! clip_pos   = projection * screen_pos
//! ```
//!
//! This means pan/zoom only updates a small uniform buffer (32 bytes), not
//! all the vertex/instance data. For charts with thousands of data points,
//! this is the key to smooth interaction.

use crate::Viewport;
use bytemuck::{Pod, Zeroable};

/// Parameters describing a data range and its target plot area.
///
/// Used to construct a [`DataTransform`] that maps data coordinates
/// to screen coordinates within the plot area.
#[derive(Debug, Clone, Copy)]
pub struct DataRangeParams {
    /// Plot area X offset in screen pixels (from left edge of viewport).
    pub plot_x: f32,
    /// Plot area Y offset in screen pixels (from top edge of viewport).
    pub plot_y: f32,
    /// Plot area width in screen pixels.
    pub plot_width: f32,
    /// Plot area height in screen pixels.
    pub plot_height: f32,
    /// Minimum data X value.
    pub data_x_min: f64,
    /// Maximum data X value.
    pub data_x_max: f64,
    /// Minimum data Y value.
    pub data_y_min: f64,
    /// Maximum data Y value.
    pub data_y_max: f64,
}

impl DataRangeParams {
    /// Create new data range parameters.
    pub fn new(
        plot_x: f32,
        plot_y: f32,
        plot_width: f32,
        plot_height: f32,
        data_x_min: f64,
        data_x_max: f64,
        data_y_min: f64,
        data_y_max: f64,
    ) -> Self {
        Self {
            plot_x,
            plot_y,
            plot_width,
            plot_height,
            data_x_min,
            data_x_max,
            data_y_min,
            data_y_max,
        }
    }
}

/// High-level data-to-screen transform.
///
/// Combines a viewport (for the projection matrix) with an optional data range
/// mapping. When no data range is set, data coordinates equal screen coordinates
/// (identity transform).
///
/// # Example
///
/// ```ignore
/// // Identity transform: data coords = screen pixels
/// let transform = DataTransform::identity(viewport);
///
/// // Data range transform: maps data [0..100, 0..50] to a 400x300 plot area
/// let transform = DataTransform::from_data_range(viewport, DataRangeParams {
///     plot_x: 80.0, plot_y: 20.0,
///     plot_width: 400.0, plot_height: 300.0,
///     data_x_min: 0.0, data_x_max: 100.0,
///     data_y_min: 0.0, data_y_max: 50.0,
/// });
///
/// renderer.render_transformed(pass, &transform);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct DataTransform {
    uniform: TransformUniform,
}

impl DataTransform {
    /// Create an identity transform (data coordinates = screen coordinates).
    pub fn identity(viewport: Viewport) -> Self {
        let logical = viewport.to_logical();
        Self {
            uniform: TransformUniform::identity(logical.width, logical.height),
        }
    }

    /// Create a transform that maps data coordinates to a plot area on screen.
    ///
    /// Data point `(data_x, data_y)` maps to screen position:
    /// - `screen_x = plot_x + (data_x - data_x_min) / (data_x_max - data_x_min) * plot_width`
    /// - `screen_y = plot_y + plot_height - (data_y - data_y_min) / (data_y_max - data_y_min) * plot_height`
    ///
    /// Y is flipped because screen Y goes downward but data Y typically goes upward.
    pub fn from_data_range(viewport: Viewport, params: DataRangeParams) -> Self {
        let logical = viewport.to_logical();
        Self {
            uniform: TransformUniform::for_data_range(
                logical.width,
                logical.height,
                params.plot_x,
                params.plot_y,
                params.plot_width,
                params.plot_height,
                params.data_x_min as f32,
                params.data_x_max as f32,
                params.data_y_min as f32,
                params.data_y_max as f32,
            ),
        }
    }

    /// Get the GPU-ready uniform data.
    pub(crate) fn uniform(&self) -> &TransformUniform {
        &self.uniform
    }
}

/// GPU uniform buffer for data-to-screen coordinate transformation.
///
/// Contains an orthographic projection matrix and a scale+offset transform
/// for mapping data coordinates to screen pixels.
///
/// Layout (80 bytes, 16-byte aligned):
/// ```text
/// offset 0:  mat4x4<f32> projection  (64 bytes)
/// offset 64: vec2<f32>   scale        (8 bytes)
/// offset 72: vec2<f32>   offset       (8 bytes)
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable, PartialEq)]
pub(crate) struct TransformUniform {
    /// Orthographic projection matrix.
    pub(crate) projection: [[f32; 4]; 4],
    /// Scale: `screen_pos = data_pos * scale + offset`.
    pub(crate) scale: [f32; 2],
    /// Offset: `screen_pos = data_pos * scale + offset`.
    pub(crate) offset: [f32; 2],
}

impl TransformUniform {
    /// Identity data transform (data coords = screen coords).
    pub(crate) fn identity(viewport_width: f32, viewport_height: f32) -> Self {
        Self {
            projection: Self::ortho_matrix(viewport_width, viewport_height),
            scale: [1.0, 1.0],
            offset: [0.0, 0.0],
        }
    }

    /// Create transform for mapping data coordinates to a plot area.
    ///
    /// Data point (data_x, data_y) maps to screen position:
    /// - screen_x = plot_x + (data_x - data_x_min) / (data_x_max - data_x_min) * plot_width
    /// - screen_y = plot_y + plot_height - (data_y - data_y_min) / (data_y_max - data_y_min) * plot_height
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn for_data_range(
        viewport_width: f32,
        viewport_height: f32,
        plot_x: f32,
        plot_y: f32,
        plot_width: f32,
        plot_height: f32,
        data_x_min: f32,
        data_x_max: f32,
        data_y_min: f32,
        data_y_max: f32,
    ) -> Self {
        // screen = data * scale + offset
        let scale_x = plot_width / (data_x_max - data_x_min);
        let scale_y = -plot_height / (data_y_max - data_y_min); // Negative for Y flip

        let offset_x = plot_x - data_x_min * scale_x;
        let offset_y = plot_y + plot_height - data_y_min * scale_y;

        Self {
            projection: Self::ortho_matrix(viewport_width, viewport_height),
            scale: [scale_x, scale_y],
            offset: [offset_x, offset_y],
        }
    }

    /// Create an orthographic projection matrix for the given viewport size.
    ///
    /// Maps (0,0) to top-left, (width, height) to bottom-right.
    pub(crate) fn ortho_matrix(width: f32, height: f32) -> [[f32; 4]; 4] {
        [
            [2.0 / width, 0.0, 0.0, 0.0],
            [0.0, -2.0 / height, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0, 1.0],
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use astrelis_core::geometry::{PhysicalPosition, PhysicalSize, ScaleFactor};

    fn test_viewport() -> Viewport {
        Viewport {
            position: PhysicalPosition::new(0.0, 0.0),
            size: PhysicalSize::new(800.0, 600.0),
            scale_factor: ScaleFactor(1.0),
        }
    }

    #[test]
    fn test_identity_transform() {
        let transform = DataTransform::identity(test_viewport());
        let u = transform.uniform();
        assert_eq!(u.scale, [1.0, 1.0]);
        assert_eq!(u.offset, [0.0, 0.0]);
    }

    #[test]
    fn test_data_range_transform() {
        let params = DataRangeParams::new(
            100.0, 50.0, // plot origin
            600.0, 400.0, // plot size
            0.0, 10.0, // data x range
            0.0, 100.0, // data y range
        );
        let transform = DataTransform::from_data_range(test_viewport(), params);
        let u = transform.uniform();

        // scale_x = 600 / (10 - 0) = 60
        assert!((u.scale[0] - 60.0).abs() < 0.001);
        // scale_y = -400 / (100 - 0) = -4
        assert!((u.scale[1] - (-4.0)).abs() < 0.001);
    }

    #[test]
    fn test_ortho_matrix_dimensions() {
        let matrix = TransformUniform::ortho_matrix(800.0, 600.0);
        // Check that the matrix has the right scale factors
        assert!((matrix[0][0] - 2.0 / 800.0).abs() < 0.0001);
        assert!((matrix[1][1] - (-2.0 / 600.0)).abs() < 0.0001);
        assert!((matrix[2][2] - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_transform_uniform_size() {
        // Ensure the uniform matches the expected GPU layout (80 bytes)
        assert_eq!(std::mem::size_of::<TransformUniform>(), 80);
    }
}
