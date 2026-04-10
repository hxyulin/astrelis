//! Cross-crate puffin test — verifies profiling works from a downstream crate.
//!
//! ```sh
//! cargo run -p astrelis-gpu-wgpu --example puffin_test --features astrelis-profiling/puffin
//! ```

fn do_work() {
    astrelis_profiling::profile_function!();
    std::thread::sleep(std::time::Duration::from_millis(16));
}

fn main() {
    astrelis_profiling::init();
    eprintln!("Running 300 frames from astrelis-gpu-wgpu. Connect puffin_viewer to 127.0.0.1:8585");

    for i in 0..300 {
        astrelis_profiling::new_frame();
        do_work();
        if i % 60 == 0 {
            eprintln!("Frame {i}...");
        }
    }

    astrelis_profiling::finish();
}
