# astrelis-compositor

`astrelis-compositor` interleaves GPU scene callbacks with ordered UI paint
layers. `RenderView` remains texture-backed by default; its composited content
explicitly requests the direct path. Axis-aligned rectangular views render into
the shared color/MSAA attachment, while rounded, path-clipped, transformed, or
forced-texture views use managed offscreen allocations and the existing image
pipeline.

Run the vertical slice with:

```sh
cargo run -p astrelis-ui-widgets --example scene_views
```

Compare warmed, submitted, GPU-complete frames on the current adapter with:

```sh
cargo bench -p astrelis-compositor --bench frame_paths
```

Results are machine and backend dependent. Direct composition remains opt-in;
the benchmark is evidence for an application-specific optimization decision,
not a universal threshold.
