//! Headless sprite renderer integration tests.

use astrelis_core::{
    color::Color,
    geometry::Size,
    math::{Affine2, Vec2},
};
use astrelis_gpu::{
    DeviceDescriptor, RequestAdapterOptions, TextureDescriptor, TextureDimension, TextureFormat,
    TextureUsages,
};
use astrelis_render::{Antialiasing, RenderTarget};
use astrelis_render_2d::{
    Camera2D, DrawList2D, Renderer2D, RendererOptions, SpriteDraw, TextureOptions,
};

#[test]
fn renders_into_an_oversized_offscreen_allocation() {
    pollster::block_on(async {
        let instance = astrelis_gpu_wgpu::create_instance(Default::default());
        let adapter = match instance
            .request_adapter(RequestAdapterOptions::default())
            .await
        {
            Ok(adapter) => adapter,
            Err(error) => {
                eprintln!("skipping 2D GPU test: {error}");
                return;
            }
        };
        let (device, queue) = adapter
            .request_device(DeviceDescriptor::default())
            .await
            .unwrap();
        let target_texture = device.create_texture(TextureDescriptor {
            label: Some("2D offscreen target".into()),
            size: astrelis_gpu::Extent3d::d2(96, 96),
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::RENDER_ATTACHMENT,
        });
        let target = RenderTarget {
            view: target_texture.create_view(Default::default()),
            allocation_size: Size::new(96, 96),
            render_size: Size::new(64, 48),
            scale_factor: 1.0,
            clear_color: Color::BLACK,
        };
        let mut renderer = Renderer2D::new(
            device.clone(),
            queue.clone(),
            RendererOptions {
                antialiasing: Antialiasing::Msaa4,
            },
        )
        .unwrap();
        let texture = renderer
            .create_texture_rgba8(
                Size::new(1, 1),
                &[255, 255, 255, 255],
                TextureOptions::default(),
            )
            .unwrap();
        let mut list = DrawList2D::new();
        list.draw_sprite(SpriteDraw {
            texture,
            source: None,
            transform: Affine2::IDENTITY,
            size: Vec2::splat(20.0),
            pivot: Vec2::splat(0.5),
            tint: Color::RED,
            layer: 0,
        });
        let mut encoder = device.create_command_encoder(Default::default());
        let stats = renderer
            .render(&mut encoder, &target, &Camera2D::default(), &list)
            .unwrap();
        assert_eq!(stats.instances, 1);
        queue.submit([encoder.finish().unwrap()]).unwrap();
        device.poll(astrelis_gpu::PollMode::Wait).unwrap();
    });
}
