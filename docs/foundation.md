# Rendering foundation acceptance

Astrelis should let an application express ordinary 2D and 3D rendering clearly,
extend it with custom GPU work, and understand the cost. RXUI can build UI policy
on that foundation; Astrelis does not own an event loop, scene graph, or display list.

This milestone establishes consumer examples, regression checks, and a reproducible
performance baseline. Adding those checks does not itself mean every performance
target has passed.

## API acceptance

| Condition | Evidence |
| --- | --- |
| A short route to a first draw, with useful defaults | `triangle`, plus the crate's first-draw documentation |
| Resources, per-draw data, frames, and passes have distinct ownership | `TextureBinding` / `TextureDraw`; frame and pass borrowing doctests |
| Applications control window creation, scheduling, resize, and presentation | Every example contains its own window/event loop; acceptance examples pause, suspend, resize, and change MSAA |
| Independent renderers share passes and frames | Renderer integration tests; framebuffer compositing examples |
| Creation, uploads, preparation, and submission have recognizable boundaries | Resources are retained in application state; explicit preparation and `finish()` |
| Invalid combinations fail usefully without committing abandoned work | Device/layout/attachment tests, shader diagnostics, submission-order regression tests |
| Custom GPU work fits alongside the wrappers | Custom materials, vertex streams, bindings, attachment descriptors, wrapped pass access, and encoder access |

The main consumer checks are two single-file examples:

```sh
cargo run -p astrelis --example ui_workload
cargo run -p astrelis --example scene_3d
```

`ui_workload` reuses two images/bindings while changing rectangle placement and
tint, clips a grid, restores the clip for subsequent draws, renders into a layer,
and composites its live sampled output onto a window. Resizing replaces layer
storage without application rebinding. Space changes layer MSAA between 1x and
4x when supported; P pauses animation.

`scene_3d` draws nine indexed cubes with Uint16 indices and a separate instance
stream. A custom WGSL shader reads an application-owned camera uniform and
per-instance object data, projects into perspective, and uses depth testing.
Space changes surface MSAA between 1x and 4x when supported; P pauses animation.
Geometry, materials, and bind groups persist across frames.

These are rendering workloads, not a widget framework or scene framework. They
require only astrelis, winit 0.30, and pollster 0.4 when copied into an application.
Each owns its lifecycle and error handling. Neither contains smoke-test flags or
shared example infrastructure.

## Performance acceptance

- Initially aim for approximately **10–15% or less additional CPU recording cost**
  against equivalent optimized direct-wgpu code on comparable prepared paths.
  Keep absolute costs beside percentages, especially at timer-scale durations.
  Do not silently substitute total frame time for recording time to pass this target.
- A fixed prepared workload should create no new pipelines, image bindings,
  samplers, or GPU buffers after warm-up. Parameter updates should reuse storage.
- Draw recording and uploads should scale approximately linearly with draws and
  bytes; compatible textured rectangles should have a batching path.
- Equivalent workloads should produce comparable GPU work. Passes, draws,
  attachment operations, and uploaded bytes must be matched before interpreting timings.
- Repeated frames and resource replacements should not cause unbounded retention.
  Pool capacity retained after a peak workload should be deliberate and documented.
- Track preparation, recording, upload/submission, and GPU execution separately
  where reliable. Sustained changes outside normal measurement variance need review.

The headless harness is in `crates/astrelis/benches/rendering.rs`:

```sh
cargo bench -p astrelis --bench rendering -- \
  --counts 100,1000,10000 --samples 40 --warmup 8 \
  > rendering.csv 2> rendering.log
```

Use `--help` for settings. The harness rejects debug builds and invalid settings.
CSV goes to stdout; adapter/settings, startup observations, and equivalence checks
go to stderr. Run on an otherwise idle machine, repeat the whole suite, and retain
both outputs with the commit/toolchain/OS. Compare like hardware, backend, build
profile, validation configuration, and workload. There is no universal CI timing
threshold yet.

The five original workloads cover repeated mesh draws, alternating mesh buffers,
prepared texture draws, individual dynamic texture draws, and batched dynamic
textures. Four additional variants measure scoped mesh, prepared texture, dynamic
texture, and batched texture drawing; scope setup remains inside the recording timer.
Both sides use the same device, shaders, formats, geometry, instance data, blend
state, scissor/viewport, and store operations. Direct wgpu binds unchanged state
once and changes vertex/index buffers only when necessary. Both dynamic paths
pack their input and upload 64-byte instances in reusable 64 KiB pages; batches
split at 1,024 instances. Astrelis also performs its public API validation and
resource tracking; that cost intentionally remains in the measurements.

Every workload/count checks all output pixels against the reference before timing,
and rejects an empty image. Warm-up samples are omitted; raw/wrapped order alternates
within each pair. Each submission completes before the next sample, so the suite
isolates CPU work rather than measuring throughput with multiple frames in flight.
The 16x16 destination minimizes raster work and does not represent a real scene's
GPU cost.

CSV phases:

| Metric | Included work |
| --- | --- |
| `begin` | Encoder/frame acquisition and pass creation, including viewport/scissor setup |
| `record` | Draw calls and CPU parameter packing; prepared pipeline cache lookups and API validation |
| `finish` | Pass teardown, queued parameter uploads, encoder finish, submission, and managed bookkeeping |
| `cpu_total` | Sum of the three CPU phases for each sample, summarized independently |
| `completion_wait` | Host wait until submission completion; **not GPU execution time** |

Each metric reports median, p95, absolute median difference, and relative median
difference. Preparation observations are separate and are **not** matched cold
pipeline benchmarks: the backend can reuse compiled shader state for the later
raw pipeline. Binding creation throughput, allocation counts, peak-memory recovery,
real-scene GPU timing, and frames-in-flight throughput remain separate measurements.

The upload reuse regression test checks actual GPU buffer identities, retained CPU
page capacities, scratch capacity, and pipeline identity over fixed batches,
abandoned recordings, and resizes. It does not claim to instrument every allocator
or GPU resource creation site. Existing tests cover prepared bindings/materials and
live source replacement. These are targeted structural checks, not a full memory profiler.

## Gate status and follow-up

The [initial baseline](performance/baseline.md) established substantial recording
cost in individual calls. The [scoped drawing experiment](performance/scoped-drawing.md)
amortizes immutable checks and pipeline selection without removing the convenience
API. Prepared scopes approach direct-wgpu recording cost; dynamic validation and
individual convenience calls still have measurable overhead. See the measured
ranges and outliers before treating the percentage target as universally passed.

The API checks now include restoration after raw/other renderer access, renderer
reuse through mesh/prepared scopes, pixel viewport revalidation, custom geometry
and application bindings, invalid batch rejection, and live-source snapshots.
Scope methods do not merge or reorder draws automatically. Existing instancing
and `draw_many` remain the way to reduce GPU command count when compatible.

Future changes should preserve these checks and run the same reference workloads.
Record both CPU recording and total CPU cost, and use absolute differences alongside
percentages near the timer/noise scale. Changing ranges/data still needs validation.

A Vulkan or DX12 baseline and reliable real-scene GPU measurements are still needed
before making portability or GPU-performance claims. Collect those on available
hardware rather than inferring them from a Metal run.
