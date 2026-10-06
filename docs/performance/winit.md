# Window lifecycle and runner performance

The native runner adds a small CPU dispatch cost while retaining Astrelis's
application-owned passes, pipelines, and resources. Idle OnDemand produces no
periodic redraws. The measurements below separate synthetic scheduling work from
native prepared-frame work; native totals are dominated by surface acquisition
and compositor/display pacing.

## Native prepared frames

Measured on Apple M3 Pro / Metal, optimized Rust 1.98.1, with the root workspace's
Cargo.lock. Each process shows a 128×128 physical surface, single sampling, no
depth/stencil, one clear/store pass and 100 scoped draws of the same indexed mesh.
Pipelines and resources persist. Each run excludes 32 warmup frames and collects
320 successful submissions. Three runs alternate direct/runner order. The window
uses AlwaysOnTop to keep unrelated desktop windows from pausing acquisition.

The direct variant owns a winit ApplicationHandler and WindowContext; the managed
variant uses the real Runner/Handler. Both use the same preparation, recording,
pass, mesh, and presentation notification. This comparison measures the optional
driver over the low-level helper, not raw wgpu against Astrelis.

Ranges of the three per-run medians:

| CPU stage | Direct loop | Managed runner |
|---|---:|---:|
| Native event callback through preparation | 0.209–0.250 µs | 0.458–0.833 µs |
| Acquisition through render callback entry | 8,150–8,206 µs | 8,158–8,207 µs |
| Pass and draw recording | 7.38–10.63 µs | 7.08–10.92 µs |
| Notification, finish, and submission hook entry | 87.38–106.00 µs | 82.54–109.33 µs |
| Total sampled interval | 8.272–8.300 ms | 8.286–8.302 ms |

The paired dispatch/preparation median differences are approximately 0.25–0.58 µs.
Total frame medians remain within normal acquisition/presentation variation here;
they cannot isolate the driver's intrinsic CPU cost. Acquisition can block on
surface availability/backpressure even though the integration introduces no
explicit GPU completion waits. These timings are not GPU execution time, display
completion latency, or an application FPS guarantee.

The interval starts inside the raw native-event callback and ends at submission
notification. It includes preparation, acquisition, recording and finish, but
excludes the outer winit event dispatch, before-wait scheduling, callbacks after
the sample, and startup. Timing and sample storage are benchmark-only; the vector
is reserved before rendering. Cold pipeline creation is excluded. Percentiles and
adapter logs are retained in the [raw artifacts](winit/metadata.json).

## Isolated scheduling

The scheduling benchmark includes the driver's actual private Schedule source. It
models one continuously drawing window and other idle windows in a HashMap with
integer keys, consuming a redraw, recording successful submission state, and
performing the deadline/native-request eligibility scan. It uses a cached Instant
and excludes all native calls, WindowContext state, real callbacks, rendering,
allocations during the timed loop, and OS event processing. It is a narrow
bookkeeping measurement rather than the complete runner cost.

Each of three runs collects 40 batches of 10,000 iterations, after warmup:

| Registered windows | Range of per-run median bookkeeping |
|---:|---:|
| 1 | 8.25–10.42 ns |
| 4 | 10.90–13.18 ns |
| 16 | 25.16–38.43 ns |
| 256 | 344.67–352.94 ns |

The registry performs O(1) ID lookup and O(number of registered windows) scanning.
Native redraw requests remain coalesced. Code inspection confirms that the driver
does not clone window Arcs or RenderFormat color vectors during rendering, query
native dimensions on ordinary frames, allocate an additional recording buffer,
open a pass, create a pipeline, submit another command buffer, or wait for GPU
completion. This is not an allocator profile of winit/wgpu: their backend work and
application allocations remain outside that claim. Lifecycle/configuration changes
may allocate attachments, caches, queues and registry entries.

## Lifecycle validation

The [native validation log](winit/native-validation.log) comes from a temporary
independent consumer, with [its source snapshot](winit/native-validation.rs)
retained for review. It creates two surfaces sharing one graphics device, draws a
shared mesh, and tests:

- Idle OnDemand and independent explicit redraws.
- Invalidation during prepare and submitted; Continuous and non-spinning Skip.
- One-shot earliest deadlines surviving immediate draws and unavailable windows.
- Hidden windows, cached DPI, duplicate resize notifications and zero-sized targets.
- Supported 1x/4x MSAA, rejected settings, repeated suspend/resume and generation.
- Depth24PlusStencil8 settings preserved across recreation.
- A close queued during prepare preventing acquisition, and last-window shutdown.
- Errors in resumed, window_created, prepare, render, submitted, window_closed,
  and exiting, with cleanup continuing and the first error retained.
- Inspection of the borrowed handler after run returns.

The fixture keeps windows unobscured and lets native startup settle before its
idle assertion. OS redraw/exposure events are valid invalidations; they must not
be confused with periodic runner redraws. A separate normal-window native UI
check used the standalone runner_multi_window example for rendered output,
MSAA/animation keys, close veto, independent close, and last-window exit. Neither
example contains a smoke mode or shared support module.

Recreation was deliberately triggered through suspend/resume; an actual backend
SurfaceLost event was not induced. Recovery pacing, bounded losses, unavailable
states, and invalidations are covered by pure scheduler regressions. Device loss,
mobile suspension, Windows/Linux runtime behavior, and browser event-loop support
are not established by these macOS checks.

The [AccessKit consumer](winit/accesskit-consumer.rs) compiles against the exported
Handler/AppContext with accesskit_winit 0.30.0 and an actual typed proxy, without a
message envelope. RXUI was not modified. The workspace passes 143 unit/GPU tests,
28 rustdoc tests, strict native Clippy, documentation with warnings denied, and
all standalone example builds. The wasm library compile passes; the native driver
is deliberately absent there. An additional wasm Clippy attempt reports existing
core Arc-with-non-Send/Sync warnings for web GPU handles; that target is not claimed
to pass strict Clippy.

## Reproduction and artifacts

Run native variants in separate processes while the test window is unobscured.
Do not run GPU tests, other rendering benchmarks, or builds concurrently with the
measurement. Closing the benchmark early is reported as incomplete.

```sh
cargo bench -p astrelis-winit --bench lifecycle -- --direct
cargo bench -p astrelis-winit --bench lifecycle -- --runner
cargo bench -p astrelis-winit --bench scheduling
```

Native recordings are lifecycle-direct-{1,2,3}.csv and lifecycle-runner-{1,2,3}.csv;
each has an adjacent adapter log. Pure scheduling data is scheduling-{1,2,3}.csv.
[Metadata and source fingerprints](winit/metadata.json) identify the measured tree,
toolchain and settings. Native validation is a disposable consumer binary, not a
library test that opens windows automatically. To reproduce it, copy the snapshot
into a temporary Cargo binary depending on astrelis-winit, then run success or the
named error modes. The AccessKit snapshot is a temporary library consumer with the
additional pinned accesskit_winit dependency.
