# Path preparation and drawing evidence

Three local Metal runs on Apple M3 Pro, with 6 warm-up pairs and 30 measured pairs
per case. Tables report the median of three run medians, in microseconds.
Raw/wrapped order alternates within each pair. Both coverage modes and counts
100/1,000/10,000 pass full exact-pixel comparisons in each run: 72 comparisons total.

The drawing workload is one retained 8×8 quad path at ordered affine placements
on a 64×64 linear RGBA8 single-sampled target. The reference uses direct wgpu
drawing with the same WGSL, vertex/instance layouts, 2 interior triangles, 8 fringe
triangles when coverage is enabled, premultiplied blending, viewport/scissor,
attachment operations, and reusable 64 KiB parameter pages. Individual/scoped
cases each issue N draws; batch cases issue ceil(N/1,024) draws on this device.
Both sides upload 64 bytes per placement.

**The raw reference uses the same Astrelis frame/pass scaffolding.** This isolates
drawing/parameter upload overhead; it is not a whole-library comparison against
an entirely unwrapped frame implementation. `record` includes parameter packing
and draw recording (plus wrapped validation/pipeline selection), excluding pass
creation. `cpu_total` includes acquisition/pass setup, recording, uploads, finishing
and managed submission. Completion waits and readback are outside timing. GPU
execution, presentation, throughput with frames in flight, and other backends are
not measured.

## Warm drawing, coverage enabled, 1,000 placements

| Route | Direct-wgpu record | Astrelis record | Record difference | Direct reference CPU total | Astrelis CPU total | GPU draws |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Individual | 16.58 | 59.94 | +43.36 | 157.02 | 202.21 | 1,000 |
| Scoped | 16.52 | 33.63 | +17.10 | 159.75 | 174.06 | 1,000 |
| Batch | 2.29 | 10.00 | +7.71 | 28.38 | 37.29 | 1 |
| Painter batch | 2.40 | 8.58 | +6.19 | 30.60 | 36.42 | 1 |

The strict approximately 10–15% additional CPU recording target is **not met**
by these dynamic path descriptors. At 1,000 placements the measured batch
recording difference is about 7.7 microseconds, but its relative cost is substantially
larger than that target. Scoped drawing amortizes pipeline selection while retaining
per-placement validation/uploads and GPU draw count. Explicit batching is the main
reduction in CPU work and GPU command count. Do not use the smaller CPU-total
percentage to claim that the recording gate passed. Differences between Painter
and direct batching are not evidence of a systematic Painter speedup: cases are
paired against their own references and these timings vary.

## Explicit preparation

Preparation includes synchronous tessellation, boundary processing, exact-sized
GPU buffer allocation, and initial upload encoding/copy. Pipeline creation, GPU
execution/completion, and dropping the result are excluded. There is no path
geometry cache: each measured preparation creates a new resource while tessellator
scratch is warmed. Tolerance is 0.1 path units. Wave stress cases use tightly spaced,
alternating high-curvature cubic segments; strokes use width 3, round joins/caps,
and union tessellation. These are substantially more demanding than the icon and
ordinary polyline cases.

| Preparation | Segments | Median CPU microseconds |
| --- | ---: | ---: |
| Heart icon fill | 4 | 46.00 |
| Ordinary stroked polyline | 128 | 255.25 |
| Cubic wave fill | 16 | 209.60 |
| Cubic wave stroke | 16 | 1,080.98 |
| Cubic wave fill | 64 | 1,054.63 |
| Cubic wave stroke | 64 | 6,629.90 |
| Cubic wave fill | 256 | 8,988.17 |
| Cubic wave stroke | 256 | 58,913.31 |

The 256-curve stroked stress path retains 32,426 vertices, 136,770 indices, and
1,584,712 bytes. Canceling shared triangle edges before union tessellation avoids
feeding internal diagonals into the fill intersection algorithm. Large complex
preparation remains too expensive for an ordinary redraw budget; retain geometry
and schedule preparation deliberately. Background CPU preparation remains future work.

## Structural and consumer checks

GPU regression tests verify fixed vertex/index handles, pipeline identity, reused
parameter-page handles, and CPU scratch capacity over warmed frames. They check
exactly 64 uploaded parameter bytes per placement, whole-batch validation, page
splitting, and a 1,000-byte application device buffer limit. These are targeted
checks, rather than a global allocation profiler. Dropping the last prepared owner
before submission still produces correct pixels through wgpu-managed immutable
buffer lifetimes.

The full suite passes 110 unit/GPU tests and 21 doctests. Strict Clippy across all
targets, formatting, native documentation, and the wasm32 library compile pass.
A temporary copy of the standalone `paths` example presented eight native frames,
resized 840×840 → 640×480 → 840×840, switched 1× → 4× MSAA, toggled coverage, and
suspended/restored a zero-sized target. A headless gallery readback was visually
inspected. Test instrumentation remains outside examples.

Coverage is an approximate centered screen-space band. Subpixel features, narrow
gaps, acute corners, and touching/intersecting bands remain quality limitations.
None plus target MSAA selects geometric sample coverage. These CPU measurements
do not establish quality or GPU performance for arbitrary large paths.

Artifacts: [run 1 CSV](paths-metal-m3-pro-run-1.csv), [run 1 log](paths-metal-m3-pro-run-1.log),
[run 2 CSV](paths-metal-m3-pro-run-2.csv), [run 2 log](paths-metal-m3-pro-run-2.log),
[run 3 CSV](paths-metal-m3-pro-run-3.csv), [run 3 log](paths-metal-m3-pro-run-3.log),
and [source/artifact metadata](paths-metal-m3-pro-metadata.json).
