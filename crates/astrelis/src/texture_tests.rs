use super::*;
use crate::framebuffer::tests::pixels;
use crate::{FramebufferOptions, MeshRenderer, TextureOptions, Vertex};

fn output(graphics: &GraphicsContext, count: u32) -> crate::Framebuffer {
    graphics
        .create_framebuffer(FramebufferOptions::new(64, 64).sample_count(count).usage(
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
        ))
        .unwrap()
}

fn pixel(bytes: &[u8], x: usize, y: usize) -> [u8; 4] {
    bytes[(y * 64 + x) * 4..(y * 64 + x + 1) * 4]
        .try_into()
        .unwrap()
}

fn near(actual: [u8; 4], expected: [u8; 4]) {
    for (a, e) in actual.into_iter().zip(expected) {
        assert!(a.abs_diff(e) <= 2, "{actual:?} != {expected:?}");
    }
}

fn image(graphics: &GraphicsContext, width: u32, height: u32, bytes: &[u8]) -> crate::Texture {
    let texture = graphics
        .create_texture(TextureOptions::new(width, height).format(wgpu::TextureFormat::Rgba8Unorm))
        .unwrap();
    texture.write(bytes).unwrap();
    texture
}

#[test]
fn uploads_validate_before_queueing_and_regions_preserve_other_texels() {
    pollster::block_on(async {
        let graphics = GraphicsContext::headless().await.unwrap();
        let device = graphics.device();
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        assert!(matches!(
            graphics.create_texture(TextureOptions::new(0, 2)),
            Err(Error::InvalidTextureSize { .. })
        ));
        assert!(matches!(
            graphics.create_texture(TextureOptions::new(u32::MAX, 2)),
            Err(Error::InvalidTextureSize { .. })
        ));
        for format in [
            wgpu::TextureFormat::Depth24Plus,
            wgpu::TextureFormat::Bc1RgbaUnorm,
        ] {
            assert!(matches!(
                graphics.create_texture(TextureOptions::new(2, 2).format(format)),
                Err(Error::UnsupportedTextureFormat { .. })
            ));
        }
        assert!(matches!(
            graphics.create_texture(TextureOptions::new(2, 2).usage(wgpu::TextureUsages::empty())),
            Err(Error::InvalidTextureUsage { .. })
        ));
        let no_upload = graphics
            .create_texture(TextureOptions::new(2, 2).usage(wgpu::TextureUsages::TEXTURE_BINDING))
            .unwrap();
        assert!(matches!(
            no_upload.write(&[0; 16]),
            Err(Error::InvalidTextureUsage { .. })
        ));
        let texture = image(&graphics, 3, 2, &[255, 0, 0, 255].repeat(6)); // 12-byte rows need no 256-byte padding.
        assert!(matches!(
            texture.write(&[0; 23]),
            Err(Error::InvalidTextureData {
                expected: 24,
                actual: 23
            })
        ));
        for (origin, size) in [([0, 0], [0, 1]), ([2, 0], [2, 1]), ([u32::MAX, 0], [1, 1])] {
            assert!(matches!(
                texture.write_region(origin, size, &[0; 4]),
                Err(Error::InvalidTextureRegion)
            ));
        }
        texture
            .write_region([1, 0], [1, 2], &[0, 255, 0, 255].repeat(2))
            .unwrap();
        let mut renderer = TextureRenderer::new(&graphics);
        let binding = renderer
            .create_binding(
                texture.view(),
                TextureDrawOptions::new().filter(TextureFilter::Nearest),
            )
            .unwrap();
        let mut target = output(&graphics, 1);
        let bytes = pixels(&graphics, &mut target, |frame| {
            let mut pass = frame.render_pass().begin().unwrap();
            renderer.draw(&mut pass, &binding).unwrap();
        });
        assert_eq!(pixel(&bytes, 8, 8), [255, 0, 0, 255]);
        assert_eq!(pixel(&bytes, 32, 8), [0, 255, 0, 255]);
        assert_eq!(pixel(&bytes, 56, 56), [255, 0, 0, 255]);
        assert!(scope.pop().await.is_none());
    });
}

#[test]
fn crops_filters_srgb_alpha_tint_and_replacement_produce_expected_pixels() {
    pollster::block_on(async {
        let graphics = GraphicsContext::headless().await.unwrap();
        let scope = graphics
            .device()
            .push_error_scope(wgpu::ErrorFilter::Validation);
        let mut renderer = TextureRenderer::new(&graphics);
        let mut target = output(&graphics, 1);
        let texture = image(
            &graphics,
            2,
            2,
            &[
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
            ],
        );
        let binding = renderer
            .create_binding(
                texture.view(),
                TextureDrawOptions::new().filter(TextureFilter::Nearest),
            )
            .unwrap();
        let bytes = pixels(&graphics, &mut target, |frame| {
            renderer
                .draw(&mut frame.render_pass().begin().unwrap(), &binding)
                .unwrap();
        });
        assert_eq!(pixel(&bytes, 8, 8), [255, 0, 0, 255]);
        assert_eq!(pixel(&bytes, 56, 8), [0, 255, 0, 255]);
        assert_eq!(pixel(&bytes, 8, 56), [0, 0, 255, 255]);
        assert_eq!(pixel(&bytes, 56, 56), [255, 255, 255, 255]);
        let crop = renderer
            .create_binding(
                texture.view(),
                TextureDrawOptions::new()
                    .source([0.5, 0.0, 0.5, 0.5])
                    .destination([0.25, 0.25, 0.5, 0.5])
                    .filter(TextureFilter::Nearest),
            )
            .unwrap();
        let bytes = pixels(&graphics, &mut target, |frame| {
            renderer
                .draw(&mut frame.render_pass().begin().unwrap(), &crop)
                .unwrap();
        });
        assert_eq!(pixel(&bytes, 32, 32), [0, 255, 0, 255]);
        assert_eq!(pixel(&bytes, 8, 8), [0; 4]);
        let center = renderer
            .create_binding(
                texture.view(),
                TextureDrawOptions::new().source([0.5, 0.5, 0.0, 0.0]),
            )
            .unwrap();
        let bytes = pixels(&graphics, &mut target, |frame| {
            renderer
                .draw(&mut frame.render_pass().begin().unwrap(), &center)
                .unwrap();
        });
        near(pixel(&bytes, 32, 32), [128, 128, 128, 255]);
        let srgb = graphics.create_texture(TextureOptions::new(1, 1)).unwrap();
        srgb.write(&[128, 128, 128, 255]).unwrap();
        let srgb = renderer
            .create_binding(srgb.view(), TextureDrawOptions::new())
            .unwrap();
        let bytes = pixels(&graphics, &mut target, |frame| {
            renderer
                .draw(&mut frame.render_pass().begin().unwrap(), &srgb)
                .unwrap();
        });
        near(pixel(&bytes, 32, 32), [55, 55, 55, 255]);
        for (alpha, bytes) in [
            (TextureAlpha::Straight, [255, 64, 0, 128]),
            (TextureAlpha::Premultiplied, [128, 32, 0, 128]),
        ] {
            let texture = image(&graphics, 1, 1, &bytes);
            for blend in [TextureBlend::Alpha, TextureBlend::Replace] {
                let binding = renderer
                    .create_binding(
                        texture.view(),
                        TextureDrawOptions::new()
                            .alpha(alpha)
                            .blend(blend)
                            .tint([0.5, 1.0, 1.0, 0.5]),
                    )
                    .unwrap();
                let bytes = pixels(&graphics, &mut target, |frame| {
                    renderer
                        .draw(
                            &mut frame
                                .render_pass()
                                .clear_color(wgpu::Color::BLACK)
                                .begin()
                                .unwrap(),
                            &binding,
                        )
                        .unwrap();
                });
                near(
                    pixel(&bytes, 32, 32),
                    [
                        32,
                        16,
                        0,
                        if blend == TextureBlend::Alpha {
                            255
                        } else {
                            64
                        },
                    ],
                );
            }
        }
        assert!(scope.pop().await.is_none());
    });
}

#[test]
fn bindings_snapshot_sources_rebind_after_resize_and_share_renderer_layouts() {
    pollster::block_on(async {
        let graphics = GraphicsContext::headless().await.unwrap();
        let scope = graphics
            .device()
            .push_error_scope(wgpu::ErrorFilter::Validation);
        let mut renderer = TextureRenderer::new(&graphics);
        let mut other = TextureRenderer::new(&graphics);
        let mut source = output(&graphics, 4);
        let old = source.color_view().unwrap().clone();
        let mut binding = renderer
            .create_binding(
                &old,
                TextureDrawOptions::new().alpha(TextureAlpha::Premultiplied),
            )
            .unwrap();
        let old_group = binding.group.clone();
        let parameters = binding.parameters.clone();
        renderer.rebind(&mut binding, &old).unwrap();
        assert_eq!(old_group, binding.group);
        let red = image(&graphics, 1, 1, &[255, 0, 0, 255]);
        let green = image(&graphics, 1, 1, &[0, 255, 0, 255]);
        let mut snapshots = renderer
            .create_binding(red.view(), TextureDrawOptions::new())
            .unwrap();
        let mut target = output(&graphics, 1);
        let bytes = pixels(&graphics, &mut target, |frame| {
            let mut pass = frame
                .render_pass()
                .scissor_rect(0, 0, 32, 64)
                .begin()
                .unwrap();
            renderer.draw(&mut pass, &snapshots).unwrap();
            renderer.rebind(&mut snapshots, green.view()).unwrap();
            pass.set_scissor_rect(32, 0, 32, 64).unwrap();
            other.draw(&mut pass, &snapshots).unwrap();
        });
        assert_eq!(pixel(&bytes, 8, 8), [255, 0, 0, 255]);
        assert_eq!(pixel(&bytes, 56, 8), [0, 255, 0, 255]);
        source.resize(32, 32).unwrap();
        assert_eq!(binding.view, old);
        renderer
            .rebind(&mut binding, source.color_view().unwrap())
            .unwrap();
        assert_ne!(binding.group, old_group);
        assert_eq!(binding.parameters, parameters);
        let new_group = binding.group.clone();
        source.set_sample_count(1).unwrap();
        renderer
            .rebind(&mut binding, source.color_view().unwrap())
            .unwrap();
        assert_ne!(binding.group, new_group);
        let mut frame = source.begin_frame().unwrap();
        drop(
            frame
                .render_pass()
                .clear_color(wgpu::Color::GREEN)
                .begin()
                .unwrap(),
        );
        frame.finish().unwrap();
        let bytes = pixels(&graphics, &mut target, |frame| {
            other
                .draw(&mut frame.render_pass().begin().unwrap(), &binding)
                .unwrap();
        });
        assert_eq!(pixel(&bytes, 32, 32), [0, 255, 0, 255]);
        assert!(scope.pop().await.is_none());
    });
}

#[test]
fn msaa_framebuffers_composite_in_one_submission_and_preserve_pass_state() {
    pollster::block_on(async {
        let graphics = GraphicsContext::headless().await.unwrap();
        let scope = graphics
            .device()
            .push_error_scope(wgpu::ErrorFilter::Validation);
        let mut meshes = MeshRenderer::new(&graphics);
        let mesh = graphics
            .create_mesh(
                &[
                    Vertex::new([-1., 1., 0.], [1., 0., 0., 0.5]),
                    Vertex::new([-1., -1., 0.], [1., 0., 0., 0.5]),
                    Vertex::new([1., 1., 0.], [1., 0., 0., 0.5]),
                    Vertex::new([1., -1., 0.], [1., 0., 0., 0.5]),
                ],
                &[0, 1, 2, 2, 1, 3],
            )
            .unwrap();
        let mut textures = TextureRenderer::new(&graphics);
        let mut source = output(&graphics, 4);
        let binding = textures
            .create_binding(
                source.color_view().unwrap(),
                TextureDrawOptions::new().alpha(TextureAlpha::Premultiplied),
            )
            .unwrap();
        for count in [1, 4] {
            let target = graphics
                .create_framebuffer(
                    FramebufferOptions::new(64, 64)
                        .sample_count(count)
                        .depth_stencil(wgpu::TextureFormat::Depth24PlusStencil8)
                        .usage(
                            wgpu::TextureUsages::RENDER_ATTACHMENT
                                | wgpu::TextureUsages::TEXTURE_BINDING
                                | wgpu::TextureUsages::COPY_SRC,
                        ),
                )
                .unwrap();
            let target = crate::RenderTarget::from(target);
            textures.prepare_for_target(&binding, &target).unwrap();
            textures.prepare_for_target(&binding, &target).unwrap();
            let mut target = match target {
                crate::RenderTarget::Framebuffer(target) => target,
                _ => unreachable!(),
            };
            let pipelines = textures.pipelines.len();
            let bytes = pixels(&graphics, &mut target, |frame| {
                {
                    let mut pass = frame.render_to(&mut source).begin().unwrap();
                    meshes.draw(&mut pass, &mesh).unwrap();
                }
                {
                    let mut pass = frame
                        .render_pass()
                        .clear_color(wgpu::Color::BLUE)
                        .viewport(16., 16., 32., 32., 0., 1.)
                        .scissor_rect(20, 20, 24, 24)
                        .begin()
                        .unwrap();
                    pass.as_wgpu().set_scissor_rect(0, 0, 64, 64);
                    textures.draw(&mut pass, &binding).unwrap();
                }
                {
                    let mut pass = frame
                        .render_pass()
                        .load()
                        .depth_ops(None)
                        .stencil_ops(None)
                        .scissor_rect(0, 0, 1, 1)
                        .begin()
                        .unwrap();
                    textures.draw(&mut pass, &binding).unwrap();
                }
            });
            near(pixel(&bytes, 32, 30), [128, 0, 127, 255]);
            assert_eq!(pixel(&bytes, 18, 18), [0, 0, 255, 255]);
            assert_eq!(pixel(&bytes, 50, 50), [0, 0, 255, 255]);
            assert_eq!(textures.pipelines.len(), pipelines);
        }
        assert!(scope.pop().await.is_none());
    });
}

#[test]
fn invalid_sources_feedback_and_devices_are_rejected_without_draws() {
    pollster::block_on(async {
        let graphics = GraphicsContext::headless().await.unwrap();
        let scope = graphics
            .device()
            .push_error_scope(wgpu::ErrorFilter::Validation);
        let mut renderer = TextureRenderer::new(&graphics);
        let texture = image(&graphics, 1, 1, &[255, 0, 0, 255]);
        for options in [
            TextureDrawOptions::new().destination([0., 0., -1., 1.]),
            TextureDrawOptions::new().source([f32::NAN, 0., 1., 1.]),
            TextureDrawOptions::new().tint([1., 1., 1., 2.]),
        ] {
            assert!(matches!(
                renderer.create_binding(texture.view(), options),
                Err(Error::InvalidTextureDraw)
            ));
        }
        let unfilterable = graphics
            .create_texture(TextureOptions::new(1, 1).format(wgpu::TextureFormat::R32Float))
            .unwrap();
        unfilterable.write(bytemuck::bytes_of(&0.5f32)).unwrap();
        assert!(matches!(
            renderer.create_binding(unfilterable.view(), TextureDrawOptions::new()),
            Err(Error::InvalidTextureSource { .. })
        ));
        let nearest = renderer
            .create_binding(
                unfilterable.view(),
                TextureDrawOptions::new().filter(TextureFilter::Nearest),
            )
            .unwrap();
        let mut output = output(&graphics, 1);
        let bytes = pixels(&graphics, &mut output, |frame| {
            renderer
                .draw(&mut frame.render_pass().begin().unwrap(), &nearest)
                .unwrap();
        });
        near(pixel(&bytes, 32, 32), [128, 0, 0, 255]);
        let integer = graphics
            .create_texture(TextureOptions::new(1, 1).format(wgpu::TextureFormat::Rgba8Uint))
            .unwrap();
        assert!(matches!(
            renderer.create_binding(integer.view(), TextureDrawOptions::new()),
            Err(Error::InvalidTextureSource { .. })
        ));
        let missing_usage = graphics
            .create_texture(TextureOptions::new(1, 1).usage(wgpu::TextureUsages::COPY_DST))
            .unwrap();
        assert!(matches!(
            renderer.create_binding(missing_usage.view(), TextureDrawOptions::new()),
            Err(Error::InvalidTextureSource { .. })
        ));
        let multisampled = graphics.device().create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 4,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        assert!(matches!(
            renderer.create_binding(
                &multisampled.create_view(&Default::default()),
                TextureDrawOptions::new()
            ),
            Err(Error::InvalidTextureSource { .. })
        ));
        let mut binding = renderer
            .create_binding(texture.view(), TextureDrawOptions::new())
            .unwrap();
        let group = binding.group.clone();
        assert!(matches!(
            renderer.rebind(&mut binding, integer.view()),
            Err(Error::InvalidTextureSource { .. })
        ));
        assert_eq!(group, binding.group);
        for count in [1, 4] {
            let mut target = super::tests::output(&graphics, count);
            let feedback = renderer
                .create_binding(target.color_view().unwrap(), TextureDrawOptions::new())
                .unwrap();
            let bytes = pixels(&graphics, &mut target, |frame| {
                let mut pass = frame
                    .render_pass()
                    .clear_color(wgpu::Color::BLUE)
                    .begin()
                    .unwrap();
                assert!(matches!(
                    renderer.draw(&mut pass, &feedback),
                    Err(Error::TextureFeedback)
                ));
                renderer.draw(&mut pass, &binding).unwrap();
            });
            assert_eq!(pixel(&bytes, 32, 32), [255, 0, 0, 255]);
        }
        let foreign = GraphicsContext::headless().await.unwrap();
        let foreign_renderer = TextureRenderer::new(&foreign);
        assert!(matches!(
            foreign_renderer.rebind(&mut binding, texture.view()),
            Err(Error::DeviceMismatch)
        ));
        // Cross-instance identity also protects existing meshes, materials, and additional targets.
        let foreign_texture = image(&foreign, 1, 1, &[0, 255, 0, 255]);
        let foreign_binding = foreign_renderer
            .create_binding(foreign_texture.view(), TextureDrawOptions::new())
            .unwrap();
        let foreign_mesh = foreign
            .create_mesh(
                &[
                    Vertex::new([-1., -1., 0.], [1.; 4]),
                    Vertex::new([1., -1., 0.], [1.; 4]),
                    Vertex::new([0., 1., 0.], [1.; 4]),
                ],
                &[0, 1, 2],
            )
            .unwrap();
        let mut foreign_meshes = MeshRenderer::new(&foreign);
        let mut meshes = MeshRenderer::new(&graphics);
        let own_mesh = graphics
            .create_mesh(
                &[
                    Vertex::new([-1., -1., 0.], [1.; 4]),
                    Vertex::new([1., -1., 0.], [1.; 4]),
                    Vertex::new([0., 1., 0.], [1.; 4]),
                ],
                &[0, 1, 2],
            )
            .unwrap();
        assert!(matches!(
            meshes.prepare_material(
                foreign_meshes.default_material(),
                wgpu::TextureFormat::Rgba8Unorm,
                1
            ),
            Err(Error::DeviceMismatch)
        ));
        let mut extra = super::tests::output(&foreign, 1);
        let mut frame = output.begin_frame().unwrap();
        assert!(matches!(
            frame.render_to(&mut extra).begin(),
            Err(Error::DeviceMismatch)
        ));
        {
            let mut pass = frame.render_pass().begin().unwrap();
            assert!(matches!(
                renderer.draw(&mut pass, &foreign_binding),
                Err(Error::DeviceMismatch)
            ));
            assert!(matches!(
                meshes.draw(&mut pass, &foreign_mesh),
                Err(Error::DeviceMismatch)
            ));
            assert!(matches!(
                foreign_meshes.draw(&mut pass, &own_mesh),
                Err(Error::DeviceMismatch)
            ));
            assert!(matches!(
                meshes.draw_with_material(&mut pass, &own_mesh, foreign_meshes.default_material()),
                Err(Error::DeviceMismatch)
            ));
            renderer.draw(&mut pass, &binding).unwrap();
        }
        frame.finish().unwrap();
        let mut foreign_target = super::tests::output(&foreign, 1);
        let mut frame = foreign_target.begin_frame().unwrap();
        assert!(matches!(
            renderer.draw(&mut frame.render_pass().begin().unwrap(), &binding),
            Err(Error::DeviceMismatch)
        ));
        drop(frame);
        assert!(scope.pop().await.is_none());
    });
}
