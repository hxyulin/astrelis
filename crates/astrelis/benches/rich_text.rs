//! CPU rich-text preparation scaling, with fixed embedded fonts and warm font caches.
//! New buffers include span resolution, shaping, layout, and interaction indexing.
use astrelis::{TextBuffer, TextSpan, TextStyle, TextSystem, TextWrap};
use std::{hint::black_box, time::Instant};
fn main() {
    let mut fonts = TextSystem::new();
    fonts
        .load_font(include_bytes!("../tests/fonts/SourceSans3-Regular.otf"))
        .unwrap();
    let style = TextStyle::new()
        .family("Source Sans 3")
        .font_size(16.)
        .line_height(24.);
    println!("case,spans,bytes,median_ms,p95_ms");
    for count in [100, 1000, 5000] {
        for paragraphs in [false, true] {
            let source = if paragraphs {
                "abcdefghij\n".repeat(count)
            } else {
                "abcdefghij ".repeat(count)
            };
            let spans: Vec<_> = (0..count)
                .map(|i| {
                    TextSpan::new(i * 11..i * 11 + 3)
                        .weight(700)
                        .color([0.2, 0.6, 0.9, 1.])
                })
                .collect();
            let mut samples = Vec::new();
            for iteration in 0..13 {
                let start = Instant::now();
                let mut buffer = TextBuffer::new();
                buffer
                    .set_rich_text(&source, style.clone(), spans.clone())
                    .unwrap();
                buffer.set_wrap(TextWrap::None);
                let layout = buffer.layout(&mut fonts).unwrap();
                layout.prepare_interaction().unwrap();
                black_box(layout);
                if iteration >= 2 {
                    samples.push(start.elapsed().as_secs_f64() * 1000.);
                }
            }
            samples.sort_by(f64::total_cmp);
            println!(
                "{},{count},{},{:.6},{:.6}",
                if paragraphs {
                    "rich_paragraphs"
                } else {
                    "rich_single_line"
                },
                source.len(),
                samples[5],
                samples[10]
            );
        }
    }
}
