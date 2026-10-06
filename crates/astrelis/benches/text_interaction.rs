//! CPU interaction costs: lazy index build, retained query time and index storage.
//! No GPU initialization or rendering. Run in release with cargo bench --bench text_interaction.
use astrelis::{TextBuffer, TextPosition, TextStyle, TextSystem};
use std::{hint::black_box, time::Instant};
use unicode_segmentation::UnicodeSegmentation;
fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if cfg!(debug_assertions) {
        return Err("run cargo bench --bench text_interaction".into());
    }
    let mut fonts = TextSystem::new();
    fonts.load_font(include_bytes!("../tests/fonts/SourceSans3-Regular.otf"))?;
    fonts.load_font(include_bytes!("../tests/fonts/NotoSansArabic.ttf"))?;
    println!(
        "repetitions,bytes,glyphs,lines,index_build_us,hit_us,caret_us,selection_us,selection_all_us,index_bytes"
    );
    for count in [1, 32, 256, 1024] {
        let text = "office e\u{301} العربية 123 world. ".repeat(count);
        let mut b = TextBuffer::new();
        b.set_text(
            &text,
            TextStyle::new()
                .family("Source Sans 3")
                .font_size(16.)
                .line_height(24.),
        )?;
        let mut cold = Vec::new();
        for i in 0..19 {
            b.set_width(Some(800. + (i % 2) as f32))?;
            let l = b.layout(&mut fonts)?;
            assert_eq!(
                l.interaction_bytes(),
                0,
                "cold query must use a fresh snapshot"
            );
            let start = Instant::now();
            l.prepare_interaction()?;
            let us = start.elapsed().as_secs_f64() * 1e6;
            if i >= 4 {
                cold.push(us);
            }
        }
        let layout = b.layout(&mut fonts)?;
        layout.prepare_interaction()?;
        let positions: Vec<_> = text.grapheme_indices(true).map(|(i, _)| i).collect();
        for &byte in &positions {
            assert!(layout.caret(TextPosition::new(byte)).is_some());
        }
        assert!(layout.hit_test([400., 12.]).is_some());
        let mut hit = Vec::new();
        let mut caret = Vec::new();
        let mut selection = Vec::new();
        let mut selection_all = Vec::new();
        let start_byte = positions[positions.len() / 2];
        let end_byte = positions[(positions.len() / 2 + 20).min(positions.len() - 1)];
        for _ in 0..19 {
            let start = Instant::now();
            for i in 0..1000 {
                black_box(layout.hit_test(black_box([
                    i as f32 % 800.,
                    (i % layout.lines().len()) as f32 * 24. + 12.,
                ])));
            }
            hit.push(start.elapsed().as_secs_f64() * 1e3);
            let start = Instant::now();
            for i in 0..1000 {
                black_box(layout.caret(black_box(TextPosition::new(
                    positions[(i * 149) % positions.len()],
                ))));
            }
            caret.push(start.elapsed().as_secs_f64() * 1e3);
            let start = Instant::now();
            for _ in 0..1000 {
                black_box(
                    layout
                        .selection_rects(black_box(start_byte..end_byte))?
                        .count(),
                );
            }
            selection.push(start.elapsed().as_secs_f64() * 1e3);
            let start = Instant::now();
            for _ in 0..1000 {
                black_box(layout.selection_rects(black_box(0..text.len()))?.count());
            }
            selection_all.push(start.elapsed().as_secs_f64() * 1e3);
        }
        println!(
            "{count},{},{},{},{:.3},{:.3},{:.3},{:.3},{:.3},{}",
            text.len(),
            layout.glyphs().len(),
            layout.lines().len(),
            median(cold),
            median(hit[4..].to_vec()),
            median(caret[4..].to_vec()),
            median(selection[4..].to_vec()),
            median(selection_all[4..].to_vec()),
            layout.interaction_bytes()
        );
    }
    Ok(())
}
