//! Texture-related type conversions.

use crate::texture::{Extent3d, TextureUsages};

pub(crate) fn texture_usages(u: TextureUsages) -> wgpu::TextureUsages {
    wgpu::TextureUsages::from_bits_truncate(u.bits())
}

pub(crate) fn extent3d(e: Extent3d) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width: e.width,
        height: e.height,
        depth_or_array_layers: e.depth_or_array_layers,
    }
}
