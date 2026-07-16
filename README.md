# Astrelis

A low-level native application and graphics runtime for Rust.

Astrelis is being rebuilt methodically from a small foundation. The current
workspace contains:

- `astrelis-core`: shared math, color, geometry, IDs, and logging helpers;
- `astrelis-profiling`: dependency-free CPU/GPU timeline profiling.

Windowing, event-loop, GPU, painting, text, application-runtime, and UI layers
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
