use std::sync::Arc;

use astrelis_core::profiling::{profile_function, profile_scope};
use astrelis_winit::window::WinitWindow;

use crate::context::GraphicsContext;
use crate::target::RenderTarget;

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
    pub(crate) surface_format: wgpu::TextureFormat,
}

impl FrameContext {
    pub fn surface(&self) -> &Surface {
        self.surface.as_ref().unwrap()
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.surface_format
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
            profile_scope!("submit_commands");
            self.context.queue.submit(std::iter::once(encoder.finish()));
        }

        if let Some(surface) = self.surface.take() {
            profile_scope!("present_surface");
            surface.texture.present();
        }

        // Request redraw for next frame
        self.window.request_redraw();
    }
}

/// Clear operation for a render pass.
#[derive(Debug, Clone, Copy)]
pub enum ClearOp {
    /// Load existing contents (no clear).
    Load,
    /// Clear to the specified color.
    Clear(wgpu::Color),
}

impl Default for ClearOp {
    fn default() -> Self {
        ClearOp::Load
    }
}

impl From<wgpu::Color> for ClearOp {
    fn from(color: wgpu::Color) -> Self {
        ClearOp::Clear(color)
    }
}

impl From<crate::Color> for ClearOp {
    fn from(color: crate::Color) -> Self {
        ClearOp::Clear(color.to_wgpu())
    }
}

/// Depth clear operation for a render pass.
#[derive(Debug, Clone, Copy)]
pub enum DepthClearOp {
    /// Load existing depth values.
    Load,
    /// Clear to the specified depth value (typically 1.0).
    Clear(f32),
}

impl Default for DepthClearOp {
    fn default() -> Self {
        DepthClearOp::Clear(1.0)
    }
}

/// Builder for creating render passes.
pub struct RenderPassBuilder<'a> {
    label: Option<&'a str>,
    // New simplified API
    target: Option<RenderTarget<'a>>,
    clear_op: ClearOp,
    depth_clear_op: DepthClearOp,
    // Legacy API for advanced use
    color_attachments: Vec<Option<wgpu::RenderPassColorAttachment<'a>>>,
    surface_attachment_ops: Option<(wgpu::Operations<wgpu::Color>, Option<&'a wgpu::TextureView>)>,
    depth_stencil_attachment: Option<wgpu::RenderPassDepthStencilAttachment<'a>>,
}

impl<'a> RenderPassBuilder<'a> {
    pub fn new() -> Self {
        Self {
            label: None,
            target: None,
            clear_op: ClearOp::Load,
            depth_clear_op: DepthClearOp::default(),
            color_attachments: Vec::new(),
            surface_attachment_ops: None,
            depth_stencil_attachment: None,
        }
    }

    /// Set a debug label for the render pass.
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Set the render target (Surface or Framebuffer).
    ///
    /// This is the simplified API - use this instead of manual color_attachment calls.
    pub fn target(mut self, target: RenderTarget<'a>) -> Self {
        self.target = Some(target);
        self
    }

    /// Set clear color for the render target.
    ///
    /// Pass a wgpu::Color or use ClearOp::Load to preserve existing contents.
    pub fn clear_color(mut self, color: impl Into<ClearOp>) -> Self {
        self.clear_op = color.into();
        self
    }

    /// Set depth clear operation.
    pub fn clear_depth(mut self, depth: f32) -> Self {
        self.depth_clear_op = DepthClearOp::Clear(depth);
        self
    }

    /// Load existing depth values instead of clearing.
    pub fn load_depth(mut self) -> Self {
        self.depth_clear_op = DepthClearOp::Load;
        self
    }

    // Legacy API for advanced use cases

    /// Add a color attachment manually (advanced API).
    ///
    /// For most cases, use `.target()` instead.
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

    /// Add a depth-stencil attachment manually (advanced API).
    ///
    /// For framebuffers with depth, the depth attachment is handled automatically
    /// when using `.target()`.
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

        // Build color attachments based on target or legacy API
        let mut all_attachments = Vec::new();

        if let Some(target) = &self.target {
            // New simplified API
            let color_ops = match self.clear_op {
                ClearOp::Load => wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                ClearOp::Clear(color) => wgpu::Operations {
                    load: wgpu::LoadOp::Clear(color),
                    store: wgpu::StoreOp::Store,
                },
            };

            match target {
                RenderTarget::Surface => {
                    let surface_view = frame_context.surface().view();
                    all_attachments.push(Some(wgpu::RenderPassColorAttachment {
                        view: surface_view,
                        resolve_target: None,
                        ops: color_ops,
                        depth_slice: None,
                    }));
                }
                RenderTarget::Framebuffer(fb) => {
                    all_attachments.push(Some(wgpu::RenderPassColorAttachment {
                        view: fb.render_view(),
                        resolve_target: fb.resolve_target(),
                        ops: color_ops,
                        depth_slice: None,
                    }));
                }
            }
        } else {
            // Legacy API
            if let Some((ops, resolve_target)) = self.surface_attachment_ops {
                let surface_view = frame_context.surface().view();
                all_attachments.push(Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target,
                    ops,
                    depth_slice: None,
                }));
            }
            all_attachments.extend(self.color_attachments);
        }

        // Build depth attachment
        let depth_attachment = if let Some(attachment) = self.depth_stencil_attachment {
            Some(attachment)
        } else if let Some(RenderTarget::Framebuffer(fb)) = &self.target {
            fb.depth_view().map(|view| {
                let depth_ops = match self.depth_clear_op {
                    DepthClearOp::Load => wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    DepthClearOp::Clear(depth) => wgpu::Operations {
                        load: wgpu::LoadOp::Clear(depth),
                        store: wgpu::StoreOp::Store,
                    },
                };
                wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(depth_ops),
                    stencil_ops: None,
                }
            })
        } else {
            None
        };

        let descriptor = wgpu::RenderPassDescriptor {
            label: self.label,
            color_attachments: &all_attachments,
            depth_stencil_attachment: depth_attachment,
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

impl Default for RenderPassBuilder<'_> {
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

/// Helper trait for creating render passes with common configurations.
pub trait RenderPassExt {
    /// Create a render pass that clears to the given color.
    fn clear_pass<'a>(
        &'a mut self,
        target: RenderTarget<'a>,
        clear_color: wgpu::Color,
    ) -> RenderPass<'a>;

    /// Create a render pass that loads existing content.
    fn load_pass<'a>(&'a mut self, target: RenderTarget<'a>) -> RenderPass<'a>;
}

impl RenderPassExt for FrameContext {
    fn clear_pass<'a>(
        &'a mut self,
        target: RenderTarget<'a>,
        clear_color: wgpu::Color,
    ) -> RenderPass<'a> {
        RenderPassBuilder::new()
            .target(target)
            .clear_color(clear_color)
            .build(self)
    }

    fn load_pass<'a>(&'a mut self, target: RenderTarget<'a>) -> RenderPass<'a> {
        RenderPassBuilder::new().target(target).build(self)
    }
}
