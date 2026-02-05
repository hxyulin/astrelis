//! Benchmarks for TextPipeline and caching.

use astrelis_text::{ShapedTextResult as BaseShapedTextResult, TextPipeline};
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};

// Mock shaping function that simulates some work
fn mock_shape(text: &str, font_size: f32, _wrap_width: Option<f32>) -> BaseShapedTextResult {
    // Simulate some work proportional to text length
    let glyphs = Vec::with_capacity(text.len());
    for _ in text.chars() {
        // Just dummy work
        black_box(font_size * 1.2);
    }
    BaseShapedTextResult::new((100.0, 20.0), glyphs)
}

fn bench_pipeline_request(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_request");

    group.bench_function("request_new", |b| {
        let mut pipeline = TextPipeline::new();
        let mut counter = 0;
        b.iter(|| {
            counter += 1;
            // Always new request (different text)
            let _ = pipeline.request_shape(format!("Text {}", counter), 0, 16.0, None);
        });
    });

    group.bench_function("request_cached", |b| {
        let mut pipeline = TextPipeline::new();
        // Pre-populate cache
        let req_id = pipeline.request_shape("Cached Text".to_string(), 0, 16.0, None);
        pipeline.process_pending(mock_shape);
        let _ = pipeline.take_completed(req_id);

        b.iter(|| {
            // Request same text repeatedly
            let _ = pipeline.request_shape("Cached Text".to_string(), 0, 16.0, None);
        });
    });

    group.finish();
}

fn bench_pipeline_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_processing");

    for count in [10, 100, 1000] {
        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("process_batch", count),
            &count,
            |b, &count| {
                b.iter_with_setup(
                    || {
                        let mut pipeline = TextPipeline::new();
                        for i in 0..count {
                            pipeline.request_shape(format!("Text {}", i), 0, 16.0, None);
                        }
                        pipeline
                    },
                    |mut pipeline| {
                        pipeline.process_pending(mock_shape);
                    },
                );
            },
        );
    }

    group.finish();
}

fn bench_cache_hit_rate(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_hit_rate");

    // Scenario: 90% static text, 10% dynamic text
    group.bench_function("mixed_workload_90_10", |b| {
        let mut pipeline = TextPipeline::new();
        let static_texts: Vec<String> = (0..90).map(|i| format!("Static {}", i)).collect();

        // Warm up cache for static text
        for text in &static_texts {
            pipeline.request_shape(text.clone(), 0, 16.0, None);
        }
        pipeline.process_pending(mock_shape);

        let mut dynamic_counter = 0;

        b.iter(|| {
            // Request all static texts (hits)
            for text in &static_texts {
                pipeline.request_shape(text.clone(), 0, 16.0, None);
            }

            // Request dynamic texts (misses)
            for i in 0..10 {
                pipeline.request_shape(format!("Dynamic {}-{}", dynamic_counter, i), 0, 16.0, None);
            }
            dynamic_counter += 1;

            pipeline.process_pending(mock_shape);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_pipeline_request,
    bench_pipeline_processing,
    bench_cache_hit_rate
);
criterion_main!(benches);
