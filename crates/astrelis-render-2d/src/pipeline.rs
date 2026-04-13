//! Render pipeline setup for the 2D renderer.

use astrelis_gpu::bind_group::{
    BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntry,
    BindingType, SamplerBindingType, ShaderStages, TextureSampleType,
};
use astrelis_gpu::device::GpuDevice;
use astrelis_gpu::pipeline::{
    ColorTargetState, FragmentState, MultisampleState, PipelineLayoutDescriptor, PrimitiveState,
    RenderPipelineDescriptor, VertexState,
};
use astrelis_gpu::types::{BlendState, ColorWrites};
use astrelis_gpu::resources::{BindGroup, BindGroupLayout, RenderPipeline, Sampler, TextureView};
use astrelis_gpu::shader::{ShaderModuleDescriptor, ShaderSource};
use astrelis_gpu::types::{TextureFormat, TextureViewDimension};

use crate::instance::Instance2D;

/// GPU resources for the 2D render pipeline.
pub(crate) struct Pipeline2D {
    pub pipeline: RenderPipeline,
    pub camera_bind_group_layout: BindGroupLayout,
    pub texture_bind_group_layout: BindGroupLayout,
}

impl Pipeline2D {
    /// Creates the 2D render pipeline.
    pub fn new(device: &GpuDevice, surface_format: TextureFormat) -> Self {
        astrelis_profiling::profile_function!();

        let shader = device
            .create_shader_module(&ShaderModuleDescriptor {
                label: Some("render2d_shader"),
                source: ShaderSource::Wgsl(include_str!("shader.wgsl")),
            })
            .expect("failed to compile render2d shader");

        // Group 0: Camera uniform.
        let camera_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("render2d_camera_layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::UniformBuffer {
                        has_dynamic_offset: false,
                        min_binding_size: 0,
                    },
                    count: None,
                }],
            });

        // Group 1: Texture + sampler.
        let texture_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("render2d_texture_layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Texture {
                            sample_type: TextureSampleType::Float { filterable: true },
                            view_dimension: TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Sampler(SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("render2d_pipeline_layout"),
            bind_group_layouts: &[&camera_bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("render2d_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Instance2D::layout()],
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[ColorTargetState {
                    format: surface_format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                }],
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
        });

        Self {
            pipeline,
            camera_bind_group_layout,
            texture_bind_group_layout,
        }
    }

    /// Creates a bind group for the camera uniform buffer.
    pub fn create_camera_bind_group(
        &self,
        device: &GpuDevice,
        buffer: &astrelis_gpu::Buffer,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("render2d_camera_bg"),
            layout: &self.camera_bind_group_layout,
            entries: &[BindGroupEntry::Buffer {
                binding: 0,
                buffer,
                offset: 0,
                size: None,
            }],
        })
    }

    /// Creates a bind group for a texture + sampler pair.
    pub fn create_texture_bind_group(
        &self,
        device: &GpuDevice,
        view: &TextureView,
        sampler: &Sampler,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("render2d_texture_bg"),
            layout: &self.texture_bind_group_layout,
            entries: &[
                BindGroupEntry::TextureView {
                    binding: 0,
                    view,
                },
                BindGroupEntry::Sampler {
                    binding: 1,
                    sampler,
                },
            ],
        })
    }
}
