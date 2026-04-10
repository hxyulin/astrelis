//! wgpu surface implementation.

use std::sync::Arc;

use astrelis_gpu::error::GpuError;
use astrelis_gpu::id::TextureViewId;
use astrelis_gpu::surface::{GpuSurface, SurfaceConfiguration, SurfaceTexture};
use astrelis_gpu::types::{PresentMode, TextureFormat};

use crate::convert::types as conv;
use crate::device::WgpuDevice;

/// wgpu-backed presentation surface.
pub struct WgpuSurface {
    surface: wgpu::Surface<'static>,
    device: Arc<WgpuDevice>,
    capabilities: wgpu::SurfaceCapabilities,
    config: Option<SurfaceConfiguration>,
}

impl WgpuSurface {
    pub(crate) fn new(
        surface: wgpu::Surface<'static>,
        device: Arc<WgpuDevice>,
        capabilities: wgpu::SurfaceCapabilities,
    ) -> Self {
        Self {
            surface,
            device,
            capabilities,
            config: None,
        }
    }
}

impl GpuSurface for WgpuSurface {
    type Texture = WgpuSurfaceTexture;

    fn preferred_format(&self) -> TextureFormat {
        self.capabilities
            .formats
            .first()
            .map(|f| conv::texture_format_from_wgpu(*f))
            .unwrap_or(TextureFormat::Bgra8UnormSrgb)
    }

    fn supported_formats(&self) -> Vec<TextureFormat> {
        self.capabilities
            .formats
            .iter()
            .map(|f| conv::texture_format_from_wgpu(*f))
            .collect()
    }

    fn supported_present_modes(&self) -> Vec<PresentMode> {
        self.capabilities
            .present_modes
            .iter()
            .map(|m| conv::present_mode_from_wgpu(*m))
            .collect()
    }

    fn configure(&mut self, config: &SurfaceConfiguration) {
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
        self.surface.configure(&self.device.device, &wgpu_config);
        self.config = Some(config.clone());
    }

    fn acquire(&mut self) -> Result<Self::Texture, GpuError> {
        astrelis_profiling::profile_function!();
        let current = self.surface.get_current_texture();
        let surface_texture = match current {
            wgpu::CurrentSurfaceTexture::Success(tex)
            | wgpu::CurrentSurfaceTexture::Suboptimal(tex) => tex,
            wgpu::CurrentSurfaceTexture::Timeout => return Err(GpuError::Timeout),
            wgpu::CurrentSurfaceTexture::Outdated => return Err(GpuError::SurfaceOutdated),
            wgpu::CurrentSurfaceTexture::Lost => return Err(GpuError::SurfaceLost),
            // Window is minimized or fully behind another window — skip frame.
            wgpu::CurrentSurfaceTexture::Occluded => return Err(GpuError::Timeout),
            // Validation error (driver/API bug).
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(GpuError::SurfaceError("surface validation error".into()));
            }
        };
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let view_id = self.device.texture_views.insert(view);
        Ok(WgpuSurfaceTexture {
            output: Some(surface_texture),
            view_id,
        })
    }

    fn config(&self) -> Option<&SurfaceConfiguration> {
        self.config.as_ref()
    }
}

/// A texture acquired from a wgpu surface.
pub struct WgpuSurfaceTexture {
    output: Option<wgpu::SurfaceTexture>,
    view_id: TextureViewId,
}

impl SurfaceTexture for WgpuSurfaceTexture {
    fn view(&self) -> TextureViewId {
        self.view_id
    }

    fn present(mut self) {
        astrelis_profiling::profile_function!();
        if let Some(output) = self.output.take() {
            output.present();
        }
    }
}
