use std::sync::{Arc, atomic::AtomicBool};

use crate::{Error, Frame, FrameError, GraphicsContext, pass::ColorAttachment, target};

/// Initial configuration of a persistent offscreen color framebuffer.
///
/// Pass to [`GraphicsContext::create_framebuffer`]. Options default to a
/// single-sampled, linear RGBA8 texture usable for rendering and shader sampling.
/// Include [`wgpu::TextureUsages::COPY_SRC`] to enable GPU copies or readback.
/// Depth/stencil storage is disabled by default; enable it with [`Self::depth_stencil`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use = "pass these options to create_framebuffer"]
pub struct FramebufferOptions {
    /// Physical width and height. Either being zero suspends the resource.
    pub size: [u32; 2],
    /// Color format, validated against the device's enabled features.
    pub format: wgpu::TextureFormat,
    /// Color samples per pixel; one disables MSAA.
    pub sample_count: u32,
    /// Optional depth-only, stencil-only, or combined attachment format.
    pub depth_stencil_format: Option<wgpu::TextureFormat>,
    /// Usages of depth/stencil storage, independent of color usages.
    /// Must include rendering; sampling or copies can be enabled when supported.
    pub depth_stencil_usage: wgpu::TextureUsages,
    /// Usages of the single-sampled output. Must include `RENDER_ATTACHMENT`.
    pub usage: wgpu::TextureUsages,
}

impl FramebufferOptions {
    /// Selects a size with linear RGBA8, one sample, and rendering/sampling usages.
    pub const fn new(width: u32, height: u32) -> Self {
        Self {
            size: [width, height],
            format: wgpu::TextureFormat::Rgba8Unorm,
            sample_count: 1,
            depth_stencil_format: None,
            depth_stencil_usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                .union(wgpu::TextureUsages::TEXTURE_BINDING),
        }
    }

    /// Selects the color format. Validation occurs during creation.
    pub const fn format(mut self, format: wgpu::TextureFormat) -> Self {
        self.format = format;
        self
    }

    /// Enables a target-owned depth/stencil attachment with rendering usages.
    ///
    /// Creation validates the format and intersects color and depth/stencil sample
    /// support. Storage automatically follows target size and MSAA changes.
    /// No depth/stencil texture is allocated when this option is absent.
    pub const fn depth_stencil(mut self, format: wgpu::TextureFormat) -> Self {
        self.depth_stencil_format = Some(format);
        self
    }

    /// Replaces depth/stencil usages. Include rendering and desired sampling/copy usages.
    pub const fn depth_stencil_usage(mut self, usage: wgpu::TextureUsages) -> Self {
        self.depth_stencil_usage = usage;
        self
    }

    /// Selects the initial sample count. Unsupported counts have no implicit fallback.
    pub const fn sample_count(mut self, count: u32) -> Self {
        self.sample_count = count;
        self
    }

    /// Replaces output usages. Include rendering and any desired sampling/copy usages.
    ///
    /// Transient attachments are rejected because framebuffer contents persist.
    pub const fn usage(mut self, usage: wgpu::TextureUsages) -> Self {
        self.usage = usage;
        self
    }
}

#[derive(Debug)]
struct Attachments {
    view: wgpu::TextureView,
    texture: wgpu::Texture,
    multisample_view: Option<wgpu::TextureView>,
}

/// A device-bound, reusable offscreen color texture with optional MSAA.
///
/// Create through [`GraphicsContext::create_framebuffer`]. Render independently
/// with [`Self::begin_frame`], or in another frame with [`Frame::render_to`]. All
/// passes use the same renderer API as surfaces. The resolved output stays
/// single-sampled, even with MSAA enabled. Optional depth/stencil storage has
/// the same dimensions and sample count as the render attachment and is not resolved.
///
/// Contents persist after submission. The first managed pass must clear; later
/// load passes may preserve earlier drawing, including across recordings. Dropping
/// a frame submits nothing and does not initialize newly recorded attachments.
///
/// Resize and sample-count changes replace attachments and invalidate prior
/// contents. Rebuild application bind groups using the new [`Self::color_view`].
/// Make these changes between recordings. Raw handles cloned before a change
/// still reference the old textures. Zero dimensions release all attachments.
#[derive(Debug)]
pub struct Framebuffer {
    pub(crate) graphics: GraphicsContext,
    options: FramebufferOptions,
    attachments: Option<Attachments>,
    supported_sample_counts: Vec<u32>,
    pub(crate) initialized: Arc<AtomicBool>,
    pub(crate) depth_stencil: Option<crate::depth_stencil::DepthStencilAttachment>,
}

impl Framebuffer {
    pub(crate) fn create(
        graphics: &GraphicsContext,
        options: FramebufferOptions,
    ) -> Result<Self, Error> {
        target::validate_size(graphics, options.size[0], options.size[1])?;
        let format = options.format;
        let features = target::format_features(graphics, format);
        if format.is_depth_stencil_format()
            || format.is_multi_planar_format()
            || !graphics
                .device()
                .features()
                .contains(format.required_features())
            || !features
                .allowed_usages
                .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
        {
            return Err(Error::UnsupportedColorFormat { format });
        }
        if !options
            .usage
            .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
            || options
                .usage
                .contains(wgpu::TextureUsages::TRANSIENT_ATTACHMENT)
            || !features.allowed_usages.contains(options.usage)
        {
            return Err(Error::InvalidFramebufferUsage {
                format,
                usage: options.usage,
            });
        }
        let mut supported_sample_counts = target::sample_counts(features);
        crate::depth_stencil::restrict_counts(
            graphics,
            &mut supported_sample_counts,
            options.depth_stencil_format,
            options.depth_stencil_usage,
        )?;
        target::validate_sample_count(format, options.sample_count, &supported_sample_counts)?;
        Ok(Self {
            graphics: graphics.clone(),
            attachments: allocate(graphics, options),
            options,
            supported_sample_counts,
            initialized: Arc::new(AtomicBool::new(false)),
            depth_stencil: crate::depth_stencil::allocate(
                graphics,
                options.size,
                options.sample_count,
                options.depth_stencil_format,
                options.depth_stencil_usage,
            ),
        })
    }

    /// Starts standalone command recording without acquiring a surface image.
    ///
    /// The frame exclusively borrows this resource until finish or drop. Finish
    /// submits once without presenting or waiting. A zero size returns
    /// [`FrameError::Suspended`]; there are no surface acquisition failures.
    pub fn begin_frame(&mut self) -> Result<Frame<'_, 'static>, FrameError> {
        self.begin_recording()
    }

    pub(crate) fn begin_recording<'window>(&mut self) -> Result<Frame<'_, 'window>, FrameError> {
        if self.options.size.contains(&0) {
            return Err(FrameError::Suspended);
        }
        Ok(Frame::for_framebuffer(self))
    }

    /// Replaces attachments at a changed physical size, discarding previous contents.
    ///
    /// A zero dimension releases attachments. Repeating the same size does nothing.
    /// Returns [`Error::InvalidTargetSize`] without changes if device limits are exceeded.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), Error> {
        target::validate_size(&self.graphics, width, height)?;
        if self.options.size == [width, height] {
            return Ok(());
        }
        let options = FramebufferOptions {
            size: [width, height],
            ..self.options
        };
        self.replace(options);
        Ok(())
    }

    /// Changes MSAA using cached capabilities, replacing attachments and contents.
    ///
    /// A repeated count does nothing. Returns [`Error::UnsupportedSampleCount`]
    /// without changes for unsupported counts. No surface or capability query is made.
    pub fn set_sample_count(&mut self, count: u32) -> Result<(), Error> {
        if count == self.options.sample_count {
            return Ok(());
        }
        target::validate_sample_count(self.options.format, count, &self.supported_sample_counts)?;
        self.replace(FramebufferOptions {
            sample_count: count,
            ..self.options
        });
        Ok(())
    }

    /// Returns physical dimensions, including zero while suspended.
    pub fn size(&self) -> [u32; 2] {
        self.options.size
    }

    /// Returns the fixed color format.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.options.format
    }

    /// Returns the current color attachment sample count.
    pub fn sample_count(&self) -> u32 {
        self.options.sample_count
    }

    /// Borrows sample counts cached during creation without allocation or queries.
    pub fn supported_sample_counts(&self) -> &[u32] {
        &self.supported_sample_counts
    }

    /// Returns the usages configured for the single-sampled output.
    pub fn usage(&self) -> wgpu::TextureUsages {
        self.options.usage
    }

    /// Returns the optional depth/stencil format, including while suspended.
    pub fn depth_stencil_format(&self) -> Option<wgpu::TextureFormat> {
        self.options.depth_stencil_format
    }

    /// Returns depth/stencil usages, or `None` when this attachment is disabled.
    pub fn depth_stencil_usage(&self) -> Option<wgpu::TextureUsages> {
        self.options
            .depth_stencil_format
            .map(|_| self.options.depth_stencil_usage)
    }

    /// Borrows depth/stencil storage; disabled attachments return `None`.
    ///
    /// Returns `TargetSuspended` when enabled at zero size. Sampling/copies require
    /// matching creation usages and aspect-compatible views. Storage has the target's
    /// sample count and is never resolved. Raw writes do not mark managed contents
    /// initialized. Resize/MSAA replaces storage and invalidates old bindings.
    pub fn depth_stencil_texture(&self) -> Result<Option<&wgpu::Texture>, Error> {
        if self.options.depth_stencil_format.is_none() {
            return Ok(None);
        }
        Ok(Some(
            &self
                .depth_stencil
                .as_ref()
                .ok_or(Error::TargetSuspended)?
                .texture,
        ))
    }

    /// Borrows the attachment view, with the same rules as its texture.
    pub fn depth_stencil_view(&self) -> Result<Option<&wgpu::TextureView>, Error> {
        if self.options.depth_stencil_format.is_none() {
            return Ok(None);
        }
        Ok(Some(
            &self
                .depth_stencil
                .as_ref()
                .ok_or(Error::TargetSuspended)?
                .view,
        ))
    }

    /// Borrows the resolved single-sampled texture for copies and low-level integration.
    ///
    /// Returns [`Error::TargetSuspended`] at zero size. Raw writes do not update
    /// managed initialization; record a wrapped clear before using a managed load.
    pub fn color_texture(&self) -> Result<&wgpu::Texture, Error> {
        Ok(&self
            .attachments
            .as_ref()
            .ok_or(Error::TargetSuspended)?
            .texture)
    }

    /// Borrows the resolved output view for shader sampling and custom GPU work.
    ///
    /// Returns [`Error::TargetSuspended`] at zero size. Sampling requires
    /// `TEXTURE_BINDING` in the creation usages. Never sample an output while
    /// rendering into that same framebuffer in the same pass.
    pub fn color_view(&self) -> Result<&wgpu::TextureView, Error> {
        Ok(&self
            .attachments
            .as_ref()
            .ok_or(Error::TargetSuspended)?
            .view)
    }

    pub(crate) fn attachment(&self) -> Result<ColorAttachment<'_>, Error> {
        let attachments = self.attachments.as_ref().ok_or(Error::TargetSuspended)?;
        Ok(ColorAttachment {
            view: attachments
                .multisample_view
                .as_ref()
                .unwrap_or(&attachments.view),
            resolve_target: attachments
                .multisample_view
                .as_ref()
                .map(|_| &attachments.view),
            format: self.options.format,
            size: self.options.size,
            sample_count: self.options.sample_count,
        })
    }

    fn replace(&mut self, options: FramebufferOptions) {
        self.attachments = allocate(&self.graphics, options);
        self.depth_stencil = crate::depth_stencil::allocate(
            &self.graphics,
            options.size,
            options.sample_count,
            options.depth_stencil_format,
            options.depth_stencil_usage,
        );
        self.options = options;
        // Pending recordings retain the old generation's marker, never this one.
        self.initialized = Arc::new(AtomicBool::new(false));
    }
}

impl From<Framebuffer> for crate::RenderTarget<'static> {
    fn from(framebuffer: Framebuffer) -> Self {
        Self::Framebuffer(framebuffer)
    }
}

fn allocate(graphics: &GraphicsContext, options: FramebufferOptions) -> Option<Attachments> {
    let [width, height] = options.size;
    if width == 0 || height == 0 {
        return None;
    }
    let texture = graphics.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("Astrelis framebuffer output"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: options.format,
        usage: options.usage,
        view_formats: &[],
    });
    Some(Attachments {
        view: texture.create_view(&Default::default()),
        texture,
        multisample_view: target::create_multisample_view(
            graphics,
            options.format,
            options.size,
            options.sample_count,
        ),
    })
}

#[cfg(test)]
#[path = "framebuffer_tests.rs"]
pub(crate) mod tests;
