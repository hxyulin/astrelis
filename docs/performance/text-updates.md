# Changing-text performance

Measured on 2026-10-05, Apple M3 Pro / Metal, macOS 27.0.1, Rust 1.98.1, release. This compares the CPU text path and GPU renderer before and after paragraph reuse, snapshot allocation/font-lookup improvements, and safe geometry-buffer recycling. The public preparation/drawing API stays the same; stats add geometry allocation/reuse counters.

Three sequential release runs per state use eight warmups and forty measured samples. Fonts are pinned SourceSans3, NotoSansArabic, and the original color fixture. GPU execution is **not measured**. [Metadata](text-updates/metadata.json) contains commands, conditions, baseline qualifications, source/font/artifact hashes, and original logs.

```sh
cargo bench -p astrelis --bench text
cargo bench -p astrelis --bench text_workloads -- --no-timestamps
```

## Results

All values below are **per-run median ranges in microseconds**. Each raw CSV also records p95. The document workload changes one middle paragraph between two equal-length Latin/Arabic strings, with 49 shaped glyphs per paragraph, 320-unit wrapping width, and warmed fonts. It measures setters, shaping/reflow, and snapshot construction; no GPU is initialized. The label workload changes 60 of 600 visible numeric labels, then records all 600 separately prepared texts into a 1920×1080 framebuffer.

| Workload/stage | Before (µs) | After (µs) |
| --- | ---: | ---: |
| Edit one paragraph in 10 paragraphs | 191.10–192.50 | 22.35–23.79 |
| Edit one paragraph in 100 paragraphs | 1896.83–1983.65 | 49.92–53.56 |
| Edit one paragraph in 1,000 paragraphs | 19669.69–19818.21 | 339.37–344.65 |
| Update 60 labels: content/layout/preparation | 645.27–715.88 | 450.92–457.08 |
| Update 60 labels: content/layout only | 226.71–235.29 | 200.86–206.02 |
| Update 60 labels: cached glyph geometry only | 344.67–375.92 | 242.17–248.19 |
| Record all 600 updated labels (1x MSAA) | 52.92–53.21 | 67.02–67.71 |
| Frame CPU for all 600 updated labels (1x MSAA) | 188.12–189.88 | 204.48–206.02 |

The median of the three label-update medians falls from 714.25 to 452.17 µs, about **37% less CPU preparation time**. Editing one paragraph among 1,000 falls from 19.77 ms to 0.341 ms, about **58× faster** for this retained-document workload. These are workload-specific local results, not platform-wide speedups.

There is a recording tradeoff: each independent prepared text now retains its geometry ownership token through GPU completion. For 600 labels, recording increases by approximately 14–15 µs and frame CPU by 15–18 µs. Preparation savings are larger in the measured changing-label workload, but a fully static label grid pays the additional lease-tracking cost. Small/repeated-resource passes avoid allocating a hash set; larger passes deduplicate ownership tokens with a hash set rather than a quadratic scan.

Update timing includes formatting, setters, layout, prepared-text construction, and dropping replaced texts. The two diagnostic subintervals exclude some of that bookkeeping, so their medians need not sum to the overall median. Diagnostic timer calls exist in both baseline and final runs. Warm update assertions run outside the interval: zero glyph-cache misses, zero atlas writes, zero new geometry buffers, and exactly 60 geometry-buffer reuses per measured iteration.

Frame CPU includes acquisition, pass setup, recording, parameter uploads, teardown, and submission. GPU completion waits and readbacks are outside every timed CPU interval. Preparation writes are flushed and completed before the separately measured draw stream. Updates and frame CPU are separate intervals, **not end-to-end frame latency**. Both 1x and supported 4x MSAA results, visible static/mixed-color workloads, unique glyphs, and cache churn are retained in the raw GPU CSVs.

## Implementation and ownership

Content edits preserve matching leading and trailing paragraphs, including when inserting/removing middle paragraphs. Changed paragraphs retain their String/shaping scratch when paragraph counts match. Font selection/features and font-system generation still invalidate shaping; metrics, wrapping, and alignment invalidate layout appropriately. Every resulting snapshot rebuilds absolute UTF-8 source offsets and owns its text/fonts. Single-face runs avoid per-glyph font-map hashing and a hash-map allocation; edited snapshots reserve geometry capacity from the previous layout, capped by the new text length.

This is paragraph-level reuse. A changed paragraph still undergoes full advanced shaping, and the snapshot visits the entire document. Large single-paragraph edits therefore have different performance from the paragraph benchmark. It adds no persistent shaped-string cache and changes no shaping algorithm, glyph quality, or atlas representation.

Recycling keeps at most 64 returned buffers and 1 MiB of buffer capacity per renderer, including buffers awaiting GPU completion. Individual recycled buffers are at most 64 KiB. Small allocations round capacity to a power of two; larger paragraphs remain exact-sized and are released. Prepared resources remain caller-owned and can exceed this recycle budget. Atlas budgets remain separate.

Released geometry returns a buffer plus a weak ownership token to the recycle pool. Prepared texts/clones, recordings, and submitted GPU work retain that token. The pool can write a returned buffer only when its token has no strong owners. GPU completion callbacks retain Send ownership tokens instead of WebGPU buffer/texture handles. Clearing/dropping the renderer releases its recycle pool while retained resources continue working.

Tests compare multilingual wrapped edits, insertions/deletions, blank text, CR/LF variants, style/font-generation changes, and retained source clusters against fresh layouts. Pixel/lifetime tests verify clones, pending recordings, submitted draws, recycled uploads, cache clearing, renderer replacement, and bounded recycling. The full workspace passes 90 tests and 18 doctests, strict Clippy, native docs, formatting, and wasm32 library compilation.

## First-use cost and follow-up

Cold coverage/MTSDF generation is unchanged in this pass. A ten-second sample of the cold MTSDF benchmark found outline-distance computation and error correction dominate CPU preparation. The [distance-field baseline](text-distance-fields.md) remains the source for cold generation costs: roughly 52–54 ms for 31 cold cached entries at 64 pixels/em, and 555–565 ms for 256 distinct drawable glyphs plus a blank entry. Those figures include generation, atlas work, and geometry, with GPU completion outside timing.

The next performance step should split CPU glyph preparation from GPU atlas insertion/upload. Application-controlled workers or startup prebaking could then generate missing glyphs while the UI draws already prepared content, with explicit completion and per-frame upload budgets. This would improve responsiveness; generator throughput still needs its own measurement and optimization. After that, shared geometry upload pages/batching can target hundreds of simultaneous label changes, while viewport-aware layout should precede large editor/document workloads.

## Raw results

- CPU before: [run 1](text-updates/cpu-before-1.csv), [run 2](text-updates/cpu-before-2.csv), [run 3](text-updates/cpu-before-3.csv).
- CPU after: [run 1](text-updates/cpu-after-1.csv), [run 2](text-updates/cpu-after-2.csv), [run 3](text-updates/cpu-after-3.csv).
- GPU before: [run 1](text-updates/gpu-before-1.csv), [run 2](text-updates/gpu-before-2.csv), [run 3](text-updates/gpu-before-3.csv).
- GPU after: [run 1](text-updates/gpu-after-1.csv), [run 2](text-updates/gpu-after-2.csv), [run 3](text-updates/gpu-after-3.csv).
