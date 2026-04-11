# astrelis-profiling

A custom in-engine profiler for the Astrelis engine. CPU and GPU
spans are collected into a single global timeline and rendered
in-process by the `astrelis-profiling-egui` viewer — no external
tool is required.

Enabled by default via the `enabled` Cargo feature. When active,
the hot path for `profile_scope!` and `profile_function!` is
roughly ~100 ns per scope (thread-local write under an uncontended
mutex). Compile with `--no-default-features` for zero-cost release
builds, or call `set_enabled(false)` for a runtime toggle (~1 ns
per-scope cost for the atomic check).

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

// Each frame (drains thread-local buffers into the timeline):
astrelis_profiling::new_frame();
```

## Counters and plots

```rust
astrelis_profiling::profile_counter!("gpu_memory", "buffer_bytes", 1024u64);
astrelis_profiling::profile_plot!("frame_time_ms", 16.3);
```

## GPU profiling

GPU timestamps are collected by `wgpu-profiler` in `astrelis-gpu`
and submitted to the global timeline via
`astrelis_profiling::gpu::report_gpu_frame`. They share the same
nanosecond axis as CPU spans once calibration has run.

## Viewer

The in-engine viewer lives in the sibling
[`astrelis-profiling-egui`](../astrelis-profiling-egui) crate. It
exposes an egui widget that reads from the global timeline and
renders a flame graph of the most recent frame (Stage 1) and — in
later stages — a scrollable multi-frame timeline.

## License

MIT
