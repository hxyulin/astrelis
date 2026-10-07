use super::*;
use crate::framebuffer::tests::pixels;
use crate::{
    Framebuffer, FramebufferOptions, ShapeDraw, ShapeRenderer, TextBuffer, TextStyle, TextSystem,
};

fn same_geometry(a: &GeometryBuffer, b: &GeometryBuffer) -> bool {
    std::ptr::eq::<Geometry>(&**a, &**b)
}

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
            .prepare_text(&layout("M"), TextRasterOptions::new().raster_scale(2.))
            .unwrap();
        assert_eq!(text.size(), [20., 24.]);
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
                    TextDraw::new([0., 0.])
                        .color([1., 0., 0., 1.])
                        .transform(Transform2D::scale(2., 2.)),
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
                        .transform(
                            Transform2D::scale(2., 2.).then(Transform2D::translation(2., 0.)),
                        ),
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
            Err(Error::UnsupportedTextFormat { .. })
        ));
        let prepared = renderer
            .prepare_text(&cpu, TextRasterOptions::new())
            .unwrap();
        assert_eq!(prepared.skipped_glyphs(), [1]);
        for scale in [0., -1., f32::INFINITY, f32::NAN, 1000.] {
            assert!(matches!(
                renderer.prepare_text(&cpu, TextRasterOptions::new().raster_scale(scale)),
                Err(TextRenderError::InvalidOptions)
            ));
        }
        assert!(matches!(
            renderer.prepare_text(&cpu, TextRasterOptions::new().raster_scale(2.)),
            Err(TextRenderError::GlyphTooLarge)
        ));
        assert!(renderer.stats().cached_glyphs <= 2);
        let foreign = GraphicsContext::headless().await.unwrap();
        let mut other = TextRenderer::new(&foreign);
        let bytes = pixels(&g, &mut t, |frame| {
            let mut pass = frame.render_pass().begin().unwrap();
            assert!(matches!(
                other.draw(&mut pass, &prepared, TextDraw::default()),
                Err(Error::DeviceMismatch)
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
                            .transform(Transform2D::scale(f32::MAX, f32::MAX))
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

#[test]
fn mtsdf_reuses_fields_across_font_sizes_dpi_and_keeps_explicit_metadata() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let mut renderer = TextRenderer::new(&g);
        let mut fonts = super::super::TextSystem::new();
        fonts
            .load_font(include_bytes!("../../tests/fonts/TestColor.ttf"))
            .unwrap();
        let mut buffer = super::super::TextBuffer::new();
        let style = super::super::TextStyle::new()
            .family("Astrelis Test Color")
            .font_size(20.)
            .line_height(24.);
        buffer.set_text("M ", style.clone()).unwrap();
        let cpu = buffer.layout(&mut fonts).unwrap();
        let settings = MtsdfOptions::new();
        let first = renderer.prepare_text(&cpu, settings).unwrap();
        assert_eq!(first.preparation(), TextPreparation::Mtsdf(settings));
        assert_eq!(first.raster_scale(), 1.);
        assert_eq!(first.skipped_glyphs(), [1]);
        assert_eq!(first.data.batches[0].page.kind, Kind::Mtsdf);
        let before = renderer.stats();
        let second = renderer
            .prepare_text(&cpu, settings.raster_scale(2.))
            .unwrap();
        assert_eq!(second.size(), first.size());
        let a = first.ink_bounds().unwrap();
        let b = second.ink_bounds().unwrap();
        assert_eq!([b.x, b.y, b.width, b.height], [a.x, a.y, a.width, a.height]);
        buffer
            .set_style(style.font_size(40.).line_height(48.))
            .unwrap();
        let larger = renderer
            .prepare_text(&buffer.layout(&mut fonts).unwrap(), settings)
            .unwrap();
        assert_eq!(renderer.stats().cache_misses, before.cache_misses);
        assert_eq!(renderer.stats().uploaded_bytes, before.uploaded_bytes);
        assert!(Arc::ptr_eq(
            &first.data.batches[0].page,
            &larger.data.batches[0].page
        ));
        assert_eq!(renderer.stats().atlas_bytes, 1024 * 1024 * 4);
        let changed = renderer
            .prepare_text(&cpu, settings.range_em(0.125))
            .unwrap();
        assert!(renderer.stats().cache_misses > before.cache_misses);
        assert!(changed.ink_bounds().unwrap().width < a.width);
        // Outline geometry does not inherit coverage's physical raster size cap.
        let huge = renderer
            .prepare_text(&cpu, settings.raster_scale(50.))
            .unwrap();
        assert_eq!(huge.size(), first.size());
    });
}

#[test]
fn mtsdf_and_intrinsic_artwork_preserve_order_color_opacity_and_msaa() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let errors = g.device().push_error_scope(wgpu::ErrorFilter::Validation);
        let mut renderer = TextRenderer::new(&g);
        let cpu = layout("M😀M😁M");
        let prepared = renderer.prepare_text(&cpu, MtsdfOptions::new()).unwrap();
        assert_eq!(prepared.glyph_count(), 5);
        assert_eq!(
            prepared
                .data
                .batches
                .iter()
                .map(|b| b.page.kind)
                .collect::<Vec<_>>(),
            [
                Kind::Mtsdf,
                Kind::Color,
                Kind::Mtsdf,
                Kind::Color,
                Kind::Mtsdf
            ]
        );
        let mask = renderer
            .prepare_text(&layout("M"), MtsdfOptions::new().raster_scale(2.))
            .unwrap();
        let mut shapes = ShapeRenderer::new(&g);
        for samples in [1, 4] {
            let mut t = target(&g, samples);
            renderer.prepare(&t.render_format()).unwrap();
            let before = renderer.stats();
            let bytes = pixels(&g, &mut t, |frame| {
                let mut pass = frame.render_pass().begin().unwrap();
                renderer
                    .draw(
                        &mut pass,
                        &prepared,
                        TextDraw::default().color([0., 0., 1., 1.]).opacity(0.5),
                    )
                    .unwrap();
            });
            near(pixel(&bytes, 5, 10), [0, 0, 128, 128]);
            near(pixel(&bytes, 25, 10), [127, 0, 0, 127]);
            near(pixel(&bytes, 45, 10), [0, 0, 128, 128]);
            assert_eq!(renderer.stats().cache_misses, before.cache_misses);
            assert_eq!(renderer.stats().uploaded_bytes, before.uploaded_bytes);
            assert_eq!(renderer.stats().geometry_bytes, before.geometry_bytes);
            assert_eq!(
                renderer.stats().parameter_bytes - before.parameter_bytes,
                48
            );
            assert_eq!(renderer.stats().draw_calls - before.draw_calls, 5);
            let bytes = pixels(&g, &mut t, |frame| {
                let mut pass = frame.render_pass().begin().unwrap();
                pass.set_scissor_rect(4, 4, 30, 40).unwrap();
                renderer
                    .draw(
                        &mut pass,
                        &mask,
                        TextDraw::default()
                            .color([1., 0., 0., 1.])
                            .transform(Transform2D::scale(2., 2.)),
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
                        &mask,
                        TextDraw::default()
                            .color([0., 0., 1., 1.])
                            .opacity(0.5)
                            .transform(
                                Transform2D::scale(2., 2.).then(Transform2D::translation(2., 0.)),
                            ),
                    )
                    .unwrap();
            });
            near(pixel(&bytes, 0, 10), [0; 4]);
            near(pixel(&bytes, 40, 10), [0; 4]);
            near(pixel(&bytes, 15, 15), [0, 128, 128, 255]);
        }
        assert!(errors.pop().await.is_none());
    });
}

#[test]
fn mtsdf_page_leases_survive_cache_clear_and_release_on_gpu_completion() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let mut t = target(&g, 1);
        let mut renderer = TextRenderer::with_options(
            &g,
            TextRendererOptions {
                page_size: 128,
                max_pages: 1,
                ..Default::default()
            },
        )
        .unwrap();
        let cpu = layout("M");
        let prepared = renderer.prepare_text(&cpu, MtsdfOptions::new()).unwrap();
        let mut frame = t.begin_frame().unwrap();
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
            renderer.prepare_text(&cpu, MtsdfOptions::new()),
            Err(TextRenderError::AtlasFull)
        ));
        let submission = frame.finish().unwrap();
        g.device()
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(std::time::Duration::from_secs(10)),
            })
            .unwrap();
        assert_eq!(renderer.stats().live_pages, 0);
        let result = pixels(&g, &mut t, |frame| {
            drop(frame.render_pass().load_all().begin().unwrap());
        });
        near(pixel(&result, 5, 10), [255; 4]);
        assert!(renderer.prepare_text(&cpu, MtsdfOptions::new()).is_ok());
    });
}

#[test]
fn mtsdf_invalid_options_and_page_pressure_leave_retained_resources_usable() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let mut renderer = TextRenderer::with_options(
            &g,
            TextRendererOptions {
                page_size: 128,
                max_pages: 1,
                ..Default::default()
            },
        )
        .unwrap();
        let cpu = layout("M");
        let options = MtsdfOptions::new();
        let prepared = renderer.prepare_text(&cpu, options).unwrap();
        let before = renderer.stats();
        for invalid in [
            options.pixels_per_em(0),
            options.pixels_per_em(257),
            options.range_em(0.),
            options.range_em(f32::NAN),
            options.range_em(1.1),
            options.range_em(0.01),
            options.raster_scale(0.),
            options.raster_scale(f32::INFINITY),
        ] {
            assert!(matches!(
                renderer.prepare_text(&cpu, invalid),
                Err(TextRenderError::InvalidOptions)
            ));
        }
        assert_eq!(renderer.stats().cache_misses, before.cache_misses);
        assert!(matches!(
            renderer.prepare_text(&cpu, options.pixels_per_em(256)),
            Err(TextRenderError::GlyphTooLarge)
        ));
        assert!(matches!(
            renderer.prepare_text(&layout("😀"), options),
            Err(TextRenderError::AtlasFull)
        ));
        let mut t = target(&g, 1);
        let mut painter = crate::Painter::new(&g);
        let bytes = pixels(&g, &mut t, |frame| {
            let mut pass = frame.render_pass().begin().unwrap();
            let mut paint = painter.begin(&mut pass).unwrap();
            assert!(
                paint
                    .draw_text(&prepared, TextDraw::default().opacity(2.))
                    .is_err()
            );
            paint.draw_text(&prepared, TextDraw::default()).unwrap();
        });
        near(pixel(&bytes, 5, 10), [255; 4]);
        drop(prepared);
        renderer.clear_cache();
        assert!(renderer.prepare_text(&cpu, options).is_ok());
    });
}

#[test]
fn mtsdf_multilingual_cff_variable_weight_and_italic_match_unhinted_coverage() {
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
        let mut renderer = TextRenderer::new(&g);
        let mut images = Vec::new();
        for (family, content, weight, slant) in [
            (
                "Source Sans 3",
                "AV ffi e\u{301}",
                400,
                super::super::FontSlant::Normal,
            ),
            (
                "Source Sans 3",
                "AV office",
                400,
                super::super::FontSlant::Italic,
            ),
            (
                "Source Sans 3",
                "B8@g",
                400,
                super::super::FontSlant::Normal,
            ),
            (
                "Noto Sans Arabic",
                "مب",
                400,
                super::super::FontSlant::Normal,
            ),
            (
                "Noto Sans Arabic",
                "مب",
                700,
                super::super::FontSlant::Normal,
            ),
        ] {
            buffer
                .set_text(
                    content,
                    super::super::TextStyle::new()
                        .family(family)
                        .font_size(28.)
                        .line_height(38.)
                        .weight(weight)
                        .slant(slant),
                )
                .unwrap();
            let cpu = buffer.layout(&mut system).unwrap();
            assert!(cpu.missing_glyphs().is_empty());
            let field = renderer.prepare_text(&cpu, MtsdfOptions::new()).unwrap();
            let coverage = renderer
                .prepare_text(
                    &cpu,
                    TextRasterOptions::new().hinting(false).raster_scale(3.),
                )
                .unwrap();
            assert_eq!(field.glyph_count(), coverage.glyph_count());
            for transform in [
                Transform2D::IDENTITY,
                Transform2D::rotation(0.1).then(Transform2D::translation(4., 0.)),
                Transform2D::scale(0.75, 1.25).then(Transform2D::translation(3.5, 0.5)),
            ] {
                let render = |renderer: &mut TextRenderer,
                              t: &mut Framebuffer,
                              p: &PreparedText,
                              tr: Transform2D| {
                    pixels(&g, t, |frame| {
                        let mut pass = frame.render_pass().begin().unwrap();
                        renderer
                            .draw(&mut pass, p, TextDraw::default().transform(tr))
                            .unwrap();
                    })
                };
                let actual = render(&mut renderer, &mut t, &field, transform);
                let reference = render(&mut renderer, &mut t, &coverage, transform);
                let area: u64 = reference
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|p| u64::from(p[3]))
                    .sum();
                let error: u64 = actual
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(reference.as_chunks::<4>().0.iter())
                    .map(|(a, b)| u64::from(a[3].abs_diff(b[3])))
                    .sum();
                assert!(area > 10000);
                assert!(
                    error as f64 / (area as f64) < 0.25,
                    "{family} weight {weight}: alpha error {error}/{area}"
                );
                if transform == Transform2D::IDENTITY {
                    images.push(actual);
                }
            }
        }
        assert_ne!(
            images[3], images[4],
            "variable weight must affect generated outlines"
        );
    });
}

#[test]
fn geometry_reuse_preserves_clones_recordings_and_submitted_draws() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let mut target = target(&g, 1);
        let errors = g.device().push_error_scope(wgpu::ErrorFilter::Validation);
        let mut renderer = TextRenderer::new(&g);
        renderer.prepare(&target.render_format()).unwrap();
        let original = renderer
            .prepare_text(&layout("M"), TextRasterOptions::new())
            .unwrap();
        let expected = pixels(&g, &mut target, |frame| {
            let mut pass = frame.render_pass().begin().unwrap();
            renderer
                .draw(&mut pass, &original, TextDraw::default())
                .unwrap();
        });
        let retained = original.clone();
        drop(original);
        let other = renderer
            .prepare_text(&layout("😀"), TextRasterOptions::new())
            .unwrap();
        assert_eq!(renderer.stats().geometry_buffer_allocations, 2);
        assert_eq!(renderer.stats().geometry_buffer_reuses, 0);
        let owner = Arc::downgrade(retained.data.lease.as_ref().unwrap());
        let mut frame = target.begin_frame().unwrap();
        {
            let mut pass = frame.render_pass().begin().unwrap();
            renderer
                .draw(&mut pass, &retained, TextDraw::default())
                .unwrap();
        }
        drop(retained);
        assert!(owner.upgrade().is_some());
        let third = renderer
            .prepare_text(&layout("😁"), TextRasterOptions::new())
            .unwrap();
        assert_eq!(renderer.stats().geometry_buffer_allocations, 3);
        assert_eq!(renderer.stats().geometry_buffer_reuses, 0);
        let submission = frame.finish().unwrap();
        g.device()
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(std::time::Duration::from_secs(10)),
            })
            .unwrap();
        assert!(owner.upgrade().is_none());
        let actual = pixels(&g, &mut target, |_| {});
        assert_eq!(
            actual, expected,
            "later preparation changed an earlier draw"
        );
        let reused = renderer
            .prepare_text(&layout("😀"), TextRasterOptions::new())
            .unwrap();
        assert_eq!(renderer.stats().geometry_buffer_allocations, 3);
        assert_eq!(renderer.stats().geometry_buffer_reuses, 1);
        let expected = pixels(&g, &mut target, |frame| {
            let mut pass = frame.render_pass().begin().unwrap();
            renderer
                .draw(&mut pass, &other, TextDraw::default())
                .unwrap();
        });
        let actual = pixels(&g, &mut target, |frame| {
            let mut pass = frame.render_pass().begin().unwrap();
            renderer
                .draw(&mut pass, &reused, TextDraw::default())
                .unwrap();
        });
        assert_eq!(
            actual, expected,
            "recycled geometry did not upload correctly"
        );
        drop(third);
        renderer.clear_cache();
        // Clearing the renderer or dropping it must not invalidate retained storage.
        drop(renderer);
        let mut independent = TextRenderer::new(&g);
        let actual = pixels(&g, &mut target, |frame| {
            let mut pass = frame.render_pass().begin().unwrap();
            independent
                .draw(&mut pass, &reused, TextDraw::default())
                .unwrap();
        });
        assert_eq!(actual, expected);
        assert!(errors.pop().await.is_none());
    });
}

#[test]
fn batched_text_matches_individual_metadata_pixels_and_order() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let errors = g.device().push_error_scope(wgpu::ErrorFilter::Validation);
        let layouts = [
            layout(""),
            layout("M"),
            layout("😀"),
            layout("M😁"),
            layout(" "),
            layout("M😀M😁M"),
        ];
        for settings in [
            TextPreparation::from(TextRasterOptions::new()),
            TextRasterOptions::new().raster_scale(2.).into(),
            MtsdfOptions::new().into(),
        ] {
            let mut renderer = TextRenderer::new(&g);
            let individual: Vec<_> = layouts
                .iter()
                .map(|l| renderer.prepare_text(l, settings).unwrap())
                .collect();
            let before = renderer.stats();
            let batched = renderer.prepare_texts(&layouts, settings).unwrap();
            let after = renderer.stats();
            assert_eq!(after.cache_misses, before.cache_misses);
            assert_eq!(after.uploaded_bytes, before.uploaded_bytes);
            assert_eq!(
                after.geometry_buffer_allocations - before.geometry_buffer_allocations,
                1
            );
            assert_eq!(batched.len(), layouts.len());
            let shared = batched[1].data.buffer.as_ref().unwrap();
            assert_eq!(batched[1].data.buffer_range.start, 0);
            assert_eq!(batched[2].data.buffer_range.start, 48);
            assert!(same_geometry(
                shared,
                batched[2].data.buffer.as_ref().unwrap()
            ));
            for (a, b) in individual.iter().zip(&batched) {
                assert_eq!(a.size(), b.size());
                assert_eq!(a.ink_bounds(), b.ink_bounds());
                assert_eq!(a.skipped_glyphs(), b.skipped_glyphs());
                assert_eq!(a.glyph_count(), b.glyph_count());
                assert_eq!(a.preparation(), b.preparation());
            }
            assert!(batched[0].data.buffer.is_none());
            assert!(batched[4].data.buffer.is_none());
            for samples in [1, 4] {
                let mut t = target(&g, samples);
                let render =
                    |renderer: &mut TextRenderer, t: &mut Framebuffer, texts: &[PreparedText]| {
                        pixels(&g, t, |frame| {
                            let mut pass = frame.render_pass().begin().unwrap();
                            pass.set_scissor_rect(4, 4, 56, 56).unwrap();
                            // Reverse the returned order and overlap translucent content.
                            for (i, text) in texts.iter().enumerate().rev() {
                                renderer
                                    .draw(
                                        &mut pass,
                                        text,
                                        TextDraw::new([i as f32 * 5., i as f32 * 4.])
                                            .color([0.2, 0.7, 1., 0.6])
                                            .transform(Transform2D::rotation(0.07)),
                                    )
                                    .unwrap();
                            }
                        })
                    };
                let expected = render(&mut renderer, &mut t, &individual);
                let actual = render(&mut renderer, &mut t, &batched);
                assert_eq!(actual, expected);
            }
        }
        assert!(errors.pop().await.is_none());
    });
}

#[test]
fn batch_chunks_keep_storage_and_atlas_ownership_independent() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let mut renderer = TextRenderer::new(&g);
        let cpu = layout("M");
        let layouts = vec![cpu; 1366]; // 1365 glyphs fit in the first 64 KiB buffer.
        let before = renderer.stats();
        let mut texts = renderer
            .prepare_texts(&layouts, TextRasterOptions::new())
            .unwrap();
        assert_eq!(
            renderer.stats().geometry_buffer_allocations - before.geometry_buffer_allocations,
            2
        );
        assert!(same_geometry(
            texts[0].data.buffer.as_ref().unwrap(),
            texts[1364].data.buffer.as_ref().unwrap()
        ));
        assert!(!same_geometry(
            texts[0].data.buffer.as_ref().unwrap(),
            texts[1365].data.buffer.as_ref().unwrap()
        ));
        let kept = texts.remove(1000);
        let GeometryBuffer::Shared(storage) = kept.data.buffer.as_ref().unwrap() else {
            panic!("expected shared geometry");
        };
        let shared = Arc::downgrade(storage);
        drop(texts);
        assert!(shared.upgrade().is_some());
        let mut target = target(&g, 1);
        let mut frame = target.begin_frame().unwrap();
        let token = Arc::downgrade(&kept.data.buffer.as_ref().unwrap().lease);
        {
            let mut pass = frame.render_pass().begin().unwrap();
            renderer
                .draw(&mut pass, &kept, TextDraw::default())
                .unwrap();
        }
        drop(kept);
        assert!(shared.upgrade().is_none());
        assert!(token.upgrade().is_some());
        drop(frame);
        assert!(token.upgrade().is_none());

        // Sharing geometry does not lease a sibling's unrelated color atlas page.
        let mut two = renderer
            .prepare_texts([layout("M"), layout("😀")], TextRasterOptions::new())
            .unwrap();
        let color_budget = Arc::downgrade(&two[1].data.batches[0].page.allocation);
        let mask = two.remove(0);
        drop(two);
        renderer.clear_cache();
        assert!(color_budget.upgrade().is_none());
        assert_eq!(renderer.stats().live_pages, 1);
        drop(mask);
        assert_eq!(renderer.stats().live_pages, 0);

        let big = layout(&"M".repeat(1400));
        let mixed = renderer
            .prepare_texts(
                [layout("M"), big, layout(""), layout("M")],
                TextRasterOptions::new(),
            )
            .unwrap();
        assert_eq!(mixed[1].glyph_count(), 1400);
        assert_eq!(
            mixed[1].data.buffer.as_ref().unwrap().buffer.size(),
            1400 * 48
        );
        assert!(mixed[2].data.buffer.is_none());
        assert!(!same_geometry(
            mixed[0].data.buffer.as_ref().unwrap(),
            mixed[3].data.buffer.as_ref().unwrap()
        ));
    });
}

#[test]
fn failed_and_empty_batches_preserve_existing_resources_and_painter_usage() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let mut painter = crate::Painter::new(&g);
        let mut target = target(&g, 1);
        let cpu = layout("M");
        let retained = painter
            .prepare_texts([cpu.as_ref()], TextRasterOptions::new())
            .unwrap();
        let expected = pixels(&g, &mut target, |frame| {
            let mut pass = frame.render_pass().begin().unwrap();
            let mut paint = painter.begin(&mut pass).unwrap();
            paint.draw_text(&retained[0], TextDraw::default()).unwrap();
        });
        let before = painter.text().stats();
        assert!(
            painter
                .prepare_texts(std::iter::empty::<&TextLayout>(), TextRasterOptions::new())
                .unwrap()
                .is_empty()
        );
        let blanks = painter
            .prepare_texts([layout(""), layout(" ")], TextRasterOptions::new())
            .unwrap();
        assert!(blanks.iter().all(|t| t.glyph_count() == 0));
        assert_eq!(
            before.geometry_buffer_allocations,
            painter.text().stats().geometry_buffer_allocations
        );
        assert_eq!(
            before.geometry_buffer_reuses,
            painter.text().stats().geometry_buffer_reuses
        );
        assert!(matches!(
            painter.prepare_texts(
                std::iter::empty::<&TextLayout>(),
                TextRasterOptions::new().raster_scale(0.)
            ),
            Err(TextRenderError::InvalidOptions)
        ));
        let mut fonts = TextSystem::new();
        fonts
            .load_font(include_bytes!("../../tests/fonts/TestColor.ttf"))
            .unwrap();
        let mut huge = TextBuffer::new();
        huge.set_text(
            "M",
            TextStyle::new()
                .family("Astrelis Test Color")
                .font_size(600.),
        )
        .unwrap();
        let huge = huge.layout(&mut fonts).unwrap();
        // Fail after an earlier chunk was built/uploaded. No partial result vector
        // escapes, and the caller's previous resource still renders identically.
        let mut inputs = vec![cpu.clone(); 1366];
        inputs.push(huge);
        assert!(matches!(
            painter.prepare_texts(&inputs, TextRasterOptions::new()),
            Err(TextRenderError::InvalidOptions)
        ));
        let mut small = TextRenderer::with_options(
            &g,
            TextRendererOptions {
                page_size: 32,
                max_pages: 1,
                ..Default::default()
            },
        )
        .unwrap();
        let old = small.prepare_text(&cpu, TextRasterOptions::new()).unwrap();
        assert!(matches!(
            small.prepare_texts([cpu.clone(), layout("😀")], TextRasterOptions::new()),
            Err(TextRenderError::AtlasFull)
        ));
        assert_eq!(old.glyph_count(), 1);
        let actual = pixels(&g, &mut target, |frame| {
            let mut pass = frame.render_pass().begin().unwrap();
            let mut paint = painter.begin(&mut pass).unwrap();
            paint.draw_text(&retained[0], TextDraw::default()).unwrap();
        });
        assert_eq!(actual, expected);
    });
}

#[test]
fn shared_geometry_is_reused_only_after_submitted_work_completes() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let errors = g.device().push_error_scope(wgpu::ErrorFilter::Validation);
        let mut renderer = TextRenderer::new(&g);
        let layouts = [layout("M"), layout("M")];
        let texts = renderer
            .prepare_texts(&layouts, TextRasterOptions::new())
            .unwrap();
        let token = Arc::downgrade(&texts[0].data.buffer.as_ref().unwrap().lease);
        let mut target = target(&g, 1);
        let expected = pixels(&g, &mut target, |frame| {
            let mut pass = frame.render_pass().begin().unwrap();
            renderer
                .draw(&mut pass, &texts[1], TextDraw::new([16., 0.]))
                .unwrap();
        });
        let mut frame = target.begin_frame().unwrap();
        {
            let mut pass = frame.render_pass().begin().unwrap();
            renderer
                .draw(&mut pass, &texts[1], TextDraw::new([16., 0.]))
                .unwrap();
        }
        drop(texts);
        assert!(token.upgrade().is_some());
        let other = renderer
            .prepare_texts([layout("😀"), layout("😁")], TextRasterOptions::new())
            .unwrap();
        assert_eq!(renderer.stats().geometry_buffer_allocations, 2);
        assert_eq!(renderer.stats().geometry_buffer_reuses, 0);
        let submission = frame.finish().unwrap();
        g.device()
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(std::time::Duration::from_secs(10)),
            })
            .unwrap();
        assert!(token.upgrade().is_none());
        let actual = pixels(&g, &mut target, |_| {});
        assert_eq!(actual, expected);
        let reused = renderer
            .prepare_texts(&layouts, TextRasterOptions::new())
            .unwrap();
        assert_eq!(renderer.stats().geometry_buffer_allocations, 2);
        assert_eq!(renderer.stats().geometry_buffer_reuses, 1);
        let actual = pixels(&g, &mut target, |frame| {
            let mut pass = frame.render_pass().begin().unwrap();
            renderer
                .draw(&mut pass, &reused[1], TextDraw::new([16., 0.]))
                .unwrap();
        });
        assert_eq!(actual, expected);
        drop(other);
        assert!(errors.pop().await.is_none());
    });
}

#[test]
fn batching_and_capacity_buckets_respect_application_device_limits() {
    pollster::block_on(async {
        let base = GraphicsContext::headless().await.unwrap();
        let (device, queue) = base
            .adapter()
            .request_device(&wgpu::DeviceDescriptor {
                required_limits: wgpu::Limits {
                    max_buffer_size: 1000,
                    max_uniform_buffer_binding_size: 256,
                    max_storage_buffer_binding_size: 256,
                    ..Default::default()
                },
                ..Default::default()
            })
            .await
            .unwrap();
        let g = GraphicsContext::from_wgpu(
            base.instance().clone(),
            base.adapter().clone(),
            device,
            queue,
        );
        let errors = g.device().push_error_scope(wgpu::ErrorFilter::Validation);
        let mut renderer = TextRenderer::new(&g);
        let inputs = [layout("M"), layout(&"M".repeat(20)), layout("M")];
        let texts = renderer
            .prepare_texts(&inputs, TextRasterOptions::new())
            .unwrap();
        assert_eq!(texts.len(), 3);
        assert_eq!(renderer.stats().geometry_buffer_allocations, 3);
        for text in &texts {
            let buffer = &text.data.buffer.as_ref().unwrap().buffer;
            assert!(buffer.size() <= 1000);
            assert!(text.data.buffer_range.end <= buffer.size());
        }
        let individual = renderer
            .prepare_text(&inputs[1], TextRasterOptions::new())
            .unwrap();
        assert_eq!(individual.data.buffer.as_ref().unwrap().buffer.size(), 1000);
        g.device()
            .poll(wgpu::PollType::Wait {
                submission_index: Some(g.queue().submit([])),
                timeout: Some(std::time::Duration::from_secs(10)),
            })
            .unwrap();
        assert!(errors.pop().await.is_none());
    });
}

#[test]
fn caret_and_selection_updates_reuse_prepared_glyphs_and_geometry() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let mut t = target(&g, 1);
        let errors = g.device().push_error_scope(wgpu::ErrorFilter::Validation);
        let mut fonts = TextSystem::new();
        fonts
            .load_font(include_bytes!("../../tests/fonts/SourceSans3-Regular.otf"))
            .unwrap();
        let mut b = TextBuffer::new();
        b.set_text(
            "office abc",
            TextStyle::new()
                .family("Source Sans 3")
                .font_size(12.)
                .line_height(16.),
        )
        .unwrap();
        let layout = b.layout(&mut fonts).unwrap();
        let mut renderer = TextRenderer::new(&g);
        renderer.prepare(&t.render_format()).unwrap();
        let prepared = renderer
            .prepare_text(&layout, TextRasterOptions::new())
            .unwrap();
        let geometry = prepared.data.buffer.as_ref().unwrap();
        let before = renderer.stats();
        let mut shapes = ShapeRenderer::new(&g);
        shapes.prepare(&t.render_format()).unwrap();
        for byte in [0, 1, 3, 6, 8, 10] {
            let bytes = pixels(&g, &mut t, |frame| {
                let mut pass = frame.render_pass().begin().unwrap();
                for rect in layout.selection_rects(0..byte).unwrap() {
                    shapes
                        .draw(&mut pass, ShapeDraw::rect(rect, [0.1, 0.3, 0.7, 0.5]))
                        .unwrap();
                }
                renderer
                    .draw(&mut pass, &prepared, TextDraw::default())
                    .unwrap();
                let caret = layout.caret(super::super::TextPosition::new(byte)).unwrap();
                shapes
                    .draw(
                        &mut pass,
                        ShapeDraw::rect(
                            crate::Rect::new(caret.origin[0], caret.origin[1], 1., caret.height),
                            [1.; 4],
                        ),
                    )
                    .unwrap();
            });
            assert!(bytes.iter().any(|&b| b != 0));
            assert!(same_geometry(
                geometry,
                prepared.data.buffer.as_ref().unwrap()
            ));
            let stats = renderer.stats();
            assert_eq!(stats.uploaded_bytes, before.uploaded_bytes);
            assert_eq!(stats.geometry_bytes, before.geometry_bytes);
            assert_eq!(
                stats.geometry_buffer_allocations,
                before.geometry_buffer_allocations
            );
            assert!(Arc::ptr_eq(&layout, &b.layout(&mut fonts).unwrap()));
        }
        assert!(errors.pop().await.is_none());
    });
}

#[test]
fn span_colors_and_decorations_render_in_one_draw() {
    pollster::block_on(async {
        let g = GraphicsContext::headless().await.unwrap();
        let mut t = target(&g, 1);
        let errors = g.device().push_error_scope(wgpu::ErrorFilter::Validation);
        let mut system = TextSystem::new();
        system
            .load_font(include_bytes!("../../tests/fonts/SourceSans3-Regular.otf"))
            .unwrap();
        let mut buffer = TextBuffer::new();
        let (red, green) = ([1., 0., 0., 1.], [0., 1., 0., 1.]);
        buffer
            .set_rich_text(
                "MM",
                TextStyle::new()
                    .family("Source Sans 3")
                    .font_size(28.)
                    .line_height(32.),
                vec![
                    super::super::TextSpan::new(0..1).color(red),
                    super::super::TextSpan::new(1..2).color(green),
                    super::super::TextSpan::new(0..2).underline(),
                ],
            )
            .unwrap();
        let layout = buffer.layout(&mut system).unwrap();
        let underline = layout.decorations()[0].rect;
        let mut renderer = TextRenderer::new(&g);
        renderer.prepare(&t.render_format()).unwrap();
        let text = renderer
            .prepare_text(&layout, TextRasterOptions::new())
            .unwrap();
        // Two glyphs plus two underline segments; decorations extend the ink bounds.
        assert_eq!(text.glyph_count(), 4);
        assert!(text.ink_bounds().unwrap().bottom() >= underline.bottom());
        let draws = renderer.stats().draw_calls;
        let render = |renderer: &mut TextRenderer, t: &mut Framebuffer, draw: TextDraw| {
            pixels(&g, t, |frame| {
                let mut pass = frame.render_pass().begin().unwrap();
                renderer.draw(&mut pass, &text, draw).unwrap();
            })
        };
        let bytes = render(&mut renderer, &mut t, TextDraw::new([2., 2.]));
        // One glyph batch and one decoration batch.
        assert_eq!(renderer.stats().draw_calls - draws, 2);
        let split = 2. + layout.glyphs()[1].advance_origin[0];
        let (mut left, mut right) = ([0u32; 3], [0u32; 3]);
        for y in 0..(2. + underline.y) as usize - 1 {
            for x in 0..64 {
                let p = pixel(&bytes, x, y);
                let side = if (x as f32) < split {
                    &mut left
                } else {
                    &mut right
                };
                for c in 0..3 {
                    side[c] += u32::from(p[c]);
                }
            }
        }
        assert!(left[0] > 1000 && left[1] == 0 && left[2] == 0, "{left:?}");
        assert!(
            right[1] > 1000 && right[0] == 0 && right[2] == 0,
            "{right:?}"
        );
        // The underline follows each span's color at the font's underline position.
        let row = (2. + underline.y + underline.height * 0.5) as usize;
        assert!(underline.height >= 1., "{underline:?}");
        let (a, b) = (
            pixel(&bytes, 4, row),
            pixel(&bytes, split as usize + 4, row),
        );
        assert!(a[0] > 200 && a[1] == 0, "{a:?}");
        assert!(b[1] > 200 && b[0] == 0, "{b:?}");
        // Draw alpha multiplies span colors; disabling span colors uses the draw color.
        let faded = render(
            &mut renderer,
            &mut t,
            TextDraw::new([2., 2.]).color([0., 0., 1., 0.5]),
        );
        near(pixel(&faded, 4, row), [a[0] / 2, 0, 0, a[3] / 2]);
        let plain = render(
            &mut renderer,
            &mut t,
            TextDraw::new([2., 2.])
                .color([0., 0., 1., 1.])
                .span_colors(false),
        );
        let p = pixel(&plain, split as usize + 4, row);
        assert!(p[2] > 200 && p[0] == 0 && p[1] == 0, "{p:?}");
        assert!(errors.pop().await.is_none());
    });
}

#[test]
fn half_precision_packing_matches_ieee_binary16() {
    for (value, bits) in [
        (0., 0),
        (-0., 0x8000),
        (1., 0x3c00),
        (0.5, 0x3800),
        (-2., 0xc000),
        (65504., 0x7bff),
        (65520., 0x7c00),
        (5.960_464_5e-8, 1),
        (6.103_515_6e-5, 0x0400),
        (1.000_488_3, 0x3c00),
        (1.000_977, 0x3c01),
        (f32::INFINITY, 0x7c00),
    ] {
        assert_eq!(super::f16_bits(value), bits, "{value}");
    }
    assert_eq!(super::f16_bits(f32::NAN) & 0x7e00, 0x7e00);
}
