# Primitive session clip binding

Follow-up to [rounded-clip-fastpath.md](rounded-clip-fastpath.md), based on
`856caefd2875e5df9c778664b4756ccf4f107b2f`. No push or PR is part of this work.

**Performance acceptance remains blocked by host variability.** The previous
27% original-baseline 10k-line slowdown is not reproduced: the patch is faster
than original in all three 10k scoped comparisons for shapes, lines, and outlines.
Against parent, those workloads improve in the first two pairs but regress in
the third; batch/nested controls also regress. This bounded experiment supports
retaining the targeted state amortization, but cannot establish a stable speedup
or clear the broader performance blocker. No extra benchmark rounds were added.

## Change and safety evidence

Primitive sessions now bind the legacy clip vertex data and rounded intersection
once on the first valid draw, and again on the first valid draw after mutable
pass access. A session exclusively borrows its pass; its `pass()` accessor always
invalidates both pipeline/raster restoration and clip binding readiness. Between
such accesses, instance uploads only replace vertex slot zero, while legacy clips
occupy slot one. Thus both clip interfaces can safely share the same lifetime.
The change is confined to `primitive.rs`; brush/group-zero binding still runs on
every brushed draw. Other renderers and custom binding APIs keep their behavior.

Validation precedes restoration, so empty slices, invalid single draws, and
partially valid rejected batches do not upload clips or replace application
bindings. A new pixel regression compares cached sessions against a reference
that explicitly binds both clip interfaces for every draw. It exercises shapes
and lines, solid and brush shading, zero/two/full-capacity imported devices,
wrapped application group-one replacements at zero/nonzero dynamic offsets,
raw group-one/vertex-slot-one/scissor overrides, custom-material mesh rendering,
nested Painter clip scope entry/exit, viewport changes, full-stack depth changes,
and returns to disabled clipping. Translucent overlapping draws exercise order.
It also checks that empty/invalid work creates no clip upload bindings, including
after pass access. Removing pass-access invalidation makes this regression fail
at capacity two; the bounded negative-control log is retained.

`cargo test --workspace --locked` passed **215 unit tests + 30 doctests**, one
ignored. `cargo clippy --workspace --all-targets --all-features --locked -- -D
warnings`, `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features
--no-deps --locked`, and `cargo check --workspace --lib --target
wasm32-unknown-unknown --locked` passed. Formatting and diff checks passed. Pre-commit and pre-push hooks passed for the
changed files. An initial all-files hook run found pre-existing trailing whitespace
in the two prior `rounded-clip-fastpath/*.patch` evidence files; their bytes were
restored to preserve provenance. The final all-files pre-push run skips only that
whitespace hook, which is separately run across every tracked file except those
two existing patch artifacts. This exception is retained in the hook driver/logs.
[Logs, exact source delta, metadata, and unfiltered harness CSVs](session-clip-fastpath/)
are retained. The negative-control log is intentionally a failed run with the
invalidation removed; the full test log covers the restored final sources.

## Experiment and limits

All release executables were built/frozen before timing. Parent reuses the prior
fixed binary, original reuses the prior `5e5ea9df12f3539661b8d487a83b907e63641bfa`
binary. Before measurement the driver verifies both executable SHA-256 hashes,
reconstructs the prior fixed source patch on `adb79c6` and compares all tracked
crate inputs/manifests/toolchain to parent HEAD, and checks original archive
sources against git with the current primitives benchmark overlay. The unchanged
benchmark source SHA-256 is
`c1f81c30ab36a6ea3c304d9678c73a227022a4a44eee6e1a1a3c0615b46c0dc5`;
current Cargo.lock is
`d1138cd5b69d9d4cb3941295b388dce9dba311eb2abbfa6ddb9276ac0bb964c6`.
Original uses the previously documented normalized lock
`45929a1d4bee476c092e351ecbbe5dd554dc7d166229dad90d453ecdf1a743e2`:
local `log` edges and inactive GLES packages removed, retained external
versions/checksums unchanged. See the prior normalization patch. This contextual
original comparison includes upstream runtime/text/recovery changes.

Apple M5 Max, 18 CPU cores, 128 GiB, Metal, macOS 27.0.1, aarch64,
rustc/cargo 1.99.0, wgpu 30.0.1, release, default wgpu validation. No builds from
this checkout ran during timing; the parent was requested idle. Compact snapshots
retain uptime and the top 15 CPU process names without arguments. Other rustc
processes and unrelated benchmark processes were active. One-minute load across
primitive snapshots ranged **8.29–40.49**; round two overlaps a compiler burst.
Whole-workload costs move markedly between invocations, including controls. No completion wait is
inside the CPU recording intervals; these measurements do not time GPU execution.

Three rounds use before/after/original, original/after/before,
before/after/original process order, each with `--counts 100,1000,10000 --samples
25 --warmup 5`. The harness also alternates reference/variant samples internally.
Each count/workload's three invocation medians are aggregated without filtering;
ranges are ranges of invocation medians, not sample distributions or confidence
intervals. Counts/workloads share processes and are not independent observations.
The CSVs retain median/p95 recording and total CPU costs for both reference and
variant sides. `summary.csv` includes all controls and every paired percentage.
No statistical certainty, general speedup, or cross-platform claim follows.

## Bounded results

Medians of the three variant recording medians, in microseconds, are written as
parent → patch (original):

- 100 draws: shapes **4.875 → 4.416 (8.333)**; lines
  **4.375 → 4.042 (7.750)**; outlines **4.792 → 4.208 (7.500)**.
- 1,000 draws: shapes **38.875 → 41.250 (55.375)**; lines
  **36.792 → 45.333 (61.917)**; outlines **36.208 → 48.333 (56.334)**.
- 10,000 draws: shapes **433.542 → 438.917 (517.084)**; lines
  **411.000 → 456.541 (523.334)**; outlines **357.292 → 368.000 (432.167)**.

The 10k parent/patch/original ranges of invocation medians, in microseconds:
shapes **329.167–550.166 / 303.584–517.500 / 319.584–570.750**;
lines **308.458–670.875 / 279.292–555.083 / 305.375–618.959**;
outlines **303.416–524.292 / 276.708–501.625 / 284.833–521.959**.
Every count/workload/reference-side range is in
[summary.csv](session-clip-fastpath/summary.csv).

At 10k, patch versus parent changes by **+1.24%, +11.08%, +3.00%** for
shapes/lines/outlines; versus original by **−15.12%, −12.76%, −14.85%**.
The paired parent-to-patch changes are shapes **−7.77%, −5.94%, +1.24%**,
lines **−9.46%, −17.26%, +11.08%**, outlines **−8.80%, −4.32%, +3.00%**.
Against original, paired lines are **−8.54%, −10.32%, −12.76%**. First-pair
10k line recording drops **29.166 µs**; third-pair recording rises **45.541 µs**.
These absolute costs describe CPU recording in this harness, not frame latency.
At 100, shapes/lines improve against parent in all pairs; outlines in two.
At 1,000, all three improve in two pairs and regress in the third.

10k controls, parent → patch (original), in microseconds: shape batches
**232.375 → 287.500 (316.667)**, line batches **226.250 → 287.041 (300.958)**,
nested rounded batches **217.250 → 269.125 (329.708)**. These are
**+23.72%, +26.87%, +23.88%** versus parent. Nested paired changes are
**+1.21%, +36.21%, +23.88%**. The mixed scoped control changes
**3610.292 → 3248.166 (3787.875)**, with paired **+14.01%, −0.84%, −11.20%**.
These conflicting controls are material; neither the original-relative wins nor
the parent-relative losses establish causation on this host.

All **48 pixel comparisons per primitives invocation** passed (nine invocations).
The fixed binaries also passed **18 raw/wrapped rendering, 16 path, and 16 point**
pixel checks at upload-boundary counts, using the prior reproduction arguments.

`mixed_ui --frames 360 --warmup 60 --in-flight 3` passed full-static equality,
all six region probes, validation, and all phases. Static/churn/recovery
record-upload-submit p50 were **568.500 / 588.042 / 562.167 µs**, p95
**788.250 / 752.375 / 699.375 µs**. Warm static created no new clip bindings
(150 total). Churn grew idle storage **458,752 → 1,638,400 bytes**; recovery
released **18 pages** and returned to **458,752 bytes / 7 pages / 150 retained
bindings** (300 cumulative creations). This is a runtime/recovery check, not a
paired mixed-UI performance comparison.

## Reproduction

Build with `cargo bench -p astrelis --locked --bench primitives --bench mixed_ui
--bench rendering --bench paths --bench points --no-run`, copy the executables
to the frozen paths in [run.py](session-clip-fastpath/run.py), then run it only
after builds finish. It verifies reusable parent/original provenance before
measurement. Adjust local archive/binary paths for portable replay. Run
`python3 docs/performance/session-clip-fastpath/analyze.py` to regenerate summary.
Exact final source hashes and executable hashes are in `metadata.json`;
`source.patch.log` records the delta from parent without hook normalization of
patch context whitespace.
