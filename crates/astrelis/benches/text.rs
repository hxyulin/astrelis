//! CPU text stages only; fixture fonts and input are prepared outside timed stages.
use astrelis::{TextBuffer, TextStyle, TextSystem};
use std::{hint::black_box, sync::Arc, time::Instant};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const SAMPLES: usize = 40;
const WARMUP: usize = 8;
const LATIN: &[u8] = include_bytes!("../tests/fonts/SourceSans3-Regular.otf");
const ARABIC: &[u8] = include_bytes!("../tests/fonts/NotoSansArabic.ttf");

fn system() -> Result<TextSystem> {
    let mut system = TextSystem::new();
    system.load_font(LATIN)?;
    system.load_font(ARABIC)?;
    Ok(system)
}
fn style() -> TextStyle {
    TextStyle::new()
        .family("Source Sans 3")
        .font_size(20.)
        .line_height(28.)
}
fn buffer(content: &str, width: f32) -> Result<TextBuffer> {
    let mut buffer = TextBuffer::new();
    buffer.set_text(content, style())?;
    buffer.set_width(Some(width))?;
    Ok(buffer)
}
fn measure(mut work: impl FnMut() -> Result<()>, iterations: usize) -> Result<(f64, f64)> {
    let mut samples = Vec::with_capacity(SAMPLES);
    for sample in 0..WARMUP + SAMPLES {
        let start = Instant::now();
        for _ in 0..iterations {
            work()?;
        }
        let ns = start.elapsed().as_secs_f64() * 1e9 / iterations as f64;
        if sample >= WARMUP {
            samples.push(ns);
        }
    }
    samples.sort_by(f64::total_cmp);
    Ok((
        (samples[SAMPLES / 2 - 1] + samples[SAMPLES / 2]) / 2.,
        samples[(SAMPLES as f64 * 0.95).ceil() as usize - 1],
    ))
}
fn row(count: usize, glyphs: usize, stage: &str, timings: (f64, f64)) {
    println!("{count},{glyphs},{stage},{:.2},{:.2}", timings.0, timings.1);
}
fn main() -> Result<()> {
    if cfg!(debug_assertions) {
        return Err("use cargo bench --bench text".into());
    }
    eprintln!(
        "cpu_only=true; samples={SAMPLES}; warmup={WARMUP}; font_discovery=false; snapshot_hit_iterations=1000; other_iterations=1; cold_buffer_uses_warmed_font_system=true"
    );
    println!("input_scalars,glyphs,stage,median_ns,p95_ns");
    row(
        0,
        0,
        "font_load_two_faces",
        measure(
            || {
                black_box(system()?);
                Ok(())
            },
            1,
        )?,
    );
    let mut system = system()?;
    for count in [100, 1000, 10000] {
        let seed = "office AV e\u{301} العربية 123 words wrap across lines. ";
        let content: String = seed.chars().cycle().take(count).collect();
        let changed = format!("{content}!");
        let mut retained = buffer(&content, 320.)?;
        let initial = retained.layout(&mut system)?;
        assert!(initial.missing_glyphs().is_empty());
        assert!(Arc::ptr_eq(&initial, &retained.layout(&mut system)?));
        // Reflow must agree with a fresh buffer before it is benchmarked.
        retained.set_width(Some(240.))?;
        let reflowed = retained.layout(&mut system)?;
        let fresh = buffer(&content, 240.)?.layout(&mut system)?;
        assert_eq!(reflowed.glyphs(), fresh.glyphs());
        assert_eq!(reflowed.lines(), fresh.lines());
        let glyphs = initial.glyphs().len();
        row(
            count,
            glyphs,
            "new_buffer_shape_snapshot",
            measure(
                || {
                    black_box(buffer(black_box(&content), 320.)?.layout(&mut system)?);
                    Ok(())
                },
                1,
            )?,
        );
        row(
            count,
            glyphs,
            "unchanged_snapshot",
            measure(
                || {
                    black_box(retained.layout(black_box(&mut system))?);
                    Ok(())
                },
                1000,
            )?,
        );
        let mut wide = false;
        row(
            count,
            glyphs,
            "width_reflow_snapshot",
            measure(
                || {
                    wide = !wide;
                    retained.set_width(Some(if wide { 320. } else { 240. }))?;
                    black_box(retained.layout(&mut system)?);
                    Ok(())
                },
                1,
            )?,
        );
        let mut alternate = false;
        row(
            count,
            glyphs,
            "content_reshape_snapshot",
            measure(
                || {
                    alternate = !alternate;
                    retained.set_text(if alternate { &changed } else { &content }, style())?;
                    black_box(retained.layout(&mut system)?);
                    Ok(())
                },
                1,
            )?,
        );
    }
    // One changing paragraph in a larger retained document; alternate equal-length
    // contents so font/glyph selection stays warm while source text really changes.
    for paragraphs in [10, 100, 1000] {
        let seed = "office AV e\u{301} العربية 123 words wrap across lines.";
        let content = vec![seed; paragraphs].join("\n");
        let mut changed = vec![seed; paragraphs];
        changed[paragraphs / 2] = "office AV e\u{301} العربية 456 words wrap across lines.";
        let changed = changed.join("\n");
        let mut retained = buffer(&content, 320.)?;
        let initial = retained.layout(&mut system)?;
        let mut alternate = false;
        row(
            content.chars().count(),
            initial.glyphs().len(),
            "one_paragraph_edit",
            measure(
                || {
                    alternate = !alternate;
                    retained.set_text(if alternate { &changed } else { &content }, style())?;
                    black_box(retained.layout(&mut system)?);
                    Ok(())
                },
                1,
            )?,
        );
        let fresh = buffer(retained.text(), 320.)?.layout(&mut system)?;
        let edited = retained.layout(&mut system)?;
        assert_eq!(fresh.glyphs(), edited.glyphs());
        assert_eq!(fresh.lines(), edited.lines());
    }

    Ok(())
}
