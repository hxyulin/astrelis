# Scoped drawing experiment

The foundation suite was committed as `0208e39` before this experiment. This change
adds optional scoped bindings to amortize immutable compatibility checks and
pipeline selection. Existing individual calls remain available.

## API

```rust
let mut draws = meshes.bind(&mut pass, &mesh)?;
draws.draw();
draws.pass().set_scissor_rect(x, y, width, height)?;
draws.draw_range(&mesh.full_draw().instances(0..instance_count))?;
```

`MeshRenderer::bind_with_material` handles custom materials. A mesh scope checks
device, layouts, topology, and its default full range at binding. Selected ranges
still get bounds validation. The scope borrows the mesh/pass and retains a pipeline
handle; it does not keep the renderer borrowed. The same renderer can draw through
`draws.pass()` and the next scoped draw restores its original pipeline/geometry.

For changing rectangles, `textures.bind(&mut pass, &image)?` returns a scope with
`draw(draw)` and `draw_many(draws)`. Immutable image/material compatibility and
pipeline selection are checked once; each rectangle still gets parameter validation
and uses recording-owned uploads. This scope borrows renderer scratch storage to
reuse batch capacity.

For immutable data:

```rust
let mut draws = textures.bind_prepared(&mut pass, &image, &prepared)?;
draws.draw()?;
```

The prepared scope retains pipeline/image handles and borrows the pass/data without
keeping the renderer borrowed. Custom materials use `bind_prepared_with_material`.
A dynamic image scope can also lend a prepared child scope with `bind_prepared`.
Dropping a scope leaves the pass open and does not submit or change drawing order.

Pass access marks a scope dirty. Its next draw restores renderer-owned state and
wrapped raster settings. Prepared pixel-space data is rechecked after pass access;
a mismatched viewport returns an error before any draw, and the scope stays usable
if the viewport is restored. Application-owned groups remain caller-controlled.
Live framebuffer images are snapshots for a scope's duration; begin another scope
to follow storage replacement. These boundaries make the amortized checks explicit.

## Measurement

Same Apple M3 Pro / Metal, macOS 27.0.1, rustc 1.98.1, wgpu 30.0.1, optimized build,
16×16 RGBA8 destination, default wgpu validation, and completion between samples
as the [initial baseline](baseline.md). Scope construction and binding are **inside**
the recording timer. No GPU commands are merged or removed by these scopes.

Three complete runs contain all five original workloads plus four scoped variants.
Each uses 8 warm-up pairs and 40 measured pairs per workload/count; each count is
100, 1,000, or 10,000. Raw/wrapped order alternates. All 27 workload/count pixel
comparisons passed in every run.

Evidence: [run 1](scoped-metal-m3-pro-run-1.csv), [run 2](scoped-metal-m3-pro-run-2.csv),
[run 3](scoped-metal-m3-pro-run-3.csv), and [metadata/source hashes](scoped-metal-m3-pro-metadata.txt).
The original and scoped measurements below come from these same new runs, rather
than comparing against an older run's clock/thermal state. This controlled change
measures the combined benefit of amortization, not an attribution of cost to each
individual check or lookup. No sampling-profiler attribution is claimed.

## CPU recording at 1,000 draws/rectangles

Medians of the three run medians, in microseconds. Each scoped raw reference matches
its GPU commands. Dynamic paths include CPU parameter packing.

| Workload | Individual/convenience API | Scoped API | Scoped direct-wgpu reference | Recording speedup |
| --- | ---: | ---: | ---: | ---: |
| `mesh_scoped` | 34.25 | 4.67 | 4.50 | 7.34× |
| `texture_prepared_scoped` | 45.92 | 5.29 | 4.21 | 8.68× |
| `texture_dynamic_scoped` | 59.12 | 17.00 | 5.92 | 3.48× |
| `texture_batch_scoped` | 7.17 | 7.17 | 2.29 | 1.00× |

Batching already amortizes pipeline selection and image checks over the slice;
adding a scope around one batch provides essentially no recording benefit. Retain
`draw_many` as the straightforward route for compatible rectangles. Individual
dynamic calls benefit from scopes, but per-rectangle validation and uploads remain.

## Prepared paths against raw recording

Ranges of each run's reported median-overhead percentage. Negative differences
within these small timings should be treated as measurement variability, not a
claim that wrapping accelerates an equivalent raw command loop.

| Prepared scoped workload | Count | Raw recording range (µs) | Scoped recording range (µs) | Overhead range |
| --- | ---: | ---: | ---: | ---: |
| `mesh_scoped` | 1,000 | 4.33–5.54 | 4.67–6.12 | 3.69–10.52% |
| `mesh_scoped` | 10,000 | 55.62–57.54 | 56.67–57.67 | -0.51–3.67% |
| `texture_prepared_scoped` | 1,000 | 4.12–4.83 | 5.00–5.33 | 10.35–25.76% |
| `texture_prepared_scoped` | 10,000 | 56.25–58.08 | 50.29–60.38 | -13.41–7.33% |

Mesh scopes fall within the approximate 10–15% target at these counts. Prepared
texture scopes approach raw cost at 10,000 draws, but do not consistently meet
15% at 1,000. The absolute prepared-texture recording difference at 1,000 is around
one microsecond in the run medians. Keep the outliers visible and avoid declaring
the percentage gate universally passed. The CSVs also retain the 100-draw setup-
dominated case and p95 timings.

## CPU costs including uploads/submission

Medians of the three run medians, in microseconds. These totals exclude GPU completion
waits and presentation. They are summarized from per-sample totals, not sums of
separate phase medians.

| Workload | Convenience, 1,000 | Scoped, 1,000 | Scoped raw, 1,000 | Convenience, 10,000 | Scoped, 10,000 |
| --- | ---: | ---: | ---: | ---: | ---: |
| `mesh_scoped` | 96.00 | 65.58 | 65.04 | 850.04 | 544.08 |
| `texture_prepared_scoped` | 128.46 | 89.71 | 88.67 | 1154.29 | 772.92 |
| `texture_dynamic_scoped` | 156.12 | 113.12 | 101.50 | 1405.25 | 993.08 |
| `texture_batch_scoped` | 34.42 | 33.37 | 27.92 | 210.71 | 213.42 |

## Validation and remaining work

- 35 unit/GPU tests and 11 doctests passed, including scoped tests for raw/mixed
  renderer restoration, same-renderer reuse, invalid batch rejection, viewport
  revalidation, live-source snapshots, custom layouts/Uint16/nonindexed geometry,
  instancing, application bindings, MSAA/depth, and foreign/read-only/feedback errors.
- Clippy with warnings denied, formatting, documentation generation, and native
  one-off acceptance copies passed. Each UI/3D native copy presented eight frames,
  resized to 640×480, toggled MSAA twice, and suspended/restored a zero-sized target.
- The texture, geometry, and material examples demonstrate prepared/scoped drawing
  and caller-owned pass bindings. Unchanged standalone copies compiled; one-off
  native copies each presented six frames, resized to 640×480, and suspended/restored
  a zero-sized target. Test instrumentation remains outside the repository.
- Existing upload/pipeline reuse and independent-recording tests continue to pass.
- Existing individual calls still have their convenience-path overhead. Scopes
  optimize fixed-resource sequences; they do not make varying data free or reduce
  GPU draw counts automatically.
- GPU execution timing, frames-in-flight throughput, peak-memory profiling, and
  Vulkan/DX12 measurements remain outside this evidence.

This is a useful fast path with explicit API boundaries. Further optimization
should focus on real varying-data workloads and measurement noise/setup at small
counts, without dropping bounds/parameter checks or silently weakening the gate.
