use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crate::{
    Error, Framebuffer, GraphicsContext, RenderPassBuilder, pass::ColorAttachment,
    target::SurfaceTarget,
};

/// A reason a [`crate::RenderTarget`] could not provide a frame.
///
/// [`Self::Retry`] and [`Self::Suspended`] are expected acquisition outcomes.
/// Applications should schedule a later redraw for retry, or wait for a resize or
/// visibility change while suspended. Neither outcome means the device has failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameError {
    /// Acquisition timed out or remained outdated; schedule a later redraw.
    Retry,
    /// The target is zero-sized or occluded; wait for a resize or visibility change.
    Suspended,
    /// The surface was lost; recreate the target from the owning window.
    SurfaceLost,
    /// wgpu reported a surface acquisition validation failure.
    Validation,
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Retry => "frame unavailable; schedule a later redraw",
            Self::Suspended => "frame acquisition suspended until resize or visibility changes",
            Self::SurfaceLost => "surface lost; recreate the surface and render target",
            Self::Validation => "wgpu rejected surface acquisition",
        })
    }
}

impl std::error::Error for FrameError {}

#[derive(Debug, Default)]
pub(crate) struct AttachmentWrites(Vec<(Arc<AtomicBool>, bool)>);

impl AttachmentWrites {
    pub(crate) fn is_initialized(&self, state: &Arc<AtomicBool>) -> bool {
        self.0
            .iter()
            .find(|(pending, _)| Arc::ptr_eq(pending, state))
            .map_or_else(
                || state.load(Ordering::Acquire),
                |(_, initialized)| *initialized,
            )
    }

    pub(crate) fn record(&mut self, state: &Arc<AtomicBool>, initialized: bool) {
        if let Some((_, pending)) = self
            .0
            .iter_mut()
            .find(|(pending, _)| Arc::ptr_eq(pending, state))
        {
            *pending = initialized;
        } else if state.load(Ordering::Acquire) != initialized {
            self.0.push((Arc::clone(state), initialized));
        }
    }

    fn commit(self) {
        for (state, initialized) in self.0 {
            state.store(initialized, Ordering::Release);
        }
    }
}

#[derive(Debug)]
enum FrameTarget<'target, 'window> {
    Surface {
        // Views precede the acquired image in drop order.
        view: wgpu::TextureView,
        image: wgpu::SurfaceTexture,
        target: &'target mut SurfaceTarget<'window>,
        suboptimal: bool,
    },
    Framebuffer(&'target mut Framebuffer),
}

impl FrameTarget<'_, '_> {
    fn graphics(&self) -> &GraphicsContext {
        match self {
            Self::Surface { target, .. } => &target.graphics,
            Self::Framebuffer(target) => &target.graphics,
        }
    }
}

/// Command recording for a window surface or an offscreen framebuffer.
///
/// Created by [`crate::RenderTarget::begin_frame`] or [`Framebuffer::begin_frame`].
/// The default destination is exclusively borrowed until finish or drop, preventing
/// its resize and another recording. Renderers are independent of this borrow.
/// [`Self::render_to`] records additional framebuffer passes into the same encoder.
///
/// [`Self::finish`] submits once and presents only for a surface frame. Dropping
/// the frame discards all recorded commands, releases any acquired image, and
/// does not commit framebuffer initialization. Submission never waits for GPU completion.
///
/// A destination cannot be resized while its frame is still in use:
///
/// ```compile_fail
/// use astrelis::{FrameError, RenderTarget};
/// fn resize_during_frame(target: &mut RenderTarget<'_>) -> Result<(), FrameError> {
///     let frame = target.begin_frame()?;
///     target.resize(640, 480).unwrap();
///     frame.finish().unwrap();
///     Ok(())
/// }
/// ```
///
/// End the pass before consuming its frame:
///
/// ```compile_fail
/// use astrelis::{Error, Frame};
/// fn finish_during_pass(mut frame: Frame<'_, '_>) -> Result<(), Error> {
///     let pass = frame.render_pass().begin()?;
///     frame.finish()?;
///     drop(pass);
///     Ok(())
/// }
/// ```
#[derive(Debug)]
#[must_use = "call finish to submit the frame; dropping it discards the recorded work"]
pub struct Frame<'target, 'window> {
    // Recording is released before an acquired image when abandoned.
    encoder: wgpu::CommandEncoder,
    target: FrameTarget<'target, 'window>,
    initialized: bool,
    writes: AttachmentWrites,
}

impl<'target, 'window> Frame<'target, 'window> {
    pub(crate) fn new(
        target: &'target mut SurfaceTarget<'window>,
        image: wgpu::SurfaceTexture,
        suboptimal: bool,
    ) -> Self {
        let view = image.texture.create_view(&Default::default());
        let encoder = create_encoder(&target.graphics);
        Self {
            encoder,
            target: FrameTarget::Surface {
                view,
                image,
                target,
                suboptimal,
            },
            initialized: false,
            writes: AttachmentWrites::default(),
        }
    }

    pub(crate) fn for_framebuffer(target: &'target mut Framebuffer) -> Self {
        let encoder = create_encoder(&target.graphics);
        Self {
            encoder,
            target: FrameTarget::Framebuffer(target),
            initialized: false,
            writes: AttachmentWrites::default(),
        }
    }

    /// Configures a color pass writing to this frame's default destination.
    ///
    /// Defaults clear transparent black, store results, and use the full viewport
    /// and scissor. Later `load()` passes preserve individual MSAA samples.
    /// A surface frame's first pass must clear; a framebuffer may load contents
    /// from earlier submitted recordings. Dropping a builder records nothing.
    pub fn render_pass(&mut self) -> RenderPassBuilder<'_> {
        match &self.target {
            FrameTarget::Surface { view, target, .. } => RenderPassBuilder::new(
                &mut self.encoder,
                target.graphics.device(),
                ColorAttachment {
                    view: target.multisample_view.as_ref().unwrap_or(view),
                    resolve_target: target.multisample_view.as_ref().map(|_| view),
                    format: target.configuration.format,
                    size: target.size,
                    sample_count: target.sample_count,
                },
                &mut self.initialized,
            )
            .with_depth_stencil(target.depth_stencil.as_ref(), &mut self.writes),
            FrameTarget::Framebuffer(target) => RenderPassBuilder::for_framebuffer(
                &mut self.encoder,
                target.graphics.device(),
                target,
                &mut self.writes,
            ),
        }
    }

    /// Configures a pass writing to another framebuffer in this recording.
    ///
    /// Commands share the frame's encoder and submission. End this pass before
    /// sampling its resolved output in a subsequent pass. The destination is
    /// exclusively borrowed while its builder/pass is active. Change its size
    /// and MSAA between recordings; replacing attachments invalidates old views.
    ///
    /// `begin()` rejects another device, a zero-sized target, or an initial load
    /// with no submitted or earlier recorded clear. This pass never initializes
    /// a surface frame's default destination; clear that surface before finish.
    ///
    /// ```compile_fail
    /// use astrelis::{Error, Frame, Framebuffer};
    /// fn resize_during_pass(frame: &mut Frame<'_, '_>, target: &mut Framebuffer) -> Result<(), Error> {
    ///     let pass = frame.render_to(target).begin()?;
    ///     target.resize(640, 480)?;
    ///     drop(pass);
    ///     Ok(())
    /// }
    /// ```
    pub fn render_to<'pass>(
        &'pass mut self,
        target: &'pass mut Framebuffer,
    ) -> RenderPassBuilder<'pass> {
        RenderPassBuilder::for_framebuffer(
            &mut self.encoder,
            self.target.graphics().device(),
            target,
            &mut self.writes,
        )
    }

    /// Borrows the command encoder for copies, compute, and custom GPU recording.
    ///
    /// Commands participate in `finish()` or are discarded when the frame drops.
    /// Use resources on the same device. Raw writes do not update framebuffer
    /// initialization, and a surface still needs a wrapped clear before finish.
    ///
    /// Encoder access is rejected while a pass is active:
    ///
    /// ```compile_fail
    /// use astrelis::{Error, Frame};
    /// fn encode_during_pass(mut frame: Frame<'_, '_>) -> Result<(), Error> {
    ///     let pass = frame.render_pass().begin()?;
    ///     frame.encoder().insert_debug_marker("cannot encode during a pass");
    ///     drop(pass);
    ///     Ok(())
    /// }
    /// ```
    pub fn encoder(&mut self) -> &mut wgpu::CommandEncoder {
        &mut self.encoder
    }

    /// Returns the default destination's physical size, fixed for this recording.
    pub fn size(&self) -> [u32; 2] {
        match &self.target {
            FrameTarget::Surface { target, .. } => target.size,
            FrameTarget::Framebuffer(target) => target.size(),
        }
    }

    /// Returns the default destination's color format.
    pub fn format(&self) -> wgpu::TextureFormat {
        match &self.target {
            FrameTarget::Surface { target, .. } => target.configuration.format,
            FrameTarget::Framebuffer(target) => target.format(),
        }
    }

    /// Returns the default destination's optional depth/stencil format.
    pub fn depth_stencil_format(&self) -> Option<wgpu::TextureFormat> {
        match &self.target {
            FrameTarget::Surface { target, .. } => target.depth_stencil_format,
            FrameTarget::Framebuffer(target) => target.depth_stencil_format(),
        }
    }

    /// Returns the default destination's color sample count.
    ///
    /// Additional framebuffer passes can have different counts; custom renderers
    /// should use the active pass's count for pipeline selection.
    pub fn sample_count(&self) -> u32 {
        match &self.target {
            FrameTarget::Surface { target, .. } => target.sample_count,
            FrameTarget::Framebuffer(target) => target.sample_count(),
        }
    }

    /// Submits once, committing framebuffer writes and presenting a surface if owned.
    ///
    /// Offscreen frames submit without presenting. The submission index identifies
    /// queued work, not GPU completion. Suboptimal surfaces are reconfigured after
    /// presentation, which may synchronize. Empty offscreen recordings are allowed.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UninitializedFrame`] and discards all recorded work if a
    /// surface frame has no clear pass for its default destination. Clearing an
    /// additional framebuffer does not satisfy this requirement.
    pub fn finish(self) -> Result<wgpu::SubmissionIndex, Error> {
        if matches!(self.target, FrameTarget::Surface { .. }) && !self.initialized {
            return Err(Error::UninitializedFrame);
        }
        let submission = self
            .target
            .graphics()
            .queue()
            .submit([self.encoder.finish()]);
        self.writes.commit();
        if let FrameTarget::Surface {
            view,
            image,
            target,
            suboptimal,
        } = self.target
        {
            target.graphics.queue().present(image);
            drop(view);
            if suboptimal {
                target.configure();
            }
        }
        Ok(submission)
    }
}

fn create_encoder(graphics: &GraphicsContext) -> wgpu::CommandEncoder {
    graphics
        .device()
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Astrelis frame"),
        })
}
