use crate::Error;

/// A scoped color render pass that accepts drawing from multiple renderers.
///
/// Created by [`crate::Frame::begin_pass`]. Dropping the pass ends command recording
/// for that pass; submission and presentation remain the frame's responsibility.
/// The pass borrows the frame, preventing another pass or presentation until it ends.
#[derive(Debug)]
#[must_use = "keep the pass in scope while recording draws"]
pub struct RenderPass<'frame> {
    pub(crate) inner: wgpu::RenderPass<'frame>,
    pub(crate) device: &'frame wgpu::Device,
    format: wgpu::TextureFormat,
    size: [u32; 2],
}

impl<'frame> RenderPass<'frame> {
    pub(crate) fn new(
        encoder: &'frame mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        device: &'frame wgpu::Device,
        format: wgpu::TextureFormat,
        size: [u32; 2],
        load: wgpu::LoadOp<wgpu::Color>,
    ) -> Result<Self, Error> {
        if let wgpu::LoadOp::Clear(color) = load
            && ![color.r, color.g, color.b, color.a]
                .iter()
                .all(|value| value.is_finite())
        {
            return Err(Error::InvalidClearColor);
        }
        let attachments = [Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            },
        })];
        let inner = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Astrelis color pass"),
            color_attachments: &attachments,
            ..Default::default()
        });
        Ok(Self {
            inner,
            device,
            format,
            size,
        })
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
    /// Use compatible pipelines and resources from [`Self::device`]. Rendering code
    /// must set the GPU state it relies on; state changes persist between draws.
    /// The built-in mesh renderer restores its pipeline, geometry bindings, full
    /// viewport, and full scissor rectangle on each draw.
    pub fn as_wgpu(&mut self) -> &mut wgpu::RenderPass<'frame> {
        &mut self.inner
    }
}
