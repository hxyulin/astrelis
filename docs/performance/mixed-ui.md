# Sustained mixed UI workload

Three repeated 600-frame-per-phase runs on final parent `3736db1`, including tails
and allocation/recovery counters, are retained in
[the pre-push regression report](final-regression.md).

Run the application-owned headless benchmark in release mode:

```sh
cargo bench -p astrelis --bench mixed_ui --locked -- \
  --frames 1800 --warmup 60 --in-flight 3
```

For a quick pixel/validation smoke run, use `--frames 8 --warmup 2`. Positive
frame/warmup counts and 2–8 submissions in flight are accepted. Each phase runs
exactly the requested frame count. This is a small executable, not a scheduler or
a benchmark dependency for applications.

The deterministic scene uses a 1600×900 RGBA8Unorm headless attachment, 1× MSAA,
60 visible cards in a 12×5 grid, Source Sans 3 from the licensed repository font
fixture, and no system font discovery. Every card interleaves a rounded background,
an image, a retained quadratic path, a label, a 64-point polyline with markers,
and a boundary-crossing shape, under two nested rounded clips. Draw order is fixed.
The benchmark compares complete static images before/after warmup and checks first
and last card pixels for outer/inner clipping, image color, path, text, and points.
It checks wgpu validation errors and verifies region pixels after each phase too.

After warmup, the three phases are:

1. `warm_static`: retained text/path/image/points; draw parameters and clips record
   anew each frame.
2. `changing_churn`: replace six of 60 labels, replace the 64-point data, retessellate
   and upload the path, and replace a 32²/256² image and its binding each frame.
   The last frame adds 12,000 shapes, deliberately growing upload storage.
3. `recovery_static`: retain the final changed content, stop churning, and observe
   upload-pool recovery after more than 120 recordings. Short smoke runs do not
   exercise expiry.

The FIFO submission queue waits for the oldest submission only when the configured
bound is reached. It drains at phase boundaries, outside per-frame CPU intervals.
Static warmup and verification use individual waits. CPU output contains nearest
rank p50/p95/p99/max distributions for backpressure waits, content update and
preparation, recording plus upload/submission, and their combined frame wall time.
Drained phase frames/second includes all submissions' completion, with no display
pacing. These distributions include allocation, Rust/wgpu validation and driver
submission costs. Completion waits are **not GPU execution measurements**. GPU
queries are deliberately absent: local Metal timestamp reliability has previously
failed; the benchmark does not invent a substitute GPU duration.

`GraphicsContext::upload_stats()` exposes cumulative page creation/reuse/release
and clip binding creation, plus idle buffer/staging capacities and retained clip
binding counts. These are logical library resource measurements. They do not
measure RSS, peak process memory, physical GPU allocations, or when the driver
reclaims a dropped allocation. Text renderer statistics describe glyph/atlas and
geometry resources separately. The image churn sizes are known source payloads,
not physical-memory measurements. In-flight wgpu retention can outlive a library
handle. Applications continue to own frame pacing, passes, resources, draw order,
and raw encoder/pass access.

Rounded clip layouts are shared per context. Immutable clip bindings are cached
with the leased upload page at their static aligned offsets, at most 64 per page.
They expire with idle pages; this avoids an unbounded global buffer/offset cache.
Bindings beyond the cap are safely created without retention. Existing custom
shaders keep the same non-dynamic group-one uniform layout, zero-capacity fallback,
and explicit `bind_rounded_clips` interface. Managed submission ordering protects
page contents even with multiple submissions in flight; overlapping unsubmitted
recordings hold different pages. A scoped Painter clip still saves one parent
stack snapshot because custom pass access can replace that stack. Pushing a clip
no longer clones that snapshot a second time.

Local results and exact reproduction metadata are in [mixed-ui/](mixed-ui/).
Measurements describe this machine/backend and workload only. They establish no
remote, cross-platform, presentation-latency, or general application performance
claim. No frame-rate acceptance threshold is imposed.

## Local baseline: Apple M5 Max / Metal

The release run recorded 1,800 frames per phase (5,400 measured frames, plus 60
warmup frames and verification), with three submissions in flight. Drained phase
wall times were 5.084 / 7.326 / 5.556 seconds. Warm static CPU recording/upload/submit
p50/p95/p99 were 739 / 11,274 / 34,927 µs; changing/churn content preparation was
197 / 560 / 20,384 µs. The [complete CSV](mixed-ui/sustained-1800.csv) retains all
four CPU distributions, including maxima. [The log](mixed-ui/sustained-1800.log)
retains adapter/device limits, actual sample counts, pixel checks, throughput and
resource counters. These large tails occurred with other agents/builds active on
the shared machine; the run is a practical local baseline, not an isolated latency
measurement or a performance acceptance claim.

Warm static page creation and clip binding creation both stayed unchanged (7 pages,
150 cached bindings after warmup). The final churn spike raised idle upload buffers
and staging capacities from 458,752 to 1,638,400 bytes (7 → 25 pages). Recovery
released 18 pages and returned both capacities to 458,752 bytes. Recovery needed
150 new clip bindings as formerly unused pages became the steady working set;
retained bindings returned to 150. The glyph vocabulary stayed warm: 15 cache
entries and one 1 MiB atlas throughout. Image/path replacements and label geometry
exercise resource turnover, not cold glyph generation or atlas-budget recovery.

The preceding matched short runs used 360 frames per phase, 60 warmups and the
same three-submission bound. Baseline source is benchmark commit
`f9e8a0d` on parent `4ea5ee5`; final source adds counter logging/assertions, the
binding/layout reuse, and removes the redundant second parent-stack clone.
The baseline source path creates one bind group and layout per changed stack:
120 of each per static frame in this scene, inferred from the call path rather
than a driver allocation counter. After reuse, the counters verify zero new warm
upload-page clip bindings. Observed warm CPU record/upload/submit p50 changed
947 → 718 µs, with p95 18,112 → 6,520 µs. The two short sequential runs are noisy,
not randomized or repeated, and include the snapshot change; they do not isolate
bind-group cost or establish a statistically reliable speedup. Raw
[before CSV](mixed-ui/before-360.csv), [before log](mixed-ui/before-360.log),
[after CSV](mixed-ui/after-360.csv), and [after log](mixed-ui/after-360.log) are kept
without removing outliers. No remote measurement was performed.

[metadata.json](mixed-ui/metadata.json) records the machine/toolchain, exact
commands, dependency/font/benchmark/source hashes and executable hash. The source
commit containing that metadata is the final reproduction source; the before run
can be reproduced by checking out `f9e8a0d` and using its recorded command.
