# Text architecture and implementation plan

Status: the CPU font/shaping/layout milestone is implemented in `astrelis::text`.
GPU text preparation, TextRenderer, Painter text integration, and distance fields
remain planned. Their signatures below are proposals, not existing public APIs.

Build the CPU font/shaping/layout layer first. Use cosmic-text as its implementation
and keep GPU preparation independent. Start GPU rendering with grayscale coverage
and color glyph images; leave a separate distance-field preparation path for later.
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
rasterizer and optional backend shaped-string cache are disabled in this milestone.
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
preparation using a different TextSystem must reject incompatible source identity.

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

Proposed GPU API for a later milestone:

```rust,ignore
// A later milestone adds GPU preparation, outside active render passes.
let mut renderer = TextRenderer::new(&graphics);
renderer.prepare(&target.render_format())?;
let prepared = renderer.prepare_text(
    &mut text, &layout, TextRasterOptions::new().scale_factor(dpi_scale),
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
Choose a documented local raster/subpixel phase for prepared data. Fractional
movement filters that data; it must not secretly rasterize during draw.

## Glyph representations

| Representation | Proposed use | Initial scope |
| --- | --- | --- |
| Grayscale coverage | UI text at an explicitly prepared raster size | First GPU implementation |
| Color bitmap | Font-provided color glyphs, including supported emoji formats | First GPU implementation |
| MSDF/MTSDF | Text repeatedly magnified/transformed, scalable labels and effects | Separate later implementation |

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
bitmap glyphs still need the color path. Do not add an unimplemented public MSDF
enum variant or an automatic mode heuristic in the first milestone.

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

CPU-first acceptance is a font/shaping/layout milestone, not a completed GPU TextRenderer.
No particular GPU speedup or universal Unicode/font-format coverage is assumed.

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
