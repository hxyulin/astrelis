use crate::{graphics::Framebuffer, profiling::profile_function};
use bytemuck::offset_of;
use glam::{Vec2, Vec4};
use puffin::profile_scope;
use wgpu::util::DeviceExt;

use crate::{RenderContext, Window};

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, bytemuck::Zeroable)]
struct QuadInstance {
    translation: Vec2,
    rotation: f32,
    scale: Vec2,
    color: Vec4,
}

pub struct SimpleRenderer {
    instance_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
    quad_instances: Vec<QuadInstance>,
}

impl SimpleRenderer {
    pub const INSTANCE_BUF_SIZE: usize = 128;
    pub fn new(window: &Window) -> Self {
        let device = &window.context.device;

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instance_buffer"),
            size: (size_of::<QuadInstance>() * Self::INSTANCE_BUF_SIZE) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let vertices: &[f32; 8] = &[
            -0.5, -0.5, // Bottom-left
            0.5, -0.5, // Bottom-right
            0.5, 0.5, // Top-right
            -0.5, 0.5, // Top-left
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let indices: &[u16; 6] = &[0, 1, 2, 0, 2, 3];

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let bind_entries = [];
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SimpleRenderer bind group layout"),
            entries: &bind_entries,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SimpleRenderer bind group"),
            layout: &bind_group_layout,
            entries: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SimpleRenderer Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("simple_renderer.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SimpleRenderer Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let vertex_buffers = [
            // Vertex buffer layout for base quad geometry
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<[f32; 2]>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                }],
            },
            // Instance buffer layout for translation, rotation, scale, and color
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<QuadInstance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &[
                    wgpu::VertexAttribute {
                        offset: 0,
                        shader_location: 1,
                        format: wgpu::VertexFormat::Float32x2, // translation
                    },
                    wgpu::VertexAttribute {
                        offset: offset_of!(QuadInstance, rotation) as u64,
                        shader_location: 2,
                        format: wgpu::VertexFormat::Float32, // rotation
                    },
                    wgpu::VertexAttribute {
                        offset: offset_of!(QuadInstance, scale) as u64,
                        shader_location: 3,
                        format: wgpu::VertexFormat::Float32x2, // scale
                    },
                    wgpu::VertexAttribute {
                        offset: offset_of!(QuadInstance, color) as u64,
                        shader_location: 4,
                        format: wgpu::VertexFormat::Float32x4, // color
                    },
                ],
            },
        ];

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("SimpleRenderer Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &vertex_buffers,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(window.context.config.format.into())],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: window.context.sample_count,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        Self {
            instance_buffer,
            vertex_buffer,
            index_buffer,
            render_pipeline,
            bind_group,
            quad_instances: Vec::new(),
        }
    }
    pub fn submit_quad(&mut self, translation: Vec2, rotation: f32, scale: Vec2, color: Vec4) {
        profile_function!();
        self.quad_instances.push(QuadInstance {
            translation,
            rotation,
            scale,
            color,
        });
    }

    /// Renders the submitted meshes
    /// If a framebuffer is provided, it renders to the framebuffer, otherwise it renders on a
    /// surface
    pub fn render(&mut self, ctx: &mut RenderContext, fb: Option<&Framebuffer>) {
        profile_function!();
        let frame = ctx.window.context.frame.as_mut().unwrap();
        frame.passes += 1;
        let device = &ctx.window.context.device;

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SimpleRenderer Command Encoder"),
        });

        let view = match &fb {
            Some(fb) => &fb.view,
            None => &frame.view,
        };

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("SimpleRenderer Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::RED),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

        let mut copy_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SimpleRenderer Copy Encoder"),
        });

        let batches =
            (self.quad_instances.len() as f32 / Self::INSTANCE_BUF_SIZE as f32).ceil() as usize;
        for i in 0..batches {
            profile_scope!("render batch");
            let start = i * Self::INSTANCE_BUF_SIZE;
            let max = if i + 1 < batches {
                (i + 1) * Self::INSTANCE_BUF_SIZE
            } else {
                self.quad_instances.len()
            };
            let batch_count = max - start;
            assert!(batch_count <= Self::INSTANCE_BUF_SIZE);

            let contents = unsafe {
                let ptr = self.quad_instances.as_ptr() as *const u8;
                let len = self.quad_instances.len() * size_of::<QuadInstance>();
                std::slice::from_raw_parts(ptr, len)
            };
            let staging_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Quad Instance Staging Buffer"),
                contents,
                usage: wgpu::BufferUsages::COPY_SRC,
            });

            copy_encoder.copy_buffer_to_buffer(
                &staging_buffer,
                0,
                &self.instance_buffer,
                (start * std::mem::size_of::<QuadInstance>()) as u64,
                (batch_count * std::mem::size_of::<QuadInstance>()) as u64,
            );

            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            render_pass.draw_indexed(0..6, 0, 0..batch_count as u32);
        }

        {
            profile_scope!("submit copy_encoder");
            ctx.window.context.queue.submit(Some(copy_encoder.finish()));
        }
        drop(render_pass);

        self.quad_instances.clear();

        {
            profile_scope!("submit encoder");
            ctx.window.context.queue.submit(Some(encoder.finish()));
        }
    }
}
