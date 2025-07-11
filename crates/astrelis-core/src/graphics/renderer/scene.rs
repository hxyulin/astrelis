use std::collections::HashMap;

use glam::Mat4;

use crate::{
    graphics::{
        mesh::{MeshComponent, MeshHandle}, shader::{PipelineCache, PipelineCacheEntry}, MatHandle, Material, MaterialComponent, RenderableSurface
    }, world::{Registry, Transform}, Engine, RenderContext, Window
};

type RenderKey = (MeshHandle, MatHandle);
/// A Renderer for a Scene
pub struct SceneRenderer {
    instance_buffer: wgpu::Buffer,
    render_list: HashMap<RenderKey, Vec<Transform>>,
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

    pub fn encode_scene(&mut self, reg: Registry) {
        for (_ent, mesh, mat, transform) in
            reg.query::<(MeshComponent, MaterialComponent, Transform)>()
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
        ctx: &RenderContext,
        target: RenderableSurface<'_>,
    ) {
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
            let mat = engine.mats.get_mesh(mat);
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
                        // TODO: Implement the rest
                        ..Default::default()
                    },
                )
            });

            render_pass.set_pipeline(pipeline);
            for (binding, bind_group) in bind_groups {
                // Currently only support using the entire buffer
                render_pass.set_bind_group(*binding, bind_group, &[]);
            }
            // TODO: Bind our vertex and index buffers

            // Make this a seperate scope, to facilitate easier refactoring when multirendering is
            // supported:
            // Write our batch to instance buffer
            // Set instance buffer and render
            // render
        }
    }
}
