//! Render pipelines for geometry rendering.
//!
//! Creates and manages WGPU pipelines for fill and stroke rendering.

use crate::gpu_types::{FillInstance, StrokeInstance};
use crate::vertex::{FillVertex, StrokeVertex};
use astrelis_render::{Renderer, wgpu};

/// Fill pipeline shader source.
pub const FILL_SHADER: &str = include_str!("shaders/fill.wgsl");

/// Stroke pipeline shader source.
pub const STROKE_SHADER: &str = include_str!("shaders/stroke.wgsl");

/// Creates the fill render pipeline.
pub fn create_fill_pipeline(
    renderer: &Renderer,
    projection_bind_group_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = renderer.create_shader(Some("Geometry Fill Shader"), FILL_SHADER);

    let layout = renderer.create_pipeline_layout(
        Some("Geometry Fill Pipeline Layout"),
        &[projection_bind_group_layout],
        &[],
    );

    renderer.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Geometry Fill Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[FillVertex::vertex_layout(), FillInstance::vertex_layout()],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
        cache: None,
    })
}

/// Creates the stroke render pipeline.
pub fn create_stroke_pipeline(
    renderer: &Renderer,
    projection_bind_group_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = renderer.create_shader(Some("Geometry Stroke Shader"), STROKE_SHADER);

    let layout = renderer.create_pipeline_layout(
        Some("Geometry Stroke Pipeline Layout"),
        &[projection_bind_group_layout],
        &[],
    );

    renderer.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Geometry Stroke Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[StrokeVertex::vertex_layout(), StrokeInstance::vertex_layout()],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
        cache: None,
    })
}

/// Creates the projection bind group layout.
pub fn create_projection_bind_group_layout(renderer: &Renderer) -> wgpu::BindGroupLayout {
    renderer.create_bind_group_layout(
        Some("Geometry Projection Bind Group Layout"),
        &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    )
}
