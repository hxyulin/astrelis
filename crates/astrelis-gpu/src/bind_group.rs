//! Bind group and bind group layout descriptors.

use crate::id::{BindGroupLayoutId, BufferId, SamplerId, TextureViewId};
use crate::types::{TextureFormat, TextureViewDimension};

/// Shader stage visibility flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ShaderStages(u8);

impl ShaderStages {
    /// No stages.
    pub const NONE: Self = Self(0);
    /// Vertex stage.
    pub const VERTEX: Self = Self(1);
    /// Fragment stage.
    pub const FRAGMENT: Self = Self(2);
    /// Compute stage.
    pub const COMPUTE: Self = Self(4);
    /// Both vertex and fragment stages.
    pub const VERTEX_FRAGMENT: Self = Self(3);

    /// Returns `true` if all bits in `other` are set in `self`.
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Returns the raw bits.
    pub const fn bits(self) -> u8 {
        self.0
    }
}

impl std::ops::BitOr for ShaderStages {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// How a texture is sampled in a shader.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureSampleType {
    /// Float sampling (with optional filtering).
    Float {
        /// Whether the texture can be used with filtering samplers.
        filterable: bool,
    },
    /// Depth texture sampling.
    Depth,
    /// Signed integer sampling.
    Sint,
    /// Unsigned integer sampling.
    Uint,
}

/// Access mode for storage textures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StorageTextureAccess {
    /// Write-only access from the shader.
    WriteOnly,
    /// Read-only access from the shader.
    ReadOnly,
    /// Read-write access from the shader.
    ReadWrite,
}

/// Type of sampler binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SamplerBindingType {
    /// Sampler that can be used with filtering operations.
    Filtering,
    /// Sampler that uses nearest-neighbor only.
    NonFiltering,
    /// Sampler used for depth comparison.
    Comparison,
}

/// The type of a binding in a bind group layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BindingType {
    /// A uniform buffer binding.
    UniformBuffer {
        /// Whether this buffer has a dynamic offset.
        has_dynamic_offset: bool,
        /// Minimum binding size in bytes. 0 = no minimum.
        min_binding_size: u64,
    },
    /// A storage buffer binding.
    StorageBuffer {
        /// Whether this buffer has a dynamic offset.
        has_dynamic_offset: bool,
        /// Minimum binding size in bytes. 0 = no minimum.
        min_binding_size: u64,
        /// Whether the buffer is read-only from the shader.
        read_only: bool,
    },
    /// A sampled texture binding.
    Texture {
        /// How the texture is sampled.
        sample_type: TextureSampleType,
        /// View dimension.
        view_dimension: TextureViewDimension,
        /// Whether the texture is multisampled.
        multisampled: bool,
    },
    /// A storage texture binding.
    StorageTexture {
        /// Shader access mode.
        access: StorageTextureAccess,
        /// Texture format.
        format: TextureFormat,
        /// View dimension.
        view_dimension: TextureViewDimension,
    },
    /// A sampler binding.
    Sampler(SamplerBindingType),
}

/// A single entry in a bind group layout.
#[derive(Clone, Debug)]
pub struct BindGroupLayoutEntry {
    /// Binding index (matches `@binding(N)` in WGSL).
    pub binding: u32,
    /// Which shader stages can see this binding.
    pub visibility: ShaderStages,
    /// The type of this binding.
    pub ty: BindingType,
    /// For arrayed bindings, the array length. `None` = non-array.
    pub count: Option<u32>,
}

/// Describes a bind group layout.
#[derive(Clone, Debug)]
pub struct BindGroupLayoutDescriptor<'a> {
    /// Debug label.
    pub label: Option<&'a str>,
    /// Layout entries.
    pub entries: &'a [BindGroupLayoutEntry],
}

/// A single resource entry in a bind group.
#[derive(Clone, Debug)]
pub enum BindGroupEntry {
    /// A buffer binding.
    Buffer {
        /// Binding index.
        binding: u32,
        /// Buffer handle.
        buffer: BufferId,
        /// Byte offset into the buffer.
        offset: u64,
        /// Byte size of the binding. `None` = rest of buffer.
        size: Option<u64>,
    },
    /// A texture view binding.
    TextureView {
        /// Binding index.
        binding: u32,
        /// Texture view handle.
        view: TextureViewId,
    },
    /// A sampler binding.
    Sampler {
        /// Binding index.
        binding: u32,
        /// Sampler handle.
        sampler: SamplerId,
    },
}

/// Describes a bind group (a set of resources bound together).
#[derive(Clone, Debug)]
pub struct BindGroupDescriptor<'a> {
    /// Debug label.
    pub label: Option<&'a str>,
    /// The layout this bind group conforms to.
    pub layout: BindGroupLayoutId,
    /// Resource entries.
    pub entries: &'a [BindGroupEntry],
}
