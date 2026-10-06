# Brush CPU recording evidence

Three local Metal runs on Apple M3 Pro, each with six warm-up pairs and thirty
measured pairs per case. Reference/brush order alternates within pairs. Tables
report the median of three run medians, in microseconds. The benchmark's per-run
median uses the upper middle value of thirty sorted samples.

The reference is **Astrelis color-only drawing**, not direct wgpu. Both routes use
the same Astrelis frame/pass scaffolding, PipelineOptions, linear RGBA8 single-sampled
64×64 target, ordered affine placements, and hard-edge quad geometry. Paths use
retained 8×8 prepared quads; analytic shapes and lines cover equivalent rectangles.
Colors are `[0.5, 0.8, 1, 0.25]`. Brush stops are constant white so outputs can be
compared exactly while the runtime stop-buffer search/interpolation remains present.
Each run passes all 45 full exact-pixel comparisons, 135 total. Colored gradients,
hard stops, transparency, and coordinate correctness have separate GPU pixel tests.

`record` covers batch validation, packing, pipeline/binding selection, and GPU draw
recording, excluding acquisition/pass creation. `cpu_total` also includes acquisition,
pass setup, parameter uploads, finishing, and managed submission. Completion waits
and readback are outside timing. Pipelines and retained geometry/brushes are prepared
before timing; scratch/upload pages are warmed. No GPU execution, presentation,
throughput with frames in flight, or other backends are measured.

## Eight-stop linear gradient, 1,000 placements

| Renderer | Color record | Brush record | Difference | Color CPU total | Brush CPU total | Uploaded bytes: color → brush | Draws: color → brush |
| --- | ---: | ---: | ---: | ---: | ---: | --- | --- |
| Path | 10.67 | 14.79 | +4.12 | 39.46 | 42.25 | 64,000 → 64,000 | 1 → 1 |
| Shape | 18.83 | 27.92 | +9.08 | 54.58 | 66.21 | 80,000 → 112,000 | 2 → 2 |
| Line | 22.96 | 30.38 | +7.42 | 57.75 | 65.38 | 80,000 → 112,000 | 2 → 2 |

At this count, brush recording adds about 4–9 microseconds over the existing color
route. The relative increase remains meaningful. This is feature-cost evidence,
not proof that Astrelis meets the foundation's direct-wgpu 10–15% overhead gate.
The path-stage report still records that gate as unmet for its measured workload.

## Eight-stop linear gradient, 10,000 placements

| Renderer | Color record | Brush record | Color upload bytes | Brush upload bytes | Color draws | Brush draws |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Path | 98.92 | 128.46 | 640,000 | 640,000 | 10 | 10 |
| Shape | 185.17 | 271.79 | 800,000 | 1,120,000 | 13 | 18 |
| Line | 204.54 | 302.42 | 800,000 | 1,120,000 | 13 | 18 |

Brushed shapes/lines upload 112 bytes instead of 80 per placement. With 64 KiB
parameter pages that gives 585 instead of 819 instances per draw. The benchmark
therefore reports the extra primitive draw commands explicitly; it does not claim
matched GPU work between color-only and gradient drawing. Path brush records stay
64 bytes, preserving their 1,024-instance page capacity and draw count.

## Stop count and preparation

The full CSVs include solid, 2/8/64-stop linear, and eight-stop radial brushes at
100/1,000/10,000 placements, separately for each renderer. CPU drawing checks cached
color/coordinate bounds; it does not scan stops. Fragment binary search grows with
stop count; radial sampling adds a distance calculation. These CPU timings cannot
establish the GPU cost of those features or of a large, heavily overdrawn UI.

Brush creation retains 48 uniform bytes and 32 bytes per stop, plus bind-group/driver
metadata. It allocates/uploads once and performs no pipeline preparation. Creation
is not timed here. Changing immutable stops or mapping requires a replacement brush;
ordinary unchanged-frame workloads retain resources. Existing color-only rendering
creates no brush shader or binding until brush preparation/drawing is requested.

## Consumer and structural validation

The complete suite passes 119 unit/GPU tests and 23 doctests. Strict Clippy across
all targets, formatting, native documentation, and wasm32 library compilation pass.
Targeted tests check unchanged brush buffer/bind-group handles, reused upload pages,
64 uploaded path parameter bytes per placement, cross-renderer/Painter equivalence,
whole-batch rejection, owner drop before submission, and a 512-byte imported-device
buffer limit. A device with zero permitted storage buffers rejects brush creation
and brush preparation while retaining color-only rendering support.

The existing direct-wgpu path benchmark was rerun at 1,000 placements, with both
coverage modes and individual/scoped/batch/Painter routes. All eight full exact-pixel
comparisons pass after factoring common vertex functions for brush shading. This
regression run is separate from the three-run brush tables above.

A temporary instrumented copy of the standalone `brushes` example presented eight
native frames, resized 840×840 → 640×480 → 840×840, switched 1× → 4× MSAA, toggled
coverage, and suspended/restored a zero-sized target. A 384×384 headless gallery
readback was visually inspected. Instrumentation remains outside the examples.

Approximate geometry edge coverage, extreme-coordinate precision, unfiltered hard
stop transitions, and minified repeating gradients remain quality limits. This stage
is not a general filtered pattern renderer, and no universal GPU/frame-time claim
follows from the measurements above.

Artifacts: [run 1 CSV](brushes-metal-m3-pro-run-1.csv), [run 1 log](brushes-metal-m3-pro-run-1.log),
[run 2 CSV](brushes-metal-m3-pro-run-2.csv), [run 2 log](brushes-metal-m3-pro-run-2.log),
[run 3 CSV](brushes-metal-m3-pro-run-3.csv), [run 3 log](brushes-metal-m3-pro-run-3.log),
and [source/artifact metadata](brushes-metal-m3-pro-metadata.json).

Color-only regression: [path CSV](brushes-path-regression.csv),
[path log](brushes-path-regression.log).
