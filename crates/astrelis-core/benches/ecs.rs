use astrelis_core::world::{Component, Registry};
use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

pub struct CompA(pub u64);

impl Component for CompA {}

fn criterion_benchmark(c: &mut Criterion) {
    let mut g = c.benchmark_group("ecs-comp");

    for &size in &[256, 512, 1024, 2048, 4096, 8192, 16384, 32768] {
        g.throughput(criterion::Throughput::Elements(size));
        g.bench_with_input(BenchmarkId::new("insert", size), &size, |b, &size| {
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

        let base_registry = {
            let mut r = Registry::new();
            for i in 0..size {
                let e = r.new_entity();
                r.add_component(e, CompA(i));
            }
            r
        };
        g.bench_with_input(
            BenchmarkId::new("query1", size),
            &base_registry,
            |b, registry| {
                b.iter(|| {
                    let mut it = registry.query::<CompA>().unwrap();
                    while let Some((ent, comp)) = it.next() {
                        black_box((ent, comp.0));
                    }
                })
            },
        );
    }
}

criterion_group!(ecs, criterion_benchmark);
criterion_main!(ecs);
