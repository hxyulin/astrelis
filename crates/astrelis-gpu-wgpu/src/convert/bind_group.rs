//! Bind group and layout type conversions.

use astrelis_gpu::bind_group::*;

use super::types;

pub(crate) fn shader_stages(s: ShaderStages) -> wgpu::ShaderStages {
    wgpu::ShaderStages::from_bits_truncate(s.bits() as u32)
}

pub(crate) fn binding_type(ty: &BindingType) -> wgpu::BindingType {
    match ty {
        BindingType::UniformBuffer {
            has_dynamic_offset,
            min_binding_size,
        } => wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: *has_dynamic_offset,
            min_binding_size: std::num::NonZeroU64::new(*min_binding_size),
        },
        BindingType::StorageBuffer {
            has_dynamic_offset,
            min_binding_size,
            read_only,
        } => wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage {
                read_only: *read_only,
            },
            has_dynamic_offset: *has_dynamic_offset,
            min_binding_size: std::num::NonZeroU64::new(*min_binding_size),
        },
        BindingType::Texture {
            sample_type,
            view_dimension,
            multisampled,
        } => wgpu::BindingType::Texture {
            sample_type: texture_sample_type(sample_type),
            view_dimension: types::texture_view_dimension(*view_dimension),
            multisampled: *multisampled,
        },
        BindingType::StorageTexture {
            access,
            format,
            view_dimension,
        } => wgpu::BindingType::StorageTexture {
            access: storage_texture_access(*access),
            format: types::texture_format(*format),
            view_dimension: types::texture_view_dimension(*view_dimension),
        },
        BindingType::Sampler(ty) => wgpu::BindingType::Sampler(sampler_binding_type(*ty)),
    }
}

fn texture_sample_type(t: &TextureSampleType) -> wgpu::TextureSampleType {
    match t {
        TextureSampleType::Float { filterable } => {
            wgpu::TextureSampleType::Float {
                filterable: *filterable,
            }
        }
        TextureSampleType::Depth => wgpu::TextureSampleType::Depth,
        TextureSampleType::Sint => wgpu::TextureSampleType::Sint,
        TextureSampleType::Uint => wgpu::TextureSampleType::Uint,
    }
}

fn storage_texture_access(a: StorageTextureAccess) -> wgpu::StorageTextureAccess {
    match a {
        StorageTextureAccess::WriteOnly => wgpu::StorageTextureAccess::WriteOnly,
        StorageTextureAccess::ReadOnly => wgpu::StorageTextureAccess::ReadOnly,
        StorageTextureAccess::ReadWrite => wgpu::StorageTextureAccess::ReadWrite,
    }
}

fn sampler_binding_type(t: SamplerBindingType) -> wgpu::SamplerBindingType {
    match t {
        SamplerBindingType::Filtering => wgpu::SamplerBindingType::Filtering,
        SamplerBindingType::NonFiltering => wgpu::SamplerBindingType::NonFiltering,
        SamplerBindingType::Comparison => wgpu::SamplerBindingType::Comparison,
    }
}
