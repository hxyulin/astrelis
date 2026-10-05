//! Coverage/MTSDF preparation and visible retained drawing. GPU completion is outside timing.
use astrelis::{
    FramebufferOptions, GraphicsContext, MtsdfOptions, Painter, PreparedText, TextBuffer, TextDraw,
    TextPreparation, TextRasterOptions, TextStyle, TextSystem, wgpu,
};
use std::{
    collections::HashSet,
    hint::black_box,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const SAMPLES: usize = 20;
const WARMUP: usize = 4;
fn wait(g: &GraphicsContext, index: wgpu::SubmissionIndex) -> Result<()> {
    g.device().poll(wgpu::PollType::Wait {
        submission_index: Some(index),
        timeout: Some(Duration::from_secs(10)),
    })?;
    Ok(())
}
fn print(case: &str, density: u32, range: f32, stage: &str, mut values: Vec<f64>) {
    values.sort_by(f64::total_cmp);
    let n = values.len();
    println!(
        "{case},{density},{range},{stage},{:.3},{:.3}",
        (values[n / 2 - 1] + values[n / 2]) / 2.,
        values[(n as f64 * 0.95).ceil() as usize - 1]
    );
}
fn visible(p: &PreparedText) {
    let b = p.ink_bounds().unwrap();
    assert!(
        b.x + 32. >= 0.
            && b.y + 32. >= 0.
            && b.x + b.width + 32. < 1920.
            && b.y + b.height + 32. < 1080.
    );
}
fn main() -> Result<()> {
    if cfg!(debug_assertions) {
        return Err("use cargo bench --bench text_distance_fields".into());
    }
    let g = pollster::block_on(GraphicsContext::headless())?;
    eprintln!(
        "adapter={:?}; samples={SAMPLES}; warmup={WARMUP}; gpu_execution=not_measured; target=1920x1080 RGBA8 1xMSAA; waits_and_shaping=outside_timing",
        g.adapter().get_info()
    );
    let mut target = g.create_framebuffer(FramebufferOptions::new(1920, 1080))?;
    let mut fonts = TextSystem::new();
    fonts.load_font(include_bytes!("../tests/fonts/SourceSans3-Regular.otf"))?;
    fonts.load_font(include_bytes!("../tests/fonts/NotoSansArabic.ttf"))?;
    let content: String = "office AV e\u{301} العربية 123 repeated words. "
        .chars()
        .cycle()
        .take(1000)
        .collect();
    let style = TextStyle::new()
        .family("Source Sans 3")
        .font_size(20.)
        .line_height(28.);
    let mut buffer = TextBuffer::new();
    buffer.set_text(&content, style.clone())?;
    buffer.set_width(Some(1856.))?;
    let repeated = buffer.layout(&mut fonts)?;
    // Select 256 nominally distinct outline glyphs from the actual retained face.
    let probe = &repeated.fonts()[0];
    let face = swash::FontRef::from_index(probe.data(), probe.face_index() as usize).unwrap();
    let mut seen = HashSet::new();
    let mut unique_content = String::new();
    for c in (0x21..=0x2fff).filter_map(char::from_u32) {
        let glyph = face.charmap().map(c);
        if !c.is_control() && !c.is_whitespace() && glyph != 0 && seen.insert(glyph) {
            unique_content.push(c);
            unique_content.push(' ');
            if seen.len() == 256 {
                break;
            }
        }
    }
    buffer.set_text(&unique_content, style.clone())?;
    let unique = buffer.layout(&mut fonts)?;
    assert!(unique.missing_glyphs().is_empty());
    println!("case,pixels_per_em,range_em,stage,median_us,p95_us");
    for (case, layout, settings) in [
        (
            "repeated_1000",
            &repeated,
            TextPreparation::Coverage(TextRasterOptions::new()),
        ),
        (
            "repeated_1000",
            &repeated,
            MtsdfOptions::new().pixels_per_em(32).into(),
        ),
        ("repeated_1000", &repeated, MtsdfOptions::new().into()),
        (
            "repeated_1000",
            &repeated,
            MtsdfOptions::new().pixels_per_em(96).into(),
        ),
        (
            "repeated_1000",
            &repeated,
            MtsdfOptions::new().range_em(0.125).into(),
        ),
        (
            "repeated_1000",
            &repeated,
            MtsdfOptions::new().range_em(0.5).into(),
        ),
        (
            "unique_256",
            &unique,
            TextPreparation::Coverage(TextRasterOptions::new()),
        ),
        ("unique_256", &unique, MtsdfOptions::new().into()),
    ] {
        let mut painter = Painter::new(&g);
        painter.prepare(&target.render_format())?;
        let mut cold = Vec::new();
        let mut warm = Vec::new();
        let mut record = Vec::new();
        let mut frame_cpu = Vec::new();
        let (density, range) = match settings {
            TextPreparation::Coverage(_) => (0, 0.),
            TextPreparation::Mtsdf(v) => (v.pixels_per_em, v.range_em),
        };
        let mut final_stats = None;
        for sample in 0..SAMPLES + WARMUP {
            painter.text().clear_cache();
            let before = painter.text().stats();
            let start = Instant::now();
            let prepared = painter.prepare_text(black_box(layout), settings)?;
            let cold_us = start.elapsed().as_secs_f64() * 1e6;
            visible(&prepared);
            let after = painter.text().stats();
            final_stats = Some((
                prepared.glyph_count(),
                after.cache_misses - before.cache_misses,
                after.uploaded_bytes - before.uploaded_bytes,
                after.live_pages,
                after.atlas_bytes,
            ));
            wait(&g, g.queue().submit([]))?;
            let start = Instant::now();
            let cached = painter.prepare_text(black_box(layout), settings)?;
            let warm_us = start.elapsed().as_secs_f64() * 1e6;
            assert_eq!(painter.text().stats().cache_misses, after.cache_misses);
            assert_eq!(painter.text().stats().uploaded_bytes, after.uploaded_bytes);
            drop(cached);
            // Density changes reuse outline images and leave layout-unit geometry unchanged.
            if matches!(settings, TextPreparation::Mtsdf(_)) {
                let TextPreparation::Mtsdf(options) = settings else {
                    unreachable!()
                };
                let changed = painter.prepare_text(layout, options.raster_scale(2.))?;
                assert_eq!(painter.text().stats().cache_misses, after.cache_misses);
                assert_eq!(painter.text().stats().uploaded_bytes, after.uploaded_bytes);
                drop(changed);
            }
            wait(&g, g.queue().submit([]))?;
            let before = painter.text().stats();
            let start = Instant::now();
            let mut frame = target.begin_frame()?;
            let record_us;
            {
                let mut pass = frame.render_pass().begin()?;
                let start = Instant::now();
                painter
                    .begin(&mut pass)?
                    .draw_text(black_box(&prepared), TextDraw::new([32., 32.]))?;
                record_us = start.elapsed().as_secs_f64() * 1e6;
            }
            let submission = frame.finish()?;
            let total_us = start.elapsed().as_secs_f64() * 1e6;
            let after = painter.text().stats();
            assert_eq!(before.cache_misses, after.cache_misses);
            assert_eq!(before.uploaded_bytes, after.uploaded_bytes);
            assert_eq!(before.geometry_bytes, after.geometry_bytes);
            assert_eq!(after.parameter_bytes - before.parameter_bytes, 48);
            wait(&g, submission)?;
            if sample >= WARMUP {
                cold.push(cold_us);
                warm.push(warm_us);
                record.push(record_us);
                frame_cpu.push(total_us);
            }
        }
        print(case, density, range, "cold_images_atlas_geometry", cold);
        print(case, density, range, "cached_images_new_geometry", warm);
        print(case, density, range, "record_after_preparation", record);
        print(
            case,
            density,
            range,
            "frame_after_preparation_cpu",
            frame_cpu,
        );
        // Measure unchanged resources with no preparation between frames. In the
        // 100-draw case all paragraphs intentionally overlap at the same visible origin.
        let retained = painter.prepare_text(layout, settings)?;
        wait(&g, g.queue().submit([]))?;
        for draws in [1, 100] {
            let mut record = Vec::new();
            let mut total = Vec::new();
            for sample in 0..WARMUP + SAMPLES {
                let before = painter.text().stats();
                let start = Instant::now();
                let mut frame = target.begin_frame()?;
                let record_us;
                {
                    let mut pass = frame.render_pass().begin()?;
                    let start = Instant::now();
                    let mut paint = painter.begin(&mut pass)?;
                    for _ in 0..draws {
                        paint.draw_text(black_box(&retained), TextDraw::new([32., 32.]))?;
                    }
                    record_us = start.elapsed().as_secs_f64() * 1e6;
                }
                let submission = frame.finish()?;
                let cpu_us = start.elapsed().as_secs_f64() * 1e6;
                let after = painter.text().stats();
                assert_eq!(before.cache_misses, after.cache_misses);
                assert_eq!(before.uploaded_bytes, after.uploaded_bytes);
                assert_eq!(before.geometry_bytes, after.geometry_bytes);
                assert_eq!(after.parameter_bytes - before.parameter_bytes, 48 * draws);
                wait(&g, submission)?;
                if sample >= WARMUP {
                    record.push(record_us);
                    total.push(cpu_us);
                }
            }
            print(
                case,
                density,
                range,
                &format!("steady_{draws}_draw_record"),
                record,
            );
            print(
                case,
                density,
                range,
                &format!("steady_{draws}_draw_frame_cpu"),
                total,
            );
        }
        let (glyphs, keys, payload, pages, bytes) = final_stats.unwrap();
        eprintln!(
            "case={case}; density={density}; range={range}; input_scalars={}; drawable_glyphs={glyphs}; cold_key_misses={keys}; cold_payload_bytes={payload}; live_pages={pages}; atlas_bytes={bytes}",
            layout.text().chars().count()
        );
    }
    Ok(())
}
