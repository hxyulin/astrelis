# Pre-push performance check: 3736db1 versus 5e5ea9d

Runtime correctness checks passed, and the sustained mixed UI retains its warm
allocation and recovery behavior. **A clean “no performance regressions” verdict
is blocked:** comparable unclipped scoped drawing was slower in all three pairs.
The shared host prevents attributing the entire difference to the library.
No production changes, push, or PR were made for this check.

## Method and evidence

Measured 2026-10-23 Hong Kong time (2026-10-22 17:27:18–17:29:02 UTC), on
Apple M5 Max / Metal, 128 GiB, macOS 27.0.1 (26A434), aarch64, rustc 1.99.0,
wgpu 30.0.1. Both revisions were built **before** timing with the same default
optimized Cargo bench profile, target directory and identical locked dependencies.
Executables were copied aside before running; no Cargo command ran in this runner
during timing. The parent confirmed its tests/Clippy/hooks were finished.
Other agents, Flux programs, rustc processes and possible integration builds shared
the machine. Sampled one-minute host load was 48.70–68.01 on 18 CPU cores.
[Host snapshots](final-regression/host-load.jsonl) were captured every five seconds;
these are observations, not an exhaustive accounting of concurrent work.

Baseline `5e5ea9df12f3539661b8d487a83b907e63641bfa` used the **verbatim current
primitives benchmark source**, as in [nested-clipping.md](nested-clipping.md).
Current was `3736db1646612dfd91fe5f824fdd7dfeaca60f19`. Order was sequential
B1/C1/B2/C2/B3/C3, each with counts 100,1000,10000, 25 alternating paired samples
per workload and five warmup pairs. All **288** workload/count pixel checks passed.
The benchmark emits distributions, not individual sample durations; complete
unfiltered stdout CSV and stderr logs are retained for every run.

Values below are the median of the three run medians, in microseconds, with
absolute and relative changes. Ranges are the minimum–maximum **run medians**,
not confidence intervals. [Full summary CSV](final-regression/primitives-summary.csv)
covers all 96 workload/count/side rows, recording and CPU-total medians/p95s,
run ranges and the minimum/maximum change across adjacent revision pairs.
Repeated reference rows are separate observations; they are not pooled.

## Comparable CPU recording results

At 10,000 primitives:

| Workload | Baseline → current µs | Change | Baseline / current run ranges µs |
| --- | ---: | ---: | ---: |
| Shapes individual | 1068.04 → 1280.29 | +212.25 (+19.9%) | 970.3–1121.3 / 1072.0–1304.7 |
| Shapes scoped | 614.25 → 847.12 | +232.88 (+37.9%) | 521.2–636.4 / 693.8–851.0 |
| Shapes batch | 395.38 → 444.88 | +49.50 (+12.5%) | 338.7–402.3 / 396.2–446.0 |
| Lines individual | 1039.00 → 1289.17 | +250.17 (+24.1%) | 962.5–1140.3 / 1189.1–1312.2 |
| Lines scoped | 648.00 → 889.79 | +241.79 (+37.3%) | 520.5–710.6 / 812.8–893.8 |
| Lines batch | 397.75 → 401.58 | +3.83 (+1.0%) | 335.0–448.4 / 366.2–452.5 |
| Painter shapes batch | 395.67 → 398.29 | +2.62 (+0.7%) | 359.2–398.2 / 397.9–450.6 |
| Painter lines batch | 394.62 → 440.08 | +45.46 (+11.5%) | 334.5–403.8 / 401.7–448.9 |
| Painter mixed | 5980.00 → 6769.08 | +789.08 (+13.2%) | 5589.6–6180.3 / 6162.5–7491.2 |
| Painter scissor mixed | 6703.75 → 7783.21 | +1079.46 (+16.1%) | 6516.2–7529.7 / 7023.8–7949.8 |
| Fills/outlines scoped | 572.88 → 822.29 | +249.42 (+43.5%) | 498.2–582.0 / 794.3–888.7 |

Individual rows above use the reference side of the matching scoped workload.
At 1,000, scoped shapes were 58.79 → 88.54 µs (+29.75, +50.6%), scoped lines
58.29 → 91.79 (+33.50, +57.5%), and mixed Painter 518.79 → 645.50
(+126.71, +24.4%). At 100, the corresponding changes were 7.71 → 11.50
(+3.79, +49.2%), 8.67 → 12.38 (+3.71, +42.8%), and 57.67 → 74.46
(+16.79, +29.1%). Full absolute/range data for every count is in the summary.

At 10,000, scoped shapes' adjacent-pair recording changes were +9.0% to +63.3%,
lines +14.4% to +71.7%, and outlines +38.7% to +78.4%. The direction persisted,
but its magnitude varied substantially. Batch shapes ranged −1.5% to +31.3% and
batch lines −18.3% to +19.9%; those data do not prove a batch regression or parity.
For CPU totals (recording plus frame setup/upload/submit, excluding completion
wait), scoped shapes were 1098.13 → 1378.67 µs (+280.54, +25.5%), scoped lines
1152.54 → 1364.50 (+211.96, +18.4%), and mixed Painter 26195.67 → 33762.96
(+7567.29, +28.9%). Mixed Painter CPU-total p95 changed 44037.79 → 73647.21 µs;
recording p95 moved in the opposite direction. This is noisy CPU/driver evidence,
not isolated GPU timing or a latency guarantee.

Source investigation identifies a specific optimization candidate:
`primitive::record` now calls `RenderPass::bind_rounded_clips` for **each** scoped
draw. Even without clips, this calls `rounded_clip_capacity`, which queries
`Device::limits`; wgpu's core path retrieves the device from its registry and
clones its limits. It then clones/drops the shared disabled bind-group handle
before the already-bound check. Batches pay that path once per call. Cache the
immutable capacity per context (including imported-device limits) and borrow or
compare the shared disabled group before cloning it. Preserve explicit group-one
rebinding after raw-pass access, and preserve zero-capacity behavior. This is a
source-based hypothesis, not an experimentally isolated cause. A controlled
before/after test of that targeted change, plus existing reduced-limit/raw-pass
regressions, is needed before calling it a fix. No speculative production change
was included in this report. The repeated comparable scoped signal remains the
performance blocker; host noise must not be used to dismiss it or to claim proof.

## Rounded clipping feature cost

At 100 / 1,000 / 10,000, one rounded clip batch changed respectively:
5.75 → 9.17 µs (+3.42, +59.4%), 42.92 → 49.54 (+6.62, +15.4%), and
398.33 → 397.83 (−0.50, −0.1%). Two nested clips changed 4.96 → 9.33
(+4.38, +88.3%), 43.50 → 48.67 (+5.17, +11.9%), and 399.83 → 403.92
(+4.08, +1.0%). At 10,000 the nested baseline/current run ranges were
363.1–445.3 / 338.4–416.2 µs, with paired changes −9.3% to +4.1%.

Both rounded boundaries enclose the complete 64×64 attachment on **both** revisions;
pixels therefore match despite baseline nested semantics keeping only the innermost
analytic boundary. These timings compare equivalent images while current evaluates
the corrected stack. Extra stack preparation/evaluation is correct-feature cost;
the baseline cannot serve as a correct reference for real intersecting rounded
boundaries. The unclipped scoped workloads above have equivalent semantics and
must be assessed separately. No deeper-stack or GPU fragment-cost claim follows.

## Sustained current mixed UI and runtime checks

Three sequential current `mixed_ui` runs used 600 frames **per phase** (1,800 per
run), 60 warmup frames, three submissions in flight, the documented 1600×900
60-card scene and bundled Source Sans 3. Complete static equality, region probes,
post-phase verification and wgpu validation checks passed in each run.
These are current-only repetitions; the old revision lacks this benchmark and
correct nested clipping, so they are not a before/after speedup measurement.

| CPU frame wall | p50 range ms | p95 range ms | p99 range ms | Largest frame ms |
| --- | ---: | ---: | ---: | ---: |
| Warm static | 0.676–0.698 | 0.884–1.156 | 1.043–1.925 | 7.948 |
| Changing/churn | 0.848–0.951 | 1.087–1.690 | 1.236–5.860 | 17.358 |
| Recovery static | 0.632–0.713 | 0.828–1.719 | 1.316–6.035 | 17.318 |

The [mixed UI CSV](final-regression/mixed-ui-summary.csv) retains each run's p50,
p95, p99 and maximum for backpressure, update/prepare, recording/upload/submit and
frame wall. Warm recording/upload/submit p50 was 0.675–0.697 ms, and churn
preparation p50 was 0.175–0.192 ms. Across all three runs, warm page creations
stayed 7 → 7 and clip binding creations 150 → 150. Churn grew idle pages 7 → 25
and both logical buffer/staging capacities 458,752 → 1,638,400 bytes. Recovery
released 18 pages, returning both capacities to 458,752 bytes and retained
bindings to 150. Cumulative bindings became 300 because the recovered working set
needed 150 new bindings. All [counter snapshots](final-regression/mixed-ui-counters.json)
are retained. These counters do not measure process RSS or physical driver memory.

Current raw runtime checks followed [benchmark-smoke.md](benchmark-smoke.md):
rendering counts 1,1025 with five samples/one warmup; paths the same; points
256,4097 with five samples and its default warmup. All **50** equivalences
(18 rendering, 16 paths, 16 points) and nonempty-output checks passed with default
wgpu validation. Logs/CSVs are `final-regression/smoke-{rendering,paths,points}.*`.
Workspace tests and Clippy belong to the parent and were not duplicated here.

GPU execution timestamps, presentation, input-to-display latency, remote hardware,
other backends, sustained thermal behavior, and reliable frame pacing remain
unmeasured. Completion/backpressure waits are not GPU execution timings. In
particular the mixed UI's occasional 17 ms CPU frames preclude promising smooth
60 Hz presentation from these headless runs.

## Reproduce

[metadata.json](final-regression/metadata.json) retains revisions, exact run/build
commands, times, executable/source/font/lockfile SHA-256s and environment.
The [build logs](final-regression/build-current.log) and
[baseline build log](final-regression/build-baseline.log) show both release builds.
Create an isolated detached temporary checkout of 5e5ea9d and copy
`git show 3736db1:crates/astrelis/benches/primitives.rs` into its benchmark path
([retained overlay](final-regression/baseline-benchmark-overlay.patch)). Build
baseline `--bench primitives --no-run` and current
`--bench primitives --bench mixed_ui --bench rendering --bench paths --bench points
--no-run` with `cargo bench -p astrelis --locked`, the same toolchain/dependencies,
and default bench profile. Copy each build's executables immediately into a stable
directory, naming baseline `primitives-baseline` and current binaries by bench name.
Only after **both** builds finish, run:

```sh
python3 docs/performance/final-regression/run.py /absolute/binary-directory /new/results
# To regenerate the checked-in summary from these retained CSVs:
python3 docs/performance/final-regression/analyze.py
```

The [runner log](final-regression/run.log) records the exact completed sequence.
No outliers were removed. These three short alternating revision pairs on a busy
host establish neither statistical significance nor an isolated causal effect.
