use std::sync::{Arc, atomic::AtomicBool};

use crate::{Error, Framebuffer, depth_stencil::DepthStencilAttachment, frame::AttachmentWrites};

#[derive(Debug)]
enum Initialization<'frame> {
    Surface(&'frame mut bool),
    Framebuffer(&'frame Arc<AtomicBool>),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ColorAttachment<'view> {
    pub(crate) view: &'view wgpu::TextureView,
    pub(crate) resolve_target: Option<&'view wgpu::TextureView>,
    pub(crate) format: wgpu::TextureFormat,
    pub(crate) size: [u32; 2],
    pub(crate) sample_count: u32,
}

#[derive(Debug)]
pub(crate) struct PassOptions<'label> {
    pub(crate) label: Option<&'label str>,
    pub(crate) load: wgpu::LoadOp<wgpu::Color>,
    viewport: Option<[f32; 6]>,
    scissor: Option<[u32; 4]>,
    depth_stencil: Option<&'label DepthStencilAttachment>,
    depth_ops: Option<wgpu::Operations<f32>>,
    stencil_ops: Option<wgpu::Operations<u32>>,
    depth_requested: bool,
    stencil_requested: bool,
    stencil_reference: u32,
}

impl Default for PassOptions<'_> {
    fn default() -> Self {
        Self {
            label: None,
            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
            viewport: None,
            scissor: None,
            depth_stencil: None,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(0),
                store: wgpu::StoreOp::Store,
            }),
            depth_requested: false,
            stencil_requested: false,
            stencil_reference: 0,
        }
    }
}

/// Configuration for a frame's next color pass, before command recording begins.
///
/// Created by [`crate::Frame::render_pass`] or [`crate::Frame::render_to`]. Defaults clear to transparent black,
/// store the results, and use the full attachment for viewport and scissor. These
/// defaults are the same for every pass; use [`Self::load`] to preserve earlier work.
/// Attached depth defaults to clear 1.0 and stencil to clear 0, both stored.
/// Their load/store operations are independent of color and each other.
/// No GPU commands are recorded until [`Self::begin`]. Dropping the builder leaves
/// the frame unchanged. Configuration borrows the frame and requires no heap allocation.
#[derive(Debug)]
#[must_use = "call begin to record a pass; dropping this builder does nothing"]
pub struct RenderPassBuilder<'frame> {
    encoder: &'frame mut wgpu::CommandEncoder,
    attachment: Result<ColorAttachment<'frame>, Error>,
    graphics: &'frame crate::GraphicsContext,
    initialization: Initialization<'frame>,
    options: PassOptions<'frame>,
    writes: Option<&'frame mut AttachmentWrites>,
}

impl<'frame> RenderPassBuilder<'frame> {
    pub(crate) fn new(
        encoder: &'frame mut wgpu::CommandEncoder,
        graphics: &'frame crate::GraphicsContext,
        attachment: ColorAttachment<'frame>,
        initialized: &'frame mut bool,
    ) -> Self {
        Self {
            encoder,
            attachment: Ok(attachment),
            graphics,
            initialization: Initialization::Surface(initialized),
            options: PassOptions::default(),
            writes: None,
        }
    }

    pub(crate) fn for_framebuffer(
        encoder: &'frame mut wgpu::CommandEncoder,
        graphics: &'frame crate::GraphicsContext,
        framebuffer: &'frame Framebuffer,
        writes: &'frame mut AttachmentWrites,
    ) -> Self {
        let attachment = if !graphics.same_device(&framebuffer.graphics) {
            Err(Error::DeviceMismatch)
        } else {
            framebuffer.attachment()
        };
        Self {
            encoder,
            attachment,
            graphics,
            initialization: Initialization::Framebuffer(&framebuffer.initialized),
            options: PassOptions {
                depth_stencil: framebuffer.depth_stencil.as_ref(),
                ..Default::default()
            },
            writes: Some(writes),
        }
    }

    pub(crate) fn with_depth_stencil(
        mut self,
        attachment: Option<&'frame DepthStencilAttachment>,
        writes: &'frame mut AttachmentWrites,
    ) -> Self {
        self.options.depth_stencil = attachment;
        self.writes = Some(writes);
        self
    }

    /// Clears depth to a finite value in zero to one and stores it for later passes.
    ///
    /// Defaults to 1.0 for conventional `Less` depth testing. Reversed-Z shaders
    /// can clear to 0.0 and use `Greater`. Requires an attachment with depth.
    pub fn clear_depth(mut self, depth: f32) -> Self {
        self.options.depth_requested = true;
        self.options.depth_ops = Some(wgpu::Operations {
            load: wgpu::LoadOp::Clear(depth),
            store: wgpu::StoreOp::Store,
        });
        self
    }

    /// Loads and stores initialized depth, independently of the color load operation.
    pub fn load_depth(mut self) -> Self {
        self.options.depth_requested = true;
        self.options.depth_ops = Some(wgpu::Operations {
            load: wgpu::LoadOp::Load,
            store: wgpu::StoreOp::Store,
        });
        self
    }

    /// Clears stencil to an integer mask and stores it. Defaults to zero.
    pub fn clear_stencil(mut self, stencil: u32) -> Self {
        self.options.stencil_requested = true;
        self.options.stencil_ops = Some(wgpu::Operations {
            load: wgpu::LoadOp::Clear(stencil),
            store: wgpu::StoreOp::Store,
        });
        self
    }

    /// Loads and stores initialized stencil, independently of color and depth.
    pub fn load_stencil(mut self) -> Self {
        self.options.stencil_requested = true;
        self.options.stencil_ops = Some(wgpu::Operations {
            load: wgpu::LoadOp::Load,
            store: wgpu::StoreOp::Store,
        });
        self
    }

    /// Sets raw depth load/store operations; `None` makes depth read-only.
    ///
    /// Loading or read-only access requires a stored clear in an earlier submitted
    /// recording or this frame. `Discard` invalidates depth for later wrapped loads.
    /// Materials in read-only passes must disable depth writes.
    pub fn depth_ops(mut self, ops: Option<wgpu::Operations<f32>>) -> Self {
        self.options.depth_requested = true;
        self.options.depth_ops = ops;
        self
    }

    /// Sets raw stencil load/store operations; `None` makes stencil read-only.
    ///
    /// Loading/read-only access requires initialized contents. `Discard` invalidates
    /// subsequent wrapped loads. Materials must not write a read-only stencil aspect.
    pub fn stencil_ops(mut self, ops: Option<wgpu::Operations<u32>>) -> Self {
        self.options.stencil_requested = true;
        self.options.stencil_ops = ops;
        self
    }

    /// Sets the reference used by stencil comparisons and `Replace` operations.
    ///
    /// Defaults to zero. This is per-pass dynamic state, not part of a material or
    /// pipeline cache key. Use the pass setter to change it between draws.
    pub fn stencil_reference(mut self, reference: u32) -> Self {
        self.options.stencil_reference = reference;
        self
    }

    /// Sets a debug label for the pass.
    pub fn label(mut self, label: &'frame str) -> Self {
        self.options.label = Some(label);
        self
    }

    /// Selects a clear operation using linear RGBA, replacing any previous load.
    pub fn clear_color(mut self, color: wgpu::Color) -> Self {
        self.options.load = wgpu::LoadOp::Clear(color);
        self
    }

    /// Selects loading existing contents, replacing any previous clear operation.
    ///
    /// A surface needs an earlier clear in this frame. A framebuffer may also
    /// load contents initialized in a previously submitted recording.
    pub fn load(mut self) -> Self {
        self.options.load = wgpu::LoadOp::Load;
        self
    }

    /// Sets the initial viewport in physical pixels and its depth range.
    ///
    /// Parameters follow [`wgpu::RenderPass::set_viewport`]. The default depth range
    /// is zero to one. Validation occurs at [`Self::begin`].
    pub fn viewport(
        mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        min_depth: f32,
        max_depth: f32,
    ) -> Self {
        self.options.viewport = Some([x, y, width, height, min_depth, max_depth]);
        self
    }

    /// Sets the initial scissor rectangle in physical pixels.
    ///
    /// The rectangle must fit the attachment. Zero dimensions clip all drawing.
    /// Validation occurs at [`Self::begin`].
    pub fn scissor_rect(mut self, x: u32, y: u32, width: u32, height: u32) -> Self {
        self.options.scissor = Some([x, y, width, height]);
        self
    }

    /// Validates configuration and starts a scoped color pass.
    ///
    /// Every pass stores its results. With MSAA enabled, this preserves individual
    /// samples and resolves to the single-sampled output. A clear-only pass is valid.
    /// Dropping the returned pass ends recording; the frame still owns submission.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UninitializedFrame`] for an initial surface load,
    /// [`Error::UninitializedFramebuffer`] for loading unsubmitted framebuffer contents,
    /// [`Error::TargetSuspended`] for a zero-sized framebuffer,
    /// [`Error::DeviceMismatch`] for a framebuffer on another device,
    /// [`Error::InvalidClearColor`] for nonfinite clear components,
    /// [`Error::InvalidViewport`] for invalid viewport values, or
    /// [`Error::InvalidScissorRect`] for a rectangle outside the attachment.
    /// Depth/stencil configuration can return [`Error::MissingDepthAttachment`],
    /// [`Error::MissingStencilAttachment`], [`Error::InvalidClearDepth`],
    /// [`Error::UninitializedDepth`], or [`Error::UninitializedStencil`].
    /// Rejected configuration records no commands and leaves the frame unchanged.
    pub fn begin(mut self) -> Result<RenderPass<'frame>, Error> {
        let attachment = self.attachment?;
        validate_depth_stencil_options(&self.options)?;
        if matches!(self.options.load, wgpu::LoadOp::Load) {
            match &self.initialization {
                Initialization::Surface(initialized) if !**initialized => {
                    return Err(Error::UninitializedFrame);
                }
                Initialization::Framebuffer(state)
                    if !self.writes.as_ref().unwrap().is_initialized(state) =>
                {
                    return Err(Error::UninitializedFramebuffer);
                }
                _ => {}
            }
        }
        if let Some(depth_stencil) = self.options.depth_stencil {
            let writes = self.writes.as_ref().unwrap();
            let format = depth_stencil.texture.format();
            if format.has_depth_aspect()
                && needs_initialized(self.options.depth_ops)
                && !writes.is_initialized(&depth_stencil.depth_initialized)
            {
                return Err(Error::UninitializedDepth);
            }
            if format.has_stencil_aspect()
                && needs_initialized(self.options.stencil_ops)
                && !writes.is_initialized(&depth_stencil.stencil_initialized)
            {
                return Err(Error::UninitializedStencil);
            }
        }
        let depth_stencil = self.options.depth_stencil;
        let depth_ops = self.options.depth_ops;
        let stencil_ops = self.options.stencil_ops;
        let pass = RenderPass::new(self.encoder, self.graphics, attachment, self.options)?;
        match self.initialization {
            Initialization::Surface(initialized) => *initialized = true,
            Initialization::Framebuffer(state) => self.writes.as_mut().unwrap().record(state, true),
        }
        if let Some(attachment) = depth_stencil {
            let writes = self.writes.as_mut().unwrap();
            let format = attachment.texture.format();
            if format.has_depth_aspect()
                && let Some(ops) = depth_ops
            {
                writes.record(
                    &attachment.depth_initialized,
                    ops.store == wgpu::StoreOp::Store,
                );
            }
            if format.has_stencil_aspect()
                && let Some(ops) = stencil_ops
            {
                writes.record(
                    &attachment.stencil_initialized,
                    ops.store == wgpu::StoreOp::Store,
                );
            }
        }
        Ok(pass)
    }
}

/// A scoped color render pass that accepts drawing from multiple renderers.
///
/// Created by [`RenderPassBuilder::begin`]. Dropping the pass ends command recording
/// for that pass; submission and presentation remain the frame's responsibility.
/// The pass borrows the frame, preventing another pass or encoder access until it ends.
/// Viewport, scissor, and dynamic stencil reference belong to this pass and
/// persist across mesh draws. Depth/stencil tests and writes belong to materials.
#[derive(Debug)]
#[must_use = "keep the pass in scope while recording draws"]
pub struct RenderPass<'frame> {
    pub(crate) inner: wgpu::RenderPass<'frame>,
    graphics: &'frame crate::GraphicsContext,
    format: wgpu::TextureFormat,
    size: [u32; 2],
    sample_count: u32,
    viewport: [f32; 6],
    scissor: [u32; 4],
    depth_stencil_format: Option<wgpu::TextureFormat>,
    depth_read_only: bool,
    stencil_read_only: bool,
    stencil_reference: u32,
    color_texture: wgpu::Texture,
    resolve_texture: Option<wgpu::Texture>,
}

impl<'frame> RenderPass<'frame> {
    pub(crate) fn new(
        encoder: &'frame mut wgpu::CommandEncoder,
        graphics: &'frame crate::GraphicsContext,
        attachment: ColorAttachment<'_>,
        options: PassOptions<'_>,
    ) -> Result<Self, Error> {
        let device = graphics.device();
        validate_depth_stencil_options(&options)?;
        let ColorAttachment {
            view,
            resolve_target,
            format,
            size,
            sample_count,
        } = attachment;
        if let wgpu::LoadOp::Clear(color) = options.load
            && ![color.r, color.g, color.b, color.a]
                .iter()
                .all(|value| value.is_finite())
        {
            return Err(Error::InvalidClearColor);
        }
        let viewport =
            options
                .viewport
                .unwrap_or([0.0, 0.0, size[0] as f32, size[1] as f32, 0.0, 1.0]);
        let scissor = options.scissor.unwrap_or([0, 0, size[0], size[1]]);
        validate_viewport(device, viewport)?;
        validate_scissor(size, scissor)?;
        let attachments = [Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target,
            ops: wgpu::Operations {
                load: options.load,
                store: wgpu::StoreOp::Store,
            },
        })];
        let depth_stencil_format = options
            .depth_stencil
            .map(|attachment| attachment.texture.format());
        let depth_stencil_attachment =
            options
                .depth_stencil
                .map(|attachment| wgpu::RenderPassDepthStencilAttachment {
                    view: &attachment.view,
                    depth_ops: depth_stencil_format
                        .filter(|format| format.has_depth_aspect())
                        .and(options.depth_ops),
                    stencil_ops: depth_stencil_format
                        .filter(|format| format.has_stencil_aspect())
                        .and(options.stencil_ops),
                });
        let inner = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: options.label,
            color_attachments: &attachments,
            depth_stencil_attachment,
            ..Default::default()
        });
        let mut pass = Self {
            inner,
            graphics,
            format,
            size,
            sample_count,
            viewport,
            scissor,
            depth_stencil_format,
            depth_read_only: options.depth_ops.is_none(),
            stencil_read_only: options.stencil_ops.is_none(),
            stencil_reference: options.stencil_reference,
            color_texture: view.texture().clone(),
            resolve_texture: resolve_target.map(|view| view.texture().clone()),
        };
        pass.apply_raster_state();
        Ok(pass)
    }

    pub(crate) fn same_device(&self, graphics: &crate::GraphicsContext) -> bool {
        self.graphics.same_device(graphics)
    }

    pub(crate) fn uses_color_texture(&self, texture: &wgpu::Texture) -> bool {
        &self.color_texture == texture || self.resolve_texture.as_ref() == Some(texture)
    }

    /// Sets the viewport for subsequent draws, in physical pixels.
    ///
    /// Values must be finite, dimensions nonnegative and within device limits,
    /// positions within wgpu's device viewport bounds, and `0 <= min_depth <=
    /// max_depth <= 1`. A viewport may extend beyond the attachment; scissoring
    /// limits rasterization to its bounds. Zero dimensions produce no fragments.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidViewport`] without changing state for invalid values.
    pub fn set_viewport(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        min_depth: f32,
        max_depth: f32,
    ) -> Result<(), Error> {
        let viewport = [x, y, width, height, min_depth, max_depth];
        validate_viewport(self.graphics.device(), viewport)?;
        self.viewport = viewport;
        self.apply_raster_state();
        Ok(())
    }

    /// Sets the scissor rectangle for subsequent draws, in physical pixels.
    ///
    /// The rectangle must fit the attachment; zero dimensions clip all drawing.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidScissorRect`] without changing state for invalid bounds.
    pub fn set_scissor_rect(
        &mut self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Result<(), Error> {
        let scissor = [x, y, width, height];
        validate_scissor(self.size, scissor)?;
        self.scissor = scissor;
        self.apply_raster_state();
        Ok(())
    }

    /// Changes stencil comparisons and replacement values for subsequent draws.
    ///
    /// Mesh renderers restore this wrapped state on every draw, along with viewport
    /// and scissor. Changing the reference creates no pipeline or GPU resource.
    pub fn set_stencil_reference(&mut self, reference: u32) {
        self.stencil_reference = reference;
        if self
            .depth_stencil_format
            .is_some_and(|format| format.has_stencil_aspect())
        {
            self.inner.set_stencil_reference(reference);
        }
    }

    /// Returns the optional depth/stencil format required by compatible pipelines.
    pub fn depth_stencil_format(&self) -> Option<wgpu::TextureFormat> {
        self.depth_stencil_format
    }

    /// Reports whether an attached depth aspect is read-only in this pass.
    pub fn depth_read_only(&self) -> bool {
        self.depth_stencil_format
            .is_some_and(|format| format.has_depth_aspect())
            && self.depth_read_only
    }

    /// Reports whether an attached stencil aspect is read-only in this pass.
    pub fn stencil_read_only(&self) -> bool {
        self.depth_stencil_format
            .is_some_and(|format| format.has_stencil_aspect())
            && self.stencil_read_only
    }

    /// Returns the color attachment format used to select compatible pipelines.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Returns the color attachment sample count required by compatible pipelines.
    ///
    /// Custom renderers must use this count in [`wgpu::MultisampleState`]. The
    /// built-in [`crate::MeshRenderer`] selects it automatically.
    pub fn sample_count(&self) -> u32 {
        self.sample_count
    }

    /// Returns the attachment's physical pixel dimensions.
    pub fn size(&self) -> [u32; 2] {
        self.size
    }

    /// Returns the device that all resources used in this pass must belong to.
    pub fn device(&self) -> &wgpu::Device {
        self.graphics.device()
    }

    /// Borrows the underlying wgpu pass for application-defined rendering.
    ///
    /// Use pipelines matching [`Self::format`], [`Self::sample_count`],
    /// [`Self::depth_stencil_format`], and aspect read-only flags, and
    /// resources from [`Self::device`]. Raw changes
    /// persist in wgpu's state but do not update this wrapper's viewport, scissor,
    /// or stencil reference.
    /// [`crate::MeshRenderer`] reapplies the wrapper's chosen viewport and scissor
    /// and stencil reference before each draw. Use the wrapped setters to control
    /// mesh rasterization and stencil comparisons.
    /// Custom renderers are responsible for the GPU state their draws need.
    pub fn as_wgpu(&mut self) -> &mut wgpu::RenderPass<'frame> {
        &mut self.inner
    }

    pub(crate) fn apply_raster_state(&mut self) {
        let [x, y, width, height, min_depth, max_depth] = self.viewport;
        self.inner
            .set_viewport(x, y, width, height, min_depth, max_depth);
        let [x, y, width, height] = self.scissor;
        self.inner.set_scissor_rect(x, y, width, height);
        if self
            .depth_stencil_format
            .is_some_and(|format| format.has_stencil_aspect())
        {
            self.inner.set_stencil_reference(self.stencil_reference);
        }
    }
}

fn validate_viewport(device: &wgpu::Device, viewport: [f32; 6]) -> Result<(), Error> {
    let [x, y, width, height, min_depth, max_depth] = viewport;
    let max = device.limits().max_texture_dimension_2d as f32;
    let bound = max * 2.0;
    if !viewport.iter().all(|value| value.is_finite())
        || width < 0.0
        || height < 0.0
        || width > max
        || height > max
        || x < -bound
        || y < -bound
        || x + width > bound - 1.0
        || y + height > bound - 1.0
        || !(0.0..=1.0).contains(&min_depth)
        || !(0.0..=1.0).contains(&max_depth)
        || min_depth > max_depth
    {
        return Err(Error::InvalidViewport);
    }
    Ok(())
}

fn validate_scissor(size: [u32; 2], scissor: [u32; 4]) -> Result<(), Error> {
    let [x, y, width, height] = scissor;
    if x.checked_add(width).is_none_or(|right| right > size[0])
        || y.checked_add(height).is_none_or(|bottom| bottom > size[1])
    {
        return Err(Error::InvalidScissorRect);
    }
    Ok(())
}

fn needs_initialized<T>(ops: Option<wgpu::Operations<T>>) -> bool {
    ops.is_none_or(|ops| matches!(ops.load, wgpu::LoadOp::Load))
}

fn validate_depth_stencil_options(options: &PassOptions<'_>) -> Result<(), Error> {
    let format = options
        .depth_stencil
        .map(|attachment| attachment.texture.format());
    if options.depth_requested && !format.is_some_and(|format| format.has_depth_aspect()) {
        return Err(Error::MissingDepthAttachment);
    }
    if options.stencil_requested && !format.is_some_and(|format| format.has_stencil_aspect()) {
        return Err(Error::MissingStencilAttachment);
    }
    if format.is_some_and(|format| format.has_depth_aspect())
        && let Some(wgpu::Operations {
            load: wgpu::LoadOp::Clear(depth),
            ..
        }) = options.depth_ops
        && (!depth.is_finite() || !(0.0..=1.0).contains(&depth))
    {
        return Err(Error::InvalidClearDepth);
    }
    Ok(())
}
