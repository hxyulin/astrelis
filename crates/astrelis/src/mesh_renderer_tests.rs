use std::{sync::mpsc, time::Duration};

use super::*;

fn quad(graphics: &GraphicsContext, color: [f32; 4]) -> Mesh {
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

fn pixels(
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

fn pass<'encoder>(
    graphics: &'encoder GraphicsContext,
    encoder: &'encoder mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
) -> Result<RenderPass<'encoder>, Error> {
    RenderPass::new(
        encoder,
        view,
        graphics.device(),
        wgpu::TextureFormat::Rgba8Unorm,
        [64, 64],
        load,
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
