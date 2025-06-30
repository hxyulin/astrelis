use astrelis_core::world::{Component, Registry};
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

pub struct CompA(pub usize);

impl Component for CompA {}

fn criterion_benchmark(c: &mut Criterion) {
    const SIZES: &[usize] = &[256, 512, 1024, 2048, 4096, 8192, 16384, 32768];

    for size in SIZES {
        let size = *size;

        c.bench_function(&format!("insert-{}", size), |b| {
            b.iter_batched(
                || Registry::new(),
                |mut reg| {
                    for i in 0..size {
                        let ent = reg.new_entity();
                        reg.add_component(ent, CompA(i));
                    }
                },
                BatchSize::SmallInput,
            );
        });

        let mut registry = Registry::new();
        for i in 0..size {
            let ent = registry.new_entity();
            registry.add_component(ent, CompA(i));
        }
        c.bench_function(&format!("query1-{}", size), move |b| {
            b.iter(|| {
                let mut iter = registry.query::<CompA>().unwrap();
                while let Some(next) = iter.next() {
                    black_box(next);
                }
            })
        });
    }
}

criterion_group!(ecs, criterion_benchmark);
criterion_main!(ecs);
