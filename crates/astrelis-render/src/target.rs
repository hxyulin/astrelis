//! Render target abstraction for unified surface/framebuffer rendering.

use crate::framebuffer::Framebuffer;

/// A render target that can be either a window surface or an offscreen framebuffer.
///
/// This enum simplifies render pass setup by providing a unified interface
/// for different rendering destinations.
#[derive(Debug, Clone, Copy)]
#[derive(Default)]
pub enum RenderTarget<'a> {
    /// Render to the window surface.
    ///
    /// The surface view is obtained from the FrameContext during render pass creation.
    #[default]
    Surface,

    /// Render to an offscreen framebuffer.
    ///
    /// The framebuffer manages its own color, depth, and MSAA textures.
    Framebuffer(&'a Framebuffer),
}

impl<'a> RenderTarget<'a> {
    /// Check if this target is a surface.
    pub fn is_surface(&self) -> bool {
        matches!(self, RenderTarget::Surface)
    }

    /// Check if this target is a framebuffer.
    pub fn is_framebuffer(&self) -> bool {
        matches!(self, RenderTarget::Framebuffer(_))
    }

    /// Get the framebuffer if this is a framebuffer target.
    pub fn framebuffer(&self) -> Option<&'a Framebuffer> {
        match self {
            RenderTarget::Framebuffer(fb) => Some(fb),
            _ => None,
        }
    }

    /// Get the texture format for this target.
    ///
    /// For framebuffers, returns the framebuffer's format.
    /// For surfaces, returns None (format must be obtained from surface config).
    pub fn format(&self) -> Option<wgpu::TextureFormat> {
        match self {
            RenderTarget::Surface => None,
            RenderTarget::Framebuffer(fb) => Some(fb.format()),
        }
    }

    /// Get the sample count for this target.
    ///
    /// For framebuffers, returns the framebuffer's sample count.
    /// For surfaces, returns 1 (surfaces don't support MSAA directly).
    pub fn sample_count(&self) -> u32 {
        match self {
            RenderTarget::Surface => 1,
            RenderTarget::Framebuffer(fb) => fb.sample_count(),
        }
    }

    /// Check if this target has MSAA enabled.
    pub fn has_msaa(&self) -> bool {
        self.sample_count() > 1
    }

    /// Check if this target has a depth buffer.
    pub fn has_depth(&self) -> bool {
        match self {
            RenderTarget::Surface => false,
            RenderTarget::Framebuffer(fb) => fb.has_depth(),
        }
    }

    /// Get the depth view if available.
    pub fn depth_view(&self) -> Option<&wgpu::TextureView> {
        match self {
            RenderTarget::Surface => None,
            RenderTarget::Framebuffer(fb) => fb.depth_view(),
        }
    }
}


impl<'a> From<&'a Framebuffer> for RenderTarget<'a> {
    fn from(fb: &'a Framebuffer) -> Self {
        RenderTarget::Framebuffer(fb)
    }
}
