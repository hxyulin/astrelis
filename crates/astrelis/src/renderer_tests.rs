use std::{sync::mpsc, time::Duration};

use super::*;

fn quad(graphics: &GraphicsContext, color: [f32; 4]) -> Mesh {
    Mesh::new(
        graphics,
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

fn pixels(renderer: &mut Renderer, clear: wgpu::Color, meshes: &[&Mesh]) -> Vec<u8> {
    renderer.validate(clear, meshes).unwrap();
    let graphics = renderer.graphics.clone();
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
    renderer.encode(&mut encoder, &view, texture.format(), clear, meshes);
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

#[test]
fn indexed_meshes_preserve_order_blend_and_reuse() {
    pollster::block_on(async {
        // A missing adapter is a failure, rather than silently passing a GPU test.
        let graphics = GraphicsContext::headless().await.unwrap();
        let mut renderer = Renderer::new(&graphics);
        let red = quad(&graphics, [1.0, 0.0, 0.0, 1.0]);
        let blue = quad(&graphics, [0.0, 0.0, 1.0, 0.5]);
        let center = 32 * 256 + 32 * 4;

        let blended = pixels(&mut renderer, wgpu::Color::BLACK, &[&red, &blue]);
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
        let reordered = pixels(&mut renderer, wgpu::Color::BLACK, &[&blue, &red]);
        assert_eq!(&reordered[center..center + 4], &[255, 0, 0, 255]);
        let cleared = pixels(&mut renderer, wgpu::Color::GREEN, &[]);
        assert!(
            cleared
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel == &[0, 255, 0, 255])
        );
        assert_eq!(
            renderer.pipelines.len(),
            1,
            "reuse the same format pipeline"
        );

        let shared = GraphicsContext::from_wgpu(
            graphics.instance().clone(),
            graphics.adapter().clone(),
            graphics.device().clone(),
            graphics.queue().clone(),
        );
        assert!(
            Renderer::new(&shared)
                .validate(wgpu::Color::BLACK, &[&red])
                .is_ok()
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
        assert!(matches!(
            Renderer::new(&other).validate(wgpu::Color::BLACK, &[&red]),
            Err(Error::DeviceMismatch)
        ));
    });
}
