//! End-to-end tests through the public profiling API.

#![cfg(feature = "enabled")]

use std::time::Duration;

static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn records_nested_scopes_counters_and_named_threads() {
    let _test_guard = TEST_LOCK.lock().expect("test lock poisoned");
    astrelis_profiling::init();
    astrelis_profiling::set_thread_name("integration-main");

    {
        astrelis_profiling::profile_scope!("outer_integration_scope");
        {
            astrelis_profiling::profile_scope!("inner_integration_scope");
            astrelis_profiling::profile_counter!("integration", "items_processed", 7_u64);
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    let worker = astrelis_profiling::spawn_profiled("integration-worker", || {
        astrelis_profiling::profile_scope!("worker_integration_scope");
    });
    worker.join().expect("profiled worker panicked");

    astrelis_profiling::frame_mark();

    let profiler = astrelis_profiling::Profiler::get();
    let timeline = profiler.timeline.read().expect("timeline lock poisoned");

    let spans: Vec<_> = timeline
        .thread_streams
        .values()
        .flat_map(|stream| stream.spans.iter())
        .collect();
    assert!(spans.len() >= 3);
    assert!(spans.iter().any(|span| span.parent.is_some()));

    let samples: Vec<_> = timeline
        .counter_streams
        .values()
        .flat_map(|stream| stream.samples.iter())
        .collect();
    assert!(samples.iter().any(|sample| sample.value == 7.0));

    let thread_names: Vec<_> = timeline
        .threads
        .values()
        .filter_map(|thread| profiler.strings.get(thread.name))
        .collect();
    assert!(thread_names.iter().any(|name| name == "integration-main"));
    assert!(thread_names.iter().any(|name| name == "integration-worker"));
}

#[test]
fn runtime_disable_skips_new_events() {
    let _test_guard = TEST_LOCK.lock().expect("test lock poisoned");
    astrelis_profiling::init();
    astrelis_profiling::frame_mark();

    let before = {
        let profiler = astrelis_profiling::Profiler::get();
        let timeline = profiler.timeline.read().expect("timeline lock poisoned");
        timeline
            .thread_streams
            .values()
            .map(|stream| stream.spans.len())
            .sum::<usize>()
    };

    astrelis_profiling::set_enabled(false);
    {
        astrelis_profiling::profile_scope!("runtime_disabled_integration_scope");
    }
    astrelis_profiling::set_enabled(true);
    astrelis_profiling::frame_mark();

    let profiler = astrelis_profiling::Profiler::get();
    let timeline = profiler.timeline.read().expect("timeline lock poisoned");
    let after = timeline
        .thread_streams
        .values()
        .map(|stream| stream.spans.len())
        .sum::<usize>();
    assert_eq!(after, before);
}
