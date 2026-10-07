# Per-draw, upload and preparation hazards

Measured on 2026-10-08, Apple M3 Pro / Metal, macOS 27.0.1, rustc 1.99.0, bench
profile. Each comparison ran the previous commit's and the new commit's bench
executables (built separately and copied) alternately in three rounds, with no
other Cargo jobs running. Tables give the median of the three per-run medians, in
microseconds. Raw CSVs keep every run's median and p95; [metadata](draw-hazards/metadata.json)
maps each file to its commits and command. GPU execution is not timestamped
(see the [timestamp limitation](text-workloads.md#gpu-timing-limitation)); the two
GPU-side benches use frame start to completion wall time with one submission in flight.

## Dynamic draws: first-instance addressing and merging

Shapes, lines, shadows, paths, point series and text used to bind a fresh byte
range of the upload page for every draw. Records are now stride-aligned, the page
is bound once and draws select records with `first_instance` (commit `5d4b038`).
Consecutive draws with identical state and contiguous records then merge into one
instanced command, flushed before any other command, raw access or pass end
(commit `cf91c23`).

`primitives`, 1,000 draws (`--counts 1000 --samples 30 --warmup 6`):

| Workload | Record before | After 5d4b038 | After cf91c23 | CPU total before | After 5d4b038 | After cf91c23 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `shapes_individual` | 89.1 | 77.0 | 78.8 | 244.6 | 194.2 | 133.0 |
| `shapes_scoped` | 51.7 | 40.7 | 41.2 | 204.1 | 156.5 | 94.2 |
| `lines_scoped` | 55.9 | 46.9 | 47.9 | 210.1 | 163.4 | 100.3 |
| `fills_outlines_scoped` | 52.9 | 43.0 | 44.8 | 198.4 | 147.4 | 95.6 |
| `mixed_scoped` | 322.2 | 279.8 | 322.5 | 1607.3 | 1386.6 | 1569.4 |
| `painter_mixed` | 383.9 | 336.4 | 380.4 | 1708.5 | 1479.7 | 1649.0 |
| `shapes_batch` | 23.3 | 23.3 | 25.5 | 67.3 | 69.7 | 76.8 |

The merge columns come from a separate round of runs against `5d4b038`, so compare
each column with its own baseline: mixed workloads moved by under 2% in that round
(they alternate pipelines and cannot merge), and batches are unchanged within noise.

`paths`, 1,000 placements of one retained path, coverage enabled:

| Route | Record before | After 5d4b038 | After cf91c23 | CPU total before | After 5d4b038 | After cf91c23 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Individual | 63.3 | 53.5 | 54.1 | 210.3 | 156.3 | 91.1 |
| Scoped | 36.7 | 29.4 | 25.8 | 180.3 | 137.8 | 62.5 |
| Batch | 10.3 | 8.7 | 9.1 | 39.6 | 36.5 | 47.3 |

As above, the merge columns have their own baseline round (batch CPU total 53.4 → 47.3 there).
The direct-wgpu individual reference (one ranged bind per draw) stays at about
165 µs CPU total; wrapped individual path draws are now below it. Text keeps one
per-label record bind (its per-draw record has stride zero); labels that share a
`prepare_texts` buffer now also share the geometry binding. `text_workloads` label
recording was unchanged within noise (63.2 → 62.4 µs for 600 labels).

## Path preparation

`paths` preparation cases (`tessellate_and_upload`). Tessellator scratch reuse
(`7bdc045`) produces byte-identical geometry; pooling (`16473d5`) puts vertices and
indices in one buffer from a per-renderer idle pool, reused after GPU completion.

| Case | Before | Scratch reuse | Plus pooling |
| --- | ---: | ---: | ---: |
| Fill, 16 segments | 204.3 | 112.4 | 111.1 |
| Stroke, 16 segments | 1072.8 | 537.8 | 534.0 |
| Fill, 64 segments | 1100.1 | 674.0 | 674.4 |
| Stroke, 64 segments | 6824.2 | 4254.7 | 4287.4 |
| Fill, 256 segments | 9390.0 | 7256.6 | 7311.6 |
| Stroke, 256 segments | 62292.4 | 49721.6 | 50215.5 |
| Icon fill | 47.9 | 36.4 | 30.2 |
| Polyline stroke, 128 | 272.3 | 140.0 | 132.8 |

Pooling rows come from their own round against the scratch build (icon 37.2 →
30.2, polyline 141.0 → 132.8, fill 16 117.7 → 111.1); large geometry is not pooled.
Draw recording with the per-draw lease stayed within noise (scoped CPU total
61.0 → 56.9 µs, individual 86.5 → 87.0 µs, no coverage).

`TextureRenderer::prepare_draws` was reviewed but not changed: it is a one-time
preparation of static instance data, and pooling its buffer would add a lease to
every prepared draw.

## Straight-alpha image filtering

`image_alpha`: 16 full-target layers of a magnified 512×512 RGBA8 image on a
1920×1080 target, frame start to completion (commit `f595232`). Straight-alpha
linear filtering skips the three colour gathers when the footprint's alpha is uniform.

| Image | Straight before | Straight after | Premultiplied |
| --- | ---: | ---: | ---: |
| Opaque | 1828.6 | 1492.0 | 1276.6 |
| Icon (opaque disc, transparent surround) | 1833.3 | 1513.1 | 1273.5 |
| Alpha varies between every texel | 1828.1 | 1998.7 | 1284.2 |

Premultiplied data remains the cheapest. Premultiplying on upload would reach it
for straight sources, but changes what `Texture` stores while bindings declare the
alpha of the bytes they sample.

## Attachment store operations

`store_ops`: 1920×1080, Depth24PlusStencil8, completion time per frame with the
builder defaults (store), explicit store, or discard of samples and depth/stencil.

| MSAA | Workload | Store | Discard |
| ---: | --- | ---: | ---: |
| 4x | Flat (64 translucent rounded rects) | 468.2 | 462.8 |
| 4x | Edges (plus 20,000 small ellipses) | 1659.2 | 1654.5 |
| 1x | Flat | 442.9 | 441.8 |

Discarding saves about 1% on this GPU, so the defaults still store and later
`load_color`/`load_depth` passes keep working without opt-in.

## Upload pool and text rasters

Idle upload pages are released after 120 recordings without use (`8167670`);
recordings take the most recently returned page, so steady frames keep their
pages. `primitives` at 10,000 draws stayed within noise. Writing records directly
into `Queue::write_buffer_with` staging instead of the CPU page copy was tried and
dropped: batches were about 5% faster but individual draws 30–48% slower.

Glyph rasters round to a quarter physical pixel (`59daef2`). The new
`continuous_zoom` case grows the raster scale 0.3% per frame: preparation went from
139.7 to 20.8 µs and cache misses over 40 frames from 880 to 176. See
[text workloads](text-workloads.md) for the table.

## Raw evidence

- First instance: [primitives](draw-hazards/primitives-fi-before-1.csv) (`primitives-fi-{before,after}-{1,2,3}.csv`), [paths](draw-hazards/paths-fi-before-1.csv), [text_workloads](draw-hazards/text_workloads-fi-before-1.csv).
- Draw merging: `primitives-co-*`, `paths-co-*`.
- Path preparation: `paths-scratch-*`, `paths-pool2-*`.
- Images: `image_alpha-fast3-*`. Store operations: `store_ops-{1,2,3}.csv`.
- Upload pool: `primitives-shrink-*`. Raster rounding: `text_workloads-zoom-*`.

All files are in [draw-hazards/](draw-hazards). The suite passes 161 library tests,
10 winit tests and 28 doctests; strict Clippy, formatting and docs pass.
