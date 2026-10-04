use std::collections::HashMap;

use crate::{Error, GraphicsContext, Mesh, RenderPass, Vertex};

/// A device-bound renderer for colored indexed triangle meshes.
///
/// Construct with [`Self::new`]. This is a rendering component that caches its
/// pipeline state; it is independent of the graphics context's resource factories.
///
/// Pipelines are cached by target format. Meshes are drawn in call order, with
/// alpha blending and without depth testing or back-face culling. This is a
/// clip-space mesh API. Frame acquisition, pass creation, and presentation belong
/// to the target and frame, allowing independent renderers to share a pass.
#[derive(Debug)]
pub struct MeshRenderer {
    graphics: GraphicsContext,
    shader: wgpu::ShaderModule,
    layout: wgpu::PipelineLayout,
    pipelines: HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>,
}

impl MeshRenderer {
    /// Creates a renderer sharing the context's device and queue.
    pub fn new(graphics: &GraphicsContext) -> Self {
        let shader = graphics
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Astrelis colored mesh shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("mesh.wgsl").into()),
            });
        let layout = graphics
            .device()
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Astrelis colored mesh layout"),
                bind_group_layouts: &[],
                immediate_size: 0,
            });
        Self {
            graphics: graphics.clone(),
            shader,
            layout,
            pipelines: HashMap::new(),
        }
    }

    /// Records one indexed mesh draw into an application-opened pass.
    ///
    /// This method does not acquire, clear, submit, or present. Any number of
    /// renderers sharing the pass's device can contribute draws in application order.
    /// Pipelines are cached by attachment format, and mesh data is not reuploaded.
    /// Each draw restores the mesh pipeline and vertex/index bindings and applies
    /// the pass's chosen viewport and scissor rectangle. Application clipping and
    /// viewport settings persist across draws and independent mesh renderers.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DeviceMismatch`] without recording a draw if the pass or
    /// mesh belongs to a different device.
    pub fn draw(&mut self, pass: &mut RenderPass<'_>, mesh: &Mesh) -> Result<(), Error> {
        if pass.device() != self.graphics.device() || &mesh.device != self.graphics.device() {
            return Err(Error::DeviceMismatch);
        }
        let pipeline = self.pipelines.entry(pass.format()).or_insert_with(|| {
            self.graphics
                .device()
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Astrelis colored mesh pipeline"),
                    layout: Some(&self.layout),
                    vertex: wgpu::VertexState {
                        module: &self.shader,
                        entry_point: Some("vertex_main"),
                        compilation_options: Default::default(),
                        buffers: &[Some(Vertex::layout())],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &self.shader,
                        entry_point: Some("fragment_main"),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: pass.format(),
                            blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                })
        });
        pass.apply_raster_state();
        pass.inner.set_pipeline(pipeline);
        pass.inner
            .set_vertex_buffer(0, mesh.vertex_buffer().slice(..));
        pass.inner
            .set_index_buffer(mesh.index_buffer().slice(..), wgpu::IndexFormat::Uint32);
        pass.inner.draw_indexed(0..mesh.index_count(), 0, 0..1);
        Ok(())
    }
}

#[cfg(test)]
#[path = "mesh_renderer_tests.rs"]
mod tests;
