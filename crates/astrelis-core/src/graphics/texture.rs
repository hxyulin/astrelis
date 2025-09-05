pub struct Texture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl Texture {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn create_depth_texture(dev: &wgpu::Device, conf: &wgpu::SurfaceConfiguration) -> Self {
        let size = wgpu::Extent3d {
            width: conf.width.max(1),
            height: conf.height.max(1),
            depth_or_array_layers: 1,
        };

        let desc = wgpu::TextureDescriptor {
            label: None,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
            size,
        };
        let texture = dev.create_texture(&desc);

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = dev.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::Less),
            lod_min_clamp: 0.0,
            lod_max_clamp: 100.0,
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
        }
    }

    pub fn create_msaa_texture(
        dev: &wgpu::Device,
        conf: &wgpu::SurfaceConfiguration,
        sample_count: u32,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: conf.width.max(1),
            height: conf.height.max(1),
            depth_or_array_layers: 1,
        };

        let desc = wgpu::TextureDescriptor {
            label: Some("msaa_color_texture"),
            size,
            mip_level_count: 1,
            sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: conf.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };

        let texture = dev.create_texture(&desc);
        let view = texture.create_view(&Default::default());
        let sampler = dev.create_sampler(&Default::default());

        Self {
            texture,
            view,
            sampler,
        }
    }
}
