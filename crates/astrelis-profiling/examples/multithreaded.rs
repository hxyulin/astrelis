//! Demonstrates collecting nested scopes from multiple named worker threads.
//!
//! Run with:
//!
//! ```text
//! cargo run -p astrelis-profiling --example multithreaded
//! ```

use std::time::Duration;

fn main() {
    astrelis_profiling::init();
    astrelis_profiling::set_thread_name("main");

    let workers: Vec<_> = (0..4)
        .map(|worker_index| {
            astrelis_profiling::spawn_profiled(&format!("worker-{worker_index}"), move || {
                astrelis_profiling::profile_function!();
                for batch in 0..3 {
                    astrelis_profiling::profile_scope!("process_batch");
                    let sample = worker_index * 3 + batch;
                    astrelis_profiling::profile_counter!("work", "worker_batch", sample);
                    std::hint::black_box(sample);
                    std::thread::sleep(Duration::from_millis(1));
                }
            })
        })
        .collect();

    for worker in workers {
        worker.join().expect("profiled worker panicked");
    }

    astrelis_profiling::frame_mark();

    let profiler = astrelis_profiling::Profiler::get();
    let timeline = profiler.timeline.read().expect("timeline lock poisoned");
    let span_count: usize = timeline
        .thread_streams
        .values()
        .map(|stream| stream.spans.len())
        .sum();
    let sample_count: usize = timeline
        .counter_streams
        .values()
        .map(|stream| stream.samples.len())
        .sum();

    println!(
        "collected {} spans and {} counter samples from {} threads",
        span_count,
        sample_count,
        timeline.threads.len()
    );

    assert!(timeline.threads.len() >= 4);
    assert!(span_count >= 16);
    assert_eq!(sample_count, 12);
}
