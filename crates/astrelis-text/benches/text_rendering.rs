//! Benchmarks for text rendering operations

use astrelis_core::math::Vec2;
use astrelis_render::{Color, GraphicsContext};
use astrelis_text::{FontRenderer, FontSystem, Text, TextAlign, TextWrap};
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::sync::Arc;

fn setup() -> (Arc<GraphicsContext>, FontRenderer) {
    let context = GraphicsContext::new_owned_sync().expect("Failed to create graphics context");
    let font_system = FontSystem::with_system_fonts();
    let renderer = FontRenderer::new(context.clone(), font_system);
    (context, renderer)
}

fn bench_text_prepare(c: &mut Criterion) {
    let (_context, mut renderer) = setup();
    let mut group = c.benchmark_group("text_prepare");

    // Short text
    group.bench_function("short_text", |b| {
        let text = Text::new("Hello, World!").size(16.0);
        b.iter(|| black_box(renderer.prepare(&text)));
    });

    // Medium text
    group.bench_function("medium_text", |b| {
        let text = Text::new("The quick brown fox jumps over the lazy dog. This is a medium length text for benchmarking.")
            .size(16.0);
        b.iter(|| {
            black_box(renderer.prepare(&text))
        });
    });

    // Long text with wrapping
    group.bench_function("long_text_wrapped", |b| {
        let text = Text::new(
            "Lorem ipsum dolor sit amet, consectetur adipiscing elit. \
             Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. \
             Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris. \
             Duis aute irure dolor in reprehenderit in voluptate velit esse cillum.",
        )
        .size(16.0)
        .max_width(400.0)
        .wrap(TextWrap::Word);
        b.iter(|| black_box(renderer.prepare(&text)));
    });

    // Large text
    group.bench_function("large_text", |b| {
        let large_text = "Lorem ipsum dolor sit amet. ".repeat(100);
        let text = Text::new(&large_text).size(16.0);
        b.iter(|| black_box(renderer.prepare(&text)));
    });

    group.finish();
}

fn bench_text_sizes(c: &mut Criterion) {
    let (_context, mut renderer) = setup();
    let mut group = c.benchmark_group("text_sizes");

    let content = "Sample Text for Size Testing";

    for size in [12.0, 16.0, 24.0, 32.0, 48.0] {
        group.bench_with_input(
            BenchmarkId::from_parameter(size as u32),
            &size,
            |b, &size| {
                let text = Text::new(content).size(size);
                b.iter(|| black_box(renderer.prepare(&text)));
            },
        );
    }

    group.finish();
}

fn bench_text_measurement(c: &mut Criterion) {
    let (_context, mut renderer) = setup();
    let mut group = c.benchmark_group("text_measurement");

    let very_long_text = "Lorem ipsum dolor sit amet. ".repeat(10);
    let texts: Vec<(&str, &str)> = vec![
        ("short", "Hello"),
        ("medium", "The quick brown fox"),
        ("long", "The quick brown fox jumps over the lazy dog"),
        ("very_long", &very_long_text),
    ];

    for (name, content) in texts {
        group.bench_function(name, |b| {
            let text = Text::new(content).size(16.0);
            b.iter(|| black_box(renderer.measure_text(&text)));
        });
    }

    group.finish();
}

fn bench_text_styles(c: &mut Criterion) {
    let (_context, mut renderer) = setup();
    let mut group = c.benchmark_group("text_styles");

    let content = "Styled Text Sample";

    group.bench_function("plain", |b| {
        let text = Text::new(content).size(16.0);
        b.iter(|| black_box(renderer.prepare(&text)));
    });

    group.bench_function("bold", |b| {
        let text = Text::new(content).size(16.0).bold();
        b.iter(|| black_box(renderer.prepare(&text)));
    });

    group.bench_function("italic", |b| {
        let text = Text::new(content).size(16.0).italic();
        b.iter(|| black_box(renderer.prepare(&text)));
    });

    group.bench_function("bold_italic", |b| {
        let text = Text::new(content).size(16.0).bold().italic();
        b.iter(|| black_box(renderer.prepare(&text)));
    });

    group.bench_function("colored", |b| {
        let text = Text::new(content)
            .size(16.0)
            .color(Color::rgba(1.0, 0.5, 0.2, 1.0));
        b.iter(|| black_box(renderer.prepare(&text)));
    });

    group.finish();
}

fn bench_text_alignment(c: &mut Criterion) {
    let (_context, mut renderer) = setup();
    let mut group = c.benchmark_group("text_alignment");

    let content = "Aligned Text Sample for Benchmarking";

    for align in [TextAlign::Left, TextAlign::Center, TextAlign::Right] {
        group.bench_function(format!("{:?}", align), |b| {
            let text = Text::new(content).size(16.0).align(align);
            b.iter(|| black_box(renderer.prepare(&text)));
        });
    }

    group.finish();
}

fn bench_text_wrapping(c: &mut Criterion) {
    let (_context, mut renderer) = setup();
    let mut group = c.benchmark_group("text_wrapping");

    let content = "This is a long text that needs to be wrapped across multiple lines to test wrapping performance.";

    for wrap in [TextWrap::None, TextWrap::Word] {
        group.bench_function(format!("{:?}", wrap), |b| {
            let text = Text::new(content).size(16.0).max_width(200.0).wrap(wrap);
            b.iter(|| black_box(renderer.prepare(&text)));
        });
    }

    group.finish();
}

fn bench_multiple_texts(c: &mut Criterion) {
    let (_context, mut renderer) = setup();
    let mut group = c.benchmark_group("multiple_texts");

    for count in [10, 50, 100, 500] {
        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            let texts: Vec<_> = (0..count)
                .map(|i| Text::new(format!("Text {}", i)).size(16.0))
                .collect();

            b.iter(|| {
                for text in &texts {
                    black_box(renderer.prepare(text));
                }
            });
        });
    }

    group.finish();
}

fn bench_text_draw_preparation(c: &mut Criterion) {
    let (_context, mut renderer) = setup();
    let mut group = c.benchmark_group("text_draw_preparation");

    let text = Text::new("Sample Text").size(16.0);
    let mut buffer = renderer.prepare(&text);

    group.bench_function("draw_text", |b| {
        b.iter(|| {
            renderer.draw_text(black_box(&mut buffer), black_box(Vec2::new(100.0, 100.0)));
        });
    });

    group.finish();
}

fn bench_ui_text_scenario(c: &mut Criterion) {
    let (_context, mut renderer) = setup();
    let mut group = c.benchmark_group("ui_text_scenario");

    // Simulate typical UI with labels, buttons, etc.
    group.bench_function("ui_frame", |b| {
        b.iter(|| {
            // Title
            let title = Text::new("Game Menu").size(32.0).bold();
            let mut title_buf = renderer.prepare(&title);
            renderer.draw_text(&mut title_buf, Vec2::new(400.0, 50.0));

            // Buttons
            for i in 0..5 {
                let button_text = Text::new(format!("Button {}", i + 1)).size(20.0);
                let mut button_buf = renderer.prepare(&button_text);
                renderer.draw_text(&mut button_buf, Vec2::new(350.0, 150.0 + i as f32 * 60.0));
            }

            // Status text
            let status = Text::new("Player: John | Level: 42 | HP: 100/100").size(14.0);
            let mut status_buf = renderer.prepare(&status);
            renderer.draw_text(&mut status_buf, Vec2::new(20.0, 550.0));

            // FPS counter
            let fps = Text::new("FPS: 60").size(12.0);
            let mut fps_buf = renderer.prepare(&fps);
            renderer.draw_text(&mut fps_buf, Vec2::new(750.0, 20.0));

            black_box(())
        });
    });

    group.finish();
}

fn bench_buffer_reuse(c: &mut Criterion) {
    let (_context, mut renderer) = setup();
    let mut group = c.benchmark_group("buffer_reuse");

    let text = Text::new("Reusable Text").size(16.0);

    group.bench_function("create_new_each_time", |b| {
        b.iter(|| {
            let mut buffer = renderer.prepare(&text);
            renderer.draw_text(black_box(&mut buffer), Vec2::new(100.0, 100.0));
        });
    });

    group.bench_function("reuse_buffer", |b| {
        let mut buffer = renderer.prepare(&text);
        b.iter(|| {
            renderer.draw_text(black_box(&mut buffer), Vec2::new(100.0, 100.0));
        });
    });

    group.finish();
}

fn bench_text_with_constraints(c: &mut Criterion) {
    let (_context, mut renderer) = setup();
    let mut group = c.benchmark_group("text_with_constraints");

    let content = "This is a longer piece of text that will be constrained by various width limits to test performance with different constraint scenarios.";

    for width in [100.0, 200.0, 400.0, 800.0] {
        group.bench_with_input(
            BenchmarkId::from_parameter(width as u32),
            &width,
            |b, &width| {
                let text = Text::new(content)
                    .size(16.0)
                    .max_width(width)
                    .wrap(TextWrap::Word);
                b.iter(|| black_box(renderer.prepare(&text)));
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_text_prepare,
    bench_text_sizes,
    bench_text_measurement,
    bench_text_styles,
    bench_text_alignment,
    bench_text_wrapping,
    bench_multiple_texts,
    bench_text_draw_preparation,
    bench_ui_text_scenario,
    bench_buffer_reuse,
    bench_text_with_constraints
);
criterion_main!(benches);
