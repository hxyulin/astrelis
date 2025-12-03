//! GPU data types for instance-based rendering.
//!
//! This module defines the vertex and instance data structures used for
//! efficient GPU rendering of UI elements. All types are Pod-compatible
//! for direct GPU upload.

use astrelis_core::math::Vec2;
use astrelis_render::{Color, wgpu};
use bytemuck::{Pod, Zeroable};

/// Instance data for quad rendering.
///
/// Used for drawing rectangles, rounded rectangles, and borders.
/// Each instance represents one quad that will be drawn using
/// instanced rendering with a shared vertex buffer.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct QuadInstance {
    /// Position in screen space (top-left corner)
    pub position: [f32; 2],
    /// Size of the quad (width, height)
    pub size: [f32; 2],
    /// Color (RGBA)
    pub color: [f32; 4],
    /// Border radius for rounded corners (0 = sharp corners)
    pub border_radius: f32,
    /// Border thickness (0 = filled quad, >0 = border outline)
    pub border_thickness: f32,
    /// Padding to align to 16-byte boundary for optimal GPU performance
    pub _padding: [f32; 2],
}

impl QuadInstance {
    /// Create a filled quad instance.
    pub fn filled(position: Vec2, size: Vec2, color: Color) -> Self {
        Self {
            position: position.into(),
            size: size.into(),
            color: color.into(),
            border_radius: 0.0,
            border_thickness: 0.0,
            _padding: [0.0; 2],
        }
    }

    /// Create a rounded filled quad instance.
    pub fn rounded(position: Vec2, size: Vec2, color: Color, border_radius: f32) -> Self {
        Self {
            position: position.into(),
            size: size.into(),
            color: color.into(),
            border_radius,
            border_thickness: 0.0,
            _padding: [0.0; 2],
        }
    }

    /// Create a bordered quad instance.
    pub fn bordered(
        position: Vec2,
        size: Vec2,
        color: Color,
        border_thickness: f32,
        border_radius: f32,
    ) -> Self {
        Self {
            position: position.into(),
            size: size.into(),
            color: color.into(),
            border_radius,
            border_thickness,
            _padding: [0.0; 2],
        }
    }

    /// Get the WGPU vertex buffer layout for quad instances.
    pub fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
        use wgpu::*;
        VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: VertexStepMode::Instance,
            attributes: &[
                // position
                VertexAttribute {
                    offset: 0,
                    shader_location: 2,
                    format: VertexFormat::Float32x2,
                },
                // size
                VertexAttribute {
                    offset: 8,
                    shader_location: 3,
                    format: VertexFormat::Float32x2,
                },
                // color
                VertexAttribute {
                    offset: 16,
                    shader_location: 4,
                    format: VertexFormat::Float32x4,
                },
                // border_radius
                VertexAttribute {
                    offset: 32,
                    shader_location: 5,
                    format: VertexFormat::Float32,
                },
                // border_thickness
                VertexAttribute {
                    offset: 36,
                    shader_location: 6,
                    format: VertexFormat::Float32,
                },
            ],
        }
    }
}

/// Instance data for text glyph rendering.
///
/// Each instance represents one glyph to be drawn from the font atlas.
/// Text is rendered as individual glyph instances for maximum flexibility.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct TextInstance {
    /// Position in screen space (top-left of glyph)
    pub position: [f32; 2],
    /// Size of the glyph quad in screen space
    pub size: [f32; 2],
    /// Atlas UV coordinates (top-left)
    pub atlas_uv_min: [f32; 2],
    /// Atlas UV coordinates (bottom-right)
    pub atlas_uv_max: [f32; 2],
    /// Color (RGBA)
    pub color: [f32; 4],
}

impl TextInstance {
    /// Create a new text instance.
    pub fn new(
        position: Vec2,
        size: Vec2,
        atlas_uv_min: [f32; 2],
        atlas_uv_max: [f32; 2],
        color: Color,
    ) -> Self {
        Self {
            position: position.into(),
            size: size.into(),
            atlas_uv_min,
            atlas_uv_max,
            color: color.into(),
        }
    }

    /// Get the WGPU vertex buffer layout for text instances.
    pub fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
        use wgpu::*;
        VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: VertexStepMode::Instance,
            attributes: &[
                // position
                VertexAttribute {
                    offset: 0,
                    shader_location: 2,
                    format: VertexFormat::Float32x2,
                },
                // size
                VertexAttribute {
                    offset: 8,
                    shader_location: 3,
                    format: VertexFormat::Float32x2,
                },
                // atlas_uv_min
                VertexAttribute {
                    offset: 16,
                    shader_location: 4,
                    format: VertexFormat::Float32x2,
                },
                // atlas_uv_max
                VertexAttribute {
                    offset: 24,
                    shader_location: 5,
                    format: VertexFormat::Float32x2,
                },
                // color
                VertexAttribute {
                    offset: 32,
                    shader_location: 6,
                    format: VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Vertex data for a unit quad (0,0 to 1,1).
///
/// Used as the base geometry for all quad instances.
/// Instanced rendering will scale and position this quad.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct QuadVertex {
    /// Position in normalized quad space (0-1)
    pub position: [f32; 2],
    /// UV coordinates for texturing/effects
    pub uv: [f32; 2],
}

impl QuadVertex {
    /// Create a new quad vertex.
    pub const fn new(position: [f32; 2], uv: [f32; 2]) -> Self {
        Self { position, uv }
    }

    /// Get the 6 vertices for a unit quad (two triangles).
    pub const fn unit_quad() -> [QuadVertex; 6] {
        [
            // First triangle
            QuadVertex::new([0.0, 0.0], [0.0, 0.0]),
            QuadVertex::new([1.0, 0.0], [1.0, 0.0]),
            QuadVertex::new([1.0, 1.0], [1.0, 1.0]),
            // Second triangle
            QuadVertex::new([0.0, 0.0], [0.0, 0.0]),
            QuadVertex::new([1.0, 1.0], [1.0, 1.0]),
            QuadVertex::new([0.0, 1.0], [0.0, 1.0]),
        ]
    }

    /// Get the WGPU vertex buffer layout for quad vertices.
    pub fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
        use wgpu::*;
        VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                // position
                VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: VertexFormat::Float32x2,
                },
                // uv
                VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: VertexFormat::Float32x2,
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quad_instance_size() {
        // Should be aligned to 16 bytes for optimal GPU performance
        let size = std::mem::size_of::<QuadInstance>();
        assert_eq!(size % 16, 0, "QuadInstance should be 16-byte aligned");
    }

    #[test]
    fn test_text_instance_size() {
        let size = std::mem::size_of::<TextInstance>();
        assert_eq!(size, 48, "TextInstance should be 48 bytes");
        assert_eq!(size % 16, 0, "TextInstance should be 16-byte aligned");
    }

    #[test]
    fn test_quad_vertex_size() {
        let size = std::mem::size_of::<QuadVertex>();
        assert_eq!(size, 16, "QuadVertex should be 16 bytes");
    }

    #[test]
    fn test_unit_quad_vertices() {
        let vertices = QuadVertex::unit_quad();
        assert_eq!(vertices.len(), 6);

        // Check first triangle
        assert_eq!(vertices[0].position, [0.0, 0.0]);
        assert_eq!(vertices[1].position, [1.0, 0.0]);
        assert_eq!(vertices[2].position, [1.0, 1.0]);

        // Check second triangle
        assert_eq!(vertices[3].position, [0.0, 0.0]);
        assert_eq!(vertices[4].position, [1.0, 1.0]);
        assert_eq!(vertices[5].position, [0.0, 1.0]);
    }

    #[test]
    fn test_quad_instance_creation() {
        let instance =
            QuadInstance::filled(Vec2::new(10.0, 20.0), Vec2::new(100.0, 50.0), Color::RED);

        assert_eq!(instance.position, [10.0, 20.0]);
        assert_eq!(instance.size, [100.0, 50.0]);
        assert_eq!(instance.border_thickness, 0.0);
    }

    #[test]
    fn test_text_instance_creation() {
        let instance = TextInstance::new(
            Vec2::new(5.0, 15.0),
            Vec2::new(10.0, 12.0),
            [0.1, 0.2],
            [0.3, 0.4],
            Color::WHITE,
        );

        assert_eq!(instance.position, [5.0, 15.0]);
        assert_eq!(instance.size, [10.0, 12.0]);
        assert_eq!(instance.atlas_uv_min, [0.1, 0.2]);
        assert_eq!(instance.atlas_uv_max, [0.3, 0.4]);
    }
}
