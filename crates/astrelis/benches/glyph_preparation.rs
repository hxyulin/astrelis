//! Cold work separation, with application-owned worker and retained draws during generation.
use astrelis::{
    Framebuffer, FramebufferOptions, GlyphPreparationLimits, GlyphUploadBudget, GraphicsContext,
    MtsdfOptions, PreparedText, TextBuffer, TextDraw, TextLayout, TextRasterOptions, TextRenderer,
    TextStyle, TextSystem, wgpu,
};
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
fn draw(target: &mut Framebuffer, renderer: &mut TextRenderer, text: &PreparedText) -> Result<()> {
    let mut frame = target.begin_frame()?;
    {
        let mut pass = frame.render_pass().begin()?;
        renderer.draw(&mut pass, text, TextDraw::new([32., 32.]))?;
    }
    frame.finish()?;
    Ok(())
}
fn micros(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1e6
}
fn main() -> Result<()> {
    if cfg!(debug_assertions) {
        return Err("run cargo bench --bench glyph_preparation".into());
    }
    let g = pollster::block_on(GraphicsContext::headless())?;
    eprintln!(
        "adapter={:?}; samples=3; warmup=0; cpu_wall_times_us; gpu_execution=not_measured; worker_poll_sleep=1ms; commit_budget=4keys/65536bytes",
        g.adapter().get_info()
    );
    let mut target = g.create_framebuffer(
        FramebufferOptions::new(1920, 1080)
            .usage(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC),
    )?;
    let mut fonts = TextSystem::new();
    fonts.load_font(include_bytes!("../tests/fonts/SourceSans3-Regular.otf"))?;
    fonts.load_font(include_bytes!("../tests/fonts/NotoSansArabic.ttf"))?;
    let style = TextStyle::new()
        .family("Source Sans 3")
        .font_size(20.)
        .line_height(28.);
    let mut buffer = TextBuffer::new();
    buffer.set_text("Existing text stays drawable", style.clone())?;
    let old = buffer.layout(&mut fonts)?;
    let content: String = "office AV e\u{301} العربية 123 repeated words. "
        .chars()
        .cycle()
        .take(1000)
        .collect();
    buffer.set_text(&content, style.clone())?;
    buffer.set_width(Some(1856.))?;
    let repeated = buffer.layout(&mut fonts)?;
    let font = &repeated.fonts()[0];
    let face = swash::FontRef::from_index(font.data(), font.face_index() as usize).unwrap();
    let mut seen = HashSet::new();
    let mut content = String::new();
    for c in (0x21..=0x2fff).filter_map(char::from_u32) {
        let glyph = face.charmap().map(c);
        if !c.is_control() && !c.is_whitespace() && glyph != 0 && seen.insert(glyph) {
            content.push(c);
            content.push(' ');
            if seen.len() == 256 {
                break;
            }
        }
    }
    buffer.set_text(&content, style)?;
    let unique = buffer.layout(&mut fonts)?;
    println!(
        "case,sample,keys,image_bytes,sync_us,request_us,worker_us,retained_frames,draw_p95_us,draw_max_us,commit_calls,commit_total_us,commit_max_us,finish_us,staged_wall_us"
    );
    for (case, layout) in [("repeated_1000", repeated), ("unique_256", unique)] {
        for sample in 1..=3 {
            run(case, sample, layout.clone(), old.clone(), &g, &mut target)?;
        }
    }
    Ok(())
}
fn run(
    case: &str,
    sample: usize,
    layout: Arc<TextLayout>,
    old: Arc<TextLayout>,
    g: &GraphicsContext,
    target: &mut Framebuffer,
) -> Result<()> {
    let mut renderer = TextRenderer::new(g);
    renderer.prepare(&target.render_format())?;
    let start = Instant::now();
    let synchronous = renderer.prepare_text(&layout, MtsdfOptions::new())?;
    let sync_us = micros(start);
    draw(target, &mut renderer, &synchronous)?;
    let reference = target.read_rgba8()?;
    drop(synchronous);
    renderer.clear_cache();
    let retained = renderer.prepare_text(&old, TextRasterOptions::new())?;
    draw(target, &mut renderer, &retained)?; // Warm the retained pipeline before measurement.
    let start = Instant::now();
    let request = renderer.request_glyphs(
        layout,
        MtsdfOptions::new(),
        GlyphPreparationLimits::default(),
    )?;
    let request_us = micros(start);
    let wall = Instant::now();
    let worker = std::thread::spawn(move || {
        let start = Instant::now();
        let result = request.generate();
        (result, micros(start))
    });
    let mut draws = Vec::new();
    while !worker.is_finished() {
        let start = Instant::now();
        draw(target, &mut renderer, &retained)?;
        draws.push(micros(start));
        g.device().poll(wgpu::PollType::Poll)?;
        std::thread::sleep(Duration::from_millis(1));
    }
    let (generated, worker_us) = worker.join().expect("glyph worker panicked");
    let generated = generated?;
    let keys = generated.glyph_count();
    let image_bytes = generated.image_bytes();
    let mut upload = renderer.begin_glyph_upload(generated)?;
    let mut commits = Vec::new();
    while upload.remaining() > 0 {
        let start = Instant::now();
        let progress = renderer.commit_glyphs(
            &mut upload,
            GlyphUploadBudget {
                max_glyphs: 4,
                max_bytes: 65536,
            },
        )?;
        commits.push(micros(start));
        assert!(progress.glyphs > 0 && progress.glyphs <= 4 && progress.uploaded_bytes <= 65536);
        draw(target, &mut renderer, &retained)?;
    }
    let start = Instant::now();
    let prepared = renderer.finish_glyph_upload(&upload, 1024 * 1024)?;
    let finish_us = micros(start);
    let staged_wall_us = micros(wall);
    drop(upload);
    draw(target, &mut renderer, &prepared)?;
    assert_eq!(target.read_rgba8()?, reference, "staged pixels differ");
    draws.sort_by(f64::total_cmp);
    let p95 = draws
        .get((draws.len() as f64 * 0.95).ceil().max(1.) as usize - 1)
        .copied()
        .unwrap_or(0.);
    println!(
        "{case},{sample},{keys},{image_bytes},{sync_us:.3},{request_us:.3},{worker_us:.3},{},{p95:.3},{:.3},{},{:.3},{:.3},{finish_us:.3},{staged_wall_us:.3}",
        draws.len(),
        draws.last().copied().unwrap_or(0.),
        commits.len(),
        commits.iter().sum::<f64>(),
        commits.iter().copied().fold(0., f64::max)
    );
    Ok(())
}
