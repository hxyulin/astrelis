use std::collections::HashMap;

use crate::{Error, GraphicsContext, Mesh, RenderTarget, Vertex};

/// The result of a surface render attempt.
#[derive(Debug)]
pub enum FrameStatus {
    /// Rendering was submitted and the surface texture was presented.
    Presented(wgpu::SubmissionIndex),
    /// Acquisition timed out or remained outdated; schedule a later redraw.
    Retry,
    /// The target is zero-sized or occluded; wait for a resize or visibility change.
    Suspended,
}

/// A device-bound renderer for colored indexed triangle meshes.
///
/// Pipelines are cached by target format. Meshes are drawn in slice order, with
/// alpha blending and without depth testing or back-face culling. This is a
/// clip-space mesh API, rather than a camera or scene system.
#[derive(Debug)]
pub struct Renderer {
    graphics: GraphicsContext,
    shader: wgpu::ShaderModule,
    layout: wgpu::PipelineLayout,
    pipelines: HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>,
}

impl Renderer {
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

    /// Clears a surface, draws borrowed meshes in order, submits, and presents.
    ///
    /// An empty mesh slice is a valid clear-only frame. Clear colors are linear.
    /// No mesh data is reuploaded, and normal frames do not wait for GPU
    /// completion. Surface reconfiguration may synchronize with the GPU.
    /// Outdated surfaces are reconfigured and acquisition is retried once.
    /// For [`Error::SurfaceLost`], replace the target through
    /// [`GraphicsContext::create_surface`] with the same window and current size.
    pub fn render(
        &mut self,
        target: &mut RenderTarget<'_>,
        clear: wgpu::Color,
        meshes: &[&Mesh],
    ) -> Result<FrameStatus, Error> {
        self.validate(clear, meshes)?;
        let RenderTarget::Surface(target) = target;
        if target.graphics.device() != self.graphics.device() {
            return Err(Error::DeviceMismatch);
        }
        if target.size.contains(&0) {
            return Ok(FrameStatus::Suspended);
        }
        let mut acquisition = target.surface.get_current_texture();
        if matches!(acquisition, wgpu::CurrentSurfaceTexture::Outdated) {
            target.configure();
            acquisition = target.surface.get_current_texture();
        }
        let (frame, suboptimal) = match acquisition {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Outdated => {
                return Ok(FrameStatus::Retry);
            }
            wgpu::CurrentSurfaceTexture::Occluded => return Ok(FrameStatus::Suspended),
            wgpu::CurrentSurfaceTexture::Lost => return Err(Error::SurfaceLost),
            wgpu::CurrentSurfaceTexture::Validation => return Err(Error::SurfaceValidation),
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            self.graphics
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Astrelis frame"),
                });
        self.encode(
            &mut encoder,
            &view,
            target.configuration.format,
            clear,
            meshes,
        );
        let submission = self.graphics.queue().submit([encoder.finish()]);
        self.graphics.queue().present(frame);
        // Present consumes the acquired texture before reconfiguration.
        drop(view);
        if suboptimal {
            target.configure();
        }
        Ok(FrameStatus::Presented(submission))
    }

    fn validate(&self, clear: wgpu::Color, meshes: &[&Mesh]) -> Result<(), Error> {
        if ![clear.r, clear.g, clear.b, clear.a]
            .iter()
            .all(|value| value.is_finite())
        {
            return Err(Error::InvalidClearColor);
        }
        if meshes
            .iter()
            .any(|mesh| &mesh.device != self.graphics.device())
        {
            return Err(Error::DeviceMismatch);
        }
        Ok(())
    }

    fn encode(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        clear: wgpu::Color,
        meshes: &[&Mesh],
    ) {
        let pipeline = self.pipelines.entry(format).or_insert_with(|| {
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
                            format,
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
        let attachments = [Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(clear),
                store: wgpu::StoreOp::Store,
            },
        })];
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Astrelis meshes"),
            color_attachments: &attachments,
            ..Default::default()
        });
        pass.set_pipeline(pipeline);
        for mesh in meshes {
            pass.set_vertex_buffer(0, mesh.vertex_buffer().slice(..));
            pass.set_index_buffer(mesh.index_buffer().slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.index_count(), 0, 0..1);
        }
    }
}

#[cfg(test)]
#[path = "renderer_tests.rs"]
mod tests;
