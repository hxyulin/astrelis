//! Render pipeline setup for the 3D renderer.

use astrelis_gpu::bind_group::{
    BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntry,
    BindingType, ShaderStages,
};
use astrelis_gpu::device::GpuDevice;
use astrelis_gpu::pipeline::{
    ColorTargetState, DepthStencilState, FragmentState, MultisampleState,
    PipelineLayoutDescriptor, PrimitiveState, RenderPipelineDescriptor, VertexState,
};
use astrelis_gpu::resources::{BindGroup, BindGroupLayout, RenderPipeline};
use astrelis_gpu::shader::{ShaderModuleDescriptor, ShaderSource};
use astrelis_gpu::types::{
    BlendState, ColorWrites, CompareFunction, CullMode, PrimitiveTopology, TextureFormat,
};

use crate::mesh::Vertex;
use crate::renderer::LineVertex;

/// Depth format used by the 3D pass (reverse-Z, cleared to 0.0).
pub(crate) const DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;

/// GPU resources for the 3D render pipelines.
pub(crate) struct Pipeline3D {
    /// Opaque mesh render pipeline.
    pub mesh_pipeline: RenderPipeline,
    /// Debug line render pipeline.
    pub line_pipeline: RenderPipeline,
    /// Bind group layout for the camera uniform (group 0).
    pub camera_layout: BindGroupLayout,
    /// Bind group layout for the per-draw storage buffer (group 1).
    pub draw_layout: BindGroupLayout,
}

impl Pipeline3D {
    /// Creates the mesh and line pipelines.
    pub fn new(device: &GpuDevice, surface_format: TextureFormat) -> Self {
        astrelis_profiling::profile_function!();

        let shader = device
            .create_shader_module(&ShaderModuleDescriptor {
                label: Some("render3d_shader"),
                source: ShaderSource::Wgsl(include_str!("shader.wgsl")),
            })
            .expect("failed to compile render3d shader");

        // Group 0: camera uniform (shared by both pipelines).
        let camera_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("render3d_camera_layout"),
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

        // Group 1: per-draw storage buffer (mesh pipeline only).
        let draw_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("render3d_draw_layout"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX,
                ty: BindingType::StorageBuffer {
                    has_dynamic_offset: false,
                    min_binding_size: 0,
                    read_only: true,
                },
                count: None,
            }],
        });

        let mesh_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("render3d_mesh_pipeline_layout"),
            bind_group_layouts: &[&camera_layout, &draw_layout],
            push_constant_ranges: &[],
        });

        let line_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("render3d_line_pipeline_layout"),
            bind_group_layouts: &[&camera_layout],
            push_constant_ranges: &[],
        });

        // Opaque mesh pass: depth write + reverse-Z GreaterEqual,
        // back-face culling (generators guarantee CCW-from-outside).
        let mesh_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("render3d_mesh_pipeline"),
            layout: Some(&mesh_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: "vs_mesh",
                buffers: &[Vertex::layout()],
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[ColorTargetState {
                    format: surface_format,
                    blend: None, // opaque
                    write_mask: ColorWrites::ALL,
                }],
            }),
            primitive: PrimitiveState {
                cull_mode: CullMode::Back,
                ..Default::default()
            },
            depth_stencil: Some(DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: CompareFunction::GreaterEqual,
            }),
            multisample: MultisampleState::default(),
        });

        // Debug lines: depth-tested but never depth-written, so they
        // occlude behind geometry without punching holes in it.
        let line_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("render3d_line_pipeline"),
            layout: Some(&line_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: "vs_line",
                buffers: &[LineVertex::layout()],
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
            primitive: PrimitiveState {
                topology: PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: Some(DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: CompareFunction::GreaterEqual,
            }),
            multisample: MultisampleState::default(),
        });

        Self { mesh_pipeline, line_pipeline, camera_layout, draw_layout }
    }

    /// Creates a bind group for the camera uniform buffer.
    pub fn create_camera_bind_group(
        &self,
        device: &GpuDevice,
        buffer: &astrelis_gpu::Buffer,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("render3d_camera_bg"),
            layout: &self.camera_layout,
            entries: &[BindGroupEntry::Buffer { binding: 0, buffer, offset: 0, size: None }],
        })
    }

    /// Creates a bind group for the per-draw storage buffer.
    pub fn create_draw_bind_group(
        &self,
        device: &GpuDevice,
        buffer: &astrelis_gpu::Buffer,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("render3d_draw_bg"),
            layout: &self.draw_layout,
            entries: &[BindGroupEntry::Buffer { binding: 0, buffer, offset: 0, size: None }],
        })
    }
}

