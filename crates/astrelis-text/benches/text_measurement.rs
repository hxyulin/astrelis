//! Benchmarks for text measurement operations

use std::sync::Arc;
use astrelis_render::GraphicsContext;
use astrelis_text::{FontRenderer, FontSystem, Text};
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};

fn setup() -> (Arc<GraphicsContext>, FontRenderer) {
    let context = GraphicsContext::new_owned_sync().expect("Failed to create graphics context");
    let font_system = FontSystem::with_system_fonts();
    let renderer = FontRenderer::new(context.clone(), font_system);
    (context, renderer)
}

fn bench_measure_basic(c: &mut Criterion) {
    let (_context, renderer) = setup();
    let mut group = c.benchmark_group("measure_basic");

    let long_text = "Lorem ipsum dolor sit amet. ".repeat(20);
    let texts: Vec<(&str, &str)> = vec![
        ("single_char", "A"),
        ("single_word", "Hello"),
        ("short_sentence", "Hello, World!"),
        ("medium_text", "The quick brown fox jumps over the lazy dog"),
        ("long_text", &long_text),
    ];

    for (name, content) in texts {
        group.bench_function(name, |b| {
            let text = Text::new(content).size(16.0);
            b.iter(|| black_box(renderer.measure_text(&text)));
        });
    }

    group.finish();
}

fn bench_measure_different_sizes(c: &mut Criterion) {
    let (_context, renderer) = setup();
    let mut group = c.benchmark_group("measure_different_sizes");

    let content = "Sample text for size measurement";

    for size in [8.0, 12.0, 16.0, 20.0, 24.0, 32.0, 48.0, 64.0, 96.0] {
        group.bench_with_input(
            BenchmarkId::from_parameter(size as u32),
            &size,
            |b, &size| {
                let text = Text::new(content).size(size);
                b.iter(|| black_box(renderer.measure_text(&text)));
            },
        );
    }

    group.finish();
}

fn bench_measure_unicode(c: &mut Criterion) {
    let (_context, renderer) = setup();
    let mut group = c.benchmark_group("measure_unicode");

    let texts = vec![
        ("ascii", "Hello World"),
        ("emoji", "Hello 👋 World 🌍"),
        ("mixed_scripts", "Hello мир 世界 🌍"),
        ("cjk", "日本語のテキスト中文字符"),
        ("arabic", "مرحبا بالعالم"),
    ];

    for (name, content) in texts {
        group.bench_function(name, |b| {
            let text = Text::new(content).size(16.0);
            b.iter(|| black_box(renderer.measure_text(&text)));
        });
    }

    group.finish();
}

fn bench_measure_varying_lengths(c: &mut Criterion) {
    let (_context, renderer) = setup();
    let mut group = c.benchmark_group("measure_varying_lengths");

    for length in [10, 50, 100, 500, 1000] {
        group.throughput(Throughput::Elements(length as u64));

        let content = "a".repeat(length);
        group.bench_with_input(
            BenchmarkId::from_parameter(length),
            &content,
            |b, content| {
                let text = Text::new(content).size(16.0);
                b.iter(|| black_box(renderer.measure_text(&text)));
            },
        );
    }

    group.finish();
}

fn bench_measure_with_styles(c: &mut Criterion) {
    let (_context, renderer) = setup();
    let mut group = c.benchmark_group("measure_with_styles");

    let content = "Styled text measurement";

    group.bench_function("plain", |b| {
        let text = Text::new(content).size(16.0);
        b.iter(|| black_box(renderer.measure_text(&text)));
    });

    group.bench_function("bold", |b| {
        let text = Text::new(content)
            .size(16.0)
            .weight(astrelis_text::FontWeight::Bold);
        b.iter(|| black_box(renderer.measure_text(&text)));
    });

    group.bench_function("italic", |b| {
        let text = Text::new(content)
            .size(16.0)
            .style(astrelis_text::FontStyle::Italic);
        b.iter(|| black_box(renderer.measure_text(&text)));
    });

    group.finish();
}

fn bench_measure_cached(c: &mut Criterion) {
    let (_context, renderer) = setup();
    let mut group = c.benchmark_group("measure_cached");

    let content = "Sample text for caching test";

    group.bench_function("first_measure", |b| {
        b.iter(|| {
            let text = Text::new(content).size(16.0);
            black_box(renderer.measure_text(&text))
        });
    });

    group.bench_function("repeated_measure", |b| {
        let text = Text::new(content).size(16.0);
        b.iter(|| black_box(renderer.measure_text(&text)));
    });

    group.finish();
}

fn bench_measure_batch(c: &mut Criterion) {
    let (_context, renderer) = setup();
    let mut group = c.benchmark_group("measure_batch");

    for count in [10, 50, 100, 500] {
        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            let texts: Vec<Text> = (0..count)
                .map(|i| Text::new(format!("Text item {}", i)).size(16.0))
                .collect();

            b.iter(|| {
                for text in &texts {
                    black_box(renderer.measure_text(text));
                }
            });
        });
    }

    group.finish();
}

fn bench_measure_line_breaks(c: &mut Criterion) {
    let (_context, renderer) = setup();
    let mut group = c.benchmark_group("measure_line_breaks");

    let single_line = "This is a single line of text";
    let multi_line = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
    let many_lines = (0..20)
        .map(|i| format!("Line {}", i))
        .collect::<Vec<_>>()
        .join("\n");

    group.bench_function("single_line", |b| {
        let text = Text::new(single_line).size(16.0);
        b.iter(|| black_box(renderer.measure_text(&text)));
    });

    group.bench_function("multi_line", |b| {
        let text = Text::new(multi_line).size(16.0);
        b.iter(|| black_box(renderer.measure_text(&text)));
    });

    group.bench_function("many_lines", |b| {
        let text = Text::new(&many_lines).size(16.0);
        b.iter(|| black_box(renderer.measure_text(&text)));
    });

    group.finish();
}

fn bench_measure_realistic_scenarios(c: &mut Criterion) {
    let (_context, renderer) = setup();
    let mut group = c.benchmark_group("measure_realistic_scenarios");

    group.bench_function("ui_label", |b| {
        let text = Text::new("Settings").size(14.0);
        b.iter(|| black_box(renderer.measure_text(&text)));
    });

    group.bench_function("button_text", |b| {
        let text = Text::new("Click Me").size(16.0);
        b.iter(|| black_box(renderer.measure_text(&text)));
    });

    group.bench_function("heading", |b| {
        let text = Text::new("Welcome to the Application")
            .size(24.0)
            .weight(astrelis_text::FontWeight::Bold);
        b.iter(|| black_box(renderer.measure_text(&text)));
    });

    group.bench_function("paragraph", |b| {
        let text = Text::new(
            "This is a longer paragraph of text that might appear in a UI element. \
             It contains multiple sentences and should wrap naturally when rendered.",
        )
        .size(14.0);
        b.iter(|| black_box(renderer.measure_text(&text)));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_measure_basic,
    bench_measure_different_sizes,
    bench_measure_unicode,
    bench_measure_varying_lengths,
    bench_measure_with_styles,
    bench_measure_cached,
    bench_measure_batch,
    bench_measure_line_breaks,
    bench_measure_realistic_scenarios
);
criterion_main!(benches);
