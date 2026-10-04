//! CPU stages for GPU text; GPU completion is outside all measured intervals.
use astrelis::{
    FramebufferOptions, GraphicsContext, TextBuffer, TextDraw, TextRasterOptions, TextRenderer,
    TextStyle, TextSystem, wgpu,
};
use std::{
    hint::black_box,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const SAMPLES: usize = 40;
const WARMUP: usize = 8;
fn stats(mut values: Vec<f64>) -> (f64, f64) {
    values.sort_by(f64::total_cmp);
    let n = values.len();
    (
        (values[n / 2 - 1] + values[n / 2]) / 2.,
        values[(n as f64 * 0.95).ceil() as usize - 1],
    )
}
fn print(count: usize, stage: &str, values: Vec<f64>) {
    let (m, p) = stats(values);
    println!("{count},{stage},{m:.3},{p:.3}");
}
fn wait(g: &GraphicsContext, index: wgpu::SubmissionIndex) -> Result<()> {
    g.device().poll(wgpu::PollType::Wait {
        submission_index: Some(index),
        timeout: Some(Duration::from_secs(10)),
    })?;
    Ok(())
}
fn main() -> Result<()> {
    if cfg!(debug_assertions) {
        return Err("use cargo bench --bench text_rendering".into());
    }
    let g = pollster::block_on(GraphicsContext::headless())?;
    eprintln!(
        "adapter={:?}; samples={SAMPLES}; warmup={WARMUP}; gpu_execution=not_measured; layout_shaping=outside_timing; completion_wait=outside_timing",
        g.adapter().get_info()
    );
    let mut target = g.create_framebuffer(FramebufferOptions::new(64, 64))?;
    let mut fonts = TextSystem::new();
    fonts.load_font(include_bytes!("../tests/fonts/SourceSans3-Regular.otf"))?;
    fonts.load_font(include_bytes!("../tests/fonts/NotoSansArabic.ttf"))?;
    let mut renderer = TextRenderer::new(&g);
    renderer.prepare(&target.render_format())?;
    println!("input_scalars,stage,median_us,p95_us");
    for count in [100, 1000, 10000] {
        let content: String = "office AV e\u{301} العربية 123 repeated words. "
            .chars()
            .cycle()
            .take(count)
            .collect();
        let mut buffer = TextBuffer::new();
        buffer.set_text(
            &content,
            TextStyle::new()
                .family("Source Sans 3")
                .font_size(20.)
                .line_height(28.),
        )?;
        buffer.set_width(Some(400.))?;
        let layout = buffer.layout(&mut fonts)?;
        let mut cold = Vec::new();
        let mut warm = Vec::new();
        let mut record = Vec::new();
        let mut total = Vec::new();
        for sample in 0..WARMUP + SAMPLES {
            renderer.clear_cache();
            let start = Instant::now();
            let prepared = renderer.prepare_text(black_box(&layout), TextRasterOptions::new())?;
            let cold_us = start.elapsed().as_secs_f64() * 1e6;
            // Flush preparation writes outside the measured CPU stage.
            wait(&g, g.queue().submit([]))?;
            let before = renderer.stats();
            let start = Instant::now();
            let cached = renderer.prepare_text(black_box(&layout), TextRasterOptions::new())?;
            let warm_us = start.elapsed().as_secs_f64() * 1e6;
            assert_eq!(before.cache_misses, renderer.stats().cache_misses);
            assert_eq!(before.uploaded_bytes, renderer.stats().uploaded_bytes);
            assert_eq!(prepared.glyph_count(), cached.glyph_count());
            drop(cached);
            wait(&g, g.queue().submit([]))?;
            let before = renderer.stats();
            let start_total = Instant::now();
            let mut frame = target.begin_frame()?;
            let record_us;
            {
                let mut pass = frame.render_pass().begin()?;
                let start = Instant::now();
                renderer.draw(
                    &mut pass,
                    black_box(&prepared),
                    TextDraw::new([0., 0.]).color([0.3, 0.7, 1., 0.8]),
                )?;
                record_us = start.elapsed().as_secs_f64() * 1e6;
            }
            let submission = frame.finish()?;
            let total_us = start_total.elapsed().as_secs_f64() * 1e6;
            assert_eq!(before.cache_misses, renderer.stats().cache_misses);
            assert_eq!(before.uploaded_bytes, renderer.stats().uploaded_bytes);
            wait(&g, submission)?;
            if sample >= WARMUP {
                cold.push(cold_us);
                warm.push(warm_us);
                record.push(record_us);
                total.push(total_us);
            }
        }
        print(count, "cold_raster_atlas_geometry", cold);
        print(count, "cached_atlas_new_geometry", warm);
        print(count, "prepared_record", record);
        print(count, "prepared_frame_cpu_total", total);
        eprintln!(
            "input_scalars={count}; drawable_glyphs={}; stats={:?}",
            renderer
                .prepare_text(&layout, TextRasterOptions::new())?
                .glyph_count(),
            renderer.stats()
        );
    }
    Ok(())
}
