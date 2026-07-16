//! Hot-path microbenchmark for `astrelis-profiling`.
//!
//! Measures scope recording, runtime-disabled scopes, and frame
//! aggregation under representative single- and multi-threaded loads.
//!
//! Run with:
//! ```sh
//! cargo bench -p astrelis-profiling --bench hot_path
//! ```
//!
//! # Interpreting the 4-thread results
//!
//! `scope/empty_4t_contended` reports wall-clock per-iter time under
//! criterion's standard statistics. Each "iter" corresponds to one
//! `profile_scope!` call per worker: i.e. the value *is* the per-call
//! cost when four threads are producing spans concurrently. Barrier
//! synchronization cost is amortised over large iteration counts by
//! criterion's adaptive warmup, but numbers below ~100 ns per iter
//! should be treated sceptically — barrier overhead probably dominates
//! and the true scope cost is being hidden.
//!
//! # Why a persistent worker pool
//!
//! Each `std::thread::spawn` allocates a fresh `ThreadBuffer` and
//! registers it with the profiler. If we spawned per iteration, the
//! `ThreadRegistry` would grow without bound and every subsequent
//! `frame_mark()` measurement would have to drain an ever-longer list
//! of dead buffers. Keeping a single pool of four persistent workers
//! means the registry has exactly 5 entries throughout the run
//! (1 main + 4 workers) and all benches see a consistent baseline.

#![allow(missing_docs)]

use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};

struct WorkerPool {
    start: Arc<Barrier>,
    done: Arc<Barrier>,
    count: Arc<AtomicUsize>,
    _handles: Vec<thread::JoinHandle<()>>,
}

impl WorkerPool {
    fn new(n_workers: usize) -> Self {
        let start = Arc::new(Barrier::new(n_workers + 1));
        let done = Arc::new(Barrier::new(n_workers + 1));
        let count = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::with_capacity(n_workers);
        for i in 0..n_workers {
            let start = start.clone();
            let done = done.clone();
            let count = count.clone();
            let handle = thread::Builder::new()
                .name(format!("bench_worker_{i}"))
                .spawn(move || {
                    astrelis_profiling::profile_thread!("bench_worker");
                    loop {
                        start.wait();
                        let n = count.load(Ordering::Relaxed);
                        for _ in 0..n {
                            astrelis_profiling::profile_scope!("contended_scope");
                            black_box(());
                        }
                        done.wait();
                    }
                })
                .expect("spawn worker");
            handles.push(handle);
        }
        Self {
            start,
            done,
            count,
            _handles: handles,
        }
    }

    /// Tells every worker to run `n` `profile_scope!` calls, then
    /// blocks the caller until all workers have reported done.
    fn run_each(&self, n: usize) {
        self.count.store(n, Ordering::Relaxed);
        self.start.wait();
        self.done.wait();
    }
}

/// Lazily-initialised singleton worker pool. Lives for the entire
/// bench process so registry sizes stay constant across bench
/// functions.
static POOL: OnceLock<WorkerPool> = OnceLock::new();

fn pool() -> &'static WorkerPool {
    POOL.get_or_init(|| WorkerPool::new(4))
}

/// Drains any accumulated events AND clears the timeline so the
/// following bench starts from a clean slate. Without the
/// `clear_data` call, scope-cost benches at 80M iterations would
/// leave hundreds of millions of spans in the timeline, and the
/// frame_mark benches would then measure the cost of evicting that
/// contamination instead of their own load.
fn drain() {
    astrelis_profiling::frame_mark();
    let p = astrelis_profiling::profiler::Profiler::get();
    p.timeline.write().unwrap().clear_data();
}

fn bench_scope_empty_1t(c: &mut Criterion) {
    astrelis_profiling::init();
    let _ = pool();
    drain();
    c.bench_function("scope/empty_1t", |b| {
        b.iter(|| {
            astrelis_profiling::profile_scope!("bench_empty");
            black_box(());
        });
    });
}

fn bench_scope_nested_4_1t(c: &mut Criterion) {
    astrelis_profiling::init();
    let _ = pool();
    drain();
    c.bench_function("scope/nested_4_1t", |b| {
        b.iter(|| {
            astrelis_profiling::profile_scope!("d0");
            {
                astrelis_profiling::profile_scope!("d1");
                {
                    astrelis_profiling::profile_scope!("d2");
                    {
                        astrelis_profiling::profile_scope!("d3");
                        black_box(());
                    }
                }
            }
        });
    });
}

fn bench_scope_empty_4t_contended(c: &mut Criterion) {
    astrelis_profiling::init();
    let pool = pool();
    drain();
    c.bench_function("scope/empty_4t_contended", |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();
            pool.run_each(iters as usize);
            start.elapsed()
        });
    });
}

fn bench_frame_mark_cold(c: &mut Criterion) {
    astrelis_profiling::init();
    let _ = pool();
    drain();
    let mut group = c.benchmark_group("frame_mark");
    for &span_count in &[0usize, 100, 10_000] {
        group.bench_function(format!("{span_count}_spans_1t"), |b| {
            b.iter_batched(
                || {
                    for _ in 0..span_count {
                        astrelis_profiling::profile_scope!("prep");
                        black_box(());
                    }
                },
                |_| {
                    astrelis_profiling::frame_mark();
                },
                BatchSize::PerIteration,
            );
        });
    }
    group.finish();
}

fn bench_scope_runtime_disabled(c: &mut Criterion) {
    astrelis_profiling::init();
    let _ = pool();
    drain();
    astrelis_profiling::set_enabled(false);
    c.bench_function("scope/runtime_disabled", |b| {
        b.iter(|| {
            astrelis_profiling::profile_scope!("disabled_scope");
            black_box(());
        });
    });
    astrelis_profiling::set_enabled(true);
}

fn bench_frame_mark_contended_4t(c: &mut Criterion) {
    astrelis_profiling::init();
    let pool = pool();
    drain();
    c.bench_function("frame_mark/10k_spans_each_4t", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                pool.run_each(10_000);
                let start = Instant::now();
                astrelis_profiling::frame_mark();
                total += start.elapsed();
            }
            total
        });
    });
}

criterion_group!(
    benches,
    bench_scope_empty_1t,
    bench_scope_nested_4_1t,
    bench_scope_empty_4t_contended,
    bench_scope_runtime_disabled,
    bench_frame_mark_cold,
    bench_frame_mark_contended_4t,
);
criterion_main!(benches);
