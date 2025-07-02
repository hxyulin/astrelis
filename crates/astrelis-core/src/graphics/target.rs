use puffin::profile_function;
pub use wgpu::TextureUsages;

use crate::{Extent3D, Window, graphics::texture::Texture};

pub enum TexKind {
    Color,
    Depth,
}

pub struct TexView {
    pub tex: wgpu::Texture,
    pub view: wgpu::TextureView,
}

impl TexView {
    pub fn new(
        device: &wgpu::Device,
        tex_desc: &wgpu::TextureDescriptor,
        view_desc: &wgpu::TextureViewDescriptor,
    ) -> Self {
        let tex = device.create_texture(tex_desc);
        let view = tex.create_view(view_desc);
        Self { tex, view }
    }
}

pub struct Framebuffer {
    pub(crate) color: TexView,
    pub(crate) depth: TexView,
}

impl Framebuffer {
    pub fn new(window: &Window, extent: Extent3D<u32>, usage: TextureUsages) -> Self {
        profile_function!();

        let size: wgpu::Extent3d = extent.into();
        let color = TexView::new(
            &window.context.device,
            &wgpu::TextureDescriptor {
                label: None,
                size,
                dimension: wgpu::TextureDimension::D2,
                format: window.context.config.format,
                mip_level_count: 1,
                sample_count: 1,
                view_formats: &[],
                usage,
            },
            &wgpu::TextureViewDescriptor::default(),
        );
        let depth = TexView::new(
            &window.context.device,
            &wgpu::TextureDescriptor {
                label: None,
                size,
                dimension: wgpu::TextureDimension::D2,
                format: Texture::DEPTH_FORMAT,
                mip_level_count: 1,
                sample_count: 1,
                usage,
                view_formats: &[],
            },
            &wgpu::TextureViewDescriptor::default(),
        );

        Self { color, depth }
    }
}

pub struct TargetConfig {
    pub size: (u32, u32),
    pub format: wgpu::TextureFormat,
    pub sample_count: u32,
}
