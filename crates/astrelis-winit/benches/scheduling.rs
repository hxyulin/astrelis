//! Isolated scheduler/registry scan, excluding native calls and all GPU work.
//! Uses the driver's actual private Schedule implementation with integer map keys.
use astrelis_winit::RedrawMode;
use std::{
    collections::HashMap,
    hint::black_box,
    time::{Duration, Instant},
};
#[path = "../src/scheduler.rs"]
mod scheduler;
use scheduler::Schedule;
const ITERATIONS: u32 = 10_000;
const SAMPLES: usize = 40;
fn step(windows: &mut HashMap<usize, Schedule>, now: Instant) -> usize {
    let active = windows.get_mut(&0).unwrap();
    active.begin_redraw();
    active.submitted();
    let mut requests = 0;
    for schedule in windows.values_mut() {
        schedule.advance(now);
        requests += usize::from(schedule.request_native(now, true));
        black_box(schedule.wake(true));
    }
    requests
}
fn main() {
    // Exercise cold/error paths once; only the warmed success path is timed.
    let now = Instant::now();
    let mut cold = Schedule::default();
    cold.at(now);
    cold.cancel_deadline();
    cold.availability_changed();
    cold.suspended();
    cold.unavailable();
    cold.invalidate();
    cold.retry(now, Duration::from_millis(16));
    assert!(cold.lost(3));
    println!("windows,samples,iterations_per_sample,median_ns,p95_ns");
    for count in [1, 4, 16, 256] {
        let mut windows: HashMap<_, _> = (0..count)
            .map(|id| {
                let mut s = Schedule::default();
                s.begin_redraw();
                (id, s)
            })
            .collect();
        windows.get_mut(&0).unwrap().mode(RedrawMode::Continuous);
        for _ in 0..100 {
            assert_eq!(step(&mut windows, now), 1);
        }
        let mut samples = Vec::with_capacity(SAMPLES);
        for _ in 0..SAMPLES {
            let start = Instant::now();
            for _ in 0..ITERATIONS {
                black_box(step(black_box(&mut windows), black_box(now)));
            }
            samples.push(start.elapsed().as_secs_f64() * 1e9 / f64::from(ITERATIONS));
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "{count},{SAMPLES},{ITERATIONS},{:.3},{:.3}",
            samples[SAMPLES / 2],
            samples[(SAMPLES * 95).div_ceil(100) - 1]
        );
    }
}
