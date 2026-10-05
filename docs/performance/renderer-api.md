# Renderer API refactor: CPU check

Compared base commit `c62ee32` with the uncommitted renderer API refactor on
Apple M3 Pro / Metal, macOS 27.0.1, Rust 1.98.1. The benchmark source and font
fixture are identical. [Metadata](renderer-api/metadata.json) records baseline/current
source hashes, distinct executable hashes, commands, conditions and artifact hashes.

The benchmark's 60/600/1,000 labels each have nine drawable glyphs and fit the
1920×1080 RGBA8 1xMSAA target. Every label changes each iteration. Fonts/glyph
vocabulary are warmed. Both modes retain their previous resources until replacements
are successfully prepared. Each process uses 40 measured samples and eight warmups.
This comparison selects the batched mode; CSVs retain the individual mode too.

Baseline and current executables were built separately and copied before measurement.
A shared Cargo target can reuse artifacts across checkout paths, so Astrelis release
artifacts were cleaned before the current rebuild. Build logs verify the source roots;
different binary hashes verify separate executables. Three rounds run baseline then
current with no other Cargo jobs or example windows active during timed workloads.
Earlier current-only sequential observations are included separately; they motivated
the alternating comparison and are not used to calculate the change below.

Times are microseconds, using the median of the three per-run medians. Raw CSVs
preserve each run's medians and p95. Negative change means lower CPU time.

| Labels | Stage | Before (µs) | After (µs) | Change |
| ---: | --- | ---: | ---: | ---: |
| 60 | Update/layout/prepare/replace | 234.67 | 236.15 | +0.6% |
| 60 | Geometry preparation | 57.98 | 58.25 | +0.5% |
| 60 | Recording | 7.62 | 7.08 | -7.1% |
| 60 | CPU frame | 47.04 | 46.79 | -0.5% |
| 600 | Update/layout/prepare/replace | 2180.79 | 2199.27 | +0.8% |
| 600 | Geometry preparation | 443.42 | 447.92 | +1.0% |
| 600 | Recording | 53.63 | 48.27 | -10.0% |
| 600 | CPU frame | 186.71 | 178.15 | -4.6% |
| 1,000 | Update/layout/prepare/replace | 3700.62 | 3755.00 | +1.5% |
| 1,000 | Geometry preparation | 731.98 | 759.04 | +3.7% |
| 1,000 | Recording | 89.71 | 80.60 | -10.1% |
| 1,000 | CPU frame | 285.65 | 275.06 | -3.7% |

One baseline process reports a 4.50 ms median for the 600-label update, with elevated
layout, recording and CPU-frame timings. It remains in the raw data; no samples or
runs were discarded. The three-run median limits that outlier's influence. These
remain local measurements, with fixed before/after order within each round, rather
than a cross-device or statistically conclusive performance claim.

Recording is about 10% lower for 600/1,000 labels, consistent with removing the
per-draw color-format-vector clone. Overall update cost is about 1% higher. Geometry
preparation has a small increase; the coordinate/density refactor is not claimed to
be zero-cost. The full update still includes unchanged shaping/layout work.

Batched geometry upload counts remain 1/4/7, with zero new geometry-buffer allocations
in warmed iterations and no glyph misses/atlas uploads during measured preparation.
Draw counts and 48-byte per-text parameter payloads remain unchanged. Readbacks verify
individual/batched output identity and visible first/last labels. See the original
[batching report](text-batches.md) for workload and ownership boundaries.

CPU frame time includes acquisition, pass creation, recording, upload, teardown and
submission. Completion waits/readbacks are outside timed intervals. GPU execution,
presentation latency and stencil shader cost are not measured here. The new prepared
image transform path is covered by pixel tests rather than this text benchmark:
identity drawing uploads no parameters, nonidentity drawing uploads 48 bytes per call,
and neither reuploads retained instance geometry.

## Raw evidence

- Paired baseline: [run 1](renderer-api/paired-before-1.csv), [run 2](renderer-api/paired-before-2.csv), [run 3](renderer-api/paired-before-3.csv).
- Paired current: [run 1](renderer-api/paired-after-1.csv), [run 2](renderer-api/paired-after-2.csv), [run 3](renderer-api/paired-after-3.csv).
- Earlier current-only observations: [run 1](renderer-api/text-batches-run-1.csv), [run 2](renderer-api/text-batches-run-2.csv), [run 3](renderer-api/text-batches-run-3.csv).
- Builds: [baseline](renderer-api/baseline-build.log), [current](renderer-api/current-build.log).

All 99 tests and 19 doctests pass, including new GPU readback checks for logical text
scaling, transformed retained images, stencil clipping/reference changes across all
built-in renderers, and visible-coverage stencil writes/read-only rejection. Strict
Clippy, formatting, docs, all targets and wasm32 library compilation pass. Updated
standalone Painter, Painter text and stencil examples launch successfully.
