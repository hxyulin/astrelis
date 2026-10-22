# Benchmark runtime regression checks

`cargo test` and Clippy do not execute the harness-free benchmarks. After shader
or pipeline interface changes, run these bounded release checks sequentially:

```sh
cargo bench -p astrelis --locked --bench rendering -- --counts 1,1025 --samples 5 --warmup 1
cargo bench -p astrelis --locked --bench paths -- --counts 1,1025 --samples 5 --warmup 1
cargo bench -p astrelis --locked --bench points -- --counts 256,4097 --samples 5
```

These exercise every raw/wrapped workload, including the 1024-instance upload
page boundary, both path coverage modes, polylines, markers, ring updates, and
visible-range selection. Each harness requires byte-for-byte pixel equivalence
and nonempty output before timing. Keep wgpu validation enabled. Five samples
are a runtime smoke check, not evidence for performance claims.

The three harnesses are the existing benchmarks that compile built-in WGSL into
raw pipelines. Their shared `benches/support/clipping.rs` prepends the same clip
functions and device-sized rounded-stack WGSL as the built-in renderers. Raw
pipelines bind the context's public group-one layout and a zero-count immutable
stack, allocated during setup. Texture/path references also supply disabled
stride-zero clip vertex attributes. Clipping remains enabled in the shader;
these workloads measure unclipped drawing on both routes.

The texture reference uses `fragment_straight_linear`, matching the wrapped
binding's default linear filter. Its 2×2 fixture varies both color and alpha to
detect incorrect straight-alpha filtering. This replaces the former opaque white
fixture, so historical rendering timings describe a different texture workload.
Draw counts, packing, uploads, blend state, paired sampling, and pixel checks
remain matched between the two routes.

## Verified run

On 2026-10-23, Apple M5 Max / Metal, macOS 27.0.1 (26A434), aarch64,
rustc 1.99.0, wgpu 30.0.1, release profile and default wgpu validation:

- Rendering: 18 pixel checks passed ([CSV](benchmark-smoke/rendering.csv),
  [log](benchmark-smoke/rendering.log)).
- Paths: 16 pixel checks passed ([CSV](benchmark-smoke/paths.csv),
  [log](benchmark-smoke/paths.log)).
- Points: 16 pixel checks passed ([CSV](benchmark-smoke/points.csv),
  [log](benchmark-smoke/points.log)).
- Negative control: temporarily selecting `fragment_straight` instead of
  `fragment_straight_linear` failed at `texture_prepared count=1`
  ([log](benchmark-smoke/negative-control.log)). The correct entry point was
  restored and the rendering smoke rerun successfully.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo fmt --all -- --check`, and `git diff --check` passed.

This checks runtime compatibility after nested clipping integration on parent
`09db0fe`. It does not establish timing thresholds, exercise active rounded clips,
or verify other GPU backends or reduced device limits. Production clipping code
and its active-clip regression tests are unchanged.
