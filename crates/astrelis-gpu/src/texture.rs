//! Texture, texture view, and sampler descriptors.

use crate::types::{
    AddressMode, CompareFunction, FilterMode, TextureDimension, TextureFormat, TextureViewDimension,
};

/// Usage flags for a GPU texture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureUsages(u32);

impl TextureUsages {
    /// Texture can be used as a copy source.
    pub const COPY_SRC: Self = Self(1);
    /// Texture can be used as a copy destination.
    pub const COPY_DST: Self = Self(2);
    /// Texture can be bound for sampling in a shader.
    pub const TEXTURE_BINDING: Self = Self(4);
    /// Texture can be bound as a storage texture in a shader.
    pub const STORAGE_BINDING: Self = Self(8);
    /// Texture can be used as a render target attachment.
    pub const RENDER_ATTACHMENT: Self = Self(16);

    /// Returns `true` if all bits in `other` are set in `self`.
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Returns the raw bits.
    pub const fn bits(self) -> u32 {
        self.0
    }
}

impl std::ops::BitOr for TextureUsages {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// 3D extent (width, height, depth or array layers).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Extent3d {
    /// Width in texels.
    pub width: u32,
    /// Height in texels.
    pub height: u32,
    /// Depth (for 3D textures) or array layer count.
    pub depth_or_array_layers: u32,
}

/// Describes a texture to be created.
#[derive(Clone, Debug)]
pub struct TextureDescriptor<'a> {
    /// Debug label.
    pub label: Option<&'a str>,
    /// Texture dimensions.
    pub size: Extent3d,
    /// Number of mip levels. Must be >= 1.
    pub mip_level_count: u32,
    /// Number of samples for multisampling. 1 = no multisampling.
    pub sample_count: u32,
    /// Texture dimensionality (1D, 2D, or 3D).
    pub dimension: TextureDimension,
    /// Texel format.
    pub format: TextureFormat,
    /// Usage flags.
    pub usage: TextureUsages,
}

/// Describes a view into a texture.
#[derive(Clone, Debug, Default)]
pub struct TextureViewDescriptor<'a> {
    /// Debug label.
    pub label: Option<&'a str>,
    /// Format override. `None` uses the texture's format.
    pub format: Option<TextureFormat>,
    /// Dimension override. `None` infers from the texture.
    pub dimension: Option<TextureViewDimension>,
    /// First mip level to include.
    pub base_mip_level: u32,
    /// Number of mip levels. `None` = all remaining.
    pub mip_level_count: Option<u32>,
    /// First array layer to include.
    pub base_array_layer: u32,
    /// Number of array layers. `None` = all remaining.
    pub array_layer_count: Option<u32>,
}

/// Describes a texture sampler.
#[derive(Clone, Debug)]
pub struct SamplerDescriptor<'a> {
    /// Debug label.
    pub label: Option<&'a str>,
    /// U (horizontal) addressing mode.
    pub address_mode_u: AddressMode,
    /// V (vertical) addressing mode.
    pub address_mode_v: AddressMode,
    /// W (depth) addressing mode.
    pub address_mode_w: AddressMode,
    /// Magnification filter.
    pub mag_filter: FilterMode,
    /// Minification filter.
    pub min_filter: FilterMode,
    /// Mipmap filter.
    pub mipmap_filter: FilterMode,
    /// Minimum LOD clamp.
    pub lod_min_clamp: f32,
    /// Maximum LOD clamp.
    pub lod_max_clamp: f32,
    /// Comparison function for depth samplers. `None` = regular sampler.
    pub compare: Option<CompareFunction>,
    /// Maximum anisotropy. 1 = no anisotropic filtering.
    pub anisotropy_clamp: u16,
}

impl Default for SamplerDescriptor<'_> {
    fn default() -> Self {
        Self {
            label: None,
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            mipmap_filter: FilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 32.0,
            compare: None,
            anisotropy_clamp: 1,
        }
    }
}
