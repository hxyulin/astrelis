//! Headless steady-state comparison of direct and texture compositor paths.

use astrelis_compositor::{Compositor, ViewOptions, ViewRenderTarget};
use astrelis_core::{color::Color, geometry::Size};
use astrelis_gpu::{
    DeviceDescriptor, PollMode, RequestAdapterOptions, TextureDescriptor, TextureDimension,
    TextureFormat, TextureUsages,
};
use astrelis_paint::{Brush, CompositorViewId, Painter};
use astrelis_paint_gpu::{RenderTarget, Renderer, RendererOptions};
use criterion::{Criterion, criterion_group, criterion_main};

fn bench(c: &mut Criterion) {
    let instance = astrelis_gpu_wgpu::create_instance(Default::default());
    let Ok(adapter) =
        pollster::block_on(instance.request_adapter(RequestAdapterOptions::default()))
    else {
        eprintln!("skipping compositor benchmark: no headless GPU adapter");
        return;
    };
    let (device, queue) =
        pollster::block_on(adapter.request_device(DeviceDescriptor::default())).expect("device");
    let texture = device.create_texture(TextureDescriptor {
        label: Some("compositor benchmark frame".into()),
        size: astrelis_gpu::Extent3d::d2(1280, 720),
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8UnormSrgb,
        usage: TextureUsages::RENDER_ATTACHMENT,
    });
    let view = texture.create_view(Default::default());
    let paint = Renderer::new(device.clone(), queue.clone(), RendererOptions::default()).unwrap();
    let mut compositor = Compositor::new(device.clone(), paint);
    let id = CompositorViewId::new();
    let build = |prefer_direct| {
        let mut painter = Painter::new();
        painter
            .fill_rect(
                astrelis_core::geometry::Rect::from_xywh(0.0, 0.0, 1280.0, 720.0),
                Brush::Solid(Color::BLACK),
            )
            .unwrap();
        painter
            .compositor_view(
                id,
                astrelis_core::geometry::Rect::from_xywh(128.0, 96.0, 1024.0, 512.0),
                prefer_direct,
            )
            .unwrap();
        painter
            .fill_rect(
                astrelis_core::geometry::Rect::from_xywh(140.0, 108.0, 40.0, 20.0),
                Brush::Solid(Color::WHITE),
            )
            .unwrap();
        painter.finish().unwrap()
    };
    let direct = build(true);
    let texture_backed = build(false);
    let target = RenderTarget {
        view,
        format: TextureFormat::Rgba8UnormSrgb,
        size: Size::new(1280, 720),
        scale_factor: 1.0,
        clear_color: Color::BLACK,
    };
    for (name, list) in [("direct", direct), ("texture", texture_backed)] {
        c.bench_function(&format!("compositor/{name}"), |b| {
            b.iter(|| {
                let mut encoder = device.create_command_encoder(Default::default());
                compositor
                    .render(
                        &mut encoder,
                        &list,
                        target.clone(),
                        |_| ViewOptions::default(),
                        |_, encoder, target| {
                            let (view, load) = match target {
                                ViewRenderTarget::Direct(value) => {
                                    (value.view, astrelis_gpu::LoadOp::Load)
                                }
                                ViewRenderTarget::Texture(value) => (
                                    value.view,
                                    astrelis_gpu::LoadOp::Clear(astrelis_gpu::Color {
                                        r: 0.0,
                                        g: 0.0,
                                        b: 0.0,
                                        a: 0.0,
                                    }),
                                ),
                            };
                            let pass =
                                encoder.begin_render_pass(astrelis_gpu::RenderPassDescriptor {
                                    label: Some("benchmark scene".into()),
                                    color_attachments: vec![Some(
                                        astrelis_gpu::RenderPassColorAttachment {
                                            view,
                                            resolve_target: None,
                                            load,
                                            store: astrelis_gpu::StoreOp::Store,
                                        },
                                    )],
                                    depth_stencil_attachment: None,
                                    timestamp_writes: None,
                                })?;
                            drop(pass);
                            Ok::<_, astrelis_gpu::GpuError>(())
                        },
                    )
                    .unwrap();
                queue.submit([encoder.finish().unwrap()]).unwrap();
                device.poll(PollMode::Wait).unwrap();
            })
        });
    }
}

criterion_group!(benches, bench);
criterion_main!(benches);
