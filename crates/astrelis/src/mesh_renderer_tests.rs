use std::{sync::mpsc, time::Duration};

use super::*;
use crate::Vertex;

pub(super) fn quad(graphics: &GraphicsContext, color: [f32; 4]) -> Mesh {
    graphics
        .create_mesh(
            &[
                Vertex::new([-0.75, -0.75, 0.0], color),
                Vertex::new([0.75, -0.75, 0.0], color),
                Vertex::new([0.75, 0.75, 0.0], color),
                Vertex::new([-0.75, 0.75, 0.0], color),
            ],
            &[0, 1, 2, 0, 2, 3],
        )
        .unwrap()
}

pub(super) fn pixels(
    graphics: &GraphicsContext,
    record: impl FnOnce(&mut wgpu::CommandEncoder, &wgpu::TextureView),
) -> Vec<u8> {
    let texture = graphics.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("Astrelis offscreen test"),
        size: wgpu::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let readback = graphics.device().create_buffer(&wgpu::BufferDescriptor {
        label: Some("Astrelis test readback"),
        size: 64 * 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = graphics
        .device()
        .create_command_encoder(&Default::default());
    record(&mut encoder, &view);
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: Default::default(),
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(64),
            },
        },
        texture.size(),
    );
    graphics.queue().submit([encoder.finish()]);
    let (tx, rx) = mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });
    graphics
        .device()
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(10)),
        })
        .unwrap();
    rx.recv_timeout(Duration::from_secs(10)).unwrap().unwrap();
    let result = readback.slice(..).get_mapped_range().unwrap().to_vec();
    readback.unmap();
    result
}

pub(super) fn pass<'encoder>(
    graphics: &'encoder GraphicsContext,
    encoder: &'encoder mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
) -> Result<RenderPass<'encoder>, Error> {
    let mut options = crate::pass::PassOptions::default();
    options.load = load;
    RenderPass::new(
        encoder,
        graphics,
        crate::pass::ManagedColorAttachment {
            view,
            resolve_target: None,
            format: wgpu::TextureFormat::Rgba8Unorm,
            size: [64, 64],
            sample_count: 1,
            resolved_state: None,
        },
        options,
    )
}

#[derive(Default)]
struct TestRecording {
    initialized: std::sync::Arc<std::sync::atomic::AtomicBool>,
    writes: crate::frame::AttachmentWrites,
    uploads: crate::uploads::DrawUploads,
}
fn managed_builder<'a>(
    e: &'a mut wgpu::CommandEncoder,
    g: &'a GraphicsContext,
    a: crate::pass::ManagedColorAttachment<'a>,
    r: &'a mut TestRecording,
) -> crate::RenderPassBuilder<'a> {
    crate::RenderPassBuilder::new(e, g, a, &r.initialized)
        .with_depth_stencil(None, &mut r.writes)
        .with_uploads(&mut r.uploads)
}

fn builder<'frame>(
    graphics: &'frame GraphicsContext,
    encoder: &'frame mut wgpu::CommandEncoder,
    view: &'frame wgpu::TextureView,
    initialized: &'frame mut TestRecording,
) -> crate::RenderPassBuilder<'frame> {
    managed_builder(
        encoder,
        graphics,
        crate::pass::ManagedColorAttachment {
            view,
            resolve_target: None,
            format: wgpu::TextureFormat::Rgba8Unorm,
            size: [64, 64],
            sample_count: 1,
            resolved_state: None,
        },
        initialized,
    )
}

#[test]
fn renderers_share_passes_preserve_order_and_validate_devices() {
    pollster::block_on(async {
        // A missing adapter is a failure, rather than silently passing a GPU test.
        let graphics = GraphicsContext::headless().await.unwrap();
        let mut first = MeshRenderer::new(&graphics);
        let mut second = MeshRenderer::new(&graphics);
        let red = quad(&graphics, [1.0, 0.0, 0.0, 1.0]);
        let blue = quad(&graphics, [0.0, 0.0, 1.0, 0.5]);
        let center = 32 * 256 + 32 * 4;

        let blended = pixels(&graphics, |encoder, view| {
            let mut pass = pass(
                &graphics,
                encoder,
                view,
                wgpu::LoadOp::Clear(wgpu::Color::BLACK),
            )
            .unwrap();
            first.draw(&mut pass, &red).unwrap();
            second.draw(&mut pass, &blue).unwrap();
        });
        for (&actual, expected) in blended[center..center + 4]
            .iter()
            .zip([128_u8, 0, 128, 255])
        {
            assert!(
                actual.abs_diff(expected) <= 1,
                "incorrect alpha blend: {actual} vs {expected}"
            );
        }
        assert_eq!(&blended[..4], &[0, 0, 0, 255], "clear outside the meshes");
        let reordered = pixels(&graphics, |encoder, view| {
            let mut pass = pass(
                &graphics,
                encoder,
                view,
                wgpu::LoadOp::Clear(wgpu::Color::BLACK),
            )
            .unwrap();
            second.draw(&mut pass, &blue).unwrap();
            first.draw(&mut pass, &red).unwrap();
        });
        assert_eq!(&reordered[center..center + 4], &[255, 0, 0, 255]);

        let loaded = pixels(&graphics, |encoder, view| {
            {
                let mut pass = pass(
                    &graphics,
                    encoder,
                    view,
                    wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                )
                .unwrap();
                first.draw(&mut pass, &red).unwrap();
            }
            let mut pass = pass(&graphics, encoder, view, wgpu::LoadOp::Load).unwrap();
            second.draw(&mut pass, &blue).unwrap();
        });
        assert_eq!(
            loaded, blended,
            "a second pass must preserve earlier drawing"
        );
        let cleared = pixels(&graphics, |encoder, view| {
            {
                let mut pass = pass(
                    &graphics,
                    encoder,
                    view,
                    wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                )
                .unwrap();
                first.draw(&mut pass, &red).unwrap();
            }
            drop(
                pass(
                    &graphics,
                    encoder,
                    view,
                    wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                )
                .unwrap(),
            );
        });
        assert!(
            cleared
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel == &[0, 255, 0, 255])
        );
        let restored = pixels(&graphics, |encoder, view| {
            let mut pass = pass(
                &graphics,
                encoder,
                view,
                wgpu::LoadOp::Clear(wgpu::Color::BLACK),
            )
            .unwrap();
            pass.as_wgpu().set_viewport(0.0, 0.0, 1.0, 1.0, 0.0, 1.0);
            pass.as_wgpu().set_scissor_rect(0, 0, 1, 1);
            first.draw(&mut pass, &red).unwrap();
        });
        assert_eq!(
            &restored[center..center + 4],
            &[255, 0, 0, 255],
            "restore draw state after custom rendering"
        );
        // Invalid or abandoned builders must not initialize the frame or poison recording.
        let defaults = pixels(&graphics, |encoder, view| {
            let mut initialized = TestRecording::default();
            drop(builder(&graphics, encoder, view, &mut initialized).clear_color(wgpu::Color::RED));
            let invalid_clear = wgpu::Color {
                r: f64::NAN,
                ..wgpu::Color::BLACK
            };
            assert!(matches!(
                builder(&graphics, encoder, view, &mut initialized)
                    .clear_color(invalid_clear)
                    .begin(),
                Err(Error::InvalidClearColor)
            ));
            assert!(matches!(
                builder(&graphics, encoder, view, &mut initialized)
                    .viewport(0.0, 0.0, -1.0, 64.0, 0.0, 1.0)
                    .begin(),
                Err(Error::InvalidViewport)
            ));
            assert!(matches!(
                builder(&graphics, encoder, view, &mut initialized)
                    .scissor_rect(1, 0, u32::MAX, 64)
                    .begin(),
                Err(Error::InvalidScissorRect)
            ));
            assert!(matches!(
                builder(&graphics, encoder, view, &mut initialized)
                    .load_color()
                    .begin(),
                Err(Error::UninitializedFrame)
            ));
            drop(
                builder(&graphics, encoder, view, &mut initialized)
                    .begin()
                    .unwrap(),
            );
        });
        assert!(
            defaults.iter().all(|component| *component == 0),
            "default pass clears transparent black"
        );

        let selected = pixels(&graphics, |encoder, view| {
            let mut initialized = TestRecording::default();
            drop(
                builder(&graphics, encoder, view, &mut initialized)
                    .load_color()
                    .clear_color(wgpu::Color::GREEN)
                    .begin()
                    .unwrap(),
            );
            drop(
                builder(&graphics, encoder, view, &mut initialized)
                    .clear_color(wgpu::Color::RED)
                    .load_color()
                    .begin()
                    .unwrap(),
            );
        });
        assert!(
            selected
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel == &[0, 255, 0, 255]),
            "the last clear/load selection wins"
        );

        let pixel_at = |x: usize, y: usize| y * 256 + x * 4;
        let clipped = pixels(&graphics, |encoder, view| {
            let mut initialized = TestRecording::default();
            let mut pass = builder(&graphics, encoder, view, &mut initialized)
                .label("viewport and scissor")
                .clear_color(wgpu::Color::BLACK)
                .viewport(0.0, 0.0, 32.0, 64.0, 0.0, 1.0)
                .scissor_rect(16, 0, 16, 64)
                .begin()
                .unwrap();
            // Raw changes cannot replace the viewport/scissor selected through the wrapper.
            pass.as_wgpu().set_viewport(0.0, 0.0, 1.0, 1.0, 0.0, 1.0);
            pass.as_wgpu().set_scissor_rect(0, 0, 1, 1);
            first.draw(&mut pass, &red).unwrap();
        });
        assert_eq!(
            &clipped[pixel_at(20, 32)..pixel_at(20, 32) + 4],
            &[255, 0, 0, 255]
        );
        for x in [8, 40] {
            assert_eq!(
                &clipped[pixel_at(x, 32)..pixel_at(x, 32) + 4],
                &[0, 0, 0, 255]
            );
        }
        let opaque_blue = quad(&graphics, [0.0, 0.0, 1.0, 1.0]);
        let dynamic = pixels(&graphics, |encoder, view| {
            let mut initialized = TestRecording::default();
            let mut pass = builder(&graphics, encoder, view, &mut initialized)
                .clear_color(wgpu::Color::BLACK)
                .begin()
                .unwrap();
            pass.set_viewport(0.0, 0.0, 32.0, 64.0, 0.0, 1.0).unwrap();
            first.draw(&mut pass, &red).unwrap();
            pass.set_viewport(0.0, 0.0, 64.0, 64.0, 0.0, 1.0).unwrap();
            pass.set_scissor_rect(32, 0, 32, 64).unwrap();
            assert!(matches!(
                pass.set_viewport(0.0, 0.0, 64.0, 64.0, 1.0, 0.0),
                Err(Error::InvalidViewport)
            ));
            assert!(matches!(
                pass.set_scissor_rect(1, 0, u32::MAX, 64),
                Err(Error::InvalidScissorRect)
            ));
            second.draw(&mut pass, &opaque_blue).unwrap();
        });
        assert_eq!(
            &dynamic[pixel_at(16, 32)..pixel_at(16, 32) + 4],
            &[255, 0, 0, 255]
        );
        assert_eq!(
            &dynamic[pixel_at(48, 32)..pixel_at(48, 32) + 4],
            &[0, 0, 255, 255]
        );
        let empty_clip = pixels(&graphics, |encoder, view| {
            let mut initialized = TestRecording::default();
            let mut pass = builder(&graphics, encoder, view, &mut initialized)
                .clear_color(wgpu::Color::BLACK)
                .scissor_rect(0, 0, 0, 64)
                .begin()
                .unwrap();
            first.draw(&mut pass, &red).unwrap();
        });
        assert!(
            empty_clip
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel == &[0, 0, 0, 255])
        );
        assert_eq!(first.pipelines.len(), 1, "reuse the same format pipeline");
        assert_eq!(second.pipelines.len(), 1);

        let shared = GraphicsContext::from_wgpu(
            graphics.instance().clone(),
            graphics.adapter().clone(),
            graphics.device().clone(),
            graphics.queue().clone(),
        );
        let (other_device, other_queue) = graphics
            .adapter()
            .request_device(&Default::default())
            .await
            .unwrap();
        let other = GraphicsContext::from_wgpu(
            graphics.instance().clone(),
            graphics.adapter().clone(),
            other_device,
            other_queue,
        );
        let foreign_mesh = quad(&other, [1.0; 4]);
        let mut foreign_renderer = MeshRenderer::new(&other);
        let mut shared_renderer = MeshRenderer::new(&shared);
        let checked = pixels(&graphics, |encoder, view| {
            let invalid = wgpu::Color {
                r: f64::NAN,
                ..wgpu::Color::BLACK
            };
            assert!(matches!(
                pass(&graphics, encoder, view, wgpu::LoadOp::Clear(invalid)),
                Err(Error::InvalidClearColor)
            ));
            let mut pass = pass(
                &graphics,
                encoder,
                view,
                wgpu::LoadOp::Clear(wgpu::Color::BLACK),
            )
            .unwrap();
            assert!(matches!(
                foreign_renderer.draw(&mut pass, &red),
                Err(Error::DeviceMismatch)
            ));
            assert!(matches!(
                first.draw(&mut pass, &foreign_mesh),
                Err(Error::DeviceMismatch)
            ));
            shared_renderer.draw(&mut pass, &red).unwrap();
        });
        assert_eq!(
            &checked[center..center + 4],
            &[255, 0, 0, 255],
            "rejected operations leave recording usable"
        );
    });
}

#[test]
fn multisampling_resolves_edges_and_preserves_samples_across_passes() {
    pollster::block_on(async {
        let graphics = GraphicsContext::headless().await.unwrap();
        let mut contexts = vec![graphics];
        let graphics = &contexts[0];
        let native_feature = wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
        if graphics.adapter().features().contains(native_feature) {
            let (device, queue) = graphics
                .adapter()
                .request_device(&wgpu::DeviceDescriptor {
                    required_features: native_feature,
                    ..Default::default()
                })
                .await
                .unwrap();
            contexts.push(GraphicsContext::from_wgpu(
                graphics.instance().clone(),
                graphics.adapter().clone(),
                device,
                queue,
            ));
        }
        for graphics in contexts {
            let format = wgpu::TextureFormat::Rgba8Unorm;
            let counts = crate::target::supported_sample_counts(&graphics, format);
            assert!(counts.contains(&1));
            assert!(counts.contains(&4), "RGBA8 must support portable 4x MSAA");
            // Default devices must not advertise native counts that they cannot use.
            if graphics
                .adapter()
                .get_downlevel_capabilities()
                .flags
                .contains(wgpu::DownlevelFlags::WEBGPU_TEXTURE_FORMAT_SUPPORT)
                && !graphics.device().features().contains(native_feature)
            {
                assert_eq!(counts, [1, 4]);
            }
            eprintln!(
                "MSAA counts with native format features {}: {counts:?}",
                graphics.device().features().contains(native_feature)
            );
            assert!(
                crate::target::create_multisample_view(&graphics, format, [64, 64], 1).is_none()
            );
            assert!(
                crate::target::create_multisample_view(&graphics, format, [0, 64], 4).is_none()
            );
            assert!(
                crate::target::create_multisample_view(&graphics, format, [64, 0], 4).is_none()
            );
            let resized =
                crate::target::create_multisample_view(&graphics, format, [32, 48], 4).unwrap();
            assert_eq!(
                resized.texture().size(),
                wgpu::Extent3d {
                    width: 32,
                    height: 48,
                    depth_or_array_layers: 1,
                }
            );
            assert_eq!(resized.texture().sample_count(), 4);
            let triangle = |color| {
                graphics
                    .create_mesh(
                        &[
                            Vertex::new([-0.83, -0.72, 0.0], color),
                            Vertex::new([0.61, -0.57, 0.0], color),
                            Vertex::new([0.13, 0.86, 0.0], color),
                        ],
                        &[0, 1, 2],
                    )
                    .unwrap()
            };
            let white = triangle([1.0; 4]);
            let red = triangle([1.0, 0.0, 0.0, 1.0]);
            let blue = triangle([0.0, 0.0, 1.0, 0.5]);
            let mut first = MeshRenderer::new(&graphics);
            let mut second = MeshRenderer::new(&graphics);
            for &count in &counts {
                let multisample_view =
                    crate::target::create_multisample_view(&graphics, format, [64, 64], count);
                fn attachment<'view>(
                    view: &'view wgpu::TextureView,
                    multisample_view: Option<&'view wgpu::TextureView>,
                    count: u32,
                ) -> crate::pass::ManagedColorAttachment<'view> {
                    crate::pass::ManagedColorAttachment {
                        view: multisample_view.unwrap_or(view),
                        resolve_target: multisample_view.map(|_| view),
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        size: [64, 64],
                        sample_count: count,
                        resolved_state: None,
                    }
                }
                let edges = pixels(&graphics, |encoder, view| {
                    let mut initialized = TestRecording::default();
                    let mut pass = managed_builder(
                        encoder,
                        &graphics,
                        attachment(view, multisample_view.as_ref(), count),
                        &mut initialized,
                    )
                    .clear_color(wgpu::Color::BLACK)
                    .begin()
                    .unwrap();
                    assert_eq!(pass.sample_count(), count);
                    first.draw(&mut pass, &white).unwrap();
                });
                let partial_pixels = edges
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .filter(|pixel| pixel[0] > 0 && pixel[0] < 255)
                    .count();
                if count == 1 {
                    assert_eq!(partial_pixels, 0, "1x has no sample-averaged edge pixels");
                } else {
                    assert!(
                        partial_pixels > 0,
                        "MSAA must resolve partial edge coverage"
                    );
                }

                let mut draw_layers = |split: bool| {
                    pixels(&graphics, |encoder, view| {
                        let mut initialized = TestRecording::default();
                        let mut pass = managed_builder(
                            encoder,
                            &graphics,
                            attachment(view, multisample_view.as_ref(), count),
                            &mut initialized,
                        )
                        .clear_color(wgpu::Color::BLACK)
                        .begin()
                        .unwrap();
                        first.draw(&mut pass, &red).unwrap();
                        if split {
                            drop(pass);
                            pass = managed_builder(
                                encoder,
                                &graphics,
                                attachment(view, multisample_view.as_ref(), count),
                                &mut initialized,
                            )
                            .load_color()
                            .begin()
                            .unwrap();
                        }
                        second.draw(&mut pass, &blue).unwrap();
                    })
                };
                let together = draw_layers(false);
                let separate = draw_layers(true);
                assert_eq!(
                    together, separate,
                    "load must preserve individual samples, including edge coverage"
                );
                let center = 32 * 256 + 32 * 4;
                for (&actual, expected) in separate[center..center + 4]
                    .iter()
                    .zip([128_u8, 0, 128, 255])
                {
                    assert!(actual.abs_diff(expected) <= 1);
                }
                let cleared = pixels(&graphics, |encoder, view| {
                    let mut initialized = TestRecording::default();
                    // Clearing the reused attachment starts a fresh frame.
                    drop(
                        managed_builder(
                            encoder,
                            &graphics,
                            attachment(view, multisample_view.as_ref(), count),
                            &mut initialized,
                        )
                        .clear_color(wgpu::Color::GREEN)
                        .begin()
                        .unwrap(),
                    );
                });
                assert!(
                    cleared
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .all(|p| p == &[0, 255, 0, 255])
                );
            }
            assert_eq!(first.pipelines.len(), counts.len());
            assert_eq!(second.pipelines.len(), counts.len());
            // Reusing the renderer on a single-sampled target selects its existing pipeline.
            pixels(&graphics, |encoder, view| {
                let mut pass = pass(
                    &graphics,
                    encoder,
                    view,
                    wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                )
                .unwrap();
                first.draw(&mut pass, &white).unwrap();
            });
            assert_eq!(first.pipelines.len(), counts.len());
        }
    });
}
