# Retained vector paths

`Path` owns immutable, shared CPU geometry, independent of devices and paint.
`PathRenderer` owns its shader, pipeline variants, tessellator scratch, and dynamic
parameter scratch. `PreparedPath` owns immutable device-bound vertex/index buffers.
`PathDraw` owns color, transform, draw units, and antialiasing choice. Applications
continue to own passes, attachments, draw order, and submission.

## Construction and preparation

Start with `Path::builder()`. `move_to` starts a contour and ends the previous open
one without closing it. `line_to`, `quadratic_to`, and `cubic_to` extend it; `close`
closes it. Commands after a close require another `move_to`. `build` consumes the
builder and validates finite coordinates and command order, returning an indexed
`InvalidPath` error instead of panicking. Empty paths are valid.

`prepare_path(&path, PathOptions::new())` prepares a nonzero fill. Even-odd fills
ignore contour orientation; nonzero fills allow holes through opposite winding.
Filling implicitly closes open contours. Selecting `.stroke(PathStroke::new(width))`
prepares the stroke instead, preserving open ends. Prepare twice when both are
needed. Stroke options include separate start/end caps, miter/bevel/round joins,
and a finite miter limit of at least one. The public miter limit is center-to-tip
extension divided by half-width, equivalent to the usual SVG ratio. A limit of
one bevels a right-angle join. A tested conversion handles Lyon 1.0.22's internal
full-width convention.

Preparation uses `lyon_tessellation` 1.0 (1.0.22 in Cargo.lock) fill/stroke tessellators.
Stroke triangles are consistently oriented and filled as a nonzero union, removing
overlap before shading. Translucent crossings therefore blend once. This extra
CPU preparation work is explicit, rather than an extra render pass or stencil
attachment requirement. Tessellation diagnostics return `PathTessellation`.
Shared triangle edges are canceled before union tessellation, so internal
diagonals do not inflate intersection work or retained geometry.

Tolerance defaults to 0.1 in path units. It bounds curve approximation before a
draw transform; it does not promise a pixel error after arbitrary scaling. Choose
a smaller tolerance explicitly for large zooms. Small tolerances and complex
stroke intersections increase CPU work and geometry size. Preparation is synchronous
and includes geometry allocation/upload. Retain results; identical calls deliberately
produce independent resources without a hidden global cache.

Settings and final geometry/buffer sizes are checked before allocating GPU geometry.
An empty fill, a zero-width stroke, or a degenerate path can produce an empty
resource with no buffers. Prepared geometry can be used by any PathRenderer or
Painter on the same device, including with different immutable pipeline policies.
Vertices and indices share one buffer, shared by clones. Geometry up to 64 KiB
comes from a bounded idle pool owned by the PathRenderer, rounded up to a power of
two; larger geometry is exact-sized and released. Draws lease the buffer to their
recording, so a dropped path's buffer is reused only after every recording that
drew it has completed or been abandoned. Re-preparing a changing path every frame
therefore reuses completed buffers instead of allocating. Tessellator scratch
(Lyon buffers, welding and edge storage, output vertices and indices) retains its
peak capacity until the renderer is dropped. CPU batch scratch also retains peak capacity.

## Drawing and coverage

`prepare(&RenderFormat)` prepares pipeline variants, independently of geometry.
`draw` records one resource. `bind` scopes repeated draws of different resources
with one attachment pipeline. `draw_many` instances ordered placements of one
resource, validating the entire slice before drawing and splitting at up to 1,024
instances per parameter page, further limited by application device buffer limits.
Mixed coverage choices do not create pipeline variants. Batches
whose placements all select None omit fringe triangles from the indexed draw.

Prepared geometry, including bounds, retains path units. Pixels and normalized
viewport fractions follow the same Transform2D conventions as other 2D draws.
Nonuniform transforms also distort stroke widths. Bounds exclude the screen-space
coverage band. `vertex_count`, `index_count`, and `geometry_bytes` report retained
storage, including fringe topology. Draw recording uploads 64 bytes per nonempty
placement, not the path's vertices or indices. Pipelines/pages/scratch are reusable
after warm-up. There is no sorting or implicit batching across different paths.

Coverage extrudes boundary topology in the vertex shader after transformation.
The interior boundary moves half a pixel inward and the outer boundary half a
pixel outward, interpolating opacity across that band. Internal tessellation edges
have no fringe. Reflections, rotation, shear, viewport changes, and nonuniform
scaling reuse the same resource. Corner expansion is bounded to four screen pixels.

This is approximate antialiasing, not an exact area-coverage rasterizer. Subpixel
features, narrow gaps, acute corners, and touching/intersecting coverage bands can
produce imperfect coverage. It does not fix coarse curve tessellation. Selecting
`EdgeAntialiasing::None` preserves the original triangle geometry and lets target
MSAA provide sample coverage. No SDF generation or hidden offscreen pass occurs.

Color is finite linear straight RGBA with alpha in `0..=1`; shading outputs
premultiplied RGBA. PipelineOptions, depth zero, dynamic stencil references,
read-only attachment validation, scissor/viewport controls, and state restoration
after raw/other renderer access follow the other 2D renderers. Transparent fragments
are discarded when depth/stencil writes are enabled. Stencil remains binary per
sample even when path color has fractional antialiasing coverage.

Painter exposes `paths`, `prepare_path`, `draw_path`, and `draw_paths`. Borrowed
session transforms follow each path draw's transform. Identity batches delegate
directly; transformed batches reuse Painter's CPU scratch. Preparation occurs
outside painting sessions. Painter owns neither a path scene nor a display list.

## Consumer and verification

The single-file `paths` example owns its window/event loop, handles acquisition
errors and surface recreation, and prepares a fill/stroke gallery once. Resizing
only changes placement. A toggles coverage and Space toggles supported MSAA.

Tests cover command/settings validation, both fill rules, holes and winding,
implicit fill closure, self-intersections, touching contours, Bézier tolerance,
stroke union, open caps, joins/miter limits, fractional pixel coverage, viewport
and reflection transforms, MSAA, atomic batches over multiple pages, Painter
equivalence, raw/other renderer restoration, device mismatch, stencil policies,
read-only aspects, warmed resource reuse, and dropping prepared owners before
submission or abandoning the recording.

`cargo bench -p astrelis --bench paths` compares retained quad paths with direct
wgpu using the same shader/layouts, matched triangles/instances, premultiplied
blending, 64 KiB upload pages, and full pixel checks before timing. It reports
individual/scoped and explicit batch paths separately and includes preparation
cases with 16/64/256 cubic segments. See [measurement evidence](performance/paths.md).

The subsequent [brush stage](brushes.md) adds reusable solid/linear/radial shading
through additive brush drawing APIs, independently of this prepared geometry.

This path stage does not add SVG parsing, arc commands, dashes, variable-width strokes,
inside/outside path strokes, boolean operations, image brushes, clip
stacks, or layer/effect helpers. Those remain separate capabilities.
