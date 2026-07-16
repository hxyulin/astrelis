//! Headless integration tests for the paint renderer.

use astrelis_core::{
    color::Color,
    geometry::{Point, Rect, Size},
};
use astrelis_gpu::{
    BufferDescriptor, BufferTextureCopy, BufferUsages, CommandEncoderDescriptor, DeviceDescriptor,
    Extent3d, MapMode, PollMode, RequestAdapterOptions, TextureCopy, TextureDescriptor,
    TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
};
use astrelis_paint::{Brush, FillRule, Painter, Path};
use astrelis_paint_gpu::{Antialiasing, RenderTarget, Renderer, RendererOptions};

fn gpu_test_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

#[test]
fn renders_solid_stencil_clip_and_reuses_mesh() {
    let _guard = gpu_test_lock().lock().expect("GPU test lock poisoned");
    pollster::block_on(async {
        let instance = astrelis_gpu_wgpu::create_instance(Default::default());
        let adapter = match instance
            .request_adapter(RequestAdapterOptions::default())
            .await
        {
            Ok(adapter) => adapter,
            Err(error) => {
                eprintln!("skipping paint GPU test: {error}");
                return;
            }
        };
        let (device, queue) = adapter
            .request_device(DeviceDescriptor::default())
            .await
            .expect("request device");
        let texture = device.create_texture(TextureDescriptor {
            label: Some("paint target".into()),
            size: Extent3d::d2(16, 16),
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
        });
        let view = texture.create_view(TextureViewDescriptor::default());

        let mut path = Path::builder();
        path.move_to(Point::new(0.0, 0.0)).unwrap();
        path.line_to(Point::new(16.0, 0.0)).unwrap();
        path.line_to(Point::new(16.0, 16.0)).unwrap();
        path.line_to(Point::new(0.0, 16.0)).unwrap();
        path.close().unwrap();
        let path = path.finish();
        let mut painter = Painter::new();
        painter.save();
        painter
            .clip_rect(Rect::from_xywh(4.5, 4.5, 7.0, 7.0))
            .unwrap();
        painter
            .fill_path(&path, FillRule::NonZero, Brush::Solid(Color::RED))
            .unwrap();
        painter.restore().unwrap();
        let list = painter.finish().unwrap();

        let mut renderer = Renderer::new(
            device.clone(),
            queue.clone(),
            RendererOptions {
                antialiasing: Antialiasing::None,
                ..Default::default()
            },
        )
        .expect("renderer");
        let render_target = || RenderTarget {
            view: view.clone(),
            format: TextureFormat::Rgba8Unorm,
            size: Size::new(16, 16),
            scale_factor: 1.0,
            clear_color: Color::BLACK,
        };
        let mut encoder = device.create_command_encoder(CommandEncoderDescriptor::default());
        let first = renderer
            .render(&mut encoder, &list, render_target())
            .expect("first paint render");
        queue
            .submit([encoder.finish().expect("finish first encoder")])
            .expect("submit first");
        let mut encoder = device.create_command_encoder(CommandEncoderDescriptor::default());
        let second = renderer
            .render(&mut encoder, &list, render_target())
            .expect("second paint render");
        assert_eq!(first.mesh_cache_misses, 1);
        assert_eq!(second.mesh_cache_hits, 1);

        let readback = device.create_buffer(BufferDescriptor {
            label: Some("paint readback".into()),
            size: 256 * 16,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder
            .copy_texture_to_buffer(
                &TextureCopy {
                    texture,
                    mip_level: 0,
                    origin: Default::default(),
                },
                &BufferTextureCopy {
                    buffer: readback.clone(),
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(16),
                },
                Extent3d::d2(16, 16),
            )
            .expect("copy target");
        queue
            .submit([encoder.finish().expect("finish second encoder")])
            .expect("submit second");
        let mapping = readback.map_async(MapMode::Read, 0..256 * 16);
        device.poll(PollMode::Wait).expect("wait");
        mapping.await.expect("map");
        let bytes = readback.read_mapped(0..256 * 16).expect("read");
        let pixel = |x: usize, y: usize| {
            let offset = y * 256 + x * 4;
            &bytes[offset..offset + 4]
        };
        assert_eq!(pixel(8, 8), [255, 0, 0, 255]);
        assert_eq!(pixel(2, 2), [0, 0, 0, 255]);
        readback.unmap();
    });
}
