use crate::Error;

#[derive(Debug)]
pub(crate) struct PassOptions<'label> {
    pub(crate) label: Option<&'label str>,
    pub(crate) load: wgpu::LoadOp<wgpu::Color>,
    viewport: Option<[f32; 6]>,
    scissor: Option<[u32; 4]>,
}

impl Default for PassOptions<'_> {
    fn default() -> Self {
        Self {
            label: None,
            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
            viewport: None,
            scissor: None,
        }
    }
}

/// Configuration for a frame's next color pass, before command recording begins.
///
/// Created by [`crate::Frame::render_pass`]. Defaults clear to transparent black,
/// store the results, and use the full attachment for viewport and scissor. These
/// defaults are the same for every pass; use [`Self::load`] to preserve earlier work.
/// No GPU commands are recorded until [`Self::begin`]. Dropping the builder leaves
/// the frame unchanged. Configuration borrows the frame and requires no heap allocation.
#[derive(Debug)]
#[must_use = "call begin to record a pass; dropping this builder does nothing"]
pub struct RenderPassBuilder<'frame> {
    encoder: &'frame mut wgpu::CommandEncoder,
    view: &'frame wgpu::TextureView,
    device: &'frame wgpu::Device,
    format: wgpu::TextureFormat,
    size: [u32; 2],
    initialized: &'frame mut bool,
    options: PassOptions<'frame>,
}

impl<'frame> RenderPassBuilder<'frame> {
    pub(crate) fn new(
        encoder: &'frame mut wgpu::CommandEncoder,
        view: &'frame wgpu::TextureView,
        device: &'frame wgpu::Device,
        format: wgpu::TextureFormat,
        size: [u32; 2],
        initialized: &'frame mut bool,
    ) -> Self {
        Self {
            encoder,
            view,
            device,
            format,
            size,
            initialized,
            options: PassOptions::default(),
        }
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
    /// A previous pass must have initialized this frame before loading is allowed.
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
    /// Every pass stores its results. A clear-only pass with no draws is valid.
    /// Dropping the returned pass ends recording; the frame still owns submission.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UninitializedFrame`] for an initial load,
    /// [`Error::InvalidClearColor`] for nonfinite clear components,
    /// [`Error::InvalidViewport`] for invalid viewport values, or
    /// [`Error::InvalidScissorRect`] for a rectangle outside the attachment.
    /// Rejected configuration records no commands and leaves the frame unchanged.
    pub fn begin(self) -> Result<RenderPass<'frame>, Error> {
        if !*self.initialized && matches!(self.options.load, wgpu::LoadOp::Load) {
            return Err(Error::UninitializedFrame);
        }
        let pass = RenderPass::new(
            self.encoder,
            self.view,
            self.device,
            self.format,
            self.size,
            self.options,
        )?;
        *self.initialized = true;
        Ok(pass)
    }
}

/// A scoped color render pass that accepts drawing from multiple renderers.
///
/// Created by [`RenderPassBuilder::begin`]. Dropping the pass ends command recording
/// for that pass; submission and presentation remain the frame's responsibility.
/// The pass borrows the frame, preventing another pass or encoder access until it ends.
/// Viewport and scissor settings belong to this pass and persist across mesh draws.
#[derive(Debug)]
#[must_use = "keep the pass in scope while recording draws"]
pub struct RenderPass<'frame> {
    pub(crate) inner: wgpu::RenderPass<'frame>,
    pub(crate) device: &'frame wgpu::Device,
    format: wgpu::TextureFormat,
    size: [u32; 2],
    viewport: [f32; 6],
    scissor: [u32; 4],
}

impl<'frame> RenderPass<'frame> {
    pub(crate) fn new(
        encoder: &'frame mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        device: &'frame wgpu::Device,
        format: wgpu::TextureFormat,
        size: [u32; 2],
        options: PassOptions<'_>,
    ) -> Result<Self, Error> {
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
            resolve_target: None,
            ops: wgpu::Operations {
                load: options.load,
                store: wgpu::StoreOp::Store,
            },
        })];
        let inner = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: options.label,
            color_attachments: &attachments,
            ..Default::default()
        });
        let mut pass = Self {
            inner,
            device,
            format,
            size,
            viewport,
            scissor,
        };
        pass.apply_raster_state();
        Ok(pass)
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
        validate_viewport(self.device, viewport)?;
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

    /// Returns the color attachment format used to select compatible pipelines.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Returns the attachment's physical pixel dimensions.
    pub fn size(&self) -> [u32; 2] {
        self.size
    }

    /// Returns the device that all resources used in this pass must belong to.
    pub fn device(&self) -> &wgpu::Device {
        self.device
    }

    /// Borrows the underlying wgpu pass for application-defined rendering.
    ///
    /// Use compatible pipelines and resources from [`Self::device`]. Raw changes
    /// persist in wgpu's state but do not update this wrapper's viewport or scissor.
    /// [`crate::MeshRenderer`] reapplies the wrapper's chosen viewport and scissor
    /// before each draw. Use the wrapped setters to control mesh rasterization.
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
