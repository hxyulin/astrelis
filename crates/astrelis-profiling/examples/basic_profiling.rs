//! End-to-end demo of the Astrelis in-engine profiler.
//!
//! Runs a synthetic physics + render loop, marks frames, and prints
//! a one-line summary after the run. No external tool is required:
//! the profiler collects into its global timeline in-process.
//!
//! For a live flame-graph window, see the `viewer_demo` example in
//! the `astrelis-profiling-egui` crate.
//!
//! Run with:
//!
//!     cargo run -p astrelis-profiling --example basic_profiling

use astrelis_profiling::Profiler;

fn simulate_physics() {
    astrelis_profiling::profile_function!();
    std::thread::sleep(std::time::Duration::from_millis(5));
}

fn render() {
    astrelis_profiling::profile_function!();
    {
        astrelis_profiling::profile_scope!("prepare_draw_calls");
        std::thread::sleep(std::time::Duration::from_millis(3));
    }
    {
        astrelis_profiling::profile_scope!("submit");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

fn main() {
    astrelis_profiling::init();

    for _ in 0..10 {
        astrelis_profiling::new_frame();
        simulate_physics();
        render();
    }
    // One final frame_mark so the spans from the last loop iteration
    // are drained into the timeline.
    astrelis_profiling::new_frame();
    astrelis_profiling::finish();

    let p = Profiler::get();
    let timeline = p.timeline.read().unwrap();
    let total_spans: usize = timeline.thread_streams.values().map(|s| s.spans.len()).sum();
    let frames = timeline.frame_marks.len();
    let threads = timeline.threads.len();
    let scopes = timeline.scopes.len();

    println!(
        "profiler collected: {frames} frames, {threads} thread(s), \
         {scopes} scope site(s), {total_spans} span(s)"
    );

    assert!(frames >= 10, "expected at least 10 frame marks");
    assert!(
        total_spans >= 10 * 4,
        "expected at least 40 spans (4 per frame × 10 frames)"
    );
}
