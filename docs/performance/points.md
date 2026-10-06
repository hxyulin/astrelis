# Dense-point measurements

Three local release runs on Apple M3 Pro/Metal, with four warm-up pairs and fifteen
measured pairs per case. Raw/Astrelis order alternates within pairs. Values below
are medians of three run medians. All 24 full exact-pixel comparisons pass in each
run, 72 total. Every case keeps the same PointBuffer GPU handle.

## Matched rendering

Both routes use the same retained storage, WGSL shader, transform, geometry, blend
policy, 80-byte parameter record and one draw. Raw recording directly binds a wgpu
pipeline/group/vertex buffer and records the draw; Astrelis adds compatibility,
range/style/bounds validation and recording-owned parameter packing. Both retain
Astrelis frame/pass scaffolding. The raw route also uses PointBuffer updates, so
update timings compare identical update APIs, not PointBuffer against unchecked
raw buffer writes. Uploads occur after recording for both routes. Pipelines and
parameter pages are prepared/warmed outside measured samples.

The target is 1920×1080 linear RGBA8 at 1× MSAA. Samples follow a dense oscillating
wave, with no gaps in timed GPU cases. Lines use 1.5-pixel miter/butt coverage strokes
(nine vertices per segment); markers use 1.5-pixel coverage circles (six per sample).
This measures each renderer separately, not a composed chart, scatter distribution,
round-join/cap workload, or heavily layered translucent UI.

`record_us` includes transform/style packing and draw recording, excluding frame
acquisition and pass setup. `cpu_total_us` starts before any update and ends after
finish/submit; it also includes frame/pass setup and actual parameter uploads.
`completed_frame_us` measures that same start through a submission-specific GPU
completion wait. It is **synchronized CPU wall latency**, not isolated GPU execution,
presentation latency, or steady-state throughput with frames in flight.

| Renderer | Retained | Drawn | Mode | Raw record µs | Astrelis record µs | Raw CPU total µs | Astrelis CPU total µs | Astrelis completed ms |
| --- | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: |
| polyline | 10,000 | 10,000 | unchanged | 0.25 | 0.50 | 29.04 | 29.38 | 0.444 |
| polyline | 100,000 | 100,000 | unchanged | 0.25 | 0.54 | 37.42 | 37.42 | 0.948 |
| polyline | 1,000,000 | 1,000,000 | unchanged | 0.42 | 1.38 | 47.04 | 50.29 | 6.270 |
| markers | 10,000 | 10,000 | unchanged | 0.21 | 0.46 | 29.00 | 29.17 | 0.389 |
| markers | 100,000 | 100,000 | unchanged | 0.38 | 0.75 | 41.33 | 43.58 | 1.265 |
| markers | 1,000,000 | 1,000,000 | unchanged | 1.08 | 1.79 | 57.12 | 54.75 | 9.864 |
| polyline | 1,000,000 | 4,096 | visible_4096 | 0.29 | 0.50 | 29.75 | 30.00 | 0.380 |
| markers | 1,000,000 | 4,096 | visible_4096 | 0.29 | 0.46 | 30.00 | 31.71 | 0.381 |

Record cost remains small rather than scaling with retained samples. Differences
in CPU totals/completion include run noise and are not acceleration claims. The
wrapper still adds validation/packing over raw recording; these measurements do not
establish a universal 10–15% relative-overhead gate. GPU work remains proportional
to selected samples. Reducing to 4,096 visible samples avoids most of the raw dense
work even when one million samples remain retained; this case is **range selection,
not data reduction**, and draws a different signal extent.

## Updates and retention

| Renderer | Retained | Append 256 µs | Append bytes | Replace all µs | Replace bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| polyline | 10,000 | 15.96 | 2,048 | 31.75 | 80,000 |
| polyline | 100,000 | 19.62 | 2,048 | 185.42 | 800,000 |
| polyline | 1,000,000 | 24.54 | 2,048 | 1718.75 | 8,000,000 |
| markers | 10,000 | 14.71 | 2,048 | 30.17 | 80,000 |
| markers | 100,000 | 20.38 | 2,048 | 185.50 | 800,000 |
| markers | 1,000,000 | 25.08 | 2,048 | 1759.79 | 8,000,000 |

Append includes generation of 256 incoming wave points, validation/bounds updates,
and queue writes. Replacement uploads prebuilt unchanged source data, including
validation and queue-copy cost, but excluding source generation. No existing
samples move during ring append. One million retained samples occupy 8,000,000 GPU
sample bytes; unchanged/view-only frames upload zero sample bytes, while each drawn
series uploads 80 parameter bytes. Replace-all remains O(N) and is deliberately
reported rather than presented as a cheap streaming operation.

## Application-side reduction

A separate one-off CPU harness uses History extracted from the standalone example:
32-sample block summaries, 1,920 visible buckets, and first/minimum/maximum/last
selection in chronological order. It performs four warm-ups and thirty measured
updates/queries per count, repeated three times. The harness is compiled with
`rustc -O` against the release Astrelis library. It retains vector capacity and
includes simulated source generation/CPU ring writes in append timing. These
measurements exclude GPU updates, rendering, text, grid lines, window work, and
startup hierarchy construction.

| Original history | Columns | Append/summaries µs | Reduction µs | Median output points |
| --- | ---: | ---: | ---: | ---: |
| 10,000 | 1,920 | 5.54 | 78.50 | 4,000 |
| 100,000 | 1,920 | 5.83 | 525.38 | 5,482 |
| 1,000,000 | 1,920 | 6.04 | 592.71 | 17,244 |

The source retains periodic narrow spikes and explicit gaps. Gap buckets keep
original samples; output therefore exceeds four samples per bucket at one million
points. There is no universal output-size bound proportional only to screen width:
dense gaps can approach the original count. Output-size values are the median of
thirty queries as the ring shifts. Unchanged paused example frames reuse reduced
GPU data instead of querying/reuploading it. For irregular X coordinates the
application must choose a different bucket policy.

An initial local pilot with 256-sample blocks and a non-inlined cross-crate sample
accessor took about 3.95 ms for the one-million-point query. Smaller blocks and the
inlined accessor reduce that combined cost to about 0.59 ms here. This comparison
changes both factors and does not isolate their individual contributions. Summaries
trade memory for query cost: approximately 2.5 MiB at one million samples, additional
to application-owned original data and scratch/output vectors.

## Timestamp limitation

The adapter advertises TIMESTAMP_QUERY, but calibration returned a nonzero beginning
and a zero end timestamp on this macOS 27.0.1/wgpu 30.0.1 system. The benchmark rejects
zero/reversed/unreasonable counters and disables queries before measured samples;
`gpu_pass_us` is blank. Invalid values are never wrapped into a duration or claimed
as GPU evidence. The user's [wgpu issue #9414](https://github.com/gfx-rs/wgpu/issues/9414)
remains open as checked on 2026-10-06. The current symptom is invalid render-pass
counters; this investigation has not established the exact backend/driver cause.
Completion waits remain useful wall-latency evidence, without solving pass profiling.

## Correctness and consumers

The complete suite passes 126 unit/GPU tests and 25 doctests. Strict Clippy across
all targets, native Rustdoc with warnings denied, formatting/diff checks, and
wasm32 library compilation pass. Seven new unit/GPU tests cover update atomicity and limits, wrapped logical writes,
evictions, gaps, cap/join pixel geometry, ordinary translucent joins, marker shapes,
fixed screen sizes under zoom/reflection, clipping, MSAA, normalized placement,
Painter/scoped/raw interleaving, live queue-write semantics, owner drop before submit,
stencil masks/read-only aspects, and a low-limit imported device. Repeated draws
retain point buffer/group/pipeline handles and upload pages; two series upload exactly
160 parameter bytes and zero unchanged sample bytes. Ordinary Painter preparation
remains usable when vertex-storage point rendering is unavailable.

Two one-off CPU tests compare summary queries against direct original-data scans,
check reduced chronology/extrema/gap placement across ring wrap at all five tested
capacities (257/513/10K/100K/1M), and verify identity at one bucket per point and empty
ranges. A temporary instrumented copy of streaming_chart presents eight native frames
through 1280×720 → 960×540 → 1280×720, 1×/4× MSAA, one-million-sample history,
raw/reduced rendering, markers, coverage changes and paused zoomed ranges. A headless
1280×720 chart/marker image is read back at 1×/4× MSAA and visually inspected.
Instrumentation remains outside repository examples.

Artifacts: [run 1](points-metal-m3-pro-run-1.csv), [run 2](points-metal-m3-pro-run-2.csv),
[run 3](points-metal-m3-pro-run-3.csv), corresponding logs, [reduction 1](points-reduction-run-1.csv),
[reduction 2](points-reduction-run-2.csv), [reduction 3](points-reduction-run-3.csv),
and [source/artifact metadata](points-metal-m3-pro-metadata.json).

Reproduce GPU/CPU frame measurements with:

```sh
cargo bench -p astrelis --bench points -- --samples 15
```

For the one-off reduction harness, extract the example's `const BLOCK` through the
end of `impl History`, prepend `use astrelis::Point2D;`, initialize full History and
retained input/output vectors, and repeatedly time `generate(256, input)` then
`reduce(0..history.len, 1920, output)`. The exact temporary harness and extraction
hashes are recorded in the metadata; timings above retain the specified 4/30 sampling
protocol rather than the benchmark's 4/15 paired GPU protocol.
