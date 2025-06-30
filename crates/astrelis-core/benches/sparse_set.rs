use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

fn criterion_benchmark(c: &mut Criterion) {
    const SIZES: &[usize] = &[256, 512, 1024, 2048, 4096, 8192, 16384, 32768];
}

criterion_group!(ecs, criterion_benchmark);
criterion_main!(ecs);
