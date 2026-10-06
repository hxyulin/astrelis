# Reusable brushes

`Brush` is an immutable device-bound resource created through
`GraphicsContext::create_brush(BrushOptions)`. Its borrowed description is copied
and uploaded at creation. Clones share handles; stop slices and renderers need not
outlive the result. Any shape, line, or path renderer on the same device accepts it.
Geometry and shading stay independent: replacing a brush never prepares a path.

## Coordinates and color

`BrushOptions::solid` supplies constant linear straight RGBA.
`BrushOptions::linear(start, end, stops)` projects positions onto a nonzero segment.
`BrushOptions::radial(center, radius, stops)` supplies a centered circle with a
strictly positive radius. Brush coordinates map into geometry coordinates through
`BrushOptions::transform`; drawing applies the inverse. Nonuniform scaling turns
a radial circle into an ellipse. Gradient transforms must be finite, invertible,
and have representable inverse mappings. Solids ignore transform and spread.

Geometry coordinates mean the original path coordinates, original shape rectangle,
or original line endpoints, before draw transforms and viewport conversion. There
is no hidden bounding-box normalization. For normalized drawing, gradient positions
use the same viewport fractions as geometry. A shape at x=100 does not automatically
reset a gradient defined at x=0. To reuse a local brush at multiple placements,
use geometry around the origin and move it with its draw/Painter transform.

Stop positions are finite, ordered nondecreasing, and in zero to one. At least one
stop is required; a single stop produces constant color. No sorting/clamping occurs.
Equal positions produce a right-continuous hard transition: the last stop wins at
the exact position. If the first/last stop is not at zero/one, its color extends
to that interval boundary. `GradientSpread::Clamp` extends endpoint colors;
`Repeat` repeats with period one, including negative parameters; `Reflect` alternates
forward/backward intervals with period two. Exact repeat boundaries select zero.

RGB is finite linear color (HDR permitted); alpha is zero to one. Creation converts
stops to premultiplied RGBA and the shader interpolates those values. Hidden color
in a transparent stop therefore does not bleed into visible color. There is no
automatic sRGB input conversion or perceptual/OKLab interpolation. Draw color
multiplies as a straight RGBA tint, then edge coverage multiplies the final
premultiplied result. Use white draw color to preserve the brush.

## Drawing and ownership

Independent renderers expose `prepare_brush(&RenderFormat)`, `draw_with_brush`,
`draw_many_with_brush`, and `bind_with_brush`. Brush and ordinary color variants
are separate; their immutable PipelineOptions agree. Prepare brush pipelines before
first drawing when avoiding first-use shader/pipeline creation is important.
The same brush variant handles all brush kinds and stop counts.

Painter exposes `prepare_brush` for all three renderers and
`draw_shape_with_brush` / `draw_shapes_with_brush`,
`draw_line_with_brush` / `draw_lines_with_brush`, and
`draw_path_with_brush` / `draw_paths_with_brush`. Borrowed transforms move geometry
and brush together. Batches share one brush, validate the whole slice before drawing,
preserve order, and split at device-sized upload pages. Transformed Painter batches
reuse its existing CPU scratch. Different brushes remain explicit separate draws;
there is no sorting or automatic batch reordering.

Brush shading owns bind group zero. Scope pass access invalidates pipeline/raster
state and the next draw restores them and the brush binding. Wrapped viewport,
scissor, stencil references, read-only aspects, and MSAA follow existing renderers.
When depth/stencil writes are enabled, fully transparent brush fragments are
discarded, so they do not write masks. Fractional alpha still gives binary stencil
coverage per sample rather than a soft mask.

Brush buffers and bindings are immutable and never recycled by an Astrelis pool.
wgpu commands retain bound GPU resources when the last CPU owner drops before
submission. Abandoning a frame discards that recording. There is no extra CPU
resource lease or brush-specific idle pool.

## Cost and limits

Creation allocates one 48-byte uniform buffer, one 32-byte-per-stop storage buffer,
and one bind group. It validates buffer limits, stage bindings, and fragment storage
support before GPU allocation; unsupported imported/downlevel devices return
`UnsupportedBrushLimits`. Stop counts are limited by device buffer/storage-binding
limits rather than an arbitrary fixed array size. Invalid descriptions return
`InvalidBrush`; oversized storage returns `BrushTooLarge`. Drawing rejects foreign
devices and overflowing geometry-to-brush or tint multiplication before recording.

Stops are retained and are not uploaded while drawing. Shader lookup uses binary
search over exact stops, followed by premultiplied interpolation, rather than a
precomputed ramp texture. Radial lookup adds distance calculation. Fragment work
depends on stop count, covered pixels, and overdraw; CPU recording does not scan
stops. Timing evidence is in [the brush performance report](performance/brushes.md).

Paths keep their 64-byte placement records and prepared geometry. Shapes/lines
add two coordinate-frame vectors for brush evaluation: 112 bytes instead of 80.
This means up to 585 brushed primitive instances per 64 KiB page, versus 819
ordinary ones, and up to 1,024 path instances. Smaller application device buffers
reduce those capacities. CPU scratch and upload pages retain peak capacity for reuse.
Ordinary color-only drawing does not create brush shaders/bindings or sample stops.

Changing stops, spread, or gradient mapping creates a new immutable brush. There
is no in-place update API or dynamic per-placement brush transform in this stage.
Use draw/Painter transforms to animate placements together with their brushes.
Do not recreate brushes every redraw when their settings are unchanged.

Finite-precision shader arithmetic can lose precision at extreme coordinates/scales.
Existing shape/path edge coverage remains approximate; path coverage-band vertices
interpolate their original brush coordinates through the shifted fringe. Hard
gradient transitions and repeated fine stripes are not separately antialiased,
so strong minification can alias. This is not a general filtered pattern rasterizer.

Image/pattern brushes, focal/two-circle radial gradients, conic gradients, gradient
text, custom brush shaders, and alternative color interpolation remain future work.
The existing TextureRenderer still handles images; this stage does not add clipping
stacks, layers, or effects.

The `brushes` example is a standalone native consumer with its own window/event
loop, a retained gallery, resize/surface handling, and coverage/MSAA toggles.
Tests cover rendered stops/spreads/transparency, affine/normalized/viewport/reflection
coordinates, batched/Painter equivalence, state restoration, storage/page reuse,
dropping owners before submission, device mismatch, low device limits, atomic
validation, and transparent/read-only stencil behavior.
