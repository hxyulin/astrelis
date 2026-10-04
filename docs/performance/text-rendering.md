# Coverage/color renderer CPU baseline

Measured on 2026-10-05, Apple M3 Pro / Metal, macOS 27.0.1, Rust 1.98.1, release
build. Three sequential runs use eight warmups and forty samples per stage and
input length. [Metadata](text-rendering-m3-pro-metadata.json) records adapter/compiler
details, source hashes, font provenance, work counters, and the original run logs.
Raw median/p95 measurements are in [run 1](text-rendering-m3-pro-run-1.csv),
[run 2](text-rendering-m3-pro-run-2.csv), and [run 3](text-rendering-m3-pro-run-3.csv).

```sh
cargo bench -p astrelis --bench text_rendering
```

Inputs repeat Latin ligatures/kerning, combining marks, Arabic, numbers and spaces
from the bundled Source Sans 3 and Noto Sans Arabic fonts. Paragraph width is 400
units, font size 20, line height 28, and raster scale one. Input lengths are Unicode
scalar counts; drawable image counts are 84, 851, and 8,537 respectively. The glyph
set repeats, using approximately thirty cache keys and one 1024² R8 page (1 MiB).
This benchmark measures coverage images, not color-atlas performance.

Ranges of per-run medians, in microseconds:

| Input scalars | Cold raster/atlas/geometry | Cached atlas, new geometry | Prepared draw recording | Prepared frame CPU total |
| ---: | ---: | ---: | ---: | ---: |
| 100 | 228.17–233.83 | 26.00–26.88 | 0.63–0.73 | 31.83–34.98 |
| 1,000 | 269.04–284.96 | 61.46–61.81 | 0.58–0.63 | 30.79–34.06 |
| 10,000 | 624.92–629.56 | 421.73–427.65 | 0.58–0.69 | 32.88–35.77 |

Cold preparation clears glyph/atlas/scaler caches, then rasterizes misses, allocates
pages, writes image payloads, and creates immutable glyph geometry. Renderer/pipeline
creation and CPU layout/shaping happen outside that interval. Preparation uploads
are flushed and completed outside timing before the following stage.

Cached-atlas preparation hits the glyph-image cache but still creates a new geometry
buffer. Counter assertions require zero raster misses and zero additional atlas
upload bytes in this stage. It therefore scales with visible glyph count; it is not
a free way to redraw unchanged text. Keep PreparedText between frames.

Prepared recording times one draw of that immutable text into an existing pass.
This input stays on one page, so every prepared text produces one instanced GPU
draw regardless of glyph count. Drawing copies one 48-byte placement/color record
into recording-owned storage, with no shaping, rasterization, atlas upload, or
glyph-geometry upload. Counter assertions check raster misses/atlas uploads stay
unchanged. Frame CPU total additionally includes frame/pass creation, upload and
submission; completion waits happen after timing.

The destination is 64², so most glyphs in larger paragraphs lie outside the viewport.
All glyph instances are still recorded and sent to the GPU. These figures do not
measure GPU execution, completion latency, presentation, atlas pressure, a large
unique-glyph set, or mixed mask/color page transitions. Ordered color batches and
budget/lifetime failures are covered by pixel/ownership tests rather than this
initial timing workload. No comparison with direct wgpu, Glyphon, or another text
renderer is established. Laptop scheduler and thermal state can change the results.
