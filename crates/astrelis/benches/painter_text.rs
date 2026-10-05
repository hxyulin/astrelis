//! Matched prepared-text CPU recording through direct and Painter APIs.
//! Font/layout/atlas preparation, pipeline creation, and GPU waits are outside timing.
use astrelis::{
    FramebufferOptions, GraphicsContext, Painter, TextBuffer, TextDraw, TextRasterOptions,
    TextRenderer, TextStyle, TextSystem, Transform2D, wgpu,
};
use std::{
    hint::black_box,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const SAMPLES: usize = 80;
const WARMUP: usize = 16;
fn print(count: usize, draws: usize, scoped: bool, mode: &str, mut samples: Vec<f64>) {
    samples.sort_by(f64::total_cmp);
    let n = samples.len();
    println!(
        "{count},{draws},{scoped},{mode},{:.3},{:.3}",
        (samples[n / 2 - 1] + samples[n / 2]) / 2.,
        samples[(n as f64 * 0.95).ceil() as usize - 1]
    );
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
        return Err("use cargo bench --bench painter_text".into());
    }
    let g = pollster::block_on(GraphicsContext::headless())?;
    eprintln!(
        "adapter={:?}; samples={SAMPLES}; warmup={WARMUP}; gpu_execution=not_measured; completion_wait=outside_timing",
        g.adapter().get_info()
    );
    let mut target = g.create_framebuffer(FramebufferOptions::new(64, 64))?;
    let mut fonts = TextSystem::new();
    fonts.load_font(include_bytes!("../tests/fonts/SourceSans3-Regular.otf"))?;
    fonts.load_font(include_bytes!("../tests/fonts/NotoSansArabic.ttf"))?;
    let mut painter = Painter::new(&g);
    let mut direct = TextRenderer::new(&g);
    painter.prepare(&target.render_format())?;
    direct.prepare(&target.render_format())?;
    println!("input_scalars,draws,transformed,mode,record_median_us,record_p95_us");
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
        let prepared = painter.prepare_text(&layout, TextRasterOptions::new())?;
        wait(&g, g.queue().submit([]))?;
        for draws in [1, 100] {
            for scoped in [false, true] {
                let transform = if scoped {
                    Transform2D::translation(1., 2.)
                } else {
                    Transform2D::IDENTITY
                };
                let draw = TextDraw::new([0., 0.]).color([0.3, 0.7, 1., 0.8]);
                let composed = TextDraw {
                    transform: draw.transform.then(transform),
                    ..draw
                };
                let before = painter.text().stats();
                let direct_before = direct.stats();
                let mut direct_samples = Vec::new();
                let mut painted_samples = Vec::new();
                for sample in 0..SAMPLES + WARMUP {
                    // Alternate order to reduce systematic first/second recording bias.
                    for painted in if sample % 2 == 0 {
                        [false, true]
                    } else {
                        [true, false]
                    } {
                        let mut frame = target.begin_frame()?;
                        let elapsed;
                        {
                            let mut pass = frame.render_pass().begin()?;
                            let start = Instant::now();
                            if painted {
                                let mut session = painter.begin(&mut pass)?;
                                if scoped {
                                    let mut local = session.transformed(transform)?;
                                    for _ in 0..draws {
                                        local.draw_text(black_box(&prepared), black_box(draw))?;
                                    }
                                } else {
                                    for _ in 0..draws {
                                        session.draw_text(black_box(&prepared), black_box(draw))?;
                                    }
                                }
                            } else {
                                for _ in 0..draws {
                                    direct.draw(
                                        &mut pass,
                                        black_box(&prepared),
                                        black_box(composed),
                                    )?;
                                }
                            }
                            elapsed = start.elapsed().as_secs_f64() * 1e6;
                        }
                        wait(&g, frame.finish()?)?;
                        if sample >= WARMUP {
                            if painted {
                                painted_samples.push(elapsed);
                            } else {
                                direct_samples.push(elapsed);
                            }
                        }
                    }
                }
                let after = painter.text().stats();
                assert_eq!(before.cache_misses, after.cache_misses);
                assert_eq!(before.uploaded_bytes, after.uploaded_bytes);
                assert_eq!(before.geometry_bytes, after.geometry_bytes);
                assert_eq!(
                    after.parameter_bytes - before.parameter_bytes,
                    (SAMPLES + WARMUP) as u64 * draws as u64 * 48
                );
                assert_eq!(
                    after.draw_calls - before.draw_calls,
                    direct.stats().draw_calls - direct_before.draw_calls
                );
                print(count, draws, scoped, "direct", direct_samples);
                print(count, draws, scoped, "painter", painted_samples);
            }
        }
    }
    Ok(())
}
