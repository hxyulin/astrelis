//! Headless integration tests for the paint renderer.

use astrelis_core::{
    color::Color,
    geometry::{Point, Rect, Size},
    math::{Affine2, Vec2},
};
use astrelis_gpu::{
    BufferDescriptor, BufferTextureCopy, BufferUsages, CommandEncoderDescriptor, DeviceDescriptor,
    Extent3d, MapMode, PollMode, RequestAdapterOptions, TextureCopy, TextureDataLayout,
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
};
use astrelis_paint::{
    Brush, ExternalImage, FillRule, GradientStop, ImageOptions, LinearGradient, Painter, Path,
    RadialGradient,
};
use astrelis_paint_gpu::{Antialiasing, RenderTarget, Renderer, RendererOptions};
use astrelis_text::{FontDatabase, TextLayoutContext, TextLayoutRequest};

fn gpu_test_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

#[test]
fn external_images_render_replace_unregister_and_reject_wrong_device() {
    let _guard = gpu_test_lock().lock().expect("GPU test lock poisoned");
    pollster::block_on(async {
        let instance = astrelis_gpu_wgpu::create_instance(Default::default());
        let adapter = match instance
            .request_adapter(RequestAdapterOptions::default())
            .await
        {
            Ok(adapter) => adapter,
            Err(error) => {
                eprintln!("skipping external image GPU test: {error}");
                return;
            }
        };
        let (device, queue) = adapter
            .request_device(DeviceDescriptor::default())
            .await
            .expect("request first device");
        let (other_device, _other_queue) = adapter
            .request_device(DeviceDescriptor::default())
            .await
            .expect("request second device");

        let image = ExternalImage::new(Size::new(2, 2)).expect("external image token");
        let mut painter = Painter::new();
        painter
            .draw_external_image(
                &image,
                Rect::from_xywh(0.0, 0.0, 2.0, 2.0),
                ImageOptions::default(),
            )
            .expect("record external image");
        let list = painter.finish().expect("finish display list");
        let target = device.create_texture(TextureDescriptor {
            label: Some("external image paint target".into()),
            size: Extent3d::d2(2, 2),
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
        });
        let target_view = target.create_view(TextureViewDescriptor::default());
        let mut renderer = Renderer::new(
            device.clone(),
            queue.clone(),
            RendererOptions {
                antialiasing: Antialiasing::None,
                ..Default::default()
            },
        )
        .expect("renderer");

        let mut encoder = device.create_command_encoder(CommandEncoderDescriptor::default());
        let missing = renderer
            .render(
                &mut encoder,
                &list,
                RenderTarget {
                    view: target_view.clone(),
                    format: TextureFormat::Rgba8Unorm,
                    size: Size::new(2, 2),
                    scale_factor: 1.0,
                    clear_color: Color::BLACK,
                },
            )
            .expect_err("unregistered image must fail");
        assert!(missing.to_string().contains("register_external_image"));

        let wrong_texture = other_device.create_texture(TextureDescriptor {
            label: Some("wrong-device external image".into()),
            size: Extent3d::d2(2, 2),
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING,
        });
        let wrong = renderer
            .register_external_image(
                &image,
                wrong_texture.create_view(TextureViewDescriptor::default()),
            )
            .expect_err("cross-device registration must fail");
        assert!(wrong.to_string().contains("different device"));

        let multisampled = device.create_texture(TextureDescriptor {
            label: Some("incompatible multisampled external image".into()),
            size: Extent3d::d2(2, 2),
            mip_level_count: 1,
            sample_count: 4,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT,
        });
        let incompatible = renderer
            .register_external_image(
                &image,
                multisampled.create_view(TextureViewDescriptor::default()),
            )
            .expect_err("multisampled registration must fail");
        assert!(incompatible.to_string().contains("single-sampled 2D"));

        let first = solid_texture(&device, &queue, [255, 0, 0, 255]);
        renderer
            .register_external_image(&image, first.create_view(TextureViewDescriptor::default()))
            .expect("register red texture");
        assert_eq!(
            render_external_pixel(&device, &queue, &mut renderer, &list, &target, &target_view,)
                .await,
            [255, 0, 0, 255]
        );

        let replacement = solid_texture(&device, &queue, [0, 255, 0, 255]);
        renderer
            .register_external_image(
                &image,
                replacement.create_view(TextureViewDescriptor::default()),
            )
            .expect("replace with green texture");
        assert_eq!(
            render_external_pixel(&device, &queue, &mut renderer, &list, &target, &target_view,)
                .await,
            [0, 255, 0, 255]
        );

        assert!(renderer.unregister_external_image(&image));
        assert!(!renderer.unregister_external_image(&image));
        let mut encoder = device.create_command_encoder(CommandEncoderDescriptor::default());
        assert!(
            renderer
                .render(
                    &mut encoder,
                    &list,
                    RenderTarget {
                        view: target_view,
                        format: TextureFormat::Rgba8Unorm,
                        size: Size::new(2, 2),
                        scale_factor: 1.0,
                        clear_color: Color::BLACK,
                    },
                )
                .is_err()
        );
    });
}

fn solid_texture(
    device: &astrelis_gpu::Device,
    queue: &astrelis_gpu::Queue,
    color: [u8; 4],
) -> astrelis_gpu::Texture {
    let texture = device.create_texture(TextureDescriptor {
        label: Some("external image source".into()),
        size: Extent3d::d2(2, 2),
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
    });
    queue
        .write_texture(
            &TextureCopy {
                texture: texture.clone(),
                mip_level: 0,
                origin: Default::default(),
            },
            &color.repeat(4),
            TextureDataLayout {
                offset: 0,
                bytes_per_row: Some(8),
                rows_per_image: Some(2),
            },
            Extent3d::d2(2, 2),
        )
        .expect("upload external image source");
    texture
}

async fn render_external_pixel(
    device: &astrelis_gpu::Device,
    queue: &astrelis_gpu::Queue,
    renderer: &mut Renderer,
    list: &astrelis_paint::DisplayList,
    target: &astrelis_gpu::Texture,
    target_view: &astrelis_gpu::TextureView,
) -> [u8; 4] {
    let readback = device.create_buffer(BufferDescriptor {
        label: Some("external image readback".into()),
        size: 512,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(CommandEncoderDescriptor::default());
    renderer
        .render(
            &mut encoder,
            list,
            RenderTarget {
                view: target_view.clone(),
                format: TextureFormat::Rgba8Unorm,
                size: Size::new(2, 2),
                scale_factor: 1.0,
                clear_color: Color::BLACK,
            },
        )
        .expect("render external image");
    encoder
        .copy_texture_to_buffer(
            &TextureCopy {
                texture: target.clone(),
                mip_level: 0,
                origin: Default::default(),
            },
            &BufferTextureCopy {
                buffer: readback.clone(),
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(2),
            },
            Extent3d::d2(2, 2),
        )
        .expect("copy external image result");
    queue
        .submit([encoder.finish().expect("finish external image encoder")])
        .expect("submit external image render");
    let mapping = readback.map_async(MapMode::Read, 0..512);
    device
        .poll(PollMode::Wait)
        .expect("wait for external image");
    mapping.await.expect("map external image result");
    let bytes = readback
        .read_mapped(0..4)
        .expect("read external image pixel");
    let pixel = [bytes[0], bytes[1], bytes[2], bytes[3]];
    drop(bytes);
    readback.unmap();
    pixel
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
        let gradient = LinearGradient::new(
            Point::new(0.0, 0.0),
            Point::new(12.0, 0.0),
            [
                GradientStop {
                    offset: 0.0,
                    color: Color::RED,
                },
                GradientStop {
                    offset: 1.0,
                    color: Color::BLUE,
                },
            ],
        )
        .unwrap();
        painter.save();
        painter
            .transform(Affine2::from_translation(Vec2::new(2.0, 0.0)))
            .unwrap();
        painter
            .with_opacity(0.5, |painter| {
                painter.fill_rect(
                    Rect::from_xywh(0.0, 0.0, 12.0, 4.0),
                    Brush::LinearGradient(gradient),
                )
            })
            .unwrap();
        painter.restore().unwrap();
        painter.save();
        painter
            .clip_rect(Rect::from_xywh(4.5, 4.5, 7.0, 7.0))
            .unwrap();
        painter
            .fill_path(&path, FillRule::NonZero, Brush::Solid(Color::RED))
            .unwrap();
        painter.restore().unwrap();
        let radial = RadialGradient::new(
            Point::new(14.0, 14.0),
            2.0,
            [
                GradientStop {
                    offset: 0.0,
                    color: Color::WHITE,
                },
                GradientStop {
                    offset: 1.0,
                    color: Color::BLUE,
                },
            ],
        )
        .unwrap();
        painter
            .fill_ellipse(
                Rect::from_xywh(12.0, 12.0, 4.0, 4.0),
                Brush::RadialGradient(radial),
            )
            .unwrap();
        let mut fonts = FontDatabase::default();
        let mut text_context = TextLayoutContext::new();
        let mut request = TextLayoutRequest::new("I");
        request.style.size = 12.0;
        request.style.color = Color::GREEN;
        let text = text_context
            .layout(&mut fonts, request)
            .expect("text layout");
        painter
            .draw_text(&text, Point::new(1.0, 13.0), 1.0)
            .unwrap();
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
        assert_eq!(first.gradient_cache_misses, 2);
        assert_eq!(second.gradient_cache_hits, 2);
        assert!(first.glyph_cache_misses > 0);
        assert!(first.glyph_uploads > 0);
        assert!(second.glyph_cache_hits > 0);

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
        assert!(pixel(3, 1)[0] > pixel(3, 1)[2]);
        assert!(pixel(12, 1)[2] > pixel(12, 1)[0]);
        assert!(pixel(14, 14)[0] > pixel(12, 14)[0]);
        assert!(
            (0..16).any(|y| (0..8).any(|x| {
                let value = pixel(x, y);
                value[1] > value[0] && value[1] > value[2]
            })),
            "expected at least one green text pixel"
        );
        readback.unmap();
    });
}

/// A single line of many distinct glyphs shares one atlas bind group, so the
/// per-glyph draws must coalesce into a single index-range draw. This locks in
/// the draw-call merge and its RenderStats reporting.
#[test]
fn merges_a_glyph_run_into_one_draw() {
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
            label: Some("glyph merge target".into()),
            size: Extent3d::d2(512, 32),
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
        });
        let view = texture.create_view(TextureViewDescriptor::default());

        let mut fonts = FontDatabase::default();
        let mut text_context = TextLayoutContext::new();
        let mut request = TextLayoutRequest::new("the quick brown fox jumps over the lazy dog");
        request.style.size = 16.0;
        request.style.color = Color::WHITE;
        let text = text_context
            .layout(&mut fonts, request)
            .expect("text layout");
        let glyph_count: usize = text.glyph_runs().iter().map(|run| run.glyphs.len()).sum();
        assert!(
            glyph_count >= 30,
            "fixture should shape many glyphs, got {glyph_count}"
        );

        let mut painter = Painter::new();
        painter
            .draw_text(&text, Point::new(1.0, 20.0), 1.0)
            .unwrap();
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
        let mut encoder = device.create_command_encoder(CommandEncoderDescriptor::default());
        let stats = renderer
            .render(
                &mut encoder,
                &list,
                RenderTarget {
                    view: view.clone(),
                    format: TextureFormat::Rgba8Unorm,
                    size: Size::new(512, 32),
                    scale_factor: 1.0,
                    clear_color: Color::BLACK,
                },
            )
            .expect("render");
        queue
            .submit([encoder.finish().expect("finish encoder")])
            .expect("submit");

        // Every mask glyph in the run binds the same atlas, so the whole run
        // collapses to one draw regardless of glyph count.
        assert_eq!(
            stats.draws, 1,
            "expected {glyph_count} glyphs to merge into a single draw, got {} draws",
            stats.draws
        );
    });
}
