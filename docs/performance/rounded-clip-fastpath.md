# Rounded clip recording fast path

Follow-up: [primitive session clip binding](session-clip-fastpath.md) adds lazy
session restoration and a new bounded comparison against `856caef` and original.
The verdict below describes this earlier experiment.

Push-blocker verdict: **not cleared**. The focused fix removes the repeated
immutable-limit query and redundant binding clones. Three paired runs improve
median scoped recording versus merged parent `adb79c6`, especially outlines.
However, fixed 10k lines remain slower than original `5e5ea9d` in all three
same-numbered comparisons, and nested/batch controls also move substantially.
These loaded-host results do not justify declaring the original performance
regression resolved or claiming a stable general speedup. Integrate the fix as
an improvement; retain the performance blocker for a quieter repeat/investigation.

## Change and regression coverage

`GraphicsContext::from_wgpu` computes rounded-clip capacity once from the
**device's enabled limits**, including application-restricted imports. All
library constructors use this path. Each context owns its cached value; clones
copy it, and unrelated devices cannot overwrite it. Public APIs are unchanged.

`RenderPass::bind_rounded_clips` compares the borrowed disabled/active group
against tracked group one before cloning. An actual change clones once, flushes
pending drawing in order, and records the binding. Repeated bindings create no
handle clone or GPU bind command. Empty-stack bindings remain shared immutable
resources. Full-stack/viewport changes still invalidate stack uploads; wrapped
application group-one replacements and dynamic offsets, and raw `as_wgpu`
access (including renderer raw-pass access), still invalidate tracking.
Applications retain scheduling, draw order, resource ownership, and raw wgpu access.

The new pixel regression repeatedly binds disabled and active stacks, changes
full-stack depth, returns to disabled, and interleaves an application-owned group
with a different dynamic layout at zero/nonzero offsets. Its output must equal
the uninterrupted reference and retain both clipped and unclipped pixel probes.
Restricted-device tests now also check cloned capacity and independence from the
full-capability context. Existing pixel regressions cover viewport changes, raw
group overrides, full-stack intersections, every built-in renderer, and zero
capacity imports. The tests establish correctness, not a timing threshold.

Before timing: focused rounded-clip tests passed (5), full
`cargo test --workspace --locked` passed **214 unit tests + 30 doctests**, with
one ignored test; Clippy for all workspace targets with warnings denied,
documentation with warnings denied, formatting, and diff checks passed.
[Exact logs and source patch](rounded-clip-fastpath/) are retained.

## Paired experiment

Apple M5 Max (18 CPU cores, 128 GiB), Metal integrated GPU, macOS 27.0.1
(26A434), aarch64, rustc/cargo 1.99.0, wgpu 30.0.1, release profile, default
wgpu validation. One-minute host load across recorded snapshots ranged
**34.11–52.98**. Other projects were active; no builds were launched from this
checkout during measurements. Parent Astrelis builds were requested paused.
Host snapshots retain process names/CPU usage, not command-line arguments.
This is a heavily loaded shared host, not an isolated CPU/GPU experiment.

All executables were compiled and copied to frozen paths **before** timing.
Before/after use identical `adb79c6` benchmark source and Cargo.lock. Original
`5e5ea9d` uses that same primitives source (SHA-256
`c1f81c30ab36a6ea3c304d9678c73a227022a4a44eee6e1a1a3c0615b46c0dc5`, also
used in the preceding study). Its older manifests require lock normalization:
local `log` edges and inactive GLES packages are removed; retained external
package versions/checksums are identical. The exact
[normalization diff](rounded-clip-fastpath/original-lock-normalization.patch)
is saved. Thus the original comparison is contextual, not a claim that upstream
GPU diagnostics/text/recovery changes have been isolated or reverted.

Each invocation used `--counts 100,1000,10000 --samples 25 --warmup 5`.
Process order was before/after/original, after/before/original,
before/after/original. The harness additionally alternates reference/variant
samples within every workload. All 48 pixel comparisons per invocation passed.
No completion waits are inside the reported CPU intervals; no GPU execution
timing is claimed. CSVs retain the harness's exact median/p95 output, without
outlier filtering; the harness does not emit individual sample observations.

The following numbers are medians of the three **variant record medians**, in
microseconds, written as merged-before → fixed (original):

- 100 draws: shapes **9.917 → 7.125 (8.167)**; lines
  **10.500 → 8.500 (9.625)**; outlines **9.541 → 7.500 (7.666)**.
- 1,000 draws: shapes **85.458 → 56.542 (67.459)**; lines
  **81.083 → 66.834 (66.167)**; outlines **78.875 → 67.167 (65.417)**.
- 10,000 draws: shapes **841.000 → 664.667 (671.084)**; lines
  **890.458 → 745.167 (586.917)**; outlines **828.291 → 538.541 (633.959)**.

At 10k these are −21.0%, −16.3%, −35.0% versus merged-before, and −1.0%,
+27.0%, −15.1% versus original. Pairwise before→fixed percentage changes are
shapes **−28.3%, +19.4%, −21.4%**, lines **−16.8%, +22.8%, −18.3%**, and
outlines **−35.0%, −18.2%, −37.9%**. Shapes and lines improve in all three pairs
at 100 and 1,000; outlines improve in two of three at 1,000. The earlier uniform
all-count/all-run slowdown is no longer reproduced, but 10k lines retain a
concerning original-baseline signal. Counts and workload order share a process;
these aggregate medians are not independent observations or a confidence interval.

Control medians at 10k also vary: shapes batch **444.125 → 443.584 (393.375)**,
lines batch **402.125 → 441.167 (388.792)**, nested rounded batch
**364.958 → 447.875 (318.292)**. Fixed versus merged is −0.1%, +9.7%, +22.7%;
versus original is +12.8%, +13.5%, +40.7%. Nested fixed is slower than merged in
all three pairs. This fix does not explain those movements. They prevent a
clean causal performance conclusion and must not be hidden by the scoped wins.
[summary.csv](rounded-clip-fastpath/summary.csv) contains every count/workload,
including reference sides and each before/fixed pair.

## Fixed merged-code runtime checks

`mixed_ui --frames 360 --warmup 60 --in-flight 3` passed full-static equality,
clip/image/path/text/point region probes, all phases and validation. Static,
churn, recovery record/upload/submit p50 were **396.125 / 426.875 / 435.250 µs**;
p95 **495.625 / 529.292 / 547.584 µs**. These are a runtime/resource check, not
a paired mixed-UI speedup comparison. Warm static created no new clip bindings
(150 retained); the churn spike grew idle storage 458,752 → 1,638,400 bytes;
recovery released 18 pages and returned to 458,752 bytes/7 pages/150 bindings.

Raw/wrapped rendering and paths ran with `--counts 1,1025 --samples 5 --warmup 1`;
points with `--counts 256,4097 --samples 5`. All **18 rendering, 16 path, 16 point**
pixel checks passed, including upload boundaries and both path coverage modes.
These five-sample checks establish runtime compatibility, not performance.

## Reproduction

[metadata.json](rounded-clip-fastpath/metadata.json) records exact invocations,
source/lock/executable hashes and machine metadata. [run.py](rounded-clip-fastpath/run.py)
retains the exact measurement driver and frozen executable paths used locally;
build corresponding revisions with `cargo bench -p astrelis --locked --bench
primitives --no-run` and copy each executable before invoking it. Original was
built from a separate archive with the shared benchmark source and normalized
lock, using `--offline`; its build log is saved. Fixed runtime executables were
built together with `--bench primitives --bench mixed_ui --bench rendering
--bench paths --bench points --no-run`. For portable replay, adjust the driver's
binary directory and evidence directory. Run `python3
 docs/performance/rounded-clip-fastpath/analyze.py` to regenerate the summary.
No PR, push, performance acceptance threshold, or cross-platform claim is made.
