use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::{Error, GraphicsContext};

/// The fixed vertex format accepted by the first Astrelis renderer.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Vertex {
    /// Position, interpreted as clip space by default: X/Y -1 to 1 and Z 0 to 1.
    /// Custom material vertex shaders can transform this into clip space.
    pub position: [f32; 3],
    /// Linear RGB and straight alpha, interpolated across each triangle.
    pub color: [f32; 4],
}

impl Vertex {
    /// Creates a colored clip-space vertex.
    pub const fn new(position: [f32; 3], color: [f32; 4]) -> Self {
        Self { position, color }
    }

    pub(crate) fn layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 2] =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }
}

/// An immutable indexed triangle mesh uploaded once to a GPU device.
///
/// Create meshes through [`GraphicsContext::create_mesh`].
///
/// The mesh owns its vertex and Uint32 index buffers. Rendering borrows those
/// buffers without rebuilding or uploading the mesh each frame. Drop the mesh
/// when it is no longer needed; wgpu retains resources used by in-flight work.
#[derive(Debug)]
pub struct Mesh {
    pub(crate) device: wgpu::Device,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    vertex_count: u32,
    index_count: u32,
}

impl Mesh {
    /// Validates and uploads a fixed-format vertex buffer and Uint32 index buffer.
    ///
    /// Every three indices describe one triangle. Vertex colors use straight
    /// alpha; the fragment shader premultiplies them before blending.
    pub(crate) fn upload(
        graphics: &GraphicsContext,
        vertices: &[Vertex],
        indices: &[u32],
    ) -> Result<Self, Error> {
        validate(
            vertices,
            indices,
            graphics.device().limits().max_buffer_size,
        )?;
        let vertex_buffer =
            graphics
                .device()
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Astrelis mesh vertices"),
                    contents: bytemuck::cast_slice(vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
        let index_buffer =
            graphics
                .device()
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Astrelis mesh indices"),
                    contents: bytemuck::cast_slice(indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
        Ok(Self {
            device: graphics.device().clone(),
            vertex_buffer,
            index_buffer,
            vertex_count: vertices.len() as u32,
            index_count: indices.len() as u32,
        })
    }

    /// Returns the number of uploaded vertices.
    pub const fn vertex_count(&self) -> u32 {
        self.vertex_count
    }

    /// Returns the number of uploaded Uint32 indices.
    pub const fn index_count(&self) -> u32 {
        self.index_count
    }

    /// Returns the vertex buffer for custom wgpu rendering.
    pub fn vertex_buffer(&self) -> &wgpu::Buffer {
        &self.vertex_buffer
    }

    /// Returns the Uint32 index buffer for custom wgpu rendering.
    pub fn index_buffer(&self) -> &wgpu::Buffer {
        &self.index_buffer
    }
}

fn validate(vertices: &[Vertex], indices: &[u32], max_buffer_size: u64) -> Result<(), Error> {
    if vertices.is_empty() {
        return Err(Error::EmptyVertices);
    }
    if indices.is_empty() || !indices.len().is_multiple_of(3) {
        return Err(Error::InvalidIndexCount {
            count: indices.len(),
        });
    }
    if vertices.len() > u32::MAX as usize
        || indices.len() > u32::MAX as usize
        || std::mem::size_of_val(vertices) as u64 > max_buffer_size
        || std::mem::size_of_val(indices) as u64 > max_buffer_size
    {
        return Err(Error::MeshTooLarge);
    }
    for (index, vertex) in vertices.iter().enumerate() {
        if !vertex
            .position
            .iter()
            .chain(&vertex.color)
            .all(|value| value.is_finite())
        {
            return Err(Error::InvalidVertex { index });
        }
    }
    for &index in indices {
        if index as usize >= vertices.len() {
            return Err(Error::IndexOutOfBounds {
                index,
                vertex_count: vertices.len(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_malformed_meshes_before_gpu_upload() {
        let vertex = Vertex::new([0.0; 3], [1.0; 4]);
        assert!(matches!(
            validate(&[], &[0, 0, 0], 1024),
            Err(Error::EmptyVertices)
        ));
        assert!(matches!(
            validate(&[vertex], &[0, 0], 1024),
            Err(Error::InvalidIndexCount { .. })
        ));
        assert!(matches!(
            validate(&[vertex], &[0, 1, 0], 1024),
            Err(Error::IndexOutOfBounds { index: 1, .. })
        ));
        assert!(matches!(
            validate(&[vertex], &[0, 0, 0], 8),
            Err(Error::MeshTooLarge)
        ));
        let invalid = Vertex::new([f32::NAN, 0.0, 0.0], [1.0; 4]);
        assert!(matches!(
            validate(&[invalid], &[0, 0, 0], 1024),
            Err(Error::InvalidVertex { index: 0 })
        ));
    }
}
