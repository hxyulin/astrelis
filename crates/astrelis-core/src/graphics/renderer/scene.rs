use std::collections::HashMap;

use glam::{Mat4, Vec4};
use puffin::profile_scope;
use wgpu::{DepthStencilState, VertexAttribute, util::DeviceExt};

use crate::{
    Engine, RenderContext, Window,
    graphics::{
        MatHandle, Material, MaterialComponent, RenderableSurface,
        mesh::{GpuMesh, MeshComponent, MeshHandle, Vertex},
        shader::{PipelineCache, PipelineCacheEntry},
        texture::Texture,
    },
    world::{GlobalTransform, Registry},
};

type RenderKey = (MeshHandle, MatHandle);
/// A Renderer for a Scene
pub struct SceneRenderer {
    instance_buffer: wgpu::Buffer,
    render_list: HashMap<RenderKey, Vec<GlobalTransform>>,
    pipeline_cache: PipelineCache,
    uniform_buffer: wgpu::Buffer,
}

impl SceneRenderer {
    pub const INSTANCE_BUF_SIZE: usize = 1024;

    // TODO: Use pipeline cache

    pub fn new(window: &Window) -> Self {
        let device = &window.context.device;

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instance_buffer"),
            size: (size_of::<Mat4>() * Self::INSTANCE_BUF_SIZE) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("material_uniform_buffer"),
            size: size_of::<Material>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            instance_buffer,
            uniform_buffer,
            render_list: HashMap::new(),
            pipeline_cache: PipelineCache::new(),
        }
    }

    pub fn encode_scene(&mut self, reg: &Registry) {
        for (_ent, mesh, mat, transform) in
            reg.query::<(MeshComponent, MaterialComponent, GlobalTransform)>()
        {
            let hdl = (mesh.0, mat.0);
            if let Some(transforms) = self.render_list.get_mut(&hdl) {
                transforms.push(*transform);
            } else {
                self.render_list.insert(hdl, vec![*transform]);
            }
        }
    }

    pub fn render(
        &mut self,
        engine: &mut Engine,
        ctx: &mut RenderContext,
        target: RenderableSurface<'_>,
    ) {
        let frame = ctx.window.context.frame.as_mut().unwrap();
        frame.passes += 1;
        let device = &ctx.window.context.device;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SceneRenderer Command Encoder"),
        });

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("SceneRenderer Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.get_color(&ctx.window.context),
                resolve_target: None,
                ops: wgpu::Operations {
                    // TODO: Configure sky color in the scene
                    load: wgpu::LoadOp::Clear(wgpu::Color::RED),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: target.get_depth(&ctx.window.context).map(|view| {
                wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        for ((mesh, mat), transforms) in self.render_list.drain() {
            assert!(
                transforms.len() < Self::INSTANCE_BUF_SIZE,
                "TODO: Allow multiple draw calls"
            );

            let mut copy_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("SceneRenderer Copy Encoder"),
            });

            {
                let mat = engine.mats.get_mat(mat);
                profile_scope!("setup_mat");

                let PipelineCacheEntry {
                    pipeline,
                    bind_groups,
                } = self.pipeline_cache.get_or_create_pipeline(mat.shader, || {
                    use crate::graphics::shader::{BuiltinUniform, ShaderResources, UniformType};

                    let shader = engine.shaders.get_shader_mut(mat.shader);
                    shader.create_pipeline(
                        device,
                        ShaderResources {
                            resources: HashMap::from([(
                                UniformType::Builtin(BuiltinUniform::Material),
                                self.uniform_buffer.as_entire_binding(),
                            )]),
                            targets: &[Some(ctx.window.context.config.format.into())],
                            vertex_buffers: &[
                                Vertex::buffer_layout(),
                                wgpu::VertexBufferLayout {
                                    array_stride: size_of::<GlobalTransform>() as u64,
                                    step_mode: wgpu::VertexStepMode::Instance,
                                    attributes: &[
                                        VertexAttribute {
                                            format: wgpu::VertexFormat::Float32x4,
                                            offset: 0,
                                            shader_location: 2,
                                        },
                                        VertexAttribute {
                                            format: wgpu::VertexFormat::Float32x4,
                                            offset: size_of::<Vec4>() as u64,
                                            shader_location: 3,
                                        },
                                        VertexAttribute {
                                            format: wgpu::VertexFormat::Float32x4,
                                            offset: (2 * size_of::<Vec4>()) as u64,
                                            shader_location: 4,
                                        },
                                        VertexAttribute {
                                            format: wgpu::VertexFormat::Float32x4,
                                            offset: (3 * size_of::<Vec4>()) as u64,
                                            shader_location: 5,
                                        },
                                    ],
                                },
                            ],
                            // TODO: Don't hardcode
                            depth_stencil: Some(DepthStencilState {
                                format: Texture::DEPTH_FORMAT,
                                depth_write_enabled: true,
                                bias: wgpu::DepthBiasState::default(),
                                stencil: wgpu::StencilState::default(),
                                depth_compare: wgpu::CompareFunction::Less,
                            }),
                            // TODO: Implement the rest
                            ..Default::default()
                        },
                    )
                });

                let staging_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Material Staging Buffer"),
                    contents: bytemuck::bytes_of(mat),
                    usage: wgpu::BufferUsages::COPY_SRC,
                });

                copy_encoder.copy_buffer_to_buffer(
                    &staging_buffer,
                    0,
                    &self.uniform_buffer,
                    0,
                    size_of::<Material>() as u64,
                );

                render_pass.set_pipeline(pipeline);
                for (binding, bind_group) in bind_groups {
                    // Currently only support using the entire buffer
                    render_pass.set_bind_group(*binding, bind_group, &[]);
                }
            }

            let mesh = engine.meshes.get_mesh_mut(mesh);
            let GpuMesh {
                vertex,
                index,
                vertex_count,
            } = mesh.get_or_create_gpumesh(|mesh| GpuMesh::from_mesh(mesh, device));

            {
                profile_scope!("setup_mesh");

                render_pass.set_vertex_buffer(0, vertex.slice(..));
                render_pass.set_index_buffer(index.slice(..), wgpu::IndexFormat::Uint32);
            }

            let batches = transforms.len().div_ceil(Self::INSTANCE_BUF_SIZE);
            for i in 0..batches {
                profile_scope!("render_batch");
                let start = i * Self::INSTANCE_BUF_SIZE;
                let max = if i + 1 < batches {
                    (i + 1) * Self::INSTANCE_BUF_SIZE
                } else {
                    transforms.len()
                };
                let batch_count = max - start;
                assert!(batch_count <= Self::INSTANCE_BUF_SIZE);

                let staging_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Transform Staging Buffer"),
                    contents: bytemuck::cast_slice(&transforms[start..max]),
                    usage: wgpu::BufferUsages::COPY_SRC,
                });

                copy_encoder.copy_buffer_to_buffer(
                    &staging_buffer,
                    0,
                    &self.instance_buffer,
                    (start * std::mem::size_of::<GlobalTransform>()) as u64,
                    (batch_count * std::mem::size_of::<GlobalTransform>()) as u64,
                );

                render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
                render_pass.draw_indexed(0..*vertex_count as u32, 0, 0..batch_count as u32);
            }

            {
                profile_scope!("submit_copy_encoder");
                ctx.window.context.queue.submit(Some(copy_encoder.finish()));
            }
        }

        drop(render_pass);

        {
            profile_scope!("submit_encoder");
            ctx.window.context.queue.submit(Some(encoder.finish()));
        }
    }
}
