# Distance-field text

Status: explicit MTSDF outline preparation and retained fill drawing are implemented.
Coverage remains the default. Text effects are a separate follow-up. CPU shaping,
fallback, TextLayout ownership, Painter placement, and pass clipping are unchanged.

## Preparation API

`TextRenderer::prepare_text` and `Painter::prepare_text` accept
`impl Into<TextPreparation>`. Existing TextRasterOptions calls continue to select
coverage. Choose fields explicitly:

```rust
let prepared = painter.prepare_text(
    &layout,
    MtsdfOptions::new()
        .pixels_per_em(64)
        .range_em(0.25)
        .scale_factor(dpi_scale),
)?;

// Later, without shaping, field generation, or glyph uploads:
paint.draw_text(&prepared, TextDraw::new([20., 30.]))?;
```

The enum is `TextPreparation::Coverage(TextRasterOptions)` or
`TextPreparation::Mtsdf(MtsdfOptions)`. Default is hinted coverage. Fields use
unhinted selected font outlines. There is no automatic representation selection,
regeneration on transform changes, or generation in a paint session.

MtsdfOptions defaults to 64 texels/EM, a full range of 0.25 EM, and scale factor 1.
Density must be in 16..=256. Full range must be finite, positive, at most 1 EM,
and span at least two generation texels. A 0.25-EM range encodes distances from
-0.125 to +0.125 EM. Image padding includes the outside half-range plus one
filtering texel; atlas allocation adds its existing one-texel gutter. Smaller
ranges can reduce generation cost and padding. Density 64 is a conservative
starting point for the tested fonts, not a universal quality optimum.

`scale_factor` converts layout geometry to physical pixels exactly once. It does
not change field density or cache identity. The same outline fields can serve
newly prepared geometry at different font sizes or DPI. A layout or geometry scale
change still creates a new PreparedText. Per-draw transforms can reuse it directly.

PreparedText exposes `preparation()` and `scale_factor()`. The old
`raster_options()` accessor was deliberately replaced: field resources do not have
coverage hinting settings. `size()` still measures scaled advances/line boxes.
`ink_bounds()` means prepared image-quad bounds, including field padding; it does
not become an exact outline metric. Unsupported/blank glyphs are reported through
`skipped_glyphs()`, including blank spaces. Missing characters still use the shaped
face's `.notdef` glyph where it has an outline.

## Artwork, GPU work, and cache lifetime

RGB stores multi-channel signed distance; alpha stores true signed distance for
future effects. Fill uses the RGB median with a screen-space distance gradient for
antialiasing under rotation, reflection, and nonuniform affine scaling. Very small
or highly minified fields are not a substitute for hinted coverage. Very large
magnification can reveal generation/8-bit quantization limits.

COLR and supported bitmap glyphs retain their artwork through the existing
size-dependent image path, with hinting and palette zero. Alpha-only bitmap sources
without outlines use coverage fallback. Draw RGB colors monochrome fill; intrinsic
color RGB is unchanged. Color alpha and opacity affect both. Fallback's physical
font size is still bounded by TextRendererOptions::max_raster_size; pure fields
use bounded generation density instead.

Field pages use linear RGBA8, separate from premultiplied color pages; field alpha
is distance data. A 1024² field page costs 4 MiB logically, compared with 1 MiB for
coverage. Padding increases occupied texels. Only consecutive glyphs sharing a page
are batched. Separate coverage/color and field fragment entry points keep field
reconstruction out of coverage shaders. Mixed runs switch pipelines in order.
`prepare(format)` warms both variants; drawing validates required variants before
recording commands.

Immutable geometry stays 48 bytes per glyph, and nonempty draws still upload one
48-byte placement/color record. No generation, atlas writes, or geometry uploads
occur during retained drawing. Prepared texts, recordings, and GPU completion leases
keep pages alive, including after cache clearing. Budgets count all three. AtlasFull
and GlyphTooLarge remain explicit errors; preparation never raises budgets or waits
for GPU completion. Cache hits/misses include blank results and size-independent
fallback classification entries as well as image keys.

## Generator choice and performance

The private generator uses pinned
[bymsdfgen-core 0.1.1](https://docs.rs/bymsdfgen-core/0.1.1/bymsdfgen_core/).
We consume Swash's selected weight-instance outlines and synthetic italic directly,
so there is no second font parser. Linear, quadratic, and cubic contours preserve
nonzero topology; normalization, deterministic edge coloring, overlap-aware distance
generation, error correction, and sign correction run before quantization/upload.
The dependency's parallel feature is disabled. No implicit thread pool or C++/FFI
build is introduced, and Astrelis continues to forbid unsafe code in its own crate.
The library builds on native and wasm32; browser execution was not measured.

We also prototyped fdsm 0.8.0. Both produced the tested fill comparisons and both
had substantial cold cost. bymsdfgen-core was selected for its smaller normal
dependency graph (arrayvec only), overlap support, and comparable prototype cost.
It is a young dependency; pinning and representative regression tests are deliberate.
This comparison did not establish parity with every font or the original C++ msdfgen.

See the [measurement and quality report](performance/text-distance-fields.md) for
release preparation costs, warmed geometry, retained recording, atlas bytes, and
captures at 1x/2x DPI and 1x/4x MSAA. Cold preparation belongs ahead of use, or can
be staged in small caller-controlled groups. A worker-friendly CPU preparation or
prebaked-field API is a useful next performance step. GPU execution remains
unmeasured on the local backend with unreliable timestamps.

## Effects remain separate

Outline/shadow options must specify units, screen-space versus transformed width,
distance-range limits, quad expansion, and intrinsic color behavior. Per-glyph
shadows can obscure earlier glyph fills; whole-layer effects need a deliberate
ordering policy. Wide blur/glow should use offscreen filtering instead of claiming
an unlimited field range. The stored alpha channel is not yet a public effect API.
