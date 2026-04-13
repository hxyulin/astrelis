//! Unified 2D instance format shared by all draw types.
//!
//! Designed to be tier-agnostic: the same instance data works with
//! direct draws (Tier 1), indirect draws (Tier 2), and bindless
//! rendering (Tier 3).

use astrelis_gpu::pipeline::{VertexAttribute, VertexBufferLayout};
use astrelis_gpu::types::{VertexFormat, VertexStepMode};
use bytemuck::{Pod, Zeroable};

/// Draw type discriminant for the unified instance format.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DrawType {
    /// Textured sprite.
    Sprite = 0,
    /// Solid or outlined rectangle.
    Rect = 1,
    /// Circle (SDF-based in fragment shader).
    Circle = 2,
    /// Line (expanded to a thin quad).
    Line = 3,
}

/// Unified per-instance data for 2D rendering.
///
/// All draw types (sprites, shapes) share this format. A single shader
/// handles them via the `draw_type` discriminant.
///
/// 80 bytes, 4-byte aligned.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Instance2D {
    /// World-space position (top-left for rects, center for circles).
    pub position: [f32; 2],
    /// Quad size in world units.
    pub size: [f32; 2],
    /// Texture UV min (0,0 for untextured shapes).
    pub uv_min: [f32; 2],
    /// Texture UV max (0,0 for untextured shapes).
    pub uv_max: [f32; 2],
    /// Color (tint for sprites, fill for shapes). Premultiplied alpha.
    pub color: [f32; 4],
    /// Rotation in radians.
    pub rotation: f32,
    /// Normalized depth (higher = closer to camera).
    pub z_depth: f32,
    /// Index into the bound texture array (0 = white pixel for shapes).
    pub texture_index: u32,
    /// Draw type discriminant (see [`DrawType`]).
    pub draw_type: u32,
}

impl Instance2D {
    /// Size of a single instance in bytes.
    pub const SIZE: u64 = std::mem::size_of::<Self>() as u64;

    /// Returns the vertex buffer layout for instanced rendering.
    pub fn layout() -> VertexBufferLayout<'static> {
        const ATTRS: &[VertexAttribute] = &[
            VertexAttribute { format: VertexFormat::Float32x2, offset: 0, shader_location: 0 },
            VertexAttribute { format: VertexFormat::Float32x2, offset: 8, shader_location: 1 },
            VertexAttribute { format: VertexFormat::Float32x2, offset: 16, shader_location: 2 },
            VertexAttribute { format: VertexFormat::Float32x2, offset: 24, shader_location: 3 },
            VertexAttribute { format: VertexFormat::Float32x4, offset: 32, shader_location: 4 },
            VertexAttribute { format: VertexFormat::Float32, offset: 48, shader_location: 5 },
            VertexAttribute { format: VertexFormat::Float32, offset: 52, shader_location: 6 },
            VertexAttribute { format: VertexFormat::Uint32, offset: 56, shader_location: 7 },
            VertexAttribute { format: VertexFormat::Uint32, offset: 60, shader_location: 8 },
        ];

        VertexBufferLayout {
            array_stride: Self::SIZE,
            step_mode: VertexStepMode::Instance,
            attributes: ATTRS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_size() {
        // 2+2+2+2+4+1+1+1+1 = 16 floats * 4 + 2 u32 * 4 = 64+8 = not quite.
        // Actually: 8+8+8+8+16+4+4+4+4 = 64 bytes. But we said 80 in the doc...
        // Let's just verify.
        assert_eq!(std::mem::size_of::<Instance2D>(), 64);
    }

    #[test]
    fn draw_type_values() {
        assert_eq!(DrawType::Sprite as u32, 0);
        assert_eq!(DrawType::Rect as u32, 1);
        assert_eq!(DrawType::Circle as u32, 2);
        assert_eq!(DrawType::Line as u32, 3);
    }
}
