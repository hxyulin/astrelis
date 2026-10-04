use crate::{Error, RenderPassBuilder, target::SurfaceTarget};

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

/// One acquired surface image and its command recording.
///
/// Created by [`crate::RenderTarget::begin_frame`]. The frame exclusively borrows
/// its target until it is presented or dropped, preventing resize and acquisition
/// of another frame through that target. It does not borrow a renderer: multiple
/// renderers on the same device can contribute to its passes.
///
/// Dropping a frame discards its recorded commands and releases the acquired image
/// without submitting or presenting it. Call [`Self::present`] explicitly to submit.
///
/// A target cannot be resized while its frame is still in use:
///
/// ```compile_fail
/// use astrelis::{FrameError, RenderTarget};
/// fn resize_during_frame(target: &mut RenderTarget<'_>) -> Result<(), FrameError> {
///     let frame = target.begin_frame()?;
///     target.resize(640, 480).unwrap();
///     frame.present().unwrap();
///     Ok(())
/// }
/// ```
///
/// End the pass before consuming its frame:
///
/// ```compile_fail
/// use astrelis::{Error, Frame};
/// fn present_during_pass(mut frame: Frame<'_, '_>) -> Result<(), Error> {
///     let pass = frame.render_pass().begin()?;
///     frame.present()?;
///     drop(pass);
///     Ok(())
/// }
/// ```
#[derive(Debug)]
#[must_use = "call present to submit the frame; dropping it discards the recorded work"]
pub struct Frame<'target, 'window> {
    // Recording and views are released before the acquired image when abandoned.
    encoder: wgpu::CommandEncoder,
    view: wgpu::TextureView,
    image: wgpu::SurfaceTexture,
    target: &'target mut SurfaceTarget<'window>,
    suboptimal: bool,
    initialized: bool,
}

impl<'target, 'window> Frame<'target, 'window> {
    pub(crate) fn new(
        target: &'target mut SurfaceTarget<'window>,
        image: wgpu::SurfaceTexture,
        suboptimal: bool,
    ) -> Self {
        let view = image.texture.create_view(&Default::default());
        let encoder =
            target
                .graphics
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Astrelis frame"),
                });
        Self {
            encoder,
            view,
            image,
            target,
            suboptimal,
            initialized: false,
        }
    }

    /// Configures a color pass targeting this frame's acquired image.
    ///
    /// The builder defaults to clearing transparent black, storing results, and
    /// using the full viewport and scissor rectangle. Call `begin()` to record.
    /// Defaults are consistent for each pass; call `load()` on later passes to
    /// preserve earlier drawing. Dropping a builder records nothing.
    pub fn render_pass(&mut self) -> RenderPassBuilder<'_> {
        RenderPassBuilder::new(
            &mut self.encoder,
            &self.view,
            self.target.graphics.device(),
            self.target.configuration.format,
            self.target.size,
            &mut self.initialized,
        )
    }

    /// Borrows the command encoder for copies, compute, and custom GPU recording.
    ///
    /// Available before, between, and after scoped passes. Commands remain part of
    /// this frame: `present()` submits them, while dropping the frame discards them.
    /// Use resources from the context's device and leave encoder ownership with the
    /// frame. Raw commands do not initialize the managed surface attachment; record
    /// a wrapped clear pass before presentation.
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

    /// Returns the frame's physical pixel dimensions, fixed at acquisition.
    pub fn size(&self) -> [u32; 2] {
        self.target.size
    }

    /// Returns the frame's color attachment format.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.target.configuration.format
    }

    /// Consumes the frame, submits its commands, and requests presentation once.
    ///
    /// The submission index identifies queued work; it does not mean the GPU has
    /// completed it. Normal presentation does not wait for GPU completion. A
    /// suboptimal surface is reconfigured after presentation, which may synchronize.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UninitializedFrame`] and discards the frame if no clear pass
    /// was recorded. A clear-only pass is sufficient to initialize the frame.
    pub fn present(self) -> Result<wgpu::SubmissionIndex, Error> {
        if !self.initialized {
            return Err(Error::UninitializedFrame);
        }
        let submission = self.target.graphics.queue().submit([self.encoder.finish()]);
        self.target.graphics.queue().present(self.image);
        drop(self.view);
        if self.suboptimal {
            self.target.configure();
        }
        Ok(submission)
    }
}
