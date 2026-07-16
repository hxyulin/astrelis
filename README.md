# Astrelis

A low-level native application and graphics runtime for Rust.

Astrelis is being rebuilt methodically from a small foundation. The current
workspace contains:

- `astrelis-core`: shared math, color, geometry, IDs, and logging helpers;
- `astrelis-gpu`: backend-neutral GPU resources, commands, and surfaces;
- `astrelis-gpu-wgpu`: native wgpu implementation and GPU profiling bridge;
- `astrelis-platform`: backend-neutral windows, lifecycle, and input;
- `astrelis-platform-winit`: desktop winit implementation;
- `astrelis-platform-test`: deterministic display-free scripted backend;
- `astrelis-profiling`: dependency-free CPU/GPU timeline profiling.

Painting, text, application-runtime, and UI layers
will be added as separately testable vertical slices.

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
