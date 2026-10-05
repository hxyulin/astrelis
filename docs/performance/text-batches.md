# Batched text preparation

Measured on 2026-10-05, Apple M3 Pro / Metal, macOS 27.0.1, Rust 1.98.1, release. Three sequential runs compare `prepare_text` calls with `prepare_texts` on the same implementation and workloads. Each uses eight warmups and forty measured samples. [Metadata](text-batches/metadata.json) records commands, source/font/artifact hashes, conditions, and original logs. GPU execution is not measured.

```sh
cargo bench -p astrelis --bench text_batches
cargo bench -p astrelis --bench text_workloads -- --no-timestamps
```

## Matched workloads

Both modes retain their previous prepared resources until all replacement resources have been successfully prepared. The benchmark changes every label each sample, using `Item 00000`-style text, SourceSans3 at 14 units with a 21-unit line height. Each label contains nine drawable glyphs plus a blank space. All 60/600/1,000 labels fit in a 1920×1080 RGBA8Unorm framebuffer with 1x MSAA. Fonts and the glyph vocabulary are warmed outside measured stages.

The initial resources are prepared individually or together according to the mode. The single-layout recycle pool retains at most 64 returned buffers, so 600/1,000-label replacement batches require new individual allocations each iteration. This is a measured consequence of that bounded policy, not evidence that every possible individual-update strategy has the same cost. Dropping/replacing each old label immediately can reuse buffers more aggressively; the legacy partial-update workload still exercises that path.

Per-run median ranges in microseconds; raw CSVs also include p95:

| Changed labels | Individual update/layout/prepare/replace | Batched update/layout/prepare/replace | Individual geometry preparation | Batched geometry preparation | Uploads: individual → batched |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 60 | 410.69–422.27 | 229.98–235.90 | 231.27–240.08 | 55.90–60.27 | 60 → 1 |
| 600 | 5349.21–5415.85 | 2126.38–2136.33 | 3145.31–3170.62 | 418.48–421.38 | 600 → 4 |
| 1,000 | 9403.02–9508.54 | 3610.79–3647.69 | 5459.13–5511.15 | 695.27–700.92 | 1,000 → 7 |

Using the median of the three run medians, geometry preparation is **4.1× faster** for 60 labels, **7.5× faster** for 600 labels, **7.8× faster** for 1,000 labels. The corresponding overall update reductions are 44% for 60, 60% for 600, 61% for 1,000. These are workload-specific local CPU measurements.

Layout now dominates batched updates: around 1.69 ms of the roughly 2.13 ms update interval for 600 labels, and 2.89–2.92 ms of 3.61–3.65 ms for 1,000 labels. Batched preparation does not change shaping. The small difference in layout timings between the two modes is not a shaping optimization claim.

| Labels drawn | Individual record CPU | Batched record CPU | Individual frame CPU | Batched frame CPU |
| ---: | ---: | ---: | ---: | ---: |
| 60 | 8.79–8.92 | 7.71–8.04 | 52.29–54.31 | 46.92–47.50 |
| 600 | 73.25–75.48 | 53.06–53.71 | 222.42–227.71 | 177.40–181.92 |
| 1,000 | 123.54–129.04 | 88.69–89.06 | 366.79–370.92 | 278.23–282.83 |

Every result is still drawn separately with its own placement/color. GPU draw counts remain 60/600/1,000, and parameter payload remains 48 bytes per nonempty draw. Recording improves because shared geometry uses one retained completion token per buffer instead of one per label; each draw separately retains only its actual atlas allocations. This is preparation batching and lifetime deduplication, not draw-call batching.

## Timing and validation

The overall update interval includes formatting, setters, layout, preparation, and dropping replaced texts. The geometry subinterval measures preparation alone. Frame CPU includes acquisition, pass creation, recording, upload, teardown, and submission. Previous preparation writes are flushed/completed before recording. Completion waits and readbacks are outside all timed intervals; these separate CPU intervals are not end-to-end frame latency.

Warm assertions check no glyph-cache misses or atlas writes, exact logical geometry bytes, and 1/4/7 geometry uploads per batched iteration. Batched warm iterations allocate zero geometry buffers. Individual and batched modes produce identical whole-frame readback bytes, and the first/last labels are verified visible. Original logs contain allocation/reuse counters and renderer stats.

The legacy `text_workloads` benchmark was also run three times with its original partial-update strategy. Current 60-of-600 update medians are 463–478 µs, geometry preparation 252–267 µs, and recording 68–71 µs. The earlier [changing-text baseline](text-updates.md) reported 451–457 µs, 242–248 µs, and 67–68 µs respectively. These are separate sequential measurement sets, not paired old/new library runs; they show a small timing increase rather than establishing zero overhead. Dedicated geometry stays inline on the single-layout path to avoid an unnecessary shared-owner allocation. All raw legacy results are included.

Correctness checks cover ordered outputs, per-text metadata, coverage, MTSDF/color fallback, DPI, clipping, transforms, 1x/4x MSAA, empty/blank inputs, page boundaries, oversized texts, late validation failure, atlas exhaustion, clones, abandoned/submitted recordings, GPU-completion reuse, and independent atlas ownership. An application-created device with a 1,000-byte buffer limit verifies chunking and capacity buckets respect supplied limits. The full workspace passes 95 tests and 19 doctests, strict Clippy, formatting, docs, and wasm32 library compilation. The updated standalone Painter example launches successfully.

## API and memory

`TextRenderer::prepare_texts` and `Painter::prepare_texts` accept iterators whose items implement `AsRef<TextLayout>`, including borrowed snapshots and `Arc<TextLayout>` collections. They return one independently drawable `PreparedText` per input in input order. Coverage/MTSDF settings and scale apply to all layouts; their fonts, font sizes and paragraph constraints may differ.

Small layouts share consecutive buffers with at most 64 KiB of glyph payload, capped by the device buffer limit. A text is never split between buffers. Larger layouts use dedicated exact-sized buffers. Retaining one small result can retain its entire shared buffer; atlas ownership remains specific to that text. Empty results lease no geometry. Returned buffers remain subject to the 64-buffer / 1 MiB recycle limit, and active resources remain caller-owned.

For these nine-glyph labels, 151 labels fit in one chunk. The 60-label batch uses a 32 KiB capacity bucket, compared with 60 individual 512-byte buffers (30 KiB). The 600-label batch uses four 64 KiB buffers (256 KiB), compared with 300 KiB of individual capacity. The 1,000-label batch uses seven 64 KiB buffers (448 KiB), compared with 500 KiB individually. These are logical geometry capacities, excluding backend/allocator overhead and separate 1 MiB R8 atlas storage. Keeping one member of a batch can retain more storage than its individual buffer would.

An error returns no partial result vector. Earlier work may populate caches or upload completed chunks, but previous resources remain usable. No shaping, implicit submission, waiting, worker scheduling, or asynchronous glyph generation is added. Internally, glyph/atlas resolution is separated from geometry upload, providing a place to continue the CPU-glyph/GPU-upload split. Cold MTSDF generation remains synchronous and unchanged.

## Raw results

- batches: [run 1](text-batches/batches-run-1.csv), [run 2](text-batches/batches-run-2.csv), [run 3](text-batches/batches-run-3.csv).
- workloads: [run 1](text-batches/workloads-run-1.csv), [run 2](text-batches/workloads-run-2.csv), [run 3](text-batches/workloads-run-3.csv).
