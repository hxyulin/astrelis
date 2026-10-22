# Nested rounded clipping: Apple M5 Max / Metal

The repeated final-parent comparison is in
[the pre-push regression report](final-regression.md), including three alternating
revision pairs at 100/1,000/10,000 primitives and a comparable scoped-draw slowdown
that remains unresolved under shared-host noise.

The nested clipping change keeps all transformed rounded boundaries active, adds
point-renderer coverage, and exposes the same shader interface for custom work.
Unclipped draws upload no clip data. Their shared disabled binding is cached so
consecutive compatible draws still merge. Nonempty intersections upload one
uniform per changed stack/viewport, using the existing leased upload pages; the
legacy innermost vertex record remains available to custom texture shaders.

Measured on Apple M5 Max integrated graphics, Metal, macOS 27.0.1 (26A434), arm64,
rustc 1.99.0, wgpu 30.0.1, optimized Cargo bench profile. The 64×64 RGBA8 workload
uses one sample. These are CPU recording/total measurements, not GPU execution
timings. Other work was running on this machine; one short run per revision with
12 alternating paired samples and 3 warm-up pairs cannot establish statistical
significance or a GPU performance claim.

Baseline source: `5e5ea9df12f3539661b8d487a83b907e63641bfa`. Both runs use this
change's `primitives` benchmark source, including the added nested case. The nested
case places two rounded clips around the whole attachment so its pixels match the
unclipped reference on either revision. Pixel regressions separately test actual
intersecting boundaries; the baseline's nested semantics are incorrect there.

Command for each revision:

```sh
cargo bench -p astrelis --bench primitives --locked -- \
  --counts 100,1000 --samples 12 --warmup 3
```

All 32 workload/count pixel equivalence checks passed for each revision. At 1,000
shapes, median CPU recording time in microseconds changed as follows:

- Direct shape batch: 43.104 → 48.042.
- Unclipped Painter shape batch: 43.125 → 39.729.
- One rounded clip around a Painter shape batch: 47.541 → 59.062.
- Two nested rounded clips around a Painter shape batch: 48.166 → 59.084.
- Unclipped mixed Painter workload: 568.645 → 601.688.

The extra uniform preparation/binding costs CPU time for clipped workloads. The
fragment shader evaluates each active boundary; deeper stacks increase fragment
work. No extra attachment, pass, or clipped pipeline variant is created. Capacity
is checked against device buffer/uniform limits and capped at 64; smaller limits
produce smaller shader arrays, including a zero-capacity unclipped fallback.

Raw evidence: [baseline CSV](nested-clipping-metal-baseline.csv),
[baseline log](nested-clipping-metal-baseline.log),
[current CSV](nested-clipping-metal-current.csv),
[current log](nested-clipping-metal-current.log). Logs retain adapter details and
all output-equivalence results. CSV includes median/p95 recording and CPU totals.

## Restricted imported devices

The integration repair disables analytic clipping when the enabled device has
fewer than two bind groups or two fragment uniform buffers per stage, reserving
one uniform for brushes. Capacity zero omits group one from built-in layouts,
WGSL and draw bindings. Nonempty clip requests still fail atomically. Unclipped
points retain their original one-group storage-buffer requirement.

Reran the command above on the same Apple M5 Max / Metal, macOS 27.0.1 (26A434),
arm64, rustc 1.99.0 and wgpu 30.0.1, with 12 samples and 3 warm-up pairs.
All 32 workload/count pixel equivalence checks passed. The previous current-run
and repair recording medians at 1,000 shapes were 39.729 → 47.001 µs for
unclipped Painter batches, 59.062 → 36.876 µs with one rounded clip, and
59.084 → 49.541 µs with two clips.
These short runs overlapped other compilation/test work and do not establish a
performance change. GPU execution timing was not measured. The benchmark uses
ordinary device limits; imported-device pixel/error-scope regressions separately
cover one bind group and zero/one fragment uniforms, plus ordinary-device nesting.

Raw evidence: [repair CSV](restricted-clipping-metal.csv) and
[repair log](restricted-clipping-metal.log).
