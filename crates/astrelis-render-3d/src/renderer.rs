//! The 3D renderer: draw list, depth target, debug lines.

use astrelis_gpu::pipeline::{VertexAttribute, VertexBufferLayout};
use astrelis_gpu::types::{VertexFormat, VertexStepMode};

/// A debug-line vertex (28 bytes): world-space position + color.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct LineVertex {
    /// World-space position of this endpoint.
    pub position: [f32; 3],
    /// RGBA color.
    pub color: [f32; 4],
}

impl LineVertex {
    /// Vertex buffer layout matching the WGSL line vertex inputs.
    // used by Pipeline3D (pipeline.rs)
    #[allow(dead_code)]
    pub(crate) fn layout() -> VertexBufferLayout<'static> {
        const ATTRS: [VertexAttribute; 2] = [
            VertexAttribute { format: VertexFormat::Float32x3, offset: 0, shader_location: 0 },
            VertexAttribute { format: VertexFormat::Float32x4, offset: 12, shader_location: 1 },
        ];
        VertexBufferLayout {
            array_stride: std::mem::size_of::<LineVertex>() as u64,
            step_mode: VertexStepMode::Vertex,
            attributes: &ATTRS,
        }
    }
}
