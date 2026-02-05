//! Shared pipeline creation helpers for all render tiers.

use bytemuck::{Pod, Zeroable};

use super::types::UnifiedInstance2D;

/// Unit quad vertex with position and tex_coords.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct QuadVertex2D {
    pub position: [f32; 2],
    pub tex_coords: [f32; 2],
}

unsafe impl Pod for QuadVertex2D {}
unsafe impl Zeroable for QuadVertex2D {}

impl QuadVertex2D {
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: &[wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
            0 => Float32x2,  // position
            1 => Float32x2,  // tex_coords
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<QuadVertex2D>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: ATTRS,
        }
    }
}

/// Unit quad vertices: two triangles covering [0,0] to [1,1].
pub const UNIT_QUAD_VERTICES: &[QuadVertex2D] = &[
    QuadVertex2D {
        position: [0.0, 0.0],
        tex_coords: [0.0, 0.0],
    },
    QuadVertex2D {
        position: [1.0, 0.0],
        tex_coords: [1.0, 0.0],
    },
    QuadVertex2D {
        position: [0.0, 1.0],
        tex_coords: [0.0, 1.0],
    },
    QuadVertex2D {
        position: [1.0, 0.0],
        tex_coords: [1.0, 0.0],
    },
    QuadVertex2D {
        position: [1.0, 1.0],
        tex_coords: [1.0, 1.0],
    },
    QuadVertex2D {
        position: [0.0, 1.0],
        tex_coords: [0.0, 1.0],
    },
];

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Create the unit quad vertex buffer.
pub fn create_quad_vbo(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::Buffer {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("batched_quad_vbo"),
        size: std::mem::size_of_val(UNIT_QUAD_VERTICES) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&buffer, 0, bytemuck::cast_slice(UNIT_QUAD_VERTICES));
    buffer
}

/// Create the projection uniform buffer.
pub fn create_projection_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("batched_projection"),
        size: 64, // mat4x4<f32>
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Create the projection bind group layout (group 1 for standard, group 1 for bindless).
pub fn create_projection_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("batched_projection_layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    })
}

/// Create the projection bind group.
pub fn create_projection_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("batched_projection_bg"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    })
}

/// Create the standard per-texture bind group layout (group 0 for Tier 1 & 2).
pub fn create_standard_texture_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("batched_standard_texture_layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

/// Create the bindless texture array bind group layout (group 0 for Tier 3).
pub fn create_bindless_texture_bind_group_layout(
    device: &wgpu::Device,
    max_textures: u32,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("batched_bindless_texture_layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: std::num::NonZeroU32::new(max_textures),
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

/// Depth stencil state used by all tiers.
fn depth_stencil_state(depth_write: bool) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: depth_write,
        depth_compare: wgpu::CompareFunction::GreaterEqual,
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    }
}

/// Create a render pipeline for the batched renderer.
///
/// `opaque`: if true, creates the opaque pass pipeline (depth_write=true, no blend).
/// Otherwise creates the transparent pass pipeline (depth_write=false, alpha blend).
pub fn create_batched_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    pipeline_layout: &wgpu::PipelineLayout,
    surface_format: wgpu::TextureFormat,
    opaque: bool,
) -> wgpu::RenderPipeline {
    let label = if opaque {
        "batched_opaque_pipeline"
    } else {
        "batched_transparent_pipeline"
    };

    let blend = if opaque {
        None
    } else {
        Some(wgpu::BlendState::ALPHA_BLENDING)
    };

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[QuadVertex2D::layout(), UnifiedInstance2D::layout()],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None, // UI quads are 2D, no culling needed
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: Some(depth_stencil_state(opaque)),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

/// Create an instance buffer with the given capacity.
pub fn create_instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("batched_instance_buffer"),
        size: (capacity * std::mem::size_of::<UnifiedInstance2D>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Create a depth texture for the given dimensions.
pub fn create_depth_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("batched_depth_texture"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

/// A 1x1 white fallback texture for solid quads that need a texture bound.
pub fn create_fallback_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::Texture, wgpu::TextureView, wgpu::Sampler) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("batched_fallback_texture"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &[255, 255, 255, 255],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4),
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("batched_fallback_sampler"),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    (texture, view, sampler)
}
