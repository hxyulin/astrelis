//! Render target abstraction for unified surface/framebuffer rendering.

use crate::framebuffer::Framebuffer;

/// Configuration for how to handle depth in a render pass.
#[derive(Debug, Clone, Copy, Default)]
pub enum DepthConfig<'a> {
    /// No depth testing.
    #[default]
    None,
    /// Use the provided depth view, loading existing values.
    Load(&'a wgpu::TextureView),
    /// Use the provided depth view, clearing to the specified value.
    Clear(&'a wgpu::TextureView, f32),
}

/// A render target that can be either a window surface or an offscreen framebuffer.
///
/// This enum simplifies render pass setup by providing a unified interface
/// for different rendering destinations.
#[derive(Debug, Clone, Copy, Default)]
pub enum RenderTarget<'a> {
    /// Render to the window surface.
    ///
    /// The surface view is obtained from the FrameContext during render pass creation.
    #[default]
    Surface,

    /// Render to the window surface with an attached depth buffer.
    ///
    /// This variant allows rendering to the surface while using a depth texture
    /// for z-ordering, which is essential for UI systems and 3D overlays.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let depth_view = frame.window_depth_view().expect("Window has depth");
    /// frame.clear_and_render(
    ///     RenderTarget::surface_with_depth_clear(&depth_view, 0.0),
    ///     Color::BLACK,
    ///     |pass| { /* rendering */ },
    /// );
    /// ```
    SurfaceWithDepth {
        /// The depth texture view to attach.
        depth_view: &'a wgpu::TextureView,
        /// How to handle the depth buffer: None = load, Some(v) = clear to v.
        clear_value: Option<f32>,
    },

    /// Render to an offscreen framebuffer.
    ///
    /// The framebuffer manages its own color, depth, and MSAA textures.
    Framebuffer(&'a Framebuffer),
}

impl<'a> RenderTarget<'a> {
    /// Create a surface target (no depth).
    pub fn surface() -> Self {
        RenderTarget::Surface
    }

    /// Create a surface target with a depth buffer that loads existing values.
    pub fn surface_with_depth(depth: &'a wgpu::TextureView) -> Self {
        RenderTarget::SurfaceWithDepth {
            depth_view: depth,
            clear_value: None,
        }
    }

    /// Create a surface target with a depth buffer that clears to the specified value.
    ///
    /// For reverse-Z depth (recommended), use 0.0 as the clear value.
    /// For standard depth, use 1.0 as the clear value.
    pub fn surface_with_depth_clear(depth: &'a wgpu::TextureView, clear: f32) -> Self {
        RenderTarget::SurfaceWithDepth {
            depth_view: depth,
            clear_value: Some(clear),
        }
    }

    /// Create a framebuffer target.
    pub fn framebuffer(fb: &'a Framebuffer) -> Self {
        RenderTarget::Framebuffer(fb)
    }

    /// Check if this target is a surface (with or without depth).
    pub fn is_surface(&self) -> bool {
        matches!(
            self,
            RenderTarget::Surface | RenderTarget::SurfaceWithDepth { .. }
        )
    }

    /// Check if this target is a framebuffer.
    pub fn is_framebuffer(&self) -> bool {
        matches!(self, RenderTarget::Framebuffer(_))
    }

    /// Get the framebuffer if this is a framebuffer target.
    pub fn framebuffer_ref(&self) -> Option<&'a Framebuffer> {
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
            RenderTarget::Surface | RenderTarget::SurfaceWithDepth { .. } => None,
            RenderTarget::Framebuffer(fb) => Some(fb.format()),
        }
    }

    /// Get the sample count for this target.
    ///
    /// For framebuffers, returns the framebuffer's sample count.
    /// For surfaces, returns 1 (surfaces don't support MSAA directly).
    pub fn sample_count(&self) -> u32 {
        match self {
            RenderTarget::Surface | RenderTarget::SurfaceWithDepth { .. } => 1,
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
            RenderTarget::SurfaceWithDepth { .. } => true,
            RenderTarget::Framebuffer(fb) => fb.has_depth(),
        }
    }

    /// Get the depth view if available.
    pub fn depth_view(&self) -> Option<&'a wgpu::TextureView> {
        match self {
            RenderTarget::Surface => None,
            RenderTarget::SurfaceWithDepth { depth_view, .. } => Some(depth_view),
            RenderTarget::Framebuffer(fb) => fb.depth_view(),
        }
    }

    /// Get the depth clear value if this target clears depth.
    pub fn depth_clear_value(&self) -> Option<f32> {
        match self {
            RenderTarget::SurfaceWithDepth { clear_value, .. } => *clear_value,
            _ => None,
        }
    }
}

impl<'a> From<&'a Framebuffer> for RenderTarget<'a> {
    fn from(fb: &'a Framebuffer) -> Self {
        RenderTarget::Framebuffer(fb)
    }
}
