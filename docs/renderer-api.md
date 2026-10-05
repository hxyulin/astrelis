# Renderer API contracts and migration

Astrelis keeps independent device-bound renderers composed by Painter. A renderer
owns shaders, pipeline variants, caches and scratch storage. Resources own retained
content. Draw descriptors own placement/appearance values. The application owns
window lifecycle, targets, frame acquisition, passes, draw order and submission.
There is no renderer base trait, automatic sorting, display list, or implicit flush.

## Preparation and errors

All renderers prepare attachment-compatible pipelines from `&RenderFormat`, or
from a surface with `prepare_for_target(&target)`. The format includes color slots,
MSAA count, and optional depth/stencil format, and works for framebuffer/custom
passes without acquiring a frame. Mesh material preparation uses
`prepare_material(&material, &format)`; `try_prepare_material` captures shader
validation errors asynchronously. Texture preparation additionally takes a source
binding because filtering/alpha mode affect its variant.

```rust,ignore
meshes.prepare(&target.render_format())?;
meshes.prepare_material(&material, &framebuffer.render_format())?;
shapes.prepare(&target.render_format())?;
lines.prepare(&target.render_format())?;
text.prepare(&target.render_format())?;
textures.prepare(&image, &target.render_format())?;
```

The old mesh `(color_format, sample_count)` arguments and `prepare_for_format`
method are removed. Supplying a complete format avoids accidentally preparing a
color-only pipeline for a pass with depth/stencil storage.

Pipeline preparation and drawing return `Error`, including `InvalidTextDraw` for
invalid text placement/color/opacity. `prepare_text`/`prepare_texts` return
`TextRenderError` for atlas exhaustion, invalid raster settings or font image
failures. Drawing retained text cannot report `AtlasFull` or `InvalidRaster`.
Rejected draws record no draw commands; earlier successful calls remain recorded.
Preparation failures may populate caches but preserve existing prepared resources.

## Geometry, density, and transforms

All 2D descriptors accept `.transform(impl Into<Transform2D>)`. The shared type
also accepts six-element arrays. `TextureDraw::transform` stores `Transform2D`;
`transform_2d` is removed. Geometry transforms precede viewport conversion. Painter
applies the session transform after each draw's transform, in the draw's units.
Pixels and normalized viewport fractions retain their existing meanings.

Text geometry, origins, `PreparedText::size()` and image-quad bounds now retain the
layout's units. `raster_scale` replaces `scale_factor` in preparation options and
metadata: it selects coverage/color image texels per layout unit and never scales
geometry. MTSDF outline density remains `pixels_per_em`; its `raster_scale` controls
only color/bitmap fallback. Coverage hinting/rounding can affect quad bounds, but
advance/line-box measurements remain equal to the layout's measurements.

```rust,ignore
let prepared = painter.prepare_text(
    &layout, TextRasterOptions::new().raster_scale(dpi),
)?;
let mut paint = painter.begin(&mut pass)?;
let mut logical = paint.transformed(Transform2D::scale(dpi, dpi))?;
logical.fill_rect(Rect::new(20., 20., 100., 30.), background)?;
logical.draw_text(&prepared, TextDraw::new([24., 24.]).color(foreground))?;
```

Direct text callers that previously relied on preparation scaling must now apply
an explicit draw transform. Scale geometry once, including the origin. If origins
are already physical pixels, use a zero local origin and append physical translation
after scaling. Raster density is chosen independently: oversampling for quality is
valid without magnifying text. Unchanged prepared text remains reusable for movement,
color, opacity and transforms. New coverage/color density requires preparation;
pure outline geometry does not need rebuilding solely for a DPI transform.

## Immutable built-in pipeline configuration

`PipelineOptions` selects blending, color writes, and optional wgpu depth/stencil
state for built-in 2D shading. Shapes, lines and textures accept it through
`with_options`; text uses `TextRendererOptions::pipeline`. Painter's `with_options`
constructor applies one policy to all four renderers. Text retains its independent
atlas/cache budgets and can be replaced through `painter.text()`.
`TextRendererOptions` is now `Clone` rather than `Copy`, because it owns pipeline
configuration; clone options explicitly when configuring multiple text renderers.

Defaults use premultiplied source-over and ignore depth/stencil. Explicit states
must match the attachment format. Read-only pass aspects reject writing states
before recording. Built-in 2D vertices use depth zero. Pipeline configuration is
fixed for a renderer's lifetime; multiple renderers can share a device/pass and
prepared resources. Shader/resource layouts remain renderer-specific.

For a stencil mask already written with reference 1:

```rust,ignore
let face = wgpu::StencilFaceState {
    compare: wgpu::CompareFunction::Equal,
    ..Default::default()
};
let state = wgpu::DepthStencilState::stencil(
    wgpu::TextureFormat::Depth24PlusStencil8,
    wgpu::StencilState {
        front: face, back: face, read_mask: 0xff, write_mask: 0,
    },
);
let mut painter = Painter::with_options(
    &graphics, PipelineOptions::new().depth_stencil(Some(state)),
);
painter.prepare(&target.render_format())?;
// Inside an application-owned pass with this depth/stencil attachment:
pass.set_stencil_reference(1);
let mut paint = painter.begin(&mut pass)?;
paint.fill_rect(rect, color)?;
paint.draw_text(&prepared, TextDraw::default())?;
```

The application controls mask creation, clearing/loading, nested-mask policy,
and the dynamic stencil reference. Wrapped pass setters affect subsequent draws
and create no pipeline variant. Writers use a separate built-in fragment entry
that discards fully transparent fragments, so masks follow visible shape/glyph/image
coverage rather than their enclosing quads. Stencil coverage is binary per sample. Fractional analytic alpha does not become
a fractional stencil value; use texture masks or actual multisampled mask geometry
when soft clip coverage is needed.
Default/read-only shading keeps its original fragment path.

## Retained images through Painter

`Painter::prepare_images(&draws, viewport)` uploads immutable texture placements.
`PaintSession::draw_prepared_images(&image, &prepared)` applies the session transform,
current clipping and immediate ordering. The source binding remains independent
of placements. UVs, tint and each instance's own transform remain retained.

An identity session uses the ordinary prepared path with no recurring upload.
An additional transform uploads 48 bytes once per call; instance geometry stays
immutable. Mixed pixel/normalized instances apply that transform in their respective
units. Pixel placements must use their prepared viewport size; normalized-only
placements adapt to new viewport sizes. Transform validation uses retained bounds,
not CPU copies of the instance array.

`TextureRenderer::draw_prepared_transformed` exposes this built-in path directly.
Warm it with `prepare_transformed`; ordinary `prepare` warms only ordinary shading.
`Painter::prepare_image` warms both variants. Custom texture materials retain their
existing vertex interface and ordinary prepared drawing path.

`paint.pass()` permits wrapped state changes and application rendering through
`as_wgpu()`. Subsequent built-in draws restore their required pipeline/buffers and
the wrapped raster state. Raw changes do not update wrapped clipping/reference
settings. Scoped transformations preserve the parent geometry transform, but do
not save/restore viewport, scissor or bindings.

## Performance and verification

Text and primitive recording read compact attachment compatibility values directly
from the pass, avoiding the `RenderFormat` color-vector clone in their hot paths.
Public `render_format()` still returns an owned snapshot for application use.
Dynamic batch/upload behavior and resource completion ownership are preserved.
Transformed retained images add one small placement upload and a dedicated cached
pipeline; ordinary image rendering incurs no additional transform upload.

GPU readback tests cover mixed density/geometry scaling, retained versus dynamic
image placement under affine transforms and viewport/scissor changes, stencil
reference changes across all built-in renderers, coverage/MTSDF/color fallback,
1x/4x MSAA, visible-coverage stencil writes, missing attachments, and read-only
rejection. Existing lifetime, cache-pressure and submission tests remain applicable.

The [CPU comparison](performance/renderer-api.md) includes alternating baseline/current
runs, raw timings, source/executable hashes, and measurement limitations.
