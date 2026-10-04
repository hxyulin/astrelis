use super::*;
use crate::framebuffer::tests::pixels;
use crate::{
    Framebuffer, FramebufferOptions, MeshRenderer, TextureBindingOptions, TextureDraw,
    TextureOptions, TextureRenderer, Vertex,
};

fn target(g: &GraphicsContext, samples: u32, depth: bool) -> Framebuffer {
    let mut options = FramebufferOptions::new(64, 64).sample_count(samples).usage(
        wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
    );
    if depth {
        options = options.depth_stencil(wgpu::TextureFormat::Depth24PlusStencil8);
    }
    g.create_framebuffer(options).unwrap()
}
fn pixel(p: &[u8], x: usize, y: usize) -> [u8; 4] {
    p[(y * 64 + x) * 4..(y * 64 + x + 1) * 4]
        .try_into()
        .unwrap()
}
fn near(p: [u8; 4], expected: [u8; 4]) {
    for (a, b) in p.into_iter().zip(expected) {
        assert!(a.abs_diff(b) <= 2, "{p:?} != {expected:?}");
    }
}
const RED: [f32; 4] = [1., 0., 0., 1.];
const GREEN: [f32; 4] = [0., 1., 0., 1.];
const BLUE: [f32; 4] = [0., 0., 1., 1.];

#[test]
fn shapes_and_line_caps_have_expected_coverage() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let errors = g.device().push_error_scope(wgpu::ErrorFilter::Validation);
        let mut t = target(&g, 1, false);
        let mut shapes = ShapeRenderer::new(&g);
        let mut lines = LineRenderer::new(&g);
        shapes.prepare(&t.render_format()).unwrap();
        lines.prepare(&t.render_format()).unwrap();
        let bytes = pixels(&g, &mut t, |f| {
            let mut p = f.render_pass().begin().unwrap();
            shapes
                .draw_many(
                    &mut p,
                    &[
                        ShapeDraw::rect(Rect::new(2., 2., 12., 12.), RED),
                        ShapeDraw::rounded_rect(Rect::new(20., 2., 16., 16.), 8., GREEN),
                        ShapeDraw::ellipse(Rect::new(42., 2., 18., 12.), BLUE),
                    ],
                )
                .unwrap();
            for (y, cap) in [
                (28., LineCap::Butt),
                (40., LineCap::Square),
                (52., LineCap::Round),
            ] {
                lines
                    .draw(
                        &mut p,
                        LineDraw::new([10., y], [24., y], RED).width(8.).cap(cap),
                    )
                    .unwrap();
            }
            lines
                .draw_many(
                    &mut p,
                    &[
                        LineDraw::new([44., 28.], [44., 28.], GREEN)
                            .width(8.)
                            .cap(LineCap::Round),
                        LineDraw::new([44., 42.], [44., 42.], BLUE)
                            .width(8.)
                            .cap(LineCap::Square),
                        LineDraw::new([44., 54.], [44., 54.], RED).width(8.),
                    ],
                )
                .unwrap();
        });
        assert_eq!(pixel(&bytes, 8, 8), [255, 0, 0, 255]);
        assert_eq!(pixel(&bytes, 28, 10), [0, 255, 0, 255]);
        assert_eq!(pixel(&bytes, 20, 2), [0; 4]);
        assert_eq!(pixel(&bytes, 51, 8), [0, 0, 255, 255]);
        assert_eq!(pixel(&bytes, 42, 2), [0; 4]);
        assert_eq!(pixel(&bytes, 7, 28), [0; 4]);
        assert_eq!(pixel(&bytes, 7, 40), [255, 0, 0, 255]);
        assert_eq!(pixel(&bytes, 7, 52), [255, 0, 0, 255]);
        assert_eq!(pixel(&bytes, 6, 48), [0; 4]);
        assert_eq!(pixel(&bytes, 44, 28), [0, 255, 0, 255]);
        assert_eq!(pixel(&bytes, 41, 39), [0, 0, 255, 255]);
        assert_eq!(pixel(&bytes, 44, 54), [0; 4]);
        assert!(errors.pop().await.is_none());
    });
}
#[test]
fn batches_match_individual_draws_and_preserve_translucent_order() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let mut t = target(&g, 1, false);
        let mut shapes = ShapeRenderer::new(&g);
        let mut lines = LineRenderer::new(&g);
        let draws = [
            ShapeDraw::rect(Rect::new(4., 4., 40., 40.), [1., 0., 0., 0.5]),
            ShapeDraw::ellipse(Rect::new(12., 12., 40., 40.), [0., 0., 1., 0.5]),
            ShapeDraw::rounded_rect(Rect::new(20., 20., 32., 32.), 8., [0., 1., 0., 0.5]),
        ];
        let segments = [
            LineDraw::new([4., 58.], [58., 4.], RED)
                .width(3.)
                .cap(LineCap::Round),
            LineDraw::new([4., 4.], [58., 58.], [0., 1., 0., 0.5])
                .width(4.)
                .cap(LineCap::Square),
        ];
        let individual = pixels(&g, &mut t, |f| {
            let mut p = f.render_pass().begin().unwrap();
            for d in draws {
                shapes.draw(&mut p, d).unwrap();
            }
            for d in segments {
                lines.draw(&mut p, d).unwrap();
            }
        });
        let batched = pixels(&g, &mut t, |f| {
            let mut p = f.render_pass().begin().unwrap();
            shapes.draw_many(&mut p, &draws).unwrap();
            lines.draw_many(&mut p, &segments).unwrap();
        });
        assert_eq!(individual, batched);
        near(pixel(&batched, 32, 25), [32, 128, 64, 223]);
        // Batch crosses three page boundaries; the last overlapping draw must win.
        let mut many = vec![ShapeDraw::rect(Rect::new(0., 0., 64., 64.), RED); 2050];
        many[2049].color = BLUE;
        let bytes = pixels(&g, &mut t, |f| {
            shapes
                .draw_many(&mut f.render_pass().begin().unwrap(), &many)
                .unwrap();
        });
        assert_eq!(pixel(&bytes, 32, 32), [0, 0, 255, 255]);
    });
}
#[test]
fn viewport_transforms_clipping_and_fractional_edges_are_consistent() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let mut t = target(&g, 1, false);
        let mut shapes = ShapeRenderer::new(&g);
        let bytes = pixels(&g, &mut t, |f| {
            let mut p = f.render_pass().begin().unwrap();
            p.set_viewport(16., 8., 32., 40., 0., 1.).unwrap();
            p.set_scissor_rect(16, 8, 32, 40).unwrap();
            shapes
                .draw(
                    &mut p,
                    ShapeDraw::rect(Rect::new(0., 0., 0.5, 0.5), RED).space(DrawSpace::Normalized),
                )
                .unwrap();
            shapes
                .draw(
                    &mut p,
                    ShapeDraw::rect(Rect::new(0., 0., 8., 8.), GREEN).transform(
                        Transform2D::rotation(std::f32::consts::FRAC_PI_2)
                            .then(Transform2D::translation(24., 24.)),
                    ),
                )
                .unwrap();
            p.set_viewport(0., 0., 64., 64., 0., 1.).unwrap();
            p.set_scissor_rect(0, 0, 64, 64).unwrap();
            shapes
                .draw(&mut p, ShapeDraw::rect(Rect::new(4.5, 52., 8., 8.), BLUE))
                .unwrap();
            shapes
                .draw(
                    &mut p,
                    ShapeDraw::rect(Rect::new(24.5, 52., 8., 8.), RED)
                        .antialiasing(EdgeAntialiasing::None),
                )
                .unwrap();
            shapes
                .draw(
                    &mut p,
                    ShapeDraw::rect(Rect::new(0., 0., 64., 64.), BLUE)
                        .transform(Transform2D::scale(0., 1.)),
                )
                .unwrap();
        });
        assert_eq!(pixel(&bytes, 18, 10), [255, 0, 0, 255]);
        assert_eq!(pixel(&bytes, 10, 10), [0; 4]);
        assert_eq!(pixel(&bytes, 34, 36), [0, 255, 0, 255]);
        near(pixel(&bytes, 4, 56), [0, 0, 128, 128]);
        assert_eq!(pixel(&bytes, 3, 56), [0; 4]);
        assert_eq!(pixel(&bytes, 24, 56), [255, 0, 0, 255]);
    });
}
#[test]
fn scoped_restoration_shares_mesh_texture_msaa_and_readonly_depth() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let errors = g.device().push_error_scope(wgpu::ErrorFilter::Validation);
        let mut t = target(&g, 4, true);
        let mut shapes = ShapeRenderer::new(&g);
        let mut lines = LineRenderer::new(&g);
        let mut meshes = MeshRenderer::new(&g);
        let mut textures = TextureRenderer::new(&g);
        let mesh = g
            .create_mesh(
                &[
                    Vertex::new([-1., -1., 0.], BLUE),
                    Vertex::new([1., -1., 0.], BLUE),
                    Vertex::new([0., 1., 0.], BLUE),
                ],
                &[0, 1, 2],
            )
            .unwrap();
        let image = g.create_texture(TextureOptions::new(1, 1)).unwrap();
        image.write(&[255; 4]).unwrap();
        let binding = textures
            .create_binding(image.view(), TextureBindingOptions::new())
            .unwrap();
        let bytes = pixels(&g, &mut t, |f| {
            // Initialize depth/stencil before taking both aspects read-only.
            drop(f.render_pass().begin().unwrap());
            let mut p = f
                .render_pass()
                .load_all()
                .depth_ops(None)
                .stencil_ops(None)
                .begin()
                .unwrap();
            let mut s = shapes.bind(&mut p).unwrap();
            s.draw(ShapeDraw::rect(Rect::new(0., 0., 64., 64.), RED))
                .unwrap();
            meshes.draw(s.pass(), &mesh).unwrap();
            textures
                .draw(
                    s.pass(),
                    &binding,
                    TextureDraw::new(Rect::new(0., 0., 8., 8.)),
                )
                .unwrap();
            lines
                .draw(
                    s.pass(),
                    LineDraw::new([8., 56.], [56., 56.], GREEN).width(4.),
                )
                .unwrap();
            s.pass().as_wgpu().set_scissor_rect(0, 0, 1, 1);
            s.draw(ShapeDraw::ellipse(Rect::new(20., 20., 24., 24.), GREEN))
                .unwrap();
            drop(s);
            let mut l = lines.bind(&mut p).unwrap();
            l.draw(LineDraw::new([4., 12.], [60., 12.], BLUE).width(4.))
                .unwrap();
            shapes
                .draw(l.pass(), ShapeDraw::rect(Rect::new(48., 16., 8., 8.), RED))
                .unwrap();
            shapes
                .draw(l.pass(), ShapeDraw::rect(Rect::new(0., 40., 64., 8.), RED))
                .unwrap();
            l.pass().set_scissor_rect(0, 0, 32, 64).unwrap();
            l.draw(LineDraw::new([4., 44.], [60., 44.], BLUE).width(4.))
                .unwrap();
        });
        assert_eq!(pixel(&bytes, 32, 32), [0, 255, 0, 255]);
        assert_eq!(pixel(&bytes, 4, 4), [255; 4]);
        assert_eq!(pixel(&bytes, 20, 12), [0, 0, 255, 255]);
        assert_eq!(pixel(&bytes, 20, 44), [0, 0, 255, 255]);
        assert_eq!(pixel(&bytes, 50, 44), [255, 0, 0, 255]);
        assert!(errors.pop().await.is_none());
    });
}
#[test]
fn invalid_batches_are_atomic_and_warm_resources_are_reused() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let mut t = target(&g, 1, false);
        let mut shapes = ShapeRenderer::new(&g);
        let mut lines = LineRenderer::new(&g);
        shapes.prepare(&t.render_format()).unwrap();
        lines.prepare(&t.render_format()).unwrap();
        let batch = vec![ShapeDraw::rect(Rect::new(0., 0., 64., 64.), RED); 10];
        let render = |f: &mut crate::Frame<'_, '_>, s: &mut ShapeRenderer, l: &mut LineRenderer| {
            let mut p = f
                .render_pass()
                .clear_color(wgpu::Color::BLUE)
                .begin()
                .unwrap();
            assert!(matches!(
                s.draw_many(
                    &mut p,
                    &[
                        batch[0],
                        ShapeDraw::ellipse(Rect::new(0., 0., -1., 1.), RED)
                    ]
                ),
                Err(Error::InvalidShapeDraw)
            ));
            assert!(matches!(
                l.draw_many(
                    &mut p,
                    &[
                        LineDraw::new([0., 32.], [64., 32.], RED).width(64.),
                        LineDraw::new([0.; 2], [1.; 2], RED).width(-1.)
                    ]
                ),
                Err(Error::InvalidLineDraw)
            ));
            // Overflow is rejected before drawing, even though the supplied values are finite.
            assert!(matches!(
                s.draw(
                    &mut p,
                    ShapeDraw::rect(Rect::new(f32::MAX, 0., f32::MAX, 8.), RED)
                ),
                Err(Error::InvalidShapeDraw)
            ));
            assert!(matches!(
                l.draw(&mut p, LineDraw::new([-f32::MAX, 0.], [f32::MAX, 0.], RED)),
                Err(Error::InvalidLineDraw)
            ));
            s.draw_many(&mut p, &batch).unwrap();
        };
        pixels(&g, &mut t, |f| render(f, &mut shapes, &mut lines));
        let buffer = g.upload_pool.lock().unwrap()[0].0.clone();
        let capacity = shapes.inner.parameters.capacity();
        let pipeline = shapes.inner.pipelines.values().next().unwrap().clone();
        pixels(&g, &mut t, |f| render(f, &mut shapes, &mut lines));
        assert_eq!(g.upload_pool.lock().unwrap()[0].0, buffer);
        assert_eq!(shapes.inner.parameters.capacity(), capacity);
        assert_eq!(shapes.inner.pipelines.values().next().unwrap(), &pipeline);
        let bytes = pixels(&g, &mut t, |f| {
            let mut p = f
                .render_pass()
                .clear_color(wgpu::Color::BLUE)
                .begin()
                .unwrap();
            assert!(
                shapes
                    .draw_many(
                        &mut p,
                        &[
                            batch[0],
                            ShapeDraw::rect(Rect::new(0., 0., 64., 64.), [0., 0., 0., 2.])
                        ]
                    )
                    .is_err()
            );
            assert!(
                lines
                    .draw_many(
                        &mut p,
                        &[
                            LineDraw::new([0., 32.], [64., 32.], RED).width(64.),
                            LineDraw::new([f32::NAN, 0.], [0.; 2], RED)
                        ]
                    )
                    .is_err()
            );
        });
        assert_eq!(pixel(&bytes, 32, 32), [0, 0, 255, 255]);
        let foreign = GraphicsContext::headless().await.unwrap();
        let mut foreign_shapes = ShapeRenderer::new(&foreign);
        let mut f = t.begin_frame().unwrap();
        let mut p = f.render_pass().begin().unwrap();
        assert!(matches!(
            foreign_shapes.bind(&mut p),
            Err(Error::DeviceMismatch)
        ));
    });
}
#[test]
fn mixed_instance_strides() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let mut t = target(&g, 1, false);
        let mut s = ShapeRenderer::new(&g);
        let mut r = TextureRenderer::new(&g);
        let i = g.create_texture(TextureOptions::new(1, 1)).unwrap();
        i.write(&[255; 4]).unwrap();
        let b = r
            .create_binding(i.view(), TextureBindingOptions::new())
            .unwrap();
        let bytes = pixels(&g, &mut t, |f| {
            let mut p = f.render_pass().begin().unwrap();
            s.draw(&mut p, ShapeDraw::rect(Rect::new(0., 0., 64., 64.), RED))
                .unwrap();
            r.draw(&mut p, &b, TextureDraw::new(Rect::new(0., 0., 8., 8.)))
                .unwrap();
        });
        assert_eq!(pixel(&bytes, 4, 4), [255; 4]);
    });
}

#[test]
fn subpixel_geometry_does_not_become_a_half_opaque_pixel() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let mut t = target(&g, 1, false);
        let mut lines = LineRenderer::new(&g);
        let mut shapes = ShapeRenderer::new(&g);
        let bytes = pixels(&g, &mut t, |f| {
            let mut p = f.render_pass().begin().unwrap();
            lines
                .draw(
                    &mut p,
                    LineDraw::new([2., 8.5], [14., 8.5], RED).width(0.25),
                )
                .unwrap();
            shapes
                .draw(
                    &mut p,
                    ShapeDraw::rect(Rect::new(20.375, 16., 0.25, 8.), GREEN),
                )
                .unwrap();
            lines
                .draw(
                    &mut p,
                    LineDraw::new([2., 32.5], [14., 32.5], BLUE).width(f32::from_bits(1)),
                )
                .unwrap();
        });
        near(pixel(&bytes, 8, 8), [64, 0, 0, 64]);
        near(pixel(&bytes, 20, 20), [0, 64, 0, 64]);
        assert_eq!(pixel(&bytes, 8, 32), [0; 4]);
    });
}
