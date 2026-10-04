use super::*;
use crate::framebuffer::tests::pixels;
use crate::{
    DrawSpace, Framebuffer, FramebufferOptions, LineCap, MeshRenderer, TextureOptions, Vertex,
};
fn target(g: &GraphicsContext) -> Framebuffer {
    g.create_framebuffer(FramebufferOptions::new(64, 64).usage(
        wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
    ))
    .unwrap()
}
fn pixel(p: &[u8], x: usize, y: usize) -> [u8; 4] {
    p[(y * 64 + x) * 4..(y * 64 + x + 1) * 4]
        .try_into()
        .unwrap()
}
const RED: [f32; 4] = [1., 0., 0., 1.];
const GREEN: [f32; 4] = [0., 1., 0., 1.];
const BLUE: [f32; 4] = [0., 0., 1., 1.];

#[test]
fn painter_matches_direct_renderers_with_custom_pass_access() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let errors = g.device().push_error_scope(wgpu::ErrorFilter::Validation);
        let mut t = target(&g);
        let mut painter = Painter::new(&g);
        let mut shapes = ShapeRenderer::new(&g);
        let mut lines = LineRenderer::new(&g);
        let mut textures = TextureRenderer::new(&g);
        let mut meshes = MeshRenderer::new(&g);
        let texture = g.create_texture(TextureOptions::new(1, 1)).unwrap();
        texture.write(&[255; 4]).unwrap();
        // A binding from an independent renderer is usable by the facade.
        let image = textures
            .create_binding(texture.view(), TextureBindingOptions::new())
            .unwrap();
        painter.prepare(&t.render_format()).unwrap();
        painter.prepare_image(&image, &t.render_format()).unwrap();
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
        let placement = TextureDraw::new(Rect::new(8., 8., 24., 24.)).tint([0., 0., 1., 0.5]);
        let ellipse = ShapeDraw::ellipse(Rect::new(16., 16., 24., 24.), [0., 1., 0., 0.5]);
        let line = LineDraw::new([4., 44.], [60., 44.], GREEN)
            .width(4.)
            .cap(LineCap::Round);
        let direct = pixels(&g, &mut t, |f| {
            let mut p = f.render_pass().begin().unwrap();
            shapes
                .draw(&mut p, ShapeDraw::rect(Rect::new(0., 0., 64., 64.), RED))
                .unwrap();
            textures.draw(&mut p, &image, placement).unwrap();
            shapes.draw(&mut p, ellipse).unwrap();
            p.set_scissor_rect(0, 0, 32, 64).unwrap();
            meshes.draw(&mut p, &mesh).unwrap();
            p.as_wgpu().set_scissor_rect(0, 0, 1, 1);
            lines.draw(&mut p, line).unwrap();
            p.set_scissor_rect(0, 0, 64, 64).unwrap();
            shapes
                .draw(
                    &mut p,
                    ShapeDraw::rounded_rect(Rect::new(48., 4., 12., 12.), 3., GREEN),
                )
                .unwrap();
        });
        let painted = pixels(&g, &mut t, |f| {
            let mut p = f.render_pass().begin().unwrap();
            {
                let mut paint = painter.begin(&mut p).unwrap();
                paint.fill_rect(Rect::new(0., 0., 64., 64.), RED).unwrap();
                paint.draw_image(&image, placement).unwrap();
                paint.draw_shape(ellipse).unwrap();
                paint.pass().set_scissor_rect(0, 0, 32, 64).unwrap();
                meshes.draw(paint.pass(), &mesh).unwrap();
                paint.pass().as_wgpu().set_scissor_rect(0, 0, 1, 1);
                paint.draw_line(line).unwrap();
            }
            // Session drop leaves the same pass available, with no pending commands.
            p.set_scissor_rect(0, 0, 64, 64).unwrap();
            shapes
                .draw(
                    &mut p,
                    ShapeDraw::rounded_rect(Rect::new(48., 4., 12., 12.), 3., GREEN),
                )
                .unwrap();
        });
        assert_eq!(direct, painted);
        assert_eq!(pixel(&painted, 54, 10), [0, 255, 0, 255]);
        assert_eq!(pixel(&painted, 20, 44), [0, 255, 0, 255]);
        assert_eq!(pixel(&painted, 50, 44), [255, 0, 0, 255]);
        assert!(errors.pop().await.is_none());
    });
}
#[test]
fn nested_transform_scopes_preserve_parent_state_and_coordinate_units() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let mut t = target(&g);
        let mut painter = Painter::new(&g);
        let texture = g.create_texture(TextureOptions::new(1, 1)).unwrap();
        texture.write(&[255; 4]).unwrap();
        let image = painter
            .create_image_binding(texture.view(), TextureBindingOptions::new())
            .unwrap();
        let bytes = pixels(&g, &mut t, |f| {
            let mut p = f.render_pass().begin().unwrap();
            let mut paint = painter.begin(&mut p).unwrap();
            paint.fill_rect(Rect::new(0., 0., 8., 8.), RED).unwrap();
            {
                let mut parent = paint
                    .transformed(Transform2D::translation(16., 4.))
                    .unwrap();
                parent
                    .fill_rect(Rect::new(0., 0., 12., 16.), GREEN)
                    .unwrap();
                {
                    let mut child = parent.transformed(Transform2D::scale(2., 2.)).unwrap();
                    child.fill_rect(Rect::new(1., 1., 4., 4.), BLUE).unwrap();
                }
                assert_eq!(parent.transform(), Transform2D::translation(16., 4.));
                parent
                    .draw_image(&image, TextureDraw::new(Rect::new(8., 12., 4., 4.)))
                    .unwrap();
                parent
                    .draw_line(LineDraw::new([0., 22.], [12., 22.], GREEN).width(4.))
                    .unwrap();
            }
            assert_eq!(paint.transform(), Transform2D::IDENTITY);
            paint
                .draw_shape(
                    ShapeDraw::rect(Rect::new(0.75, 0.75, 0.125, 0.125), RED)
                        .space(DrawSpace::Normalized),
                )
                .unwrap();
        });
        assert_eq!(pixel(&bytes, 4, 4), [255, 0, 0, 255]);
        assert_eq!(pixel(&bytes, 16, 5), [0, 255, 0, 255]);
        assert_eq!(pixel(&bytes, 20, 8), [0, 0, 255, 255]);
        assert_eq!(pixel(&bytes, 25, 17), [255; 4]);
        assert_eq!(pixel(&bytes, 20, 26), [0, 255, 0, 255]);
        assert_eq!(pixel(&bytes, 50, 50), [255, 0, 0, 255]);
    });
}
#[test]
fn transformed_batches_match_individual_painting_and_reuse_scratch() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let mut t = target(&g);
        let mut painter = Painter::new(&g);
        let texture = g.create_texture(TextureOptions::new(1, 1)).unwrap();
        texture.write(&[255; 4]).unwrap();
        let image = painter
            .create_image_binding(texture.view(), TextureBindingOptions::new())
            .unwrap();
        let shapes = [
            ShapeDraw::rect(Rect::new(0., 0., 8., 8.), RED),
            ShapeDraw::ellipse(Rect::new(8., 0., 8., 8.), GREEN),
        ];
        let lines = [LineDraw::new([0., 12.], [16., 12.], BLUE).width(2.)];
        let images = [
            TextureDraw::new(Rect::new(0., 16., 8., 8.)),
            TextureDraw::new(Rect::new(8., 16., 8., 8.)).tint([0., 1., 1., 0.5]),
        ];
        let render = |f: &mut crate::Frame<'_, '_>, painter: &mut Painter, batch: bool| {
            let mut p = f.render_pass().begin().unwrap();
            let mut paint = painter.begin(&mut p).unwrap();
            let mut child = paint
                .transformed(Transform2D::scale(2., 2.).then(Transform2D::translation(4., 4.)))
                .unwrap();
            if batch {
                child.draw_shapes(&shapes).unwrap();
                child.draw_lines(&lines).unwrap();
                child.draw_images(&image, &images).unwrap();
            } else {
                for &d in &shapes {
                    child.draw_shape(d).unwrap();
                }
                for &d in &lines {
                    child.draw_line(d).unwrap();
                }
                for &d in &images {
                    child.draw_image(&image, d).unwrap();
                }
            }
        };
        let individual = pixels(&g, &mut t, |f| render(f, &mut painter, false));
        let batched = pixels(&g, &mut t, |f| render(f, &mut painter, true));
        assert_eq!(individual, batched);
        let capacities = [
            painter.shape_scratch.capacity(),
            painter.line_scratch.capacity(),
            painter.image_scratch.capacity(),
        ];
        let buffers: Vec<_> = g
            .upload_pool
            .lock()
            .unwrap()
            .iter()
            .map(|(b, _)| b.clone())
            .collect();
        pixels(&g, &mut t, |f| render(f, &mut painter, true));
        assert_eq!(
            [
                painter.shape_scratch.capacity(),
                painter.line_scratch.capacity(),
                painter.image_scratch.capacity()
            ],
            capacities
        );
        assert!(
            g.upload_pool
                .lock()
                .unwrap()
                .iter()
                .all(|(b, _)| buffers.contains(b))
        );
    });
}
#[test]
fn invalid_children_batches_devices_and_feedback_leave_session_usable() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let foreign = GraphicsContext::headless().await.unwrap();
        let mut t = target(&g);
        let mut painter = Painter::new(&g);
        let mut foreign_painter = Painter::new(&foreign);
        let image = g.create_texture(TextureOptions::new(1, 1)).unwrap();
        image.write(&[255; 4]).unwrap();
        let binding = painter
            .create_image_binding(image.view(), TextureBindingOptions::new())
            .unwrap();
        let foreign_image = foreign.create_texture(TextureOptions::new(1, 1)).unwrap();
        let foreign_binding = foreign_painter
            .create_image_binding(foreign_image.view(), TextureBindingOptions::new())
            .unwrap();
        let feedback = painter.create_sampled_binding(&t.sampled_color()).unwrap();
        let bytes = pixels(&g, &mut t, |f| {
            let mut p = f
                .render_pass()
                .clear_color(wgpu::Color::BLUE)
                .begin()
                .unwrap();
            assert!(matches!(
                foreign_painter.begin(&mut p),
                Err(Error::DeviceMismatch)
            ));
            let mut paint = painter.begin(&mut p).unwrap();
            assert!(matches!(
                paint.transformed([f32::INFINITY; 6]),
                Err(Error::InvalidTransform2D)
            ));
            assert_eq!(paint.transform(), Transform2D::IDENTITY);
            assert!(matches!(
                paint.draw_image(&feedback, TextureDraw::default()),
                Err(Error::TextureFeedback)
            ));
            assert!(matches!(
                paint.draw_image(&foreign_binding, TextureDraw::default()),
                Err(Error::DeviceMismatch)
            ));
            {
                let mut child = paint.transformed(Transform2D::translation(1., 1.)).unwrap();
                assert!(matches!(
                    child.draw_shapes(&[
                        ShapeDraw::rect(Rect::new(0., 0., 64., 64.), RED),
                        ShapeDraw::ellipse(Rect::new(0., 0., -1., 1.), RED)
                    ]),
                    Err(Error::InvalidShapeDraw)
                ));
                assert!(matches!(
                    child.draw_lines(&[
                        LineDraw::new([0., 32.], [64., 32.], RED).width(64.),
                        LineDraw::new([0.; 2], [1.; 2], RED).width(-1.)
                    ]),
                    Err(Error::InvalidLineDraw)
                ));
                assert!(matches!(
                    child.draw_images(
                        &binding,
                        &[
                            TextureDraw::default().tint(RED),
                            TextureDraw::new(Rect::new(0., 0., -1., 1.))
                        ]
                    ),
                    Err(Error::InvalidTextureDraw)
                ));
            }
            paint.fill_rect(Rect::new(0., 0., 8., 8.), GREEN).unwrap();
        });
        assert_eq!(pixel(&bytes, 32, 32), [0, 0, 255, 255]);
        assert_eq!(pixel(&bytes, 4, 4), [0, 255, 0, 255]);
    });
}
