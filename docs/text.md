# Text architecture and implementation plan

Status: CPU font/shaping/layout and explicit coverage/color GPU preparation/drawing
are implemented in `astrelis::text`, with retained drawing integrated into Painter.
Explicit MTSDF outline fill is also implemented; text effects remain separate.

Build the CPU font/shaping/layout layer first. Use cosmic-text as its implementation
and keep GPU preparation independent. Start GPU rendering with grayscale coverage
and color glyph images; add distance fields as an explicit preparation choice.
Both paths consume the same shaped layout. These components can initially live in
the existing astrelis crate, under a text module.

## Library choice

The current [cosmic-text documentation](https://docs.rs/cosmic-text/latest/cosmic_text/)
describes font discovery/fallback, shaping, layout, and optional Swash rasterization.
Its 0.19 release uses harfrust for shaping and fontdb for discovery. Use advanced
shaping by default: the [Basic strategy](https://docs.rs/cosmic-text/latest/cosmic_text/enum.Shaping.html)
does not handle complex scripts or font fallback. Delegate font parsing to these
libraries rather than writing a TTF parser.

The previous Astrelis text crate on main used Parley. It is useful prior art for
measurement and cluster/cursor contracts; it is not a requirement to preserve that
crate's public wrapper or migrate RXUI during this work. Cosmic-text is the chosen
CPU backend because shaping and optional rasterization are available together. There is no
performance claim against Parley without a measured comparison.

[Glyphon](https://github.com/grovesNL/glyphon) is another viable implementation
option and useful reference. Its renderer prepares text areas into renderer-owned
vertex storage, then renders the previously prepared set; see its
[implementation](https://github.com/grovesNL/glyphon/blob/main/src/text_render.rs).
Astrelis needs multiple retained prepared texts and ordered interleaving with other
renderers. Prototype that ownership contract before choosing a Glyphon adapter;
the recommendation is cosmic-text plus an Astrelis-owned GPU layer, reusing frame
uploads and pass state restoration.

The workspace now declares Rust 1.98.1, matching the installed compiler. The CPU
dependency is pinned to cosmic-text 0.19.0 with `std` only. Its optional Swash
rasterizer and optional backend shaped-string cache are disabled in cosmic-text.
The GPU renderer uses Swash 0.2.10 directly against retained font bytes.
Shaped runs are retained per TextBuffer. This keeps font-generation invalidation
under Astrelis's control without accumulating an additional shared string cache.

## Ownership and stages

| Type | Owns | Requires a GPU? |
| --- | --- | --- |
| TextSystem | Font database, font source lifetime, shaping caches and scratch | No |
| TextBuffer | Mutable UTF-8 content, font style, paragraph constraints, revisions | No |
| TextLayout | Retained layout snapshot: glyph/font identities, positions, clusters, lines and metrics | No |
| TextRenderer | Device resources, pipelines, glyph raster/atlas cache | Yes |
| PreparedText | Stable prepared glyph geometry and leases on atlas allocations | Yes |
| TextDraw | Origin, transform, default color and opacity | No |

TextSystem is application-owned, separate from GraphicsContext. It can serve text
for multiple windows/devices; each GPU renderer prepares its own representation.
Embedded fonts and system font discovery are separate explicit operations. Font
handles carry source identity, so equal numeric backend IDs from different systems
cannot be confused. Layout snapshots retain the font sources they require. GPU
preparation consumes the retained fonts directly without a TextSystem borrow.
Layouts from multiple systems are safe because glyph cache keys include scoped font IDs.

TextBuffer retains backend shaping/layout state so changing wrapping constraints
can reuse shaped runs. Setters mark the appropriate work dirty. Layout evaluation
is explicit, caches the resulting snapshot, and can return a cheap shared handle
when neither content, relevant style/constraints, nor font database generation has
changed. The initial API accepts one style per buffer; fallback still produces
multiple font/script runs. Rich spans can follow without changing GPU ownership.

Layout includes selected font faces and glyph IDs, advances/offsets, line baselines,
logical layout bounds, and source cluster ranges. Preserve line identity and UTF-8
byte offsets, with an explicit mapping to whole-buffer offsets. A glyph is not a
Unicode scalar or an editing caret boundary. Preserve bidi embedding levels for
later hit testing. Cursor affinity and hit testing remain future work. Editing
policy, selection state, IME, and widget behavior belong to RXUI. Layout/raster ink bounds must remain distinct from
advance/line bounds used to measure a widget.

Implemented CPU API:

```rust,ignore
let mut text = TextSystem::new(); // No implicit system-font scan.
text.load_font(include_bytes!("fonts/ExampleSans.ttf"))?;
// Applications may also call text.load_system_fonts().

let mut paragraph = TextBuffer::new();
paragraph.set_text("Hello, العربية", TextStyle::new()
    .family("Example Sans")
    .font_size(16.0)
    .line_height(22.0))?;
paragraph.set_width(Some(320.0))?;
paragraph.set_wrap(TextWrap::WordOrGlyph);
let layout = paragraph.layout(&mut text)?;
let measured = layout.size();
// Unchanged evaluation clones the same Arc, without new shaping/layout work.
assert!(std::sync::Arc::ptr_eq(&layout, &paragraph.layout(&mut text)?));
```

Implemented GPU API:

```rust,ignore
// Prepare outside active render passes; retain PreparedText for unchanged content.
let mut renderer = TextRenderer::new(&graphics);
renderer.prepare(&target.render_format())?;
let prepared = renderer.prepare_text(
    &layout, TextRasterOptions::new().scale_factor(dpi_scale),
)?;
let mut pass = frame.render_pass().begin()?;
renderer.draw(&mut pass, &prepared,
    TextDraw::new([20.0, 30.0]).color([1.0, 1.0, 1.0, 1.0]))?;
```

Font size, line height, and paragraph width use application-selected local units.
Preparation receives a raster scale explicitly. Placement is viewport-relative
physical pixels with the existing Y-down affine convention; the renderer converts
prepared geometry to physical size once. DPI must not be applied implicitly again
by Painter or a window. Draw transforms also transform glyph quads. Magnifying a
coverage image resamples it; callers can prepare at a suitable raster size instead.
The coverage renderer uses zero local raster phase. Fractional
movement filters that data; it must not secretly rasterize during draw.

## Glyph representations

| Representation | Proposed use | Initial scope |
| --- | --- | --- |
| Grayscale coverage | UI text at an explicitly prepared raster size | First GPU implementation |
| Color bitmap | Font-provided color glyphs, including supported emoji formats | First GPU implementation |
| MTSDF | Text repeatedly magnified/transformed, scalable labels | Explicit fill preparation; effects later |

Swash exposes [Mask, Color and SubpixelMask outputs](https://docs.rs/cosmic-text/latest/cosmic_text/enum.SwashContent.html).
Use grayscale masks and color images initially. LCD subpixel rendering is a separate
compositing policy and is outside this first renderer. Color glyph RGB is intrinsic;
default text color colors mask glyphs, while draw opacity applies to both. Decode
color image encoding explicitly and match Astrelis's linear/premultiplied blending.

Distance fields do not replace shaping. Generate them from the selected face's
glyph outline after shaping, using the same glyph IDs and placement. Prefer MSDF
to a monochrome SDF for preserving sharp glyph corners; the distinction is explained
by [msdfgen](https://github.com/Chlumsky/msdfgen). SDF generation, atlas storage, and
reconstruction shaders are separate work from Swash coverage rasterization. Color
bitmap glyphs still need the color path. The implemented MTSDF variant is explicit;
there is no automatic mode heuristic.

## Preparation, ordering and cache lifetime

Shaping, rasterization, atlas insertion and geometry preparation happen before
drawing. Drawing prepared text records commands, preserving caller order with
meshes, images and primitives. Only adjacent compatible glyph batches may be
coalesced; do not sort mask/color glyphs or font runs across overlaps. Current pass
scissor and viewport apply; renderers restore their owned state after pass access.
Target MSAA/depth attachment variants follow the existing renderer preparation API.

Preparing B must not alter a retained A or commands already recorded from A. Atlas
regions are leased by prepared resources and independent recordings. Cache eviction,
clear, growth and reuse must honor those leases, then recycle storage only when
queue ordering or completion makes it safe. New pages avoid silently changing the
UVs or texture dimensions of retained allocations. Document memory budgets and an
explicit failure when space cannot be reclaimed safely, rather than an implicit
blocking wait in a draw. Keep CPU raster caches bounded too.

Glyph cache identity includes font source/face and applicable instance settings,
glyph ID, physical raster size, raster phase, and raster settings. Later distance
fields also key generation resolution/range. Text content/constraints invalidate
layout; raster-size changes invalidate coverage preparation; origin/default color/
opacity and pass clipping affect draw parameters. Repeated prepared draws perform
no shaping, rasterization, atlas uploads or pipeline creation after preparation.
An immutable prepared path should also avoid re-uploading all glyph instances.

## Implementation sequence and acceptance

1. CPU font loading, advanced shaping and retained paragraph layout. Validate
   TTF/OTF input and collections through the backend, expose usable face information,
   and report invalid fonts and unresolved glyphs predictably. Cover kerning,
   ligatures, combining marks, Arabic joining, mixed bidi, fallback, wrapping,
   alignment, empty text, line breaks, baselines and source cluster mapping with
   pinned redistributable test fonts. Measurement works headlessly. Repeated layout
   reuses its snapshot; constraints/content/font changes invalidate correctly.
2. Explicit coverage/color preparation and TextRenderer draws in existing passes.
   Pixel tests cover order, clipping, opacity, DPI, multiple prepared texts,
   simultaneous recordings, atlas pressure/lifetime and recovery from cache errors.
   Benchmark cold preparation, warmed preparation, and prepared draw recording
   separately, recording cache misses, uploaded bytes, draw counts and atlas memory.
3. Painter integration over prepared text, with a standalone copyable multilingual
   example and scoped transforms/custom renderer interleaving. Keep preparation
   available outside active sessions; draw convenience must not conceal shaping.
4. MSDF/MTSDF design and implementation, after the first path is measured. Keep CPU
   layout unchanged, test corners/thin strokes under magnification/transforms, and
   compare quality, generation cost, atlas memory and GPU execution where measurable.

The first GPU implementation covers coverage images, COLR outline layers and supported
bitmap sources through Swash. No universal Unicode/font-format coverage is assumed.

## CPU contracts and verification

Text setters validate before mutating state. Empty text occupies one configured
line box; a trailing explicit break adds another empty paragraph. Width is the
maximum line advance, not ink bounds or the alignment constraint. Zero-width
constraints allow indivisible clusters to exceed the constraint. Positions include
shaping offsets and baseline, in X-right/Y-down local units; `advance_origin` retains
the position before offsets. Cluster ranges refer to the original whole-buffer
UTF-8 bytes, preserving CR, LF, CRLF, and LFCR. Wrapped lines retain the entire
source paragraph range and expose their glyph slice separately.

No-font nonempty content returns `TextError::NoFonts`; blank paragraphs can be
measured without fonts. Missing glyphs remain `.notdef` entries in an otherwise
usable layout and their distinct source ranges are reported. Font IDs are scoped
to their TextSystem. Each layout retains selected source bytes, face index, requested
instance weight, and synthetic italic information for future rasterizers. Loading
fonts rebuilds the backend matching/fallback caches and invalidates buffer evaluation;
invalid loading leaves existing fonts and snapshots unchanged.

Unchanged evaluation returns the same `Arc<TextLayout>` in constant time. Width,
wrap, alignment, and default font-size/line-height changes reuse backend shaped
runs but rebuild layout/snapshot vectors. Content, font selection, features,
tracking, or font-generation changes reshape. Retaining old snapshots retains their
layout allocations and font references until the caller drops them. Font discovery
is explicit and potentially blocking; loading also copies bytes unless an existing
`Arc<[u8]>` is supplied with `load_font_shared`.

Run the standalone headless example and CPU-stage benchmark:

```sh
cargo run -p astrelis --example text_layout
cargo bench -p astrelis --bench text
```

The benchmark checks snapshot reuse and reflow equivalence before timing. It
separates font loading, new-buffer shaping with a warmed font system, unchanged
snapshot retrieval, width reflow, and content reshaping. It reports median/p95
nanoseconds for 100, 1,000, and 10,000 Unicode scalars with bundled Latin/Arabic
fonts. Font discovery, rasterization, GPU recording, and GPU execution are absent.
The [initial CPU baseline](performance/text.md) records three release runs with
source hashes and measurement boundaries. Pinned OFL fixtures and licenses live
in `crates/astrelis/tests/fonts`; tests cover
OTF/TTF/collections, shaping, bidi, fallback, metrics, invalidation, missing glyphs,
and ownership without requiring a window, device, or installed font.

## GPU contracts and verification

`TextRenderer::new(&graphics)` uses default bounded atlas settings;
`with_options(&graphics, TextRendererOptions)` validates custom page/budget/raster
limits. `prepare(&RenderFormat)` creates pipeline variants, and
`prepare_text(&TextLayout, TextRasterOptions)` rasterizes cache misses and creates
an immutable glyph buffer. It requires only the retained layout's font data.
`draw(&mut RenderPass, &PreparedText, TextDraw)` preserves current viewport/scissor
and uploads a 48-byte placement/color record. It restores owned state after other
renderers. Depth/stencil tests and writes are disabled, including in depth-enabled
or read-only passes. Pipeline preparation remains explicit for predictable first use.

Prepared geometry contains physical pixel positions: raster scale multiplies font
size and layout placement once. Draw origin and affine transforms are physical
pixels relative to the viewport. Magnification filters the prepared images; choose
a larger raster scale for additional resolution. `size()` measures scaled advances
and line boxes; `ink_bounds()` bounds image quads, not an exact nonzero-pixel outline.
No-image glyph indexes are reported by `skipped_glyphs()`, including blank spaces
and sources Swash cannot render. `.notdef` images remain drawable.

Coverage pages use R8; color pages use linear premultiplied RGBA8. Swash's color
outline blits are premultiplied sRGB and PNG bitmap results are straight sRGB;
preparation normalizes both before filtered sampling and source-over blending.
Draw RGB colors masks only; draw alpha/opacity affects both representations.
One transparent texel surrounds every allocation. Only consecutive glyphs on the
same atlas page are batched; mask/color transitions and drawing order are preserved.

Pages are append-only. Eviction discards whole unleased pages and creates new
textures, preserving retained UVs and already-recorded commands. Default settings
allow eight 1024² pages, 16,384 cached glyph keys, and a 512-pixel physical font-size
limit. The page budget counts retained prepared texts, active recordings, and
submitted work through completion callbacks. `clear_cache()` releases cache ownership
but cannot release those leases. `AtlasFull` never waits: drop obsolete texts and
drive queue/device progress, or configure a larger budget, before retrying.
Preparation failures may populate caches but never invalidate earlier prepared text.

Glyph images are not retained in a CPU image cache. Swash uses an eight-entry
scaler cache plus reusable decode/outline scratch; the font-key table is limited
to 64 entries. Outline extents are checked before raster output allocation. Bitmap
dimensions are checked after decode, so backend temporary decode scratch is outside
the atlas budget. Prepared geometry allocations are caller-owned and limited by the
device buffer limit, separate from atlas bytes. Repeated preparation creates a new
geometry buffer even when all glyph images hit; retain PreparedText to avoid that work.

`stats()` exposes hits/misses, atlas uploads, prepared geometry bytes, parameter
bytes, recorded draw counts, live pages, logical atlas bytes, and cached key count.
Pixel tests cover intrinsic color/opacity, linear compositing, DPI, clipping, MSAA,
renderer interleaving, multiple prepared texts and source identities, overlapping
recordings, cache clearing/eviction, exhaustion, abandonment and completion recovery.
The original geometric `TestColor.ttf` fixture contains coverage, COLR v0 layers,
and a PNG sbix glyph; its generator is committed alongside the font.

```sh
cargo run -p astrelis --example text
cargo bench -p astrelis --bench text_rendering
```

The window example owns its event loop, surface recovery, DPI preparation and width
reflow. Space toggles MSAA. The benchmark separates cold raster/atlas/geometry
preparation, cached-atlas preparation with new geometry, prepared draw recording,
and frame CPU total. Shaping and GPU completion waits are outside those intervals;
GPU execution and color-atlas workloads are not measured by this initial benchmark.
See the [GPU-text CPU baseline](performance/text-rendering.md) for three recorded runs
and exact measurement boundaries. COLR currently uses palette zero and the rasterizer's
default foreground; custom palettes/foreground, rich spans, LCD rendering, and
text effects remain future work. See the separate distance-field fill contract below.

## Painter integration

Painter owns an independent TextRenderer alongside its shape, line, and image
renderers. `Painter::prepare(format)` warms the default primitive and text pipeline
variants without touching glyph caches. Image variants still require a binding and
`prepare_image`. `Painter::prepare_text(layout, preparation)` forwards explicit atlas and
geometry preparation, returning the same PreparedText used by direct TextRenderer.
A layout owns its font sources; Painter does not own TextSystem or TextBuffer.

`PaintSession::draw_text(prepared, draw)` records immediately in caller order. It
composes the draw transform with the session transform, including the draw origin,
then delegates validation, clipping, viewport conversion, batching, and allocation
leases to TextRenderer. Prepared text from another renderer on the same device is
usable. Text errors keep TextRenderError; existing primitive, session, and pipeline
preparation methods retain Error. Attachment pipeline failures include
`Error::UnsupportedTextFormat` and map to `TextRenderError::UnsupportedFormat` in
the direct text API.

Prepared text is in physical pixels: raster scale already applied DPI. Session
transforms act on these physical glyphs. Use a physical translation to position
DPI-prepared text; do not scale it again in a logical-coordinate scope unless an
additional geometric scale is intended. DPI/content/layout changes require explicit
preparation; color, opacity, and placement changes reuse the same prepared resource.

`Painter::text()` exposes cache statistics, clearing, and direct renderer methods
outside active sessions. Custom budgets can be selected by replacing it with
`TextRenderer::with_options`; previous prepared resources retain their original
allocation leases. Draw calls perform no shaping, rasterization, atlas writes, or
static geometry uploads. Draw parameter uploads remain 48 bytes per nonempty text.
There is no implicit batching across separate text calls or session flush.

Pixel tests compare Painter with direct renderers at 1x and 4x MSAA with depth/stencil
attachments, nested transforms, 2x raster preparation, coverage and intrinsic color,
image/primitive/custom mesh ordering, viewports, and clipping after raw pass mutations.
Rejected foreign-device, opacity, and overflowing-transform draws leave the session
usable. Counters verify no new preparation work during drawing, including after cache
clearing. The standalone `painter_text` example owns its complete window lifecycle,
reflow/DPI preparation, surface recovery, and MSAA changes. The
[Painter text recording baseline](performance/painter-text.md) compares direct and
facade calls with matching draw counts; it does not measure GPU execution.

## Quality/workload check and next representation

The [screen-sized workload baseline](performance/text-workloads.md) extends the
initial repeated-glyph measurements with visible paragraphs, hundreds of labels,
ordered coverage/color page changes, content updates, 1,822 distinct shaped glyph
keys, and raster-size churn with explicit AtlasFull recovery. Readbacks verify visible
output. Optional timestamps are checked for completeness; local Metal results were
unreliable and are withheld rather than treated as GPU execution measurements.
The [quality gallery](performance/text-quality.md) captures 1x/2x DPI and 1x/4x MSAA
from the copyable text_quality example. It compares current rasterization choices,
not a distance-field implementation. The
[distance-field API](text-distance-fields.md) now implements explicit MTSDF preparation
and unchanged retained drawing. It includes selected-instance outline generation,
size/DPI cache reuse, intrinsic artwork fallback, separate coverage/field shaders,
and representation metadata. The
[distance-field report](performance/text-distance-fields.md) records preparation cost
and fill comparisons before adding effects. PreparedText::raster_options() is replaced
by preparation() and scale_factor().
