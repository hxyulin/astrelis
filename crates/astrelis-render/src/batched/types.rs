//! Core types for the batched deferred renderer.
//!
//! All three render tiers share a unified instance format and draw batch structure.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};

/// Render tier describing GPU feature availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderTier {
    /// Tier 1: Per-clip, per-texture `draw()` calls. No special features required.
    Direct,
    /// Tier 2: `multi_draw_indirect()` per texture group.
    /// Requires `INDIRECT_FIRST_INSTANCE`.
    Indirect,
    /// Tier 3: Single `multi_draw_indirect()` per frame using bindless textures.
    /// Requires `INDIRECT_FIRST_INSTANCE` + `TEXTURE_BINDING_ARRAY` + `PARTIALLY_BOUND_BINDING_ARRAY`.
    Bindless,
}

impl std::fmt::Display for RenderTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderTier::Direct => write!(f, "Direct (Tier 1)"),
            RenderTier::Indirect => write!(f, "Indirect (Tier 2)"),
            RenderTier::Bindless => write!(f, "Bindless (Tier 3)"),
        }
    }
}

/// Draw type discriminant for the unified instance format.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DrawType2D {
    /// Solid quad with SDF rounded rectangle. No texture sampling.
    Quad = 0,
    /// Text glyph: R8 atlas alpha multiplied by instance color.
    Text = 1,
    /// Image: RGBA texture multiplied by tint color.
    Image = 2,
}

/// Unified instance data shared by all three render tiers.
///
/// 96 bytes total, 16-byte aligned. Encodes quads, text glyphs, and images
/// as textured/untextured quads differentiated by `draw_type`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UnifiedInstance2D {
    /// Screen-space position (top-left corner).
    pub position: [f32; 2],
    /// Quad size in pixels.
    pub size: [f32; 2],
    /// Texture UV min (0,0 for solid quads).
    pub uv_min: [f32; 2],
    /// Texture UV max (0,0 for solid quads).
    pub uv_max: [f32; 2],
    /// Fill/tint/text color (RGBA, premultiplied alpha).
    pub color: [f32; 4],
    /// SDF corner radius in pixels.
    pub border_radius: f32,
    /// Border outline thickness (0 = filled).
    pub border_thickness: f32,
    /// Index into the texture array (0 for text atlas, 1..N for images).
    pub texture_index: u32,
    /// Draw type discriminant: 0=quad, 1=text, 2=image.
    pub draw_type: u32,
    /// Shader-based clip rect min (screen space).
    pub clip_min: [f32; 2],
    /// Shader-based clip rect max (screen space).
    pub clip_max: [f32; 2],
    /// Normalized depth (0.0=far, 1.0=near). Higher z_index maps to higher z_depth.
    pub z_depth: f32,
    /// Reserved for future use (rotation, flags, custom_data).
    pub _reserved: [f32; 3],
}

// SAFETY: UnifiedInstance is repr(C) with only f32 and u32 fields, no padding holes
unsafe impl Pod for UnifiedInstance2D {}
unsafe impl Zeroable for UnifiedInstance2D {}

impl Default for UnifiedInstance2D {
    fn default() -> Self {
        Self {
            position: [0.0; 2],
            size: [0.0; 2],
            uv_min: [0.0; 2],
            uv_max: [0.0; 2],
            color: [1.0, 1.0, 1.0, 1.0],
            border_radius: 0.0,
            border_thickness: 0.0,
            texture_index: 0,
            draw_type: DrawType2D::Quad as u32,
            clip_min: [f32::NEG_INFINITY, f32::NEG_INFINITY],
            clip_max: [f32::INFINITY, f32::INFINITY],
            z_depth: 0.0,
            _reserved: [0.0; 3],
        }
    }
}

impl UnifiedInstance2D {
    /// Returns the wgpu vertex buffer layout for instanced rendering.
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: &[wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
            // location 2: position (vec2)
            2 => Float32x2,
            // location 3: size (vec2)
            3 => Float32x2,
            // location 4: uv_min (vec2)
            4 => Float32x2,
            // location 5: uv_max (vec2)
            5 => Float32x2,
            // location 6: color (vec4)
            6 => Float32x4,
            // location 7: border_radius (f32)
            7 => Float32,
            // location 8: border_thickness (f32)
            8 => Float32,
            // location 9: texture_index (u32)
            9 => Uint32,
            // location 10: draw_type (u32)
            10 => Uint32,
            // location 11: clip_min (vec2)
            11 => Float32x2,
            // location 12: clip_max (vec2)
            12 => Float32x2,
            // location 13: z_depth (f32)
            13 => Float32,
            // location 14: _reserved (vec3)
            14 => Float32x3,
        ];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<UnifiedInstance2D>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: ATTRS,
        }
    }

    /// Size of the instance in bytes.
    pub const SIZE: u64 = std::mem::size_of::<Self>() as u64;
}

/// A texture slot in the draw batch.
pub struct TextureSlot2D {
    /// Stable ID for cache keying.
    pub id: u64,
    /// The texture view to bind.
    pub view: Arc<wgpu::TextureView>,
    /// The sampler to use.
    pub sampler: Arc<wgpu::Sampler>,
}

/// A complete draw batch submitted to the renderer each frame.
pub struct DrawBatch2D {
    /// Instances sorted by (draw_type, texture_index) for efficient batching.
    pub instances: Vec<UnifiedInstance2D>,
    /// Texture slots. Index 0 is typically the text atlas (R8), 1..N are images (RGBA).
    pub textures: Vec<TextureSlot2D>,
    /// Orthographic projection matrix.
    pub projection: [[f32; 4]; 4],
}

/// Rendering statistics from the last frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct BatchRenderStats2D {
    /// Total number of instances rendered.
    pub instance_count: u32,
    /// Number of opaque instances.
    pub opaque_count: u32,
    /// Number of transparent instances.
    pub transparent_count: u32,
    /// Number of GPU draw calls issued.
    pub draw_calls: u32,
    /// Number of bind group switches.
    pub bind_group_switches: u32,
    /// Number of pipeline switches.
    pub pipeline_switches: u32,
    /// Number of textures bound.
    pub texture_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unified_instance_size() {
        assert_eq!(std::mem::size_of::<UnifiedInstance2D>(), 96);
    }

    #[test]
    fn test_unified_instance_alignment() {
        assert!(std::mem::align_of::<UnifiedInstance2D>() <= 16);
    }

    #[test]
    fn test_draw_type_values() {
        assert_eq!(DrawType2D::Quad as u32, 0);
        assert_eq!(DrawType2D::Text as u32, 1);
        assert_eq!(DrawType2D::Image as u32, 2);
    }
}
