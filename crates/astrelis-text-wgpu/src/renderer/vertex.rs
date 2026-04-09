//! Vertex types for text and decoration rendering.

use bytemuck::{Pod, Zeroable};

/// Vertex data for text rendering.
///
/// Each text glyph is rendered as a quad with 4 vertices and 6 indices.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct TextVertex {
    /// Screen-space position `[x, y]`.
    pub position: [f32; 2],
    /// Atlas texture coordinates `[u, v]`.
    pub tex_coords: [f32; 2],
    /// RGBA color `[r, g, b, a]`.
    pub color: [f32; 4],
}

impl TextVertex {
    /// Vertex buffer layout descriptor for wgpu pipeline creation.
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TextVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // tex_coords
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // color
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Vertex data for decoration rendering (underlines, strikethrough, backgrounds).
///
/// No texture coordinates needed - decorations are solid color quads.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct DecorationVertex {
    /// Screen-space position `[x, y]`.
    pub position: [f32; 2],
    /// RGBA color `[r, g, b, a]`.
    pub color: [f32; 4],
}

impl DecorationVertex {
    /// Vertex buffer layout descriptor for wgpu pipeline creation.
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<DecorationVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // color
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_vertex_size() {
        assert_eq!(std::mem::size_of::<TextVertex>(), 32);
    }

    #[test]
    fn test_decoration_vertex_size() {
        assert_eq!(std::mem::size_of::<DecorationVertex>(), 24);
    }

    #[test]
    fn test_text_vertex_zeroed() {
        let v = TextVertex::zeroed();
        assert_eq!(v.position, [0.0, 0.0]);
        assert_eq!(v.tex_coords, [0.0, 0.0]);
        assert_eq!(v.color, [0.0, 0.0, 0.0, 0.0]);
    }
}
