# Astrelis

A low-level native application and graphics runtime for Rust.

Astrelis is being rebuilt methodically from a small foundation. The current
workspace contains:

- `astrelis-app`: shared timers, wakeups, invalidation, and frame scheduling;
- `astrelis-core`: shared math, color, geometry, IDs, and logging helpers;
- `astrelis-gpu`: backend-neutral GPU resources, commands, and surfaces;
- `astrelis-gpu-wgpu`: native wgpu implementation and GPU profiling bridge;
- `astrelis-platform`: backend-neutral windows, lifecycle, and input;
- `astrelis-platform-winit`: desktop winit implementation;
- `astrelis-platform-test`: deterministic display-free scripted backend;
- `astrelis-profiling`: dependency-free CPU/GPU timeline profiling.
- `astrelis-ui-core`: retained UI trees, Taffy layout, routed input, semantics,
  widgets, and display-list generation.

Painting, text, and the first retained UI vertical slice are available as
separately testable layers above the shared application runtime.

## Development

```sh
cargo fmt --all --check
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Profiling can be compiled without instrumentation:

```sh
cargo check -p astrelis-profiling --no-default-features
```

## License

Licensed under the MIT license. See [LICENSE-MIT](LICENSE-MIT) for details.
