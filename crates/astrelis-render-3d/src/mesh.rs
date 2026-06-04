//! Mesh data types: CPU-side vertices and GPU mesh handles.

use astrelis_gpu::pipeline::{VertexAttribute, VertexBufferLayout};
use astrelis_gpu::types::{VertexFormat, VertexStepMode};

/// A single mesh vertex (48 bytes).
///
/// Normals and UVs are present even though v1 rendering is unlit and
/// untextured: generators produce them for free, and adding them
/// later would force regenerating every mesh when lighting lands.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    /// Object-space position.
    pub position: [f32; 3],
    /// Unit normal (unused by the unlit shader; reserved for lighting).
    pub normal: [f32; 3],
    /// Texture coordinates (unused in v1; reserved for texturing).
    pub uv: [f32; 2],
    /// Per-vertex RGBA color, multiplied with the per-draw tint.
    pub color: [f32; 4],
}

impl Vertex {
    /// Vertex buffer layout matching the WGSL vertex inputs.
    pub(crate) fn layout() -> VertexBufferLayout<'static> {
        const ATTRS: [VertexAttribute; 4] = [
            VertexAttribute { format: VertexFormat::Float32x3, offset: 0, shader_location: 0 },
            VertexAttribute { format: VertexFormat::Float32x3, offset: 12, shader_location: 1 },
            VertexAttribute { format: VertexFormat::Float32x2, offset: 24, shader_location: 2 },
            VertexAttribute { format: VertexFormat::Float32x4, offset: 32, shader_location: 3 },
        ];
        VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: VertexStepMode::Vertex,
            attributes: &ATTRS,
        }
    }
}

/// CPU-side mesh data, ready for upload via `Renderer3D::create_mesh`.
///
/// Plain data — callers may edit (e.g. paint vertex colors) before
/// uploading.
pub struct MeshData {
    /// Vertex list.
    pub vertices: Vec<Vertex>,
    /// Triangle list indices into `vertices` (CCW = front face).
    pub indices: Vec<u32>,
}

/// Handle to a mesh uploaded with `Renderer3D::create_mesh`.
///
/// Plain index, valid for the lifetime of the renderer that created
/// it (no `destroy_mesh` in v1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MeshHandle(pub(crate) u32);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_is_48_bytes_pod() {
        assert_eq!(std::mem::size_of::<Vertex>(), 48);
        // Pod round-trip: cast a slice without panicking.
        let v = [Vertex {
            position: [1.0, 2.0, 3.0],
            normal: [0.0, 1.0, 0.0],
            uv: [0.5, 0.5],
            color: [1.0, 1.0, 1.0, 1.0],
        }];
        let bytes: &[u8] = bytemuck::cast_slice(&v);
        assert_eq!(bytes.len(), 48);
    }

    #[test]
    fn vertex_layout_matches_field_offsets() {
        let layout = Vertex::layout();
        assert_eq!(layout.array_stride, 48);
        let offsets: Vec<u64> = layout.attributes.iter().map(|a| a.offset).collect();
        assert_eq!(offsets, vec![0, 12, 24, 32]);
        let locations: Vec<u32> = layout.attributes.iter().map(|a| a.shader_location).collect();
        assert_eq!(locations, vec![0, 1, 2, 3]);
    }
}
