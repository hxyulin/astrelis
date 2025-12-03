//! Benchmarks for optimized collections (HashMap, HashSet)

use astrelis_core::alloc::{HashMap as AHashMap, HashSet as AHashSet};
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::collections::{HashMap as StdHashMap, HashSet as StdHashSet};

fn bench_hashmap_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("hashmap_insert");

    for size in [10, 100, 1000, 10000] {
        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(BenchmarkId::new("std", size), &size, |b, &size| {
            b.iter(|| {
                let mut map = StdHashMap::new();
                for i in 0..size {
                    map.insert(black_box(i), black_box(i * 2));
                }
                map
            });
        });

        group.bench_with_input(BenchmarkId::new("ahash", size), &size, |b, &size| {
            b.iter(|| {
                let mut map = AHashMap::new();
                for i in 0..size {
                    map.insert(black_box(i), black_box(i * 2));
                }
                map
            });
        });
    }

    group.finish();
}

fn bench_hashmap_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("hashmap_lookup");

    for size in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(size as u64));

        let std_map: StdHashMap<i32, i32> = (0..size).map(|i| (i, i * 2)).collect();
        let ahash_map: AHashMap<i32, i32> = (0..size).map(|i| (i, i * 2)).collect();

        group.bench_with_input(BenchmarkId::new("std", size), &size, |b, &size| {
            b.iter(|| {
                let mut sum = 0;
                for i in 0..size {
                    if let Some(&val) = std_map.get(&black_box(i)) {
                        sum += val;
                    }
                }
                sum
            });
        });

        group.bench_with_input(BenchmarkId::new("ahash", size), &size, |b, &size| {
            b.iter(|| {
                let mut sum = 0;
                for i in 0..size {
                    if let Some(&val) = ahash_map.get(&black_box(i)) {
                        sum += val;
                    }
                }
                sum
            });
        });
    }

    group.finish();
}

fn bench_hashmap_iteration(c: &mut Criterion) {
    let mut group = c.benchmark_group("hashmap_iteration");

    for size in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(size as u64));

        let std_map: StdHashMap<i32, i32> = (0..size).map(|i| (i, i * 2)).collect();
        let ahash_map: AHashMap<i32, i32> = (0..size).map(|i| (i, i * 2)).collect();

        group.bench_with_input(BenchmarkId::new("std", size), &std_map, |b, map| {
            b.iter(|| {
                let mut sum = 0;
                for (k, v) in map.iter() {
                    sum += k + v;
                }
                black_box(sum)
            });
        });

        group.bench_with_input(BenchmarkId::new("ahash", size), &ahash_map, |b, map| {
            b.iter(|| {
                let mut sum = 0;
                for (k, v) in map.iter() {
                    sum += k + v;
                }
                black_box(sum)
            });
        });
    }

    group.finish();
}

fn bench_hashmap_string_keys(c: &mut Criterion) {
    let mut group = c.benchmark_group("hashmap_string_keys");

    let keys: Vec<String> = (0..1000).map(|i| format!("entity_{}", i)).collect();

    group.bench_function("std_insert", |b| {
        b.iter(|| {
            let mut map = StdHashMap::new();
            for key in &keys {
                map.insert(black_box(key.clone()), black_box(42));
            }
            map
        });
    });

    group.bench_function("ahash_insert", |b| {
        b.iter(|| {
            let mut map = AHashMap::new();
            for key in &keys {
                map.insert(black_box(key.clone()), black_box(42));
            }
            map
        });
    });

    let std_map: StdHashMap<String, i32> = keys.iter().map(|k| (k.clone(), 42)).collect();
    let ahash_map: AHashMap<String, i32> = keys.iter().map(|k| (k.clone(), 42)).collect();

    group.bench_function("std_lookup", |b| {
        b.iter(|| {
            let mut sum = 0;
            for key in &keys {
                if let Some(&val) = std_map.get(black_box(key)) {
                    sum += val;
                }
            }
            sum
        });
    });

    group.bench_function("ahash_lookup", |b| {
        b.iter(|| {
            let mut sum = 0;
            for key in &keys {
                if let Some(&val) = ahash_map.get(black_box(key)) {
                    sum += val;
                }
            }
            sum
        });
    });

    group.finish();
}

fn bench_hashset_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("hashset_operations");

    for size in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(BenchmarkId::new("std_insert", size), &size, |b, &size| {
            b.iter(|| {
                let mut set = StdHashSet::new();
                for i in 0..size {
                    set.insert(black_box(i));
                }
                set
            });
        });

        group.bench_with_input(BenchmarkId::new("ahash_insert", size), &size, |b, &size| {
            b.iter(|| {
                let mut set = AHashSet::new();
                for i in 0..size {
                    set.insert(black_box(i));
                }
                set
            });
        });

        let std_set: StdHashSet<i32> = (0..size).collect();
        let ahash_set: AHashSet<i32> = (0..size).collect();

        group.bench_with_input(BenchmarkId::new("std_contains", size), &size, |b, &size| {
            b.iter(|| {
                let mut count = 0;
                for i in 0..size {
                    if std_set.contains(&black_box(i)) {
                        count += 1;
                    }
                }
                count
            });
        });

        group.bench_with_input(
            BenchmarkId::new("ahash_contains", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let mut count = 0;
                    for i in 0..size {
                        if ahash_set.contains(&black_box(i)) {
                            count += 1;
                        }
                    }
                    count
                });
            },
        );
    }

    group.finish();
}

fn bench_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_workload");

    // Simulate entity component system lookups
    group.bench_function("std_ecs_simulation", |b| {
        b.iter(|| {
            let mut components = StdHashMap::new();

            // Insert components
            for entity_id in 0..100 {
                components.insert(entity_id, (entity_id as f32, entity_id as f32 * 2.0));
            }

            // Update components
            for entity_id in 0..100 {
                if let Some(pos) = components.get_mut(&entity_id) {
                    pos.0 += 1.0;
                    pos.1 += 1.0;
                }
            }

            // Query components
            let mut sum = 0.0;
            for (_id, pos) in components.iter() {
                sum += pos.0 + pos.1;
            }

            // Remove some components
            for entity_id in 0..20 {
                components.remove(&entity_id);
            }

            black_box(sum)
        });
    });

    group.bench_function("ahash_ecs_simulation", |b| {
        b.iter(|| {
            let mut components = AHashMap::new();

            // Insert components
            for entity_id in 0..100 {
                components.insert(entity_id, (entity_id as f32, entity_id as f32 * 2.0));
            }

            // Update components
            for entity_id in 0..100 {
                if let Some(pos) = components.get_mut(&entity_id) {
                    pos.0 += 1.0;
                    pos.1 += 1.0;
                }
            }

            // Query components
            let mut sum = 0.0;
            for (_id, pos) in components.iter() {
                sum += pos.0 + pos.1;
            }

            // Remove some components
            for entity_id in 0..20 {
                components.remove(&entity_id);
            }

            black_box(sum)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_hashmap_insert,
    bench_hashmap_lookup,
    bench_hashmap_iteration,
    bench_hashmap_string_keys,
    bench_hashset_operations,
    bench_mixed_workload
);
criterion_main!(benches);
