//! Basic profiling example.
//!
//! Run with a backend feature to enable actual profiling:
//!
//!   cargo run -p astrelis-profiling --features tracy --example basic_profiling
//!   cargo run -p astrelis-profiling --features puffin --example basic_profiling
//!
//! **Tracy:** Open the Tracy profiler GUI *before* running this example so it
//! can capture the (short) session. Or use `tracy-capture -o trace.tracy` to
//! save to a file and open it afterwards.
//!
//! **Puffin:** A puffin_http server starts on localhost:8585. Open the
//! puffin_viewer application to inspect frames.
//!
//! Without any backend feature, all profiling calls compile to no-ops.

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

    astrelis_profiling::finish();
}
