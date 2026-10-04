use super::*;
use crate::framebuffer::tests::pixels;
use crate::{Framebuffer, FramebufferOptions, ShapeDraw, ShapeRenderer};

fn layout(text: &str) -> Arc<TextLayout> {
    let mut system = super::super::TextSystem::new();
    system
        .load_font(include_bytes!("../../tests/fonts/TestColor.ttf"))
        .unwrap();
    let mut buffer = super::super::TextBuffer::new();
    buffer
        .set_text(
            text,
            super::super::TextStyle::new()
                .family("Astrelis Test Color")
                .font_size(20.)
                .line_height(24.),
        )
        .unwrap();
    buffer.layout(&mut system).unwrap()
}
fn target(g: &GraphicsContext, samples: u32) -> Framebuffer {
    g.create_framebuffer(
        FramebufferOptions::new(64, 64)
            .sample_count(samples)
            .depth_stencil(wgpu::TextureFormat::Depth24PlusStencil8)
            .usage(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC),
    )
    .unwrap()
}
fn pixel(p: &[u8], x: usize, y: usize) -> [u8; 4] {
    p[(y * 64 + x) * 4..(y * 64 + x + 1) * 4]
        .try_into()
        .unwrap()
}
fn near(actual: [u8; 4], expected: [u8; 4]) {
    for (a, b) in actual.into_iter().zip(expected) {
        assert!(a.abs_diff(b) <= 3, "{actual:?} != {expected:?}");
    }
}

#[test]
fn coverage_color_and_bitmap_blend_in_linear_premultiplied_space() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let mut t = target(&g, 1);
        let errors = g.device().push_error_scope(wgpu::ErrorFilter::Validation);
        let mut renderer = TextRenderer::new(&g);
        renderer.prepare(&t.render_format()).unwrap();
        let mask = renderer
            .prepare_text(&layout("M"), TextRasterOptions::new())
            .unwrap();
        let color = renderer
            .prepare_text(&layout("😀"), TextRasterOptions::new())
            .unwrap();
        let bitmap = renderer
            .prepare_text(&layout("😁"), TextRasterOptions::new())
            .unwrap();
        assert_eq!(mask.glyph_count(), 1);
        assert_eq!(color.glyph_count(), 1);
        assert_eq!(bitmap.glyph_count(), 1);
        let bytes = pixels(&g, &mut t, |frame| {
            let mut pass = frame.render_pass().begin().unwrap();
            renderer
                .draw(
                    &mut pass,
                    &mask,
                    TextDraw::new([0., 0.]).color([0., 0., 1., 1.]).opacity(0.5),
                )
                .unwrap();
            renderer
                .draw(
                    &mut pass,
                    &color,
                    TextDraw::new([24., 0.])
                        .color([0., 0., 1., 1.])
                        .opacity(0.5),
                )
                .unwrap();
            renderer
                .draw(
                    &mut pass,
                    &bitmap,
                    TextDraw::new([0., 28.])
                        .color([1., 0., 0., 1.])
                        .opacity(0.5),
                )
                .unwrap();
        });
        near(pixel(&bytes, 5, 10), [0, 0, 128, 128]);
        near(pixel(&bytes, 27, 10), [127, 0, 0, 127]);
        // Half-transparent green COLR layer over opaque red: decode its sRGB mix before linear blending.
        near(pixel(&bytes, 34, 10), [27, 27, 0, 127]);
        near(pixel(&bytes, 41, 10), [0, 63, 0, 63]);
        near(pixel(&bytes, 5, 38), [3, 14, 64, 64]);
        assert!(errors.pop().await.is_none());
    });
}

#[test]
fn dpi_clipping_msaa_transforms_and_interleaving_preserve_order() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let mut t = target(&g, 4);
        let mut renderer = TextRenderer::new(&g);
        renderer.prepare(&t.render_format()).unwrap();
        let text = renderer
            .prepare_text(&layout("M"), TextRasterOptions::new().scale_factor(2.))
            .unwrap();
        assert_eq!(text.size(), [40., 48.]);
        let mut shapes = ShapeRenderer::new(&g);
        shapes.prepare(&t.render_format()).unwrap();
        let before = renderer.stats();
        let bytes = pixels(&g, &mut t, |frame| {
            let mut pass = frame.render_pass().begin().unwrap();
            pass.set_scissor_rect(4, 4, 30, 40).unwrap();
            renderer
                .draw(
                    &mut pass,
                    &text,
                    TextDraw::new([0., 0.]).color([1., 0., 0., 1.]),
                )
                .unwrap();
            shapes
                .draw(
                    &mut pass,
                    ShapeDraw::rect(Rect::new(10., 10., 20., 20.), [0., 1., 0., 1.]),
                )
                .unwrap();
            renderer
                .draw(
                    &mut pass,
                    &text,
                    TextDraw::new([0., 0.])
                        .color([0., 0., 1., 1.])
                        .opacity(0.5)
                        .transform_2d(Transform2D::translation(2., 0.)),
                )
                .unwrap();
        });
        near(pixel(&bytes, 0, 10), [0; 4]);
        near(pixel(&bytes, 40, 10), [0; 4]);
        near(pixel(&bytes, 15, 15), [0, 128, 128, 255]);
        near(pixel(&bytes, 5, 8), [128, 0, 128, 255]);
        assert_eq!(renderer.stats().cache_misses, before.cache_misses);
        assert_eq!(renderer.stats().uploaded_bytes, before.uploaded_bytes);
        assert_eq!(renderer.stats().geometry_bytes, before.geometry_bytes);
        assert_eq!(
            renderer.stats().parameter_bytes - before.parameter_bytes,
            96
        );
        assert_eq!(renderer.stats().draw_calls - before.draw_calls, 2);
    });
}

#[test]
fn prepared_texts_cache_clearing_and_overlapping_recordings_keep_their_images() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let mut a = target(&g, 1);
        let mut b = target(&g, 1);
        let mut renderer = TextRenderer::new(&g);
        let first = renderer
            .prepare_text(&layout("M"), TextRasterOptions::new())
            .unwrap();
        let misses = renderer.stats().cache_misses;
        let uploaded = renderer.stats().uploaded_bytes;
        let repeat = renderer
            .prepare_text(&layout("M"), TextRasterOptions::new())
            .unwrap();
        // Different TextSystems have distinct source identities and cannot alias caches.
        assert!(renderer.stats().cache_misses > misses);
        drop(repeat);
        let cpu = layout("M");
        let repeat = renderer
            .prepare_text(&cpu, TextRasterOptions::new())
            .unwrap();
        let count = renderer.stats().cache_misses;
        let bytes = renderer.stats().uploaded_bytes;
        let cached = renderer
            .prepare_text(&cpu, TextRasterOptions::new())
            .unwrap();
        assert_eq!(renderer.stats().cache_misses, count);
        assert_eq!(renderer.stats().uploaded_bytes, bytes);
        assert!(uploaded > 0);
        drop((repeat, cached));
        let mut frame_a = a.begin_frame().unwrap();
        {
            let mut pass = frame_a.render_pass().begin().unwrap();
            renderer
                .draw(
                    &mut pass,
                    &first,
                    TextDraw::new([0., 0.]).color([1., 0., 0., 1.]),
                )
                .unwrap();
        }
        drop(first);
        renderer.clear_cache();
        assert!(renderer.stats().live_pages > 0);
        let second = renderer
            .prepare_text(&layout("😀"), TextRasterOptions::new())
            .unwrap();
        let expected = pixels(&g, &mut b, |frame| {
            let mut pass = frame.render_pass().begin().unwrap();
            renderer
                .draw(&mut pass, &second, TextDraw::new([0., 0.]))
                .unwrap();
        });
        frame_a.finish().unwrap();
        let actual = pixels(&g, &mut a, |frame| {
            drop(frame.render_pass().load_all().begin().unwrap());
        });
        near(pixel(&actual, 5, 10), [255, 0, 0, 255]);
        assert_ne!(actual, expected);
    });
}

#[test]
fn atlas_exhaustion_abandonment_and_completion_release_leases() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let mut target = target(&g, 1);
        let options = TextRendererOptions {
            page_size: 32,
            max_pages: 1,
            ..Default::default()
        };
        let mut renderer = TextRenderer::with_options(&g, options).unwrap();
        let cpu = layout("M");
        let prepared = renderer
            .prepare_text(&cpu, TextRasterOptions::new())
            .unwrap();
        let mut frame = target.begin_frame().unwrap();
        {
            let mut pass = frame.render_pass().begin().unwrap();
            renderer
                .draw(&mut pass, &prepared, TextDraw::default())
                .unwrap();
        }
        drop(prepared);
        renderer.clear_cache();
        assert_eq!(renderer.stats().live_pages, 1);
        assert!(matches!(
            renderer.prepare_text(&cpu, TextRasterOptions::new()),
            Err(TextRenderError::AtlasFull)
        ));
        drop(frame);
        assert_eq!(renderer.stats().live_pages, 0);
        let prepared = renderer
            .prepare_text(&cpu, TextRasterOptions::new())
            .unwrap();
        let mut frame = target.begin_frame().unwrap();
        {
            let mut pass = frame.render_pass().begin().unwrap();
            renderer
                .draw(&mut pass, &prepared, TextDraw::default())
                .unwrap();
        }
        drop(prepared);
        renderer.clear_cache();
        let submission = frame.finish().unwrap();
        // Completion callback holds the budget until the application drives device progress.
        g.device()
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(std::time::Duration::from_secs(10)),
            })
            .unwrap();
        assert_eq!(renderer.stats().live_pages, 0);
        assert!(
            renderer
                .prepare_text(&cpu, TextRasterOptions::new())
                .is_ok()
        );
    });
}

#[test]
fn invalid_preparation_and_draws_leave_existing_text_and_pass_usable() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let mut t = target(&g, 1);
        let mut renderer = TextRenderer::with_options(
            &g,
            TextRendererOptions {
                page_size: 32,
                max_cached_glyphs: 2,
                ..Default::default()
            },
        )
        .unwrap();
        let cpu = layout("M ");
        let integer = g
            .create_framebuffer(
                FramebufferOptions::new(64, 64).format(wgpu::TextureFormat::Rgba8Uint),
            )
            .unwrap();
        assert!(matches!(
            renderer.prepare(&integer.render_format()),
            Err(TextRenderError::UnsupportedFormat { .. })
        ));
        let prepared = renderer
            .prepare_text(&cpu, TextRasterOptions::new())
            .unwrap();
        assert_eq!(prepared.skipped_glyphs(), [1]);
        for scale in [0., -1., f32::INFINITY, f32::NAN, 1000.] {
            assert!(matches!(
                renderer.prepare_text(&cpu, TextRasterOptions::new().scale_factor(scale)),
                Err(TextRenderError::InvalidOptions)
            ));
        }
        assert!(matches!(
            renderer.prepare_text(&cpu, TextRasterOptions::new().scale_factor(2.)),
            Err(TextRenderError::GlyphTooLarge)
        ));
        assert!(renderer.stats().cached_glyphs <= 2);
        let foreign = GraphicsContext::headless().await.unwrap();
        let mut other = TextRenderer::new(&foreign);
        let bytes = pixels(&g, &mut t, |frame| {
            let mut pass = frame.render_pass().begin().unwrap();
            assert!(matches!(
                other.draw(&mut pass, &prepared, TextDraw::default()),
                Err(TextRenderError::Graphics(Error::DeviceMismatch))
            ));
            assert!(
                renderer
                    .draw(&mut pass, &prepared, TextDraw::default().opacity(2.))
                    .is_err()
            );
            assert!(
                renderer
                    .draw(
                        &mut pass,
                        &prepared,
                        TextDraw::new([f32::MAX, f32::MAX])
                            .transform_2d(Transform2D::scale(f32::MAX, f32::MAX))
                    )
                    .is_err()
            );
            renderer
                .draw(&mut pass, &prepared, TextDraw::default())
                .unwrap();
        });
        near(pixel(&bytes, 5, 10), [255; 4]);
    });
}

#[test]
fn ordered_mask_color_batches_and_unleased_page_eviction_work() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let mut t = target(&g, 1);
        let mut renderer = TextRenderer::new(&g);
        let cpu = layout("M😀M😁M");
        let prepared = renderer
            .prepare_text(&cpu, TextRasterOptions::new())
            .unwrap();
        assert_eq!(prepared.data.batches.len(), 5);
        assert_eq!(prepared.glyph_count(), 5);
        let mut another = TextRenderer::new(&g);
        let bytes = pixels(&g, &mut t, |frame| {
            let mut pass = frame.render_pass().begin().unwrap();
            another
                .draw(&mut pass, &prepared, TextDraw::default())
                .unwrap();
        });
        near(pixel(&bytes, 5, 10), [255; 4]);
        near(pixel(&bytes, 25, 10), [254, 0, 0, 254]);
        let mut limited = TextRenderer::with_options(
            &g,
            TextRendererOptions {
                page_size: 32,
                max_pages: 1,
                ..Default::default()
            },
        )
        .unwrap();
        let mask = limited
            .prepare_text(&layout("M"), TextRasterOptions::new())
            .unwrap();
        let page = mask.data.batches[0].page.id;
        assert!(matches!(
            limited.prepare_text(&layout("😀"), TextRasterOptions::new()),
            Err(TextRenderError::AtlasFull)
        ));
        drop(mask);
        let color = limited
            .prepare_text(&layout("😀"), TextRasterOptions::new())
            .unwrap();
        assert_ne!(color.data.batches[0].page.id, page);
        assert_eq!(limited.stats().live_pages, 1);
        assert_eq!(limited.stats().atlas_bytes, 32 * 32 * 4);
    });
}

#[test]
fn real_multilingual_layout_rasterizes_fallback_without_new_shaping() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let mut t = target(&g, 1);
        let mut system = super::super::TextSystem::new();
        system
            .load_font(include_bytes!("../../tests/fonts/SourceSans3-Regular.otf"))
            .unwrap();
        system
            .load_font(include_bytes!("../../tests/fonts/NotoSansArabic.ttf"))
            .unwrap();
        let mut buffer = super::super::TextBuffer::new();
        buffer
            .set_text(
                "AV ffi e\u{301}\nالعربية",
                super::super::TextStyle::new()
                    .family("Source Sans 3")
                    .font_size(16.)
                    .line_height(22.),
            )
            .unwrap();
        let cpu = buffer.layout(&mut system).unwrap();
        drop(system);
        drop(buffer);
        let mut renderer = TextRenderer::new(&g);
        let prepared = renderer
            .prepare_text(&cpu, TextRasterOptions::new())
            .unwrap();
        assert!(prepared.glyph_count() > 10);
        assert!(prepared.skipped_glyphs().iter().all(|&i| {
            cpu.text()[cpu.glyphs()[i].cluster.clone()]
                .trim()
                .is_empty()
        }));
        let data = pixels(&g, &mut t, |frame| {
            let mut pass = frame.render_pass().begin().unwrap();
            renderer
                .draw(&mut pass, &prepared, TextDraw::default())
                .unwrap();
        });
        assert!(data[..64 * 22 * 4].iter().any(|&v| v != 0));
        assert!(data[64 * 22 * 4..64 * 44 * 4].iter().any(|&v| v != 0));
    });
}
