use puffin::profile_function;
pub use wgpu::TextureUsages;

use crate::{Extent3D, Window};

pub struct Framebuffer {
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
}

impl Framebuffer {
    pub fn new(window: &Window, extent: Extent3D<u32>, usage: TextureUsages) -> Self {
        profile_function!();

        let texture = window
            .context
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Framebuffer Texture"),
                size: extent.into(),
                dimension: wgpu::TextureDimension::D2,
                // TODO: Make configurable?
                format: window.context.config.format,
                mip_level_count: 1,
                sample_count: window.context.sample_count,
                view_formats: &[],
                usage,
            });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self { texture, view }
    }
}

pub struct TargetConfig {
    pub size: (u32, u32),
    pub format: wgpu::TextureFormat,
    pub sample_count: u32,
}
