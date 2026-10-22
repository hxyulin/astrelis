# Material pipeline cache reclamation

`cargo bench -p astrelis --bench pipeline_cache --locked` measures warm preparation,
material creation/preparation/drop, and explicit pruning. It uses one persistent
material and 16 transient materials per batch, with 20 samples per renderer and
10,000 warm lookups per sample. Each sample checks 17 cached variants before
pruning and exactly one live variant afterwards. Mesh and texture pixel readbacks
are checked before and after the measured workloads. Statistics scans, rendering,
submission, and readback are outside the lookup timer. Prune timing includes
releasing pipeline handles and shrinking the hash table; it does not wait for GPU
completion. Material churn includes shader/layout/pipeline creation overhead.

The run used Apple M5 Max / Metal, macOS 27.0.1, rustc 1.99.0, wgpu 30.0.1,
optimized builds, and default wgpu validation. Evidence: [CSV](pipeline-cache-m5-max.csv),
[log](pipeline-cache-m5-max.log), and [metadata](pipeline-cache-m5-max-metadata.txt).
Median warm preparation was 36.59 ns for mesh and 50.26 ns for textures with one
entry; with 16 additional stale entries it was 36.58 ns and 50.23 ns respectively.
Pruning cost 940 ns per stale mesh variant and 1,339 ns per stale texture variant
at the median. These are CPU cache-maintenance measurements, not GPU execution
or allocation-size measurements. Tail timings were noisy on the shared machine;
the CSV includes p95. There is no paired pre-change run, and no speedup claim.

Normal lookup still uses the existing hash-map entry lookup and returns a borrowed
pipeline handle. Lifetime checks and table scans occur only in explicit stats or
prune calls. The added bookkeeping is one shared lifetime marker per material and
one weak marker per cached variant; hits do not update counters or weak references.
The regression tests also exercise two variants per transient material over 32
maintenance cycles, retained material clones, complete storage reclamation, held
scopes, and pending commands after renderer destruction.

The existing `rendering` benchmark was attempted with its standard 100/1,000/10,000
counts, 40 samples, and 8 warmups. Its raw texture shader failed setup with
`no definition in scope for identifier: clip_varying`, before any timing. That
benchmark's clipping integration is outside this change; the focused benchmark
uses public renderer APIs and needs no private shader reconstruction.
