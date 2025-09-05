use puffin::profile_function;
pub use wgpu::{TextureFormat, TextureUsages};

use crate::{Extent3D, Window, graphics::texture::Texture};

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

    pub(crate) fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}

pub struct FramebufferOpts {
    /// The TextureFormat of the framebuffer, use None for the same format as the window
    pub format: Option<wgpu::TextureFormat>,
    pub depth: bool,
    pub sample_count: u32,
    pub extent: Extent3D<u32>,
    pub usage: TextureUsages,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ViewConfig {
    pub extent: wgpu::Extent3d,
    pub format: wgpu::TextureFormat,
    pub sample_count: u32,
}

impl Default for ViewConfig {
    fn default() -> Self {
        Self {
            extent: Default::default(),
            format: TextureFormat::Depth32Float,
            sample_count: 1,
        }
    }
}

pub struct Framebuffer {
    pub config: ViewConfig,
    pub(crate) color: TexView,
    pub(crate) depth: Option<TexView>,
    pub(crate) msaa: Option<TexView>,
}

impl Framebuffer {
    pub fn new(window: &Window, opts: FramebufferOpts) -> Self {
        Self::new_internal(&window.context.device, &window.context.config, opts)
    }

    pub fn new_internal(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        opts: FramebufferOpts,
    ) -> Self {
        profile_function!();

        let FramebufferOpts {
            format,
            depth,
            sample_count,
            extent,
            usage,
        } = opts;

        let size: wgpu::Extent3d = extent.into();
        let format = format.unwrap_or(config.format);

        let color = TexView::new(
            &device,
            &wgpu::TextureDescriptor {
                label: None,
                size,
                dimension: wgpu::TextureDimension::D2,
                format,
                mip_level_count: 1,
                sample_count: 1,
                view_formats: &[],
                usage,
            },
            &wgpu::TextureViewDescriptor::default(),
        );

        let depth = if depth {
            Some(TexView::new(
                &device,
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
            ))
        } else {
            None
        };

        let msaa = if sample_count > 1 {
            Some(TexView::new(
                &device,
                &wgpu::TextureDescriptor {
                    label: None,
                    size,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    mip_level_count: 1,
                    sample_count,
                    usage,
                    view_formats: &[],
                },
                &wgpu::TextureViewDescriptor::default(),
            ))
        } else {
            None
        };

        let config = ViewConfig {
            extent: extent.into(),
            format,
            sample_count,
        };

        Self {
            color,
            depth,
            msaa,
            config,
        }
    }
}

pub struct TargetConfig {
    pub size: (u32, u32),
    pub format: wgpu::TextureFormat,
    pub sample_count: u32,
}
