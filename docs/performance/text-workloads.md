# Screen-sized text and cache workload baseline

Measured on 2026-10-05, Apple M3 Pro / Metal, macOS 27.0.1, Rust 1.98.1, release.
Three sequential CPU runs follow completed checks with the window example closed.
Each case uses eight warmups and forty measured samples. [Metadata](text-workloads-m3-pro-metadata.json)
records commands, conditions, compiler/adapter, fixture/source hashes, counters,
readback results, retries, and original logs. Raw median/p95 CSVs:
[run 1](text-workloads-m3-pro-run-1.csv), [run 2](text-workloads-m3-pro-run-2.csv),
[run 3](text-workloads-m3-pro-run-3.csv).

```sh
cargo bench -p astrelis --bench text_workloads -- --no-timestamps
# Request timestamps only if advertised; incomplete GPU sample sets are withheld:
cargo bench -p astrelis --bench text_workloads
```

## Visible rendering

Outputs are 1920×1080 RGBA8Unorm framebuffers, cleared transparent each frame.
Both 1x and supported 4x MSAA run. Every prepared image quad is checked to fit
inside the viewport. GPU readback verifies substantial nonzero-alpha output,
including the first and last grid labels; no readback is timed. Drawing asserts
no new raster misses, atlas writes, or static geometry uploads and exactly
48 parameter bytes per nonempty text call. These workloads exercise text rather
than complete application scenes, presentation, or a comparison with another renderer.

The paragraph has 32 visible lines, 4,480 drawable glyphs, font size 18, and width
1888. It occupies one coverage page and one text batch. The label grid has 600
separately prepared 14-unit labels, 5,400 drawable glyphs, in 20 columns and 30 rows.
It produces 600 text calls/batches. Mixed color repeats `M😀M😁M` from the original
fixture at size 18: 3,000 visible glyphs and 3,000 ordered batches from 600 calls,
with five alternating coverage/color page runs per call. It is deliberately a
page-transition workload, not a representative emoji benchmark.

The update case changes 60 of the 600 labels each iteration, cycling through the
grid and changing numeric content. It measures setter/layout/new PreparedText
creation together, then flushes preparation writes outside timing before measuring
the retained draw stream. Its warmed glyph vocabulary is small; it is not a font
or Unicode-cache churn case. SourceSans3 and Arabic fonts are loaded explicitly;
all workload layouts require no unresolved glyph clusters.

Per-run median ranges in microseconds. Recording starts after pass creation and
includes Painter session entry plus all text calls. Frame CPU additionally includes
frame/pass setup, upload, teardown, and queue submission, excluding completion wait.
Updates are separate from frame CPU; do not read these as end-to-end frame latency.

| Workload | MSAA | Text batches | Record CPU | Frame CPU | Update/layout/prepare CPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| Clear-only control | 1x | 0 | 0.04–0.04 | 14.60–15.17 | — |
| Clear-only control | 4x | 0 | 0.04–0.04 | 13.04–15.69 | — |
| Visible paragraph | 1x | 1 | 0.50–0.54 | 31.46–34.98 | — |
| Visible paragraph | 4x | 1 | 0.54–0.60 | 31.06–36.52 | — |
| 600 visible labels | 1x | 600 | 53.10–60.44 | 184.19–205.25 | — |
| 600 visible labels | 4x | 600 | 52.46–58.35 | 190.58–199.02 | — |
| 600 mixed-color labels | 1x | 3,000 | 79.23–87.65 | 569.15–648.44 | — |
| 600 mixed-color labels | 4x | 3,000 | 76.96–86.58 | 569.96–646.38 | — |
| Update 60 of 600 labels | 1x | 600 | 52.08–56.71 | 190.98–212.85 | 668.08–754.17 |
| Update 60 of 600 labels | 4x | 600 | 51.29–57.60 | 189.35–212.77 | 703.02–796.29 |

Renderer stats accumulate across cases and both sample counts. The initial coverage
page costs 1 MiB; introducing the color fixture adds a 4 MiB RGBA page. Subsequent
cases retain both cached pages even when only coverage is drawn. Stats report logical
texture bytes rather than actual backend allocation overhead. Original fixture color
marks do not establish memory behavior for an entire emoji font.

## Preparation and bounded pressure

The unique-glyph workload enumerates distinct nominal glyphs present in bundled
SourceSans3 and Arabic, separates input characters with spaces, then shapes it.
The resulting 4,638 input scalars produce 1,822 distinct shaped face/glyph cache keys
(including blank entries) and 2,589 drawable images. Layout/shaping is outside these
preparation intervals. Cold mode clears glyph/atlas/scaler caches each sample;
cached mode retains them but builds a fresh geometry buffer. Each prepared buffer
contains 124,272 bytes. Image payload is 237,178 bytes, packed into one 1 MiB R8 page.
Generation and queue writes, not GPU completion, are timed.

Raster pressure prepares a short Latin/Arabic string at a new physical scale each
iteration: 1.0 through 3.9375 in 1/16 increments, base size 16. It uses two 256²
R8 pages, a 128 KiB logical maximum. Previous prepared resources are dropped and
queue preparation writes completed outside timing before the next iteration.
Append-only packing can strand free space among old size variants. On AtlasFull,
the caller explicitly clears caches and retries under the same budget; the timed
interval includes the failed work, clearing, and successful retry. The renderer
itself does not wait or increase its budget. All three runs needed one such retry.
The benchmark checks the page limit and logs retries plus peak live bytes.

| Preparation workload | Median CPU, μs |
| --- | ---: |
| 1,822 keys, cold | 14649.65–15578.25 |
| Cached images, new geometry | 185.60–191.88 |
| Raster-size pressure with explicit recovery | 173.71–179.44 |

Cold preparation of this larger glyph vocabulary approaches a frame budget even
though retained drawing is cheap. It belongs in explicit scene preparation or
staged groups of resources. Cached preparation still creates geometry; unchanged
content should retain PreparedText. Size churn costs real raster/cache work and
can require an explicit recovery policy with a small budget. These findings are
reasons to measure distance-field generation before selecting defaults, not evidence
that distance fields are faster.

## GPU timing limitation

The optional path requests TIMESTAMP_QUERY through GraphicsContext::from_wgpu,
uses custom pass start/end timestamps, resolves/copies after the pass, and multiplies
query deltas by Queue::get_timestamp_period. Mapping/completion waits are outside
CPU timing. It rejects zero/missing and nonmonotonic samples. A case must produce
all forty measured durations or its entire GPU result is withheld, avoiding a
biased subset of apparently valid timestamps.

On this machine the adapter advertised timestamp support, but end timestamps were
usually zero or stale, including passes with visible fragments. A diagnostic run
completed all workloads and pixel checks but produced no complete GPU sample set;
[its CSV](text-workloads-m3-pro-timestamps.csv) therefore reports unavailable GPU
results. Fresh query objects also failed to produce a complete sample set during
investigation. The three CPU baselines explicitly disable timestamps, avoiding
query resolve/copy overhead in their frame CPU numbers.

The user's [wgpu issue #9414](https://github.com/gfx-rs/wgpu/issues/9414) remains open
as checked on 2026-10-05. It reports Metal timestamp failures on macOS 26; these
local measurements use macOS 27.0.1/wgpu 30.0.1 and show a related failure pattern.
This is an observation, not proof of the same root cause. No reliable workaround
was established here. CPU completion wait is not substituted for GPU execution.
GPU execution/presentation acceptance remains open pending a working timestamp
backend or an independent profiler.
