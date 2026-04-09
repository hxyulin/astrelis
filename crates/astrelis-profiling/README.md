# astrelis-profiling

Backend-agnostic profiling macros for the Astrelis engine.

All macros compile to zero-cost no-ops when no backend feature is enabled,
so profiling instrumentation can stay in production code without overhead.

## Backends

| Feature  | Backend | Description |
|----------|---------|-------------|
| `puffin` | [puffin](https://crates.io/crates/puffin) | CPU profiling with HTTP viewer on `localhost:8585` |
| *(none)* | no-op | All macros expand to nothing |

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

// Each frame:
astrelis_profiling::new_frame();

// At shutdown:
astrelis_profiling::finish();
```

Enable a backend in your `Cargo.toml`:

```toml
[dependencies]
astrelis-profiling = { version = "0.3", features = ["puffin"] }
```

## GPU Profiling

The crate also defines a `GpuProfiler` trait for future GPU crate integration.
Implement it on your GPU context to enable GPU scope tracking.

## License

MIT
