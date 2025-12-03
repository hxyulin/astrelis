use std::sync::Arc;

use astrelis_core::profiling::profile_function;
use astrelis_winit::window::WinitWindow;

use crate::context::GraphicsContext;

/// Statistics for a rendered frame.
pub struct FrameStats {
    pub passes: usize,
    pub draw_calls: usize,
}

impl FrameStats {
    pub(crate) fn new() -> Self {
        Self {
            passes: 0,
            draw_calls: 0,
        }
    }
}

/// Surface texture and view for rendering.
pub struct Surface {
    pub(crate) texture: wgpu::SurfaceTexture,
    pub(crate) view: wgpu::TextureView,
}

impl Surface {
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture.texture
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}

/// Context for a single frame of rendering.
pub struct FrameContext {
    pub(crate) stats: FrameStats,
    pub(crate) surface: Option<Surface>,
    pub(crate) encoder: Option<wgpu::CommandEncoder>,
    pub(crate) context: &'static GraphicsContext,
    pub(crate) window: Arc<WinitWindow>,
}

impl FrameContext {
    pub fn surface(&self) -> &Surface {
        self.surface.as_ref().unwrap()
    }

    pub fn increment_passes(&mut self) {
        self.stats.passes += 1;
    }

    pub fn increment_draw_calls(&mut self) {
        self.stats.draw_calls += 1;
    }

    pub fn stats(&self) -> &FrameStats {
        &self.stats
    }

    pub fn graphics_context(&self) -> &'static GraphicsContext {
        self.context
    }

    pub fn encoder(&mut self) -> &mut wgpu::CommandEncoder {
        self.encoder.as_mut().expect("Encoder already taken")
    }

    pub fn encoder_and_surface(&mut self) -> (&mut wgpu::CommandEncoder, &Surface) {
        (
            self.encoder.as_mut().expect("Encoder already taken"),
            self.surface.as_ref().unwrap(),
        )
    }

    pub fn finish(self) {
        drop(self);
    }
}

impl Drop for FrameContext {
    fn drop(&mut self) {
        profile_function!();

        if self.stats.passes == 0 {
            tracing::error!("No render passes were executed for this frame!");
            return;
        }

        if let Some(encoder) = self.encoder.take() {
            self.context.queue.submit(std::iter::once(encoder.finish()));
        }

        if let Some(surface) = self.surface.take() {
            surface.texture.present();
        }

        // Request redraw for next frame
        self.window.request_redraw();
    }
}

/// Builder for creating render passes.
pub struct RenderPassBuilder<'a> {
    label: Option<&'a str>,
    color_attachments: Vec<Option<wgpu::RenderPassColorAttachment<'a>>>,
    surface_attachment_ops: Option<(wgpu::Operations<wgpu::Color>, Option<&'a wgpu::TextureView>)>,
    depth_stencil_attachment: Option<wgpu::RenderPassDepthStencilAttachment<'a>>,
}

impl<'a> RenderPassBuilder<'a> {
    pub fn new() -> Self {
        Self {
            label: None,
            color_attachments: Vec::new(),
            surface_attachment_ops: None,
            depth_stencil_attachment: None,
        }
    }

    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn color_attachment(
        mut self,
        view: Option<&'a wgpu::TextureView>,
        resolve_target: Option<&'a wgpu::TextureView>,
        ops: wgpu::Operations<wgpu::Color>,
    ) -> Self {
        if let Some(view) = view {
            self.color_attachments
                .push(Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target,
                    ops,
                    depth_slice: None,
                }));
        } else {
            // Store ops for later - will be filled with surface view in build()
            self.surface_attachment_ops = Some((ops, resolve_target));
        }
        self
    }

    pub fn depth_stencil_attachment(
        mut self,
        view: &'a wgpu::TextureView,
        depth_ops: Option<wgpu::Operations<f32>>,
        stencil_ops: Option<wgpu::Operations<u32>>,
    ) -> Self {
        self.depth_stencil_attachment = Some(wgpu::RenderPassDepthStencilAttachment {
            view,
            depth_ops,
            stencil_ops,
        });
        self
    }

    /// Builds the render pass and begins it on the provided frame context.
    ///
    /// This takes ownership of the CommandEncoder from the FrameContext, and releases it
    /// back to the FrameContext when the RenderPass is dropped or [`finish`](RenderPass::finish)
    /// is called.
    pub fn build(self, frame_context: &'a mut FrameContext) -> RenderPass<'a> {
        let mut encoder = frame_context.encoder.take().unwrap();

        // Fill in surface attachment if requested
        let surface_attachment = self.surface_attachment_ops.map(|(ops, resolve_target)| {
            let surface_view = frame_context.surface().view();
            wgpu::RenderPassColorAttachment {
                view: surface_view,
                resolve_target,
                ops,
                depth_slice: None,
            }
        });

        // Build combined attachments - surface first if present, then others
        let mut all_attachments = Vec::new();
        if let Some(attachment) = surface_attachment {
            all_attachments.push(Some(attachment));
        }
        all_attachments.extend(self.color_attachments);

        let descriptor = wgpu::RenderPassDescriptor {
            label: self.label,
            color_attachments: &all_attachments,
            depth_stencil_attachment: self.depth_stencil_attachment,
            occlusion_query_set: None,
            timestamp_writes: None,
        };

        let render_pass = encoder.begin_render_pass(&descriptor).forget_lifetime();

        frame_context.increment_passes();

        RenderPass {
            context: frame_context,
            encoder: Some(encoder),
            descriptor: Some(render_pass),
        }
    }
}

impl<'a> Default for RenderPassBuilder<'a> {
    fn default() -> Self {
        Self::new()
    }
}

/// A render pass wrapper that automatically returns the encoder to the frame context.
pub struct RenderPass<'a> {
    pub context: &'a mut FrameContext,
    pub(crate) encoder: Option<wgpu::CommandEncoder>,
    pub(crate) descriptor: Option<wgpu::RenderPass<'static>>,
}

impl<'a> RenderPass<'a> {
    pub fn descriptor(&mut self) -> &mut wgpu::RenderPass<'static> {
        self.descriptor.as_mut().unwrap()
    }

    pub fn finish(self) {
        drop(self);
    }
}

impl Drop for RenderPass<'_> {
    fn drop(&mut self) {
        profile_function!();

        drop(self.descriptor.take());

        // Return the encoder to the frame context
        self.context.encoder = self.encoder.take();
    }
}
