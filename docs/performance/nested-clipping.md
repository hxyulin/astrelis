# Nested rounded clipping: Apple M5 Max / Metal

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
