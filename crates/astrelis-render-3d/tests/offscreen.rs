//! Headless lit mesh renderer integration tests.

use astrelis_core::{
    color::Color,
    geometry::Size,
    math::{Mat4, Vec3},
};
use astrelis_gpu::{
    DeviceDescriptor, RequestAdapterOptions, TextureDescriptor, TextureDimension, TextureFormat,
    TextureUsages,
};
use astrelis_render::{Antialiasing, RenderTarget};
use astrelis_render_3d::{
    Camera3D, DrawList3D, Lighting, MaterialDescriptor, MeshDraw, Renderer3D, RendererOptions, cube,
};

#[test]
fn renders_lit_reverse_z_mesh_and_debug_lines() {
    pollster::block_on(async {
        let instance = astrelis_gpu_wgpu::create_instance(Default::default());
        let adapter = match instance
            .request_adapter(RequestAdapterOptions::default())
            .await
        {
            Ok(adapter) => adapter,
            Err(error) => {
                eprintln!("skipping 3D GPU test: {error}");
                return;
            }
        };
        let (device, queue) = adapter
            .request_device(DeviceDescriptor::default())
            .await
            .unwrap();
        let texture = device.create_texture(TextureDescriptor {
            label: Some("3D offscreen target".into()),
            size: astrelis_gpu::Extent3d::d2(96, 96),
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::RENDER_ATTACHMENT,
        });
        let target = RenderTarget {
            view: texture.create_view(Default::default()),
            allocation_size: Size::new(96, 96),
            render_size: Size::new(64, 48),
            scale_factor: 1.0,
            clear_color: Color::BLACK,
        };
        let mut renderer = Renderer3D::new(
            device.clone(),
            queue.clone(),
            RendererOptions {
                antialiasing: Antialiasing::Msaa4,
            },
        )
        .unwrap();
        let mesh = renderer.create_mesh(&cube(1.0)).unwrap();
        let material = renderer
            .create_material(MaterialDescriptor::default())
            .unwrap();
        let mut list = DrawList3D::new();
        list.draw_mesh(MeshDraw {
            mesh,
            material,
            transform: Mat4::IDENTITY,
            tint: Color::WHITE,
        });
        list.draw_grid(2, 1.0, Color::rgb(0.2, 0.2, 0.2));
        list.draw_axes(Mat4::IDENTITY, 1.0);
        let mut camera = Camera3D {
            position: Vec3::new(2.0, 2.0, 4.0),
            ..Default::default()
        };
        camera.look_at(Vec3::ZERO, Vec3::Y);
        let mut encoder = device.create_command_encoder(Default::default());
        let stats = renderer
            .render(&mut encoder, &target, &camera, &Lighting::default(), &list)
            .unwrap();
        assert_eq!(stats.instances, 1);
        assert!(stats.draw_calls >= 2);
        queue.submit([encoder.finish().unwrap()]).unwrap();
        device.poll(astrelis_gpu::PollMode::Wait).unwrap();
    });
}
