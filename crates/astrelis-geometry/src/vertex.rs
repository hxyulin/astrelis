//! Vertex formats for tessellated geometry.
//!
//! Defines the vertex formats used after tessellation.

use astrelis_render::wgpu;
use bytemuck::{Pod, Zeroable};

/// Vertex for filled geometry.
///
/// Simple 2D position vertex for tessellated fills.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct FillVertex {
    /// Position in 2D space
    pub position: [f32; 2],
}

impl FillVertex {
    /// Create a new fill vertex.
    pub fn new(x: f32, y: f32) -> Self {
        Self { position: [x, y] }
    }

    /// Get the WGPU vertex buffer layout.
    pub fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            }],
        }
    }
}

/// Vertex for stroked geometry.
///
/// Includes position and normal for stroke expansion.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct StrokeVertex {
    /// Position in 2D space (on the path centerline)
    pub position: [f32; 2],
    /// Normal vector (perpendicular to path)
    pub normal: [f32; 2],
    /// Distance along the path (for dash patterns)
    pub distance: f32,
    /// Side indicator (-1 or 1 for left/right of path)
    pub side: f32,
}

impl StrokeVertex {
    /// Create a new stroke vertex.
    pub fn new(x: f32, y: f32, nx: f32, ny: f32, distance: f32, side: f32) -> Self {
        Self {
            position: [x, y],
            normal: [nx, ny],
            distance,
            side,
        }
    }

    /// Get the WGPU vertex buffer layout.
    pub fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32,
                },
                wgpu::VertexAttribute {
                    offset: 20,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        }
    }
}

/// Output from tessellation: vertices and indices.
#[derive(Debug, Clone, Default)]
pub struct TessellatedMesh<V> {
    /// Vertex data
    pub vertices: Vec<V>,
    /// Index data (triangles)
    pub indices: Vec<u32>,
}

impl<V> TessellatedMesh<V> {
    /// Create a new empty mesh.
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// Create a mesh with given vertices and indices.
    pub fn from_data(vertices: Vec<V>, indices: Vec<u32>) -> Self {
        Self { vertices, indices }
    }

    /// Check if the mesh is empty.
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty() || self.indices.is_empty()
    }

    /// Get the number of vertices.
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Get the number of indices.
    pub fn index_count(&self) -> usize {
        self.indices.len()
    }

    /// Get the number of triangles.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Clear all data.
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fill_vertex_size() {
        assert_eq!(std::mem::size_of::<FillVertex>(), 8);
    }

    #[test]
    fn test_stroke_vertex_size() {
        assert_eq!(std::mem::size_of::<StrokeVertex>(), 24);
    }

    #[test]
    fn test_empty_mesh() {
        let mesh: TessellatedMesh<FillVertex> = TessellatedMesh::new();
        assert!(mesh.is_empty());
        assert_eq!(mesh.vertex_count(), 0);
        assert_eq!(mesh.triangle_count(), 0);
    }
}
