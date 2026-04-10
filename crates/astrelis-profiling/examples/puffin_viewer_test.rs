//! Minimal test to verify puffin viewer can see scopes.
//!
//! Run with:
//! ```sh
//! cargo run -p astrelis-profiling --features puffin --example puffin_viewer_test
//! ```
//! Then connect puffin_viewer to 127.0.0.1:8585.

fn do_expensive_work() {
    astrelis_profiling::profile_function!();
    std::thread::sleep(std::time::Duration::from_millis(16));
}

fn do_sub_task() {
    astrelis_profiling::profile_scope!("sub_task");
    std::thread::sleep(std::time::Duration::from_millis(5));
}

fn main() {
    astrelis_profiling::init();

    #[cfg(feature = "puffin")]
    eprintln!("Puffin scopes on: {}", astrelis_profiling::puffin::are_scopes_on());
    #[cfg(not(feature = "puffin"))]
    eprintln!("No profiling backend enabled. Run with --features puffin");

    eprintln!("Running 300 frames (~5 seconds). Connect puffin_viewer to 127.0.0.1:8585");

    for i in 0..300 {
        astrelis_profiling::new_frame();

        do_expensive_work();
        do_sub_task();

        if i % 60 == 0 {
            eprintln!("Frame {i}...");
        }
    }

    astrelis_profiling::finish();
    eprintln!("Done.");
}
