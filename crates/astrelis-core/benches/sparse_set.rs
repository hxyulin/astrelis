//! Benchmarks for SparseSet data structure

use astrelis_core::alloc::{
    HashMap,
    sparse_set::{IndexSlot, SparseSet},
};
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};

#[derive(Clone, Copy, Debug)]
struct EntityData {
    position: (f32, f32, f32),
    velocity: (f32, f32, f32),
    health: f32,
    flags: u32,
}

impl Default for EntityData {
    fn default() -> Self {
        Self {
            position: (0.0, 0.0, 0.0),
            velocity: (0.0, 0.0, 0.0),
            health: 100.0,
            flags: 0,
        }
    }
}

fn bench_sparse_set_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparse_set_insert");

    for size in [10, 100, 1000, 10000] {
        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                let mut set = SparseSet::new();
                for _ in 0..size {
                    set.push(black_box(EntityData::default()));
                }
                set
            });
        });
    }

    group.finish();
}

fn bench_sparse_set_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparse_set_access");

    for size in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(size as u64));

        let mut set = SparseSet::new();
        let indices: Vec<IndexSlot> = (0..size).map(|_| set.push(EntityData::default())).collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let mut sum = 0.0;
                for &idx in &indices {
                    let data = set.get(black_box(idx));
                    sum += data.health;
                }
                black_box(sum)
            });
        });
    }

    group.finish();
}

fn bench_sparse_set_access_mut(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparse_set_access_mut");

    for size in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter_batched(
                || {
                    let mut set = SparseSet::new();
                    let indices: Vec<IndexSlot> =
                        (0..size).map(|_| set.push(EntityData::default())).collect();
                    (set, indices)
                },
                |(mut set, indices)| {
                    for &idx in &indices {
                        let data = set.get_mut(black_box(idx));
                        data.health -= 1.0;
                        data.position.0 += data.velocity.0;
                        data.position.1 += data.velocity.1;
                        data.position.2 += data.velocity.2;
                    }
                    black_box(set)
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_sparse_set_remove(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparse_set_remove");

    for size in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter_batched(
                || {
                    let mut set = SparseSet::new();
                    let indices: Vec<IndexSlot> =
                        (0..size).map(|_| set.push(EntityData::default())).collect();
                    (set, indices)
                },
                |(mut set, indices)| {
                    for idx in indices {
                        set.remove(black_box(idx));
                    }
                    black_box(set)
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_sparse_set_iteration(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparse_set_iteration");

    for size in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(size as u64));

        let mut set = SparseSet::new();
        for _ in 0..size {
            set.push(EntityData::default());
        }

        group.bench_with_input(BenchmarkId::from_parameter(size), &set, |b, set| {
            b.iter(|| {
                let mut sum = 0.0;
                for data in set.iter() {
                    sum += data.health + data.position.0 + data.position.1 + data.position.2;
                }
                black_box(sum)
            });
        });
    }

    group.finish();
}

fn bench_sparse_set_with_holes(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparse_set_with_holes");

    for size in [100, 1000, 10000] {
        group.throughput(Throughput::Elements(size as u64));

        // Create set with 50% holes (removed entries)
        let mut set = SparseSet::new();
        let mut indices = Vec::new();
        for _ in 0..size {
            indices.push(set.push(EntityData::default()));
        }

        // Remove every other entry
        for i in (0..indices.len()).step_by(2) {
            set.remove(indices[i]);
        }

        group.bench_with_input(BenchmarkId::from_parameter(size), &set, |b, set| {
            b.iter(|| {
                let mut sum = 0.0;
                for data in set.iter() {
                    sum += data.health;
                }
                black_box(sum)
            });
        });
    }

    group.finish();
}

fn bench_sparse_set_reuse_slots(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparse_set_reuse_slots");

    group.bench_function("reuse_100_slots", |b| {
        b.iter(|| {
            let mut set = SparseSet::new();
            let mut indices = Vec::new();

            // Fill
            for _ in 0..100 {
                indices.push(set.push(EntityData::default()));
            }

            // Remove all
            for idx in indices.drain(..) {
                set.remove(idx);
            }

            // Refill (should reuse slots)
            for _ in 0..100 {
                indices.push(set.push(EntityData::default()));
            }

            black_box(set)
        });
    });

    group.finish();
}

fn bench_sparse_set_vs_hashmap(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparse_set_vs_hashmap");

    let size = 1000;
    group.throughput(Throughput::Elements(size as u64));

    // SparseSet insert + access
    group.bench_function("sparse_set", |b| {
        b.iter(|| {
            let mut set = SparseSet::new();
            let mut indices = Vec::new();

            // Insert
            for _ in 0..size {
                indices.push(set.push(black_box(EntityData::default())));
            }

            // Access and update
            let mut sum = 0.0;
            for &idx in &indices {
                let data = set.get_mut(idx);
                data.health -= 1.0;
                sum += data.health;
            }

            black_box(sum)
        });
    });

    // HashMap insert + access
    group.bench_function("hashmap", |b| {
        b.iter(|| {
            let mut map = HashMap::new();

            // Insert
            for i in 0..size {
                map.insert(black_box(i), black_box(EntityData::default()));
            }

            // Access and update
            let mut sum = 0.0;
            for i in 0..size {
                if let Some(data) = map.get_mut(&i) {
                    data.health -= 1.0;
                    sum += data.health;
                }
            }

            black_box(sum)
        });
    });

    group.finish();
}

fn bench_entity_system_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("entity_system_simulation");

    group.bench_function("sparse_set_entity_simulation", |b| {
        b.iter(|| {
            let mut entities = SparseSet::new();
            let mut active_entities = Vec::new();

            // Spawn 500 entities
            for i in 0..500 {
                let entity = EntityData {
                    position: (i as f32, i as f32, 0.0),
                    velocity: (1.0, -1.0, 0.0),
                    health: 100.0,
                    flags: i as u32,
                };
                active_entities.push(entities.push(entity));
            }

            // Simulate 100 frames
            for _frame in 0..100 {
                // Update all entities
                for &entity_id in &active_entities {
                    let entity = entities.get_mut(entity_id);
                    entity.position.0 += entity.velocity.0;
                    entity.position.1 += entity.velocity.1;
                    entity.position.2 += entity.velocity.2;
                    entity.health -= 0.1;
                }

                // Remove dead entities (health <= 0)
                active_entities.retain(|&entity_id| {
                    let entity = entities.get(entity_id);
                    if entity.health <= 0.0 {
                        entities.remove(entity_id);
                        false
                    } else {
                        true
                    }
                });

                // Spawn new entities occasionally
                if _frame % 10 == 0 && active_entities.len() < 450 {
                    for _ in 0..5 {
                        active_entities.push(entities.push(EntityData::default()));
                    }
                }
            }

            black_box(entities)
        });
    });

    group.finish();
}

fn bench_generation_check_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("generation_check_overhead");

    let mut set = SparseSet::new();
    let indices: Vec<IndexSlot> = (0..1000).map(|_| set.push(EntityData::default())).collect();

    group.bench_function("try_get_success", |b| {
        b.iter(|| {
            let mut sum = 0.0;
            for &idx in &indices {
                if let Some(data) = set.try_get(black_box(idx)) {
                    sum += data.health;
                }
            }
            black_box(sum)
        });
    });

    // Create invalid indices (wrong generation)
    let invalid_indices: Vec<IndexSlot> = indices
        .iter()
        .map(|idx| IndexSlot::new(idx.generation() + 1, idx.index()))
        .collect();

    group.bench_function("try_get_failure", |b| {
        b.iter(|| {
            let mut count = 0;
            for &idx in &invalid_indices {
                if set.try_get(black_box(idx)).is_some() {
                    count += 1;
                }
            }
            black_box(count)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_sparse_set_insert,
    bench_sparse_set_access,
    bench_sparse_set_access_mut,
    bench_sparse_set_remove,
    bench_sparse_set_iteration,
    bench_sparse_set_with_holes,
    bench_sparse_set_reuse_slots,
    bench_sparse_set_vs_hashmap,
    bench_entity_system_simulation,
    bench_generation_check_overhead
);
criterion_main!(benches);
