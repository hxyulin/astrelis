//! Integration test verifying that profiling macros actually record scopes
//! when the puffin feature is enabled.
//!
//! Run with:
//! ```sh
//! cargo test -p astrelis-profiling --features puffin
//! ```

#![cfg(feature = "puffin")]

fn profiled_fn() {
    astrelis_profiling::profile_function!();
    std::thread::sleep(std::time::Duration::from_millis(1));
}

fn profiled_scope_fn() {
    astrelis_profiling::profile_scope!("my_scope");
    std::thread::sleep(std::time::Duration::from_millis(1));
}

#[test]
fn profile_function_records_scopes() {
    astrelis_profiling::puffin::set_scopes_on(true);

    // Create a FrameView sink to capture frame data.
    let frame_view = std::sync::Arc::new(astrelis_profiling::puffin::GlobalFrameView::default());
    let fv = frame_view.clone();
    let sink_id =
        astrelis_profiling::puffin::GlobalProfiler::lock().add_sink(Box::new(move |frame| {
            fv.lock().add_frame(frame);
        }));

    // Frame 1: record some work
    astrelis_profiling::puffin::GlobalProfiler::lock().new_frame();
    profiled_fn();
    profiled_scope_fn();

    // Frame 2: finalize frame 1
    astrelis_profiling::puffin::GlobalProfiler::lock().new_frame();

    let fv = frame_view.lock();
    let latest = fv.latest_frame();
    assert!(latest.is_some(), "should have captured a frame");

    let frame = latest.unwrap();
    let unpacked = frame.unpacked().expect("should unpack frame data");
    let has_scopes = !unpacked.thread_streams.is_empty();

    assert!(
        has_scopes,
        "expected profiling thread streams to be recorded in the frame"
    );

    // Verify the streams actually contain data
    for (info, stream) in &unpacked.thread_streams {
        eprintln!(
            "Thread '{}': stream has {} bytes",
            info.name,
            stream.stream.len()
        );
        assert!(
            !stream.stream.is_empty(),
            "thread stream should contain scope data"
        );
    }

    // Cleanup
    drop(fv);
    astrelis_profiling::puffin::GlobalProfiler::lock().remove_sink(sink_id);
    astrelis_profiling::puffin::set_scopes_on(false);
}
