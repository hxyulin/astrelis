# Cold glyph work separation

`cargo bench -p astrelis --locked --bench glyph_preparation` compares synchronous
cold MTSDF preparation with an application-owned `std::thread` worker, retained
text draws while it runs, four-key / 65,536-byte atlas commits, and explicit final
geometry upload. Every result is checked against synchronous framebuffer pixels.
The workload, fonts, density 64 and range 0.25 match `text_distance_fields`:
1,000 repeated multilingual characters (31 cache entries), and 256 distinct
nominal outline glyphs plus one blank-space key (257 entries).

The benchmark uses three samples, no warmup, a 1920×1080 RGBA8 headless target and
one sample per pixel. Layout/shaping, pipeline preparation and GPU completion
waits/readback are outside stage timing. The worker loop draws existing prepared
coverage text, polls the device and sleeps one millisecond between completion
checks. Atlas commits also alternate with retained draws. `worker_us` measures
CPU-thread wall time, including OS scheduling; draw/commit/finish times measure
CPU API calls, not GPU execution, visible frame presentation or input latency.
`staged_wall_us` includes worker startup, polling/sleeps, commits, draws and finish.

Machine: Apple M5 Max (Metal), Mac17,7, arm64, 128 GiB RAM, macOS 27.0.1
(26A434), Rust 1.99.0 (b940084d7), default optimized bench profile, 2026-10-23.
Raw samples and adapter details are in `glyph-preparation-m5-max.csv` and
`glyph-preparation-m5-max.log`. No concurrent builds or tests were run by this
agent during the recorded timing run; unrelated host activity is uncontrolled.

Observed ranges across the three samples:

- 31 keys / 276,256 image bytes: worker generation 73.3–81.8 ms;
  19–37 retained draws during generation; eight commit calls, with the slowest
  call 0.269 ms; final geometry 0.123–0.187 ms. Synchronous preparation took
  130.2–250.2 ms; staged wall time was 75.8–86.8 ms.
- 257 keys / 2,869,852 image bytes: worker generation 795.4–2,616.7 ms;
  460–1,156 retained draws; 65 commit calls, with the slowest call 0.827 ms;
  final geometry 0.087–0.118 ms. Synchronous preparation took 1,282.3–1,872.2 ms;
  staged wall time was 814.1–2,625.8 ms.
- Request capture took 2.0–3.7 microseconds. Retained draw CPU p95 ranged from
  0.195 to 2.939 ms. The worst individual draw was **28.925 ms**; moving
  generation does not guarantee a frame deadline. All six pixel comparisons passed.

Timing variance is substantial. Each sample runs the synchronous path first,
then the worker path, so font memory/allocator state is warmer for the worker;
worker drawing also adds contention. These samples demonstrate that existing
text can be recorded while generation runs and that commits contain bounded
uploads, rather than establishing an end-to-end speedup. They are not an
isolated generator-throughput comparison or a statistical latency guarantee.

The purpose is responsiveness through work placement. The generator algorithm
is shared with synchronous preparation and remains expensive. Moving it to a
worker does not establish a throughput speedup. The historical M3 Pro numbers
(52–54 ms / 31 entries; 555–565 ms / 256 glyphs) use another machine/run and
must not be treated as an apples-to-apples before/after comparison.

Limits are exact work counts, not deadlines. Atlas page allocation, driver work,
cache eviction, scheduling and final geometry construction can still produce
spikes. Generation limits bound retained pixel bytes/keys, excluding font/layout
storage, metadata and one glyph's scratch. Requests generate independently of the
GPU cache, so duplicate requests repeat CPU work and deduplicate at commit.
Applications should retain finished text, bound outstanding jobs and decide
whether stale content should be discarded; clearing the renderer cache rejects
all outstanding jobs. Work is cancellable before generation or by discarding
results, but an active generator cannot be interrupted inside a glyph.
