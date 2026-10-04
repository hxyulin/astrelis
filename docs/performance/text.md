# CPU text baseline

Measured on 2026-10-05, Apple M3 Pro, macOS 27.0.1, Rust 1.98.1, release build.
Three sequential runs followed completed verification, with no other agent cargo
jobs. Each stage uses eight warmups and forty samples. Raw measurements are in
[run 1](text-m3-pro-run-1.csv), [run 2](text-m3-pro-run-2.csv), and
[run 3](text-m3-pro-run-3.csv). [Metadata](text-m3-pro-metadata.json) records the
compiler, OS, base commit, source hashes, command, and pinned font provenance.

```sh
cargo bench -p astrelis --bench text
```

The benchmark uses bundled Source Sans 3 OTF and Noto Sans Arabic variable TTF.
Inputs repeat Latin ligatures/kerning, combining marks, Arabic, numbers, and spaces.
Input lengths are Unicode scalar counts; CSV glyph counts are measured separately.
Before timing, it asserts unchanged snapshot identity and exact glyph/line agreement
between width reflow and a fresh buffer.

These are ranges of per-run medians, in microseconds:

| Input scalars | New buffer + shape + snapshot | Width reflow + snapshot | Content change + reshape + snapshot | Unchanged snapshot |
| ---: | ---: | ---: | ---: | ---: |
| 100 | 38.94–39.94 | 2.83–2.85 | 36.31–39.79 | 0.0038–0.0040 |
| 1,000 | 376.85–397.42 | 24.33–25.23 | 351.83–359.92 | 0.0038–0.0040 |
| 10,000 | 3,710.88–3,728.42 | 234.44–246.81 | 3,463.96–3,691.79 | 0.0036–0.0038 |

Loading both font files into a new empty system takes 20.15–20.75 µs median,
including byte copies, backend database construction, face parsing, and cache setup.
This excludes filesystem reading and system-font discovery. New-buffer measurements
use an already warmed TextSystem, so they are not cold process/font-cache startup
measurements. They include text copy, buffer configuration, shaping, snapshot
construction, and destruction. Content changes alternate between the original
string and the string with a final exclamation mark; widths alternate 240/320 units.

The unchanged path batches 1,000 calls per sample for timer resolution. Its result
is average nanoseconds per call, including returning/dropping an Arc; its p95 is
the percentile of batch averages, not individual-call tail latency. Other stages
use one operation per sample. All p95 results are retained in the CSV files.

Unchanged retrieval stays constant as content grows. Reflow scales with glyph count
but reuses shaped runs, taking roughly 15 times less CPU time than a new-buffer
shape on this input. Reflow still allocates a new immutable snapshot; callers keeping
old snapshots keep those allocations and font references alive. Changed content
still requires real advanced shaping. Avoid calling setters with changed content
or font settings every frame for static labels.

This is an initial CPU baseline, not a comparison with Parley, Glyphon, direct
cosmic-text, or another implementation. It does not establish wrapper overhead
relative to a backend-only API. No rasterization, atlas preparation, GPU recording,
GPU execution, presentation, or window work is measured. Laptop scheduler/thermal
noise and other text/font/script mixes can change these results. GPU acceptance
and raster-cache memory budgets belong to the next milestone.
