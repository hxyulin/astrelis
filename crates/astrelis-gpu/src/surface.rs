//! Presentation surface types.

use crate::convert::types as conv;
use crate::device::GpuDevice;
use crate::error::GpuError;
use crate::resources::TextureView;
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

/// A presentation surface tied to a window.
///
/// Manages the swap chain for presenting rendered frames to the screen.
pub struct Surface {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    capabilities: wgpu::SurfaceCapabilities,
    config: Option<SurfaceConfiguration>,
}

impl Surface {
    pub(crate) fn new(
        surface: wgpu::Surface<'static>,
        device: &GpuDevice,
        capabilities: wgpu::SurfaceCapabilities,
    ) -> Self {
        Self {
            surface,
            device: device.device.clone(),
            capabilities,
            config: None,
        }
    }

    /// Returns the preferred texture format for this surface.
    ///
    /// Using this format avoids costly format conversions on presentation.
    pub fn preferred_format(&self) -> TextureFormat {
        self.capabilities
            .formats
            .first()
            .map(|f| conv::texture_format_from_wgpu(*f))
            .unwrap_or(TextureFormat::Bgra8UnormSrgb)
    }

    /// Returns all texture formats supported by this surface.
    pub fn supported_formats(&self) -> Vec<TextureFormat> {
        self.capabilities
            .formats
            .iter()
            .map(|f| conv::texture_format_from_wgpu(*f))
            .collect()
    }

    /// Returns all presentation modes supported by this surface.
    pub fn supported_present_modes(&self) -> Vec<PresentMode> {
        self.capabilities
            .present_modes
            .iter()
            .map(|m| conv::present_mode_from_wgpu(*m))
            .collect()
    }

    /// Configures (or reconfigures) the surface for rendering.
    ///
    /// Must be called before the first [`acquire`](Surface::acquire), and
    /// again after window resize or [`GpuError::SurfaceOutdated`].
    pub fn configure(&mut self, config: &SurfaceConfiguration) {
        astrelis_profiling::profile_function!();
        let wgpu_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: conv::texture_format(config.format),
            width: config.width,
            height: config.height,
            present_mode: conv::present_mode(config.present_mode),
            alpha_mode: self
                .capabilities
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
            desired_maximum_frame_latency: config.desired_maximum_frame_latency,
        };
        self.surface.configure(&self.device, &wgpu_config);
        self.config = Some(config.clone());
    }

    /// Acquires the next texture for rendering.
    ///
    /// Returns [`GpuError::SurfaceOutdated`] if the surface needs
    /// reconfiguration (e.g., after a resize).
    pub fn acquire(&mut self) -> Result<SurfaceFrame, GpuError> {
        astrelis_profiling::profile_function!();
        let current = self.surface.get_current_texture();
        let surface_texture = match current {
            wgpu::CurrentSurfaceTexture::Success(tex)
            | wgpu::CurrentSurfaceTexture::Suboptimal(tex) => tex,
            wgpu::CurrentSurfaceTexture::Timeout => return Err(GpuError::Timeout),
            wgpu::CurrentSurfaceTexture::Outdated => return Err(GpuError::SurfaceOutdated),
            wgpu::CurrentSurfaceTexture::Lost => return Err(GpuError::SurfaceLost),
            wgpu::CurrentSurfaceTexture::Occluded => return Err(GpuError::Timeout),
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(GpuError::SurfaceError("surface validation error".into()));
            }
        };
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        Ok(SurfaceFrame {
            output: Some(surface_texture),
            view: TextureView(view),
        })
    }

    /// Returns the current configuration, if the surface has been configured.
    pub fn config(&self) -> Option<&SurfaceConfiguration> {
        self.config.as_ref()
    }
}

/// A texture frame acquired from a surface for the current frame.
///
/// Must be presented (via [`present`](SurfaceFrame::present)) or dropped
/// before the next frame can be acquired.
pub struct SurfaceFrame {
    output: Option<wgpu::SurfaceTexture>,
    view: TextureView,
}

impl SurfaceFrame {
    /// Returns a reference to the texture view for use as a render target.
    pub fn view(&self) -> &TextureView {
        &self.view
    }

    /// Presents this texture to the surface, displaying it on screen.
    pub fn present(mut self) {
        astrelis_profiling::profile_function!();
        if let Some(output) = self.output.take() {
            output.present();
        }
    }
}
