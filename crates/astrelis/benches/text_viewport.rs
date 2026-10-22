//! Exact-size viewport snapshots: cold seek, retained scrolling and edits.
//! CPU wall time only; snapshot glyph counts exclude backend shaped/layout storage.
use astrelis::{TextBuffer, TextStyle, TextSystem};
use std::{hint::black_box, time::Instant};

fn buffer(text: &str) -> TextBuffer {
    let mut buffer = TextBuffer::new();
    buffer
        .set_text(
            text,
            TextStyle::new().family("Source Sans 3").line_height(22.),
        )
        .unwrap();
    buffer.set_width(Some(640.)).unwrap();
    buffer
}
fn micros(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1e6
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if cfg!(debug_assertions) {
        return Err("run cargo bench --bench text_viewport".into());
    }
    let mut fonts = TextSystem::new();
    fonts.load_font(include_bytes!("../tests/fonts/SourceSans3-Regular.otf"))?;
    fonts.load_font(include_bytes!("../tests/fonts/NotoSansArabic.ttf"))?;
    eprintln!(
        "cpu_wall_times_us; samples=3; warmup=1; width=640; viewport=440; fonts=embedded; no_gpu; full_and_view_use_independent_buffers"
    );
    println!(
        "case,units,sample,bytes,full_glyphs,view_glyphs,total_lines,cold_full_us,cold_view_us,scroll_us,edit_full_us,edit_view_us"
    );
    for (case, unit) in [
        ("multiline", "office العربية 123 log entry words\n"),
        ("single_paragraph", "office العربية 123 log entry words "),
    ] {
        for count in [1000, 10_000] {
            let text = unit.repeat(count);
            let mut edited = text.clone();
            // Edit the middle paragraph without changing paragraph count; the
            // one-paragraph case intentionally measures full paragraph reshaping.
            let byte = text.char_indices().nth(text.chars().count() / 2).unwrap().0;
            edited.insert(byte, 'x');
            for sample in 0..4 {
                let mut full_buffer = buffer(&text);
                let mut view_buffer = buffer(&text);
                let start = Instant::now();
                let full = full_buffer.layout(&mut fonts)?;
                let cold_full = micros(start);
                let top = full.size()[1] * 0.5;
                let range = top..top + 440.;
                let start = Instant::now();
                let view = view_buffer.layout_viewport(&mut fonts, range.clone())?;
                let cold_view = micros(start);
                assert_eq!(view.size(), full.size());
                assert_eq!(view.total_line_count(), full.lines().len());
                let expected: usize = full
                    .lines()
                    .iter()
                    .filter(|l| l.top < range.end && l.top + l.height > range.start)
                    .map(|l| l.glyph_range.len())
                    .sum();
                assert_eq!(view.glyphs().len(), expected);
                assert!(view.glyphs().len() < full.glyphs().len() / 5);
                let start = Instant::now();
                for step in 1..=20 {
                    let top = top + step as f32 * 22.;
                    black_box(view_buffer.layout_viewport(&mut fonts, top..top + 440.)?);
                }
                let scroll = micros(start) / 20.;
                full_buffer.set_text(&edited, full_buffer.style().clone())?;
                view_buffer.set_text(&edited, view_buffer.style().clone())?;
                let start = Instant::now();
                black_box(full_buffer.layout(&mut fonts)?);
                let edit_full = micros(start);
                let start = Instant::now();
                black_box(view_buffer.layout_viewport(&mut fonts, range)?);
                let edit_view = micros(start);
                if sample > 0 {
                    println!(
                        "{case},{count},{sample},{},{},{},{},{cold_full:.3},{cold_view:.3},{scroll:.3},{edit_full:.3},{edit_view:.3}",
                        text.len(),
                        full.glyphs().len(),
                        view.glyphs().len(),
                        full.lines().len()
                    );
                }
            }
        }
    }
    Ok(())
}
