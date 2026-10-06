# Dense data rendering

`PointBuffer` retains XY samples on the GPU. `PolylineRenderer` and `MarkerRenderer`
read the same storage, expand geometry in the vertex shader, and record into any
compatible wrapped pass. They also integrate with Painter. Astrelis owns drawing
resources and validation; applications own chart data, scales, axes, selection,
visible ranges, downsampling, and interaction.

## Storage and updates

```rust
let mut points = graphics.create_point_buffer(PointBufferOptions::new(1_000_000).ring())?;
points.replace(&initial_samples)?;
points.append(&incoming_samples)?;
points.write(logical_start, &corrected_samples)?;
```

A `Point2D` occupies eight bytes. Ordinary coordinates must be finite; use
`Point2D::gap()` for a missing sample. A gap breaks both neighboring connections
and draws no marker. Large f64 timestamps should be rebased before converting to
f32; the library does not silently change the application's coordinate system.

Capacity is fixed and checked against storage-buffer/device limits. There is no
implicit reallocation. Linear overflow returns an error. Ring append retains the
newest capacity samples and evicts the oldest; all public ranges and range writes
index the resulting logical oldest-to-newest sequence. Oversized append validates
its whole input but uploads only the retained suffix. Ordinary append/range writes
use at most two physical writes across the ring boundary and never move old data.
`PointAppend` reports uploaded points and evictions. `clear()` only resets metadata.
`uploaded_bytes()` and `write_count()` expose cumulative update work.

Updates validate the whole input before changing metadata or queueing any write.
Validation scales with the supplied data, not with retained capacity. The buffer
keeps conservative coordinate bounds without retaining a CPU sample mirror;
partial writes and appends expand these bounds, while replace/clear reset them.
An extreme transform can fail this conservative validation even when its selected
subset would fit. Normal draw validation does not scan the sample buffer.

**Updates are live queue writes, not recorded snapshots.** They execute before the
next submission, so a write between two recorded draws affects both draws. Update
before recording/submitting a frame; separate buffers preserve independently
updated recordings. Already submitted frames obey queue ordering. Raw buffer
access exposes STORAGE/COPY_SRC/COPY_DST plus the physical head, and bypasses
finite/bounds validation and logical-length tracking. Keep those contracts when
using compute or encoder copies.

## Drawing and screen geometry

```rust
let mut lines = PolylineRenderer::new(&graphics);
let mut markers = MarkerRenderer::new(&graphics);
lines.prepare(&target.render_format())?;
markers.prepare(&target.render_format())?;
// One view shared by both renderers; samples stay in their original data space.
let view = Transform2D([scale_x, 0., 0., -scale_y, origin_x, origin_y]);
lines.draw(&mut pass, &points, PolylineDraw::new([0.2, 0.7, 1., 1.])
    .transform(view).width_pixels(1.5).join(LineJoin::Bevel)
    .range(visible_start..visible_end))?;
markers.draw(&mut pass, &points, MarkerDraw::new([1.; 4])
    .transform(view).radius_pixels(2.).shape(MarkerShape::Circle)
    .range(visible_start..visible_end))?;
```

DrawSpace and affine transforms follow the other renderers. Width and marker radius
are deliberately **physical screen pixels**, after geometry transforms and viewport
conversion; zooming does not enlarge them. Apply DPI to these values explicitly
when the application wants logical-pixel sizes. Markers stay screen aligned.
Colors are linear straight RGBA, producing premultiplied source-over output.
Circle, square, and diamond markers share the same storage and renderer.

Polylines support butt/square/round caps and miter/bevel/round joins. Miter-limit
fallback uses bevels. A selected range starts/ends its own stroke; include adjacent
samples when continuity beyond a visible range is needed, then clip the pass.
Gaps and reversals break connections; zero-length segments produce no geometry.
Ordinary connected corners share their boundary so translucent joins blend once.
Round joins use an eight-segment fan: large widths can reveal its approximation.

These are fast series strokes, without the geometric union of `PreparedPath`.
Self-intersections, reversals, and very short segments around sharp corners can
overlap and blend repeatedly. Use PathRenderer when exact translucent stroke union
matters. Marker overlaps also blend in logical sample order. Analytic coverage is
available with or without target MSAA; hard edges remain selectable. PipelineOptions
supports the shared blend/depth/stencil policies and coverage-aware stencil writing.
There is no per-point color/radius or custom point shader in this milestone.

Direct `draw` and `bind(pass, points)` scopes coexist with raw/custom rendering.
Scope `pass()` access supports clipping and interleaving; subsequent point draws
restore required state. A drawable selected range records one draw and an 80-byte
parameter record. Warmed workloads reuse the sample buffer, storage bind group,
pipelines, and recording parameter pages. Empty ranges, zero-size draws, and zero-area viewports upload no
parameters. Vertex/storage work remains proportional to the selected sample count;
there is no implicit culling or data reduction.

Point pipelines require vertex-stage storage support and compatible limits. Point
creation/preparation return errors on unsupported devices. Painter constructs the
optional renderers lazily; ordinary `prepare` remains usable without point support.
Call `painter.prepare_points(format)` before their first use. A PaintSession offers
`draw_polyline` and `draw_markers`; its transform affects sample positions while
width/radius remain physical pixels.

## Standalone streaming chart

Run:

```sh
cargo run -p astrelis --release --example streaming_chart
```

The single file owns its window, event loop, rolling original data, and reduction
hierarchy. 1/2/3 select 10K/100K/1M capacity; wheel zooms, drag pans, F follows the
tail, D toggles reduction, M toggles markers, P pauses, A toggles analytic coverage,
and Space cycles supported MSAA. It appends 256 samples per update, uploads only
new raw points, retains gaps and spikes, and caches reduced output while unchanged.

Reduction is explicit application policy for this example's monotonically spaced
X values. A hierarchy summarizes 32-sample blocks and updates affected blocks.
Visible pixel buckets retain first/minimum/maximum/last samples in chronological
order. Buckets containing a gap retain their original points, preserving exact
connection breaks. Original samples remain available for interaction or zooming.
This avoids rescanning the whole retained history every frame, but queries still
scan partial blocks and upload changed reduced output. The block size trades CPU
query cost for summary storage (about 2.5 MiB at one million points in this example).
Dense gaps can make reduced
output approach the original size. Irregular X spacing requires different bucket
selection; the example is not a general chart framework.

See [measurements and their limits](performance/points.md).
