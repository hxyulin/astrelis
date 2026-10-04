# Foundation baseline: Apple M3 Pro / Metal

These measurements establish a repeatable initial baseline. **The proposed
10–15% recording-overhead target is not met.** The consumer examples work through
the public API, but the performance gate remains open.

## Environment and method

- Apple M3 Pro integrated GPU, Metal backend, macOS 27.0.1 (26A434), aarch64.
- rustc 1.98.1; Astrelis 0.4.0-dev.0; wgpu 30.0.1; optimized Cargo bench profile.
- Base commit `55cfad2`; the rendering library is unchanged by this milestone.
  The benchmark source hash and lockfile hash are recorded in the metadata.
- Default wgpu validation configuration, one shared device, 16×16 RGBA8 output,
  single sample, matching shaders, geometry, blend and attachment operations.
- Three complete suite runs, each with 8 warm-up pairs and 40 measured pairs per
  workload/count. Order alternates raw/wrapped, and every submission completes
  before the next sample. Builds and native example checks completed before these
  three recorded runs.
- All 15 workload/count output equivalence checks passed in every run.
- CPU timers only. Completion waits are not GPU execution measurements.

Command:

```sh
cargo bench -p astrelis --bench rendering --offline -- \
  --counts 100,1000,10000 --samples 40 --warmup 8 \
  > rendering.csv 2> rendering.log
```

Raw evidence: [run 1](metal-m3-pro-run-1.csv), [run 2](metal-m3-pro-run-2.csv),
[run 3](metal-m3-pro-run-3.csv), and [environment/settings/startup observations](metal-m3-pro-metadata.txt).
The CSV files retain median/p95 for all phases and counts; the tables below do not
replace those distributions.

## Recording at 1,000 draws/rectangles

Values are the range of the three run medians, in microseconds. Dynamic paths
include CPU parameter packing. Prepared texture drawing records 1,000 individual
draw calls; batched texture drawing records one instanced draw at this count.
Raw and Astrelis command counts match within each workload.

| Workload | Direct wgpu recording | Astrelis recording | Median additional CPU cost |
| --- | ---: | ---: | ---: |
| `mesh_same` | 4.38–5.04 | 34.04–36.96 | 30.04 µs |
| `mesh_alternating` | 18.38–20.83 | 54.67–57.42 | 35.21 µs |
| `texture_prepared` | 4.75–4.83 | 46.75–46.88 | 42.00 µs |
| `texture_dynamic` | 5.96–7.75 | 58.25–60.62 | 53.67 µs |
| `texture_batch` | 2.29–2.38 | 7.04–8.92 | 4.87 µs |

The optimized raw prepared loops have very low recording cost, making relative
overhead large even when the absolute difference is tens of microseconds. That
explains the ratios; it does not satisfy the agreed percentage target. Public
validation, compatibility checks, pipeline lookup, state tracking, and handle
management are candidates to profile. These measurements do not attribute a
specific fraction of cost to any one operation.

## CPU cost including begin, uploads, teardown, and submission

Values below are medians of the three run medians, in microseconds. `cpu_total`
is calculated for each sample before summarizing; it is not a sum of separately
summarized phase medians. It excludes presentation and completion waits.

| Workload | Direct wgpu, 1,000 | Astrelis, 1,000 | Direct wgpu, 10,000 | Astrelis, 10,000 |
| --- | ---: | ---: | ---: | ---: |
| `mesh_same` | 66.67 | 96.46 | 552.04 | 862.17 |
| `mesh_alternating` | 158.79 | 195.71 | 1494.83 | 1880.58 |
| `texture_prepared` | 87.96 | 129.04 | 759.75 | 1161.33 |
| `texture_dynamic` | 101.04 | 154.42 | 864.92 | 1395.08 |
| `texture_batch` | 29.21 | 35.33 | 161.54 | 207.33 |

Batching reduces both command count and repeated wrapper work. The individual
and batch texture paths render the same input list and upload the same byte count,
but they intentionally use different draw counts. Compare each with its matched
raw reference before judging abstraction overhead.

Across these runs the individual recording paths scale approximately linearly
from 1,000 to 10,000 calls. This is a narrow synthetic workload; it establishes
neither real-scene FPS nor broad GPU performance. It also does not exercise changing
materials, multiple frames in flight, texture upload bandwidth, or peak-memory recovery.

## Acceptance assessment

- **API workflows:** both standalone acceptance examples run through the public
  API, retaining resources while changing draw data and application bindings.
- **Correctness:** matching raw/wrapped benchmark pixels, unit/GPU tests, and
  borrowing doctests pass. Native one-off copies presented eight frames each,
  verified actual 640×480 target resize, switched MSAA twice, and exercised
  zero-size suspension/restoration. Their rendered snapshots were inspected.
- **Steady-state reuse:** the regression test retains the same three upload GPU
  buffers, CPU page capacities, parameter scratch capacity, and prepared pipeline
  through submission, abandonment, and resize. This is targeted evidence rather
  than a comprehensive allocation/memory profile.
- **Recording overhead:** **not passed** against the initial 10–15% target.
- **GPU execution, memory peaks, and other backends:** not established by this suite.

The next performance work should profile and amortize per-draw checks and pipeline
selection, while retaining the current convenience entry points and error behavior.
Any proposed prepared recording or batch API should be judged by these same
workloads, plus mixed-renderer/raw-state correctness checks. Repeat on Vulkan or
DX12 hardware before generalizing backend results.
