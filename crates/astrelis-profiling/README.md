# astrelis-profiling

A dependency-free in-process profiler for Astrelis. CPU spans,
counter samples, frame marks, and externally collected GPU spans are
stored on one queryable timeline.

Enabled by default via the `enabled` Cargo feature. When active,
scope events are written to per-thread buffers and aggregated at frame
boundaries. Compile with `--no-default-features` to remove macro
instrumentation, or call `set_enabled(false)` to disable collection at
runtime. Exact costs are machine-dependent; use the included Criterion
benchmark rather than relying on a fixed advertised number.

## Usage

```rust
fn update_physics() {
    astrelis_profiling::profile_function!();

    {
        astrelis_profiling::profile_scope!("broad_phase");
        // ...
    }
    {
        astrelis_profiling::profile_scope!("narrow_phase");
        // ...
    }
}

// At startup:
astrelis_profiling::init();

// After each logical frame or update interval:
astrelis_profiling::frame_mark();
```

`frame_mark` drains every registered thread's pending events into the
global timeline. Call it after the work that belongs to the frame.

## Counters and plots

```rust
astrelis_profiling::profile_counter!("gpu_memory", "buffer_bytes", 1024u64);
astrelis_profiling::profile_plot!("frame_time_ms", 16.3);
```

## Threads

```rust
let worker = astrelis_profiling::spawn_profiled("asset-loader", || {
    astrelis_profiling::profile_scope!("load_batch");
});
worker.join().unwrap();
```

## GPU profiling

The crate does not depend on a graphics API. A GPU backend converts its
timestamp-query results into `GpuFrame`/`GpuScope` values and submits
them through `gpu::report_gpu_frame`. Once the clock offset has been
calibrated, CPU and GPU spans share the same nanosecond axis.

## Inspecting data

```rust
let profiler = astrelis_profiling::Profiler::get();
let timeline = profiler.timeline.read().unwrap();

for (thread, stream) in &timeline.thread_streams {
    println!("{thread:?}: {} spans", stream.spans.len());
}
```

The timeline is intentionally public so future inspectors and exporters
can consume it without coupling this crate to a UI framework.

## Examples and benchmarks

```sh
cargo run -p astrelis-profiling --example basic_profiling
cargo run -p astrelis-profiling --example multithreaded
cargo bench -p astrelis-profiling --bench hot_path
```

## Retention

The default timeline retains 600 frame marks and evicts older completed
span and counter data. Applications or future tooling may adjust
`Timeline::retention`.

## License

MIT
