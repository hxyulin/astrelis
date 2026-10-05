use super::*;
use crate::TextRasterOptions;
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
            ShapeDraw::ellipse(Rect::new(8., 0., 8., 8.), GREEN).stroke(Stroke::new(1.).inside()),
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
fn painter_outline_helpers_match_direct_shapes_under_affine_transforms() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let mut t = target(&g);
        let mut painter = Painter::new(&g);
        let mut shapes = ShapeRenderer::new(&g);
        let transform = Transform2D([1.2, 0.2, 0.3, 0.8, 4., 6.]);
        let rect = Rect::new(2., 2., 18., 14.);
        let rounded = Rect::new(22., 2., 16., 16.);
        let ellipse = Rect::new(8., 26., 28., 14.);
        let direct = pixels(&g, &mut t, |f| {
            let mut p = f.render_pass().begin().unwrap();
            shapes
                .draw_many(
                    &mut p,
                    &[
                        ShapeDraw::rect(rect, RED)
                            .stroke(Stroke::new(2.).inside())
                            .transform(transform),
                        ShapeDraw::rounded_rect(rounded, 5., GREEN)
                            .stroke(Stroke::new(2.))
                            .transform(transform),
                        ShapeDraw::ellipse(ellipse, BLUE)
                            .stroke(Stroke::new(2.).outside())
                            .transform(transform),
                    ],
                )
                .unwrap();
        });
        let painted = pixels(&g, &mut t, |f| {
            let mut p = f.render_pass().begin().unwrap();
            let mut paint = painter.begin(&mut p).unwrap();
            let mut local = paint.transformed(transform).unwrap();
            local
                .stroke_rect(rect, Stroke::new(2.).inside(), RED)
                .unwrap();
            local
                .stroke_rounded_rect(rounded, 5., Stroke::new(2.), GREEN)
                .unwrap();
            local
                .stroke_ellipse(ellipse, Stroke::new(2.).outside(), BLUE)
                .unwrap();
        });
        assert_eq!(direct, painted);
        assert!(painted.iter().any(|&byte| byte != 0));
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

fn text_layout(content: &str) -> std::sync::Arc<TextLayout> {
    let mut fonts = crate::TextSystem::new();
    fonts
        .load_font(include_bytes!("../tests/fonts/TestColor.ttf"))
        .unwrap();
    let mut buffer = crate::TextBuffer::new();
    buffer
        .set_text(
            content,
            crate::TextStyle::new()
                .family("Astrelis Test Color")
                .font_size(20.)
                .line_height(24.),
        )
        .unwrap();
    buffer.layout(&mut fonts).unwrap()
}

#[test]
fn painter_text_matches_direct_layers_nested_transforms_and_pass_state() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let errors = g.device().push_error_scope(wgpu::ErrorFilter::Validation);
        for samples in [1, 4] {
            let mut t = g
                .create_framebuffer(
                    FramebufferOptions::new(64, 64)
                        .sample_count(samples)
                        .depth_stencil(wgpu::TextureFormat::Depth24PlusStencil8)
                        .usage(
                            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                        ),
                )
                .unwrap();
            let mut painter = Painter::new(&g);
            let mut text = TextRenderer::new(&g);
            let mut shapes = ShapeRenderer::new(&g);
            let mut lines = LineRenderer::new(&g);
            let mut meshes = MeshRenderer::new(&g);
            let texture = g.create_texture(TextureOptions::new(1, 1)).unwrap();
            texture.write(&[255; 4]).unwrap();
            let mut images = TextureRenderer::new(&g);
            let image = images
                .create_binding(texture.view(), TextureBindingOptions::new())
                .unwrap();
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
            painter.prepare(&t.render_format()).unwrap();
            painter.prepare_image(&image, &t.render_format()).unwrap();
            // Two separately prepared resources can be drawn by either renderer.
            let mask = painter
                .prepare_text(&text_layout("M"), TextRasterOptions::new().scale_factor(2.))
                .unwrap();
            let colored = text
                .prepare_text(&text_layout("😀😁"), TextRasterOptions::new())
                .unwrap();
            let parent = Transform2D::translation(8., 2.);
            let child = Transform2D::scale(0.75, 0.75);
            let draw = TextDraw::new([2., 2.])
                .color([1., 0., 0., 0.8])
                .opacity(0.8)
                .transform_2d(Transform2D::translation(1., 0.));
            let overlay = ShapeDraw::rect(Rect::new(14., 10., 8., 8.), [0., 1., 0., 0.5]);
            let line = LineDraw::new([0., 20.], [40., 20.], GREEN).width(2.);
            let direct = pixels(&g, &mut t, |f| {
                let mut p = f.render_pass().begin().unwrap();
                p.set_viewport(4., 4., 56., 56., 0., 1.).unwrap();
                p.set_scissor_rect(4, 4, 50, 50).unwrap();
                images
                    .draw(
                        &mut p,
                        &image,
                        TextureDraw::default().tint([0.05, 0.05, 0.05, 1.]),
                    )
                    .unwrap();
                text.draw(
                    &mut p,
                    &mask,
                    TextDraw {
                        transform: draw.transform.then(child).then(parent),
                        ..draw
                    },
                )
                .unwrap();
                shapes.draw(&mut p, overlay).unwrap();
                meshes.draw(&mut p, &mesh).unwrap();
                // A raw state mutation must be restored by the following text draw.
                p.as_wgpu().set_scissor_rect(0, 0, 1, 1);
                text.draw(
                    &mut p,
                    &colored,
                    TextDraw::new([0., 30.]).opacity(0.7).transform_2d(parent),
                )
                .unwrap();
                lines
                    .draw(&mut p, line.transform(child.then(parent)))
                    .unwrap();
                text.draw(&mut p, &mask, TextDraw::new([44., 0.]).color(GREEN))
                    .unwrap();
            });
            let before = painter.text().stats();
            let painted = pixels(&g, &mut t, |f| {
                let mut p = f.render_pass().begin().unwrap();
                let mut paint = painter.begin(&mut p).unwrap();
                paint.pass().set_viewport(4., 4., 56., 56., 0., 1.).unwrap();
                paint.pass().set_scissor_rect(4, 4, 50, 50).unwrap();
                paint
                    .draw_image(&image, TextureDraw::default().tint([0.05, 0.05, 0.05, 1.]))
                    .unwrap();
                {
                    let mut local = paint.transformed(parent).unwrap();
                    {
                        let mut nested = local.transformed(child).unwrap();
                        nested.draw_text(&mask, draw).unwrap();
                    }
                    // Pass access bypasses the Painter transform, including custom renderers.
                    shapes.draw(local.pass(), overlay).unwrap();
                    meshes.draw(local.pass(), &mesh).unwrap();
                    local.pass().as_wgpu().set_scissor_rect(0, 0, 1, 1);
                    local
                        .draw_text(&colored, TextDraw::new([0., 30.]).opacity(0.7))
                        .unwrap();
                    local.transformed(child).unwrap().draw_line(line).unwrap();
                }
                assert_eq!(paint.transform(), Transform2D::IDENTITY);
                paint
                    .draw_text(&mask, TextDraw::new([44., 0.]).color(GREEN))
                    .unwrap();
            });
            assert_eq!(direct, painted);
            assert_eq!(pixel(&painted, 0, 0), [0; 4]);
            assert!(
                painted
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|p| p[0] > 30 && p[0] > p[2])
            );
            assert!(
                painted
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|p| p[1] > 100 && p[1] > p[2])
            );
            let after = painter.text().stats();
            assert_eq!(before.cache_misses, after.cache_misses);
            assert_eq!(before.uploaded_bytes, after.uploaded_bytes);
            assert_eq!(before.geometry_bytes, after.geometry_bytes);
            assert_eq!(after.parameter_bytes - before.parameter_bytes, 3 * 48);
            assert_eq!(after.draw_calls - before.draw_calls, 3);
        }
        assert!(errors.pop().await.is_none());
    });
}

#[test]
fn rejected_painter_text_draws_leave_session_and_cached_resources_usable() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let foreign = GraphicsContext::headless().await.unwrap();
        let errors = g.device().push_error_scope(wgpu::ErrorFilter::Validation);
        let mut t = target(&g);
        let mut painter = Painter::new(&g);
        let layout = text_layout("M");
        let mut other = TextRenderer::new(&foreign);
        let foreign_text = other
            .prepare_text(&layout, TextRasterOptions::new())
            .unwrap();
        let text = painter
            .prepare_text(&layout, TextRasterOptions::new())
            .unwrap();
        painter.prepare(&t.render_format()).unwrap();
        painter.text().clear_cache();
        assert_eq!(painter.text().stats().live_pages, 1);
        let before = painter.text().stats();
        let bytes = pixels(&g, &mut t, |f| {
            let mut p = f.render_pass().begin().unwrap();
            let mut paint = painter.begin(&mut p).unwrap();
            assert!(matches!(
                paint.draw_text(&foreign_text, TextDraw::default()),
                Err(TextRenderError::Graphics(Error::DeviceMismatch))
            ));
            assert!(matches!(
                paint.draw_text(&text, TextDraw::default().opacity(2.)),
                Err(TextRenderError::InvalidOptions)
            ));
            {
                let mut local = paint
                    .transformed(Transform2D::scale(f32::MAX, f32::MAX))
                    .unwrap();
                assert!(matches!(
                    local.draw_text(
                        &text,
                        TextDraw::default().transform_2d(Transform2D::scale(2., 2.))
                    ),
                    Err(TextRenderError::InvalidOptions)
                ));
            }
            paint
                .draw_text(&text, TextDraw::new([24., 0.]).color(GREEN))
                .unwrap();
        });
        assert_eq!(pixel(&bytes, 5, 10), [0; 4]);
        assert_eq!(pixel(&bytes, 29, 10), [0, 255, 0, 255]);
        let after = painter.text().stats();
        assert_eq!(after.parameter_bytes - before.parameter_bytes, 48);
        assert_eq!(after.draw_calls - before.draw_calls, 1);
        assert_eq!(after.uploaded_bytes, before.uploaded_bytes);
        assert_eq!(after.geometry_bytes, before.geometry_bytes);
        drop(text);
        assert_eq!(painter.text().stats().live_pages, 0);
        assert!(errors.pop().await.is_none());
    });
}
