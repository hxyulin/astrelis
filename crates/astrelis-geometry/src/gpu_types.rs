//! GPU data types for geometry rendering.
//!
//! Defines instance data structures for GPU-accelerated rendering.

use astrelis_render::wgpu;
use bytemuck::{Pod, Zeroable};

/// Instance data for filled geometry.
///
/// Each instance represents one filled shape.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct FillInstance {
    /// Transform matrix (2x3 stored as 2x4 for alignment)
    /// [a, b, c, d, tx, ty, 0, 0]
    pub transform: [[f32; 4]; 2],
    /// Fill color (RGBA)
    pub color: [f32; 4],
}

impl FillInstance {
    /// Create a fill instance with position offset and color.
    pub fn new(offset_x: f32, offset_y: f32, color: [f32; 4]) -> Self {
        Self {
            transform: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, offset_x, offset_y]],
            color,
        }
    }

    /// Create a fill instance with full transform.
    pub fn with_transform(transform: [[f32; 4]; 2], color: [f32; 4]) -> Self {
        Self { transform, color }
    }

    /// Get the WGPU vertex buffer layout.
    pub fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // transform row 0
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // transform row 1
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // color
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Instance data for stroked geometry.
///
/// Each instance represents one stroked path.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct StrokeInstance {
    /// Transform matrix (2x3 stored as 2x4 for alignment)
    pub transform: [[f32; 4]; 2],
    /// Stroke color (RGBA)
    pub color: [f32; 4],
    /// Stroke width
    pub width: f32,
    /// Padding for alignment
    pub _padding: [f32; 3],
}

impl StrokeInstance {
    /// Create a stroke instance.
    pub fn new(offset_x: f32, offset_y: f32, color: [f32; 4], width: f32) -> Self {
        Self {
            transform: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, offset_x, offset_y]],
            color,
            width,
            _padding: [0.0; 3],
        }
    }

    /// Get the WGPU vertex buffer layout.
    ///
    /// Note: StrokeVertex uses locations 0-3, so instance attributes start at 4.
    pub fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // transform row 0
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // transform row 1
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // color
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // width
                wgpu::VertexAttribute {
                    offset: 48,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        }
    }
}

/// Uniform data for projection matrix.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct ProjectionUniform {
    /// 4x4 projection matrix
    pub matrix: [[f32; 4]; 4],
}

impl ProjectionUniform {
    /// Create an orthographic projection matrix for 2D rendering.
    pub fn orthographic(width: f32, height: f32) -> Self {
        Self {
            matrix: [
                [2.0 / width, 0.0, 0.0, 0.0],
                [0.0, -2.0 / height, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [-1.0, 1.0, 0.0, 1.0],
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fill_instance_size() {
        // Should be 48 bytes (2x16 + 16)
        assert_eq!(std::mem::size_of::<FillInstance>(), 48);
    }

    #[test]
    fn test_stroke_instance_size() {
        // Should be 64 bytes (48 + 16)
        assert_eq!(std::mem::size_of::<StrokeInstance>(), 64);
    }

    #[test]
    fn test_projection_size() {
        // Should be 64 bytes (4x4 floats)
        assert_eq!(std::mem::size_of::<ProjectionUniform>(), 64);
    }
}
