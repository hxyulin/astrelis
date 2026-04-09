//! Presentation surface traits and types.

use crate::error::GpuError;
use crate::id::TextureViewId;
use crate::types::{PresentMode, TextureFormat};

/// Configuration for a presentation surface.
#[derive(Clone, Debug)]
pub struct SurfaceConfiguration {
    /// Texture format for the surface.
    pub format: TextureFormat,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Presentation mode (vsync behavior).
    pub present_mode: PresentMode,
    /// Maximum number of frames the presentation engine may queue.
    pub desired_maximum_frame_latency: u32,
}

/// A texture acquired from a surface for the current frame.
///
/// Must be presented (via [`present`](SurfaceTexture::present)) or dropped
/// before the next frame can be acquired.
pub trait SurfaceTexture {
    /// Returns a view into this surface texture for use as a render target.
    fn view(&self) -> TextureViewId;

    /// Presents this texture to the surface, displaying it on screen.
    fn present(self);
}

/// A presentation surface tied to a window.
///
/// Manages the swap chain for presenting rendered frames to the screen.
pub trait GpuSurface {
    /// The surface texture type returned by [`acquire`](GpuSurface::acquire).
    type Texture: SurfaceTexture;

    /// Returns the preferred texture format for this surface.
    ///
    /// Using this format avoids costly format conversions on presentation.
    fn preferred_format(&self) -> TextureFormat;

    /// Returns all texture formats supported by this surface.
    fn supported_formats(&self) -> Vec<TextureFormat>;

    /// Returns all presentation modes supported by this surface.
    fn supported_present_modes(&self) -> Vec<PresentMode>;

    /// Configures (or reconfigures) the surface for rendering.
    ///
    /// Must be called before the first [`acquire`](GpuSurface::acquire), and
    /// again after window resize or [`GpuError::SurfaceOutdated`].
    fn configure(&mut self, config: &SurfaceConfiguration);

    /// Acquires the next texture for rendering.
    ///
    /// Returns [`GpuError::SurfaceOutdated`] if the surface needs
    /// reconfiguration (e.g., after a resize).
    fn acquire(&mut self) -> Result<Self::Texture, GpuError>;

    /// Returns the current configuration, if the surface has been configured.
    fn config(&self) -> Option<&SurfaceConfiguration>;
}
