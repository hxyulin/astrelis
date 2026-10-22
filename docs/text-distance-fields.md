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
        .raster_scale(dpi_scale),
)?;

// Later, without shaping, field generation, or glyph uploads:
paint.draw_text(&prepared, TextDraw::new([20., 30.]))?;
```

The enum is `TextPreparation::Coverage(TextRasterOptions)` or
`TextPreparation::Mtsdf(MtsdfOptions)`. Default is hinted coverage. Fields use
unhinted selected font outlines. There is no automatic representation selection,
regeneration on transform changes, or generation in a paint session.

MtsdfOptions defaults to 64 texels/EM, a full range of 0.25 EM, and raster scale 1.
Density must be in 16..=256. Full range must be finite, positive, at most 1 EM,
and span at least two generation texels. A 0.25-EM range encodes distances from
-0.125 to +0.125 EM. Image padding includes the outside half-range plus one
filtering texel; atlas allocation adds its existing one-texel gutter. Smaller
ranges can reduce generation cost and padding. Density 64 is a conservative
starting point for the tested fonts, not a universal quality optimum.

`raster_scale` selects color/bitmap fallback image density. It does not scale
geometry or change outline generation density/cache identity. Outline fields
serve different font sizes and draw transforms. Prepared geometry and `size()`
retain layout units. Apply a draw/session DPI transform consistently with other
2D content. Pure outline geometry does not need rebuilding solely for DPI;
color/bitmap fallback may need preparation at a new density.

PreparedText exposes `preparation()` and `raster_scale()`. The representation
remains explicit, with coverage hinting separate from field density.
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

The private generator uses
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

## Worker generation and bounded commits

Cold MTSDF generation can take tens or hundreds of milliseconds. Use
`TextRenderer::request_glyphs(Arc<TextLayout>, settings, limits)` to capture owned
CPU work. Move the request to an application worker and call `generate()` there.
The request and `GeneratedGlyphs` are `Send`, contain no device/queue/atlas handles,
and retain font sources after buffers or the text system are dropped. Astrelis
creates no executor, starts no thread, schedules no redraw and waits for no worker.
The same path supports coverage, fractional coverage phases, color and bitmap
fallback. Synchronous `prepare_text`/`prepare_texts` remain available.

```rust,no_run
use astrelis::{TextRenderer, TextLayout, PreparedText, MtsdfOptions,
    GlyphPreparationLimits, GlyphUploadBudget, TextRenderError};
use std::sync::Arc;

fn start(renderer: &TextRenderer, layout: Arc<TextLayout>)
    -> Result<std::thread::JoinHandle<Result<astrelis::GeneratedGlyphs, TextRenderError>>,
              TextRenderError> {
    let request = renderer.request_glyphs(layout, MtsdfOptions::new(),
        GlyphPreparationLimits { max_glyphs: 4096, max_image_bytes: 16 * 1024 * 1024 })?;
    Ok(std::thread::spawn(move || request.generate()))
}

// In application state: keep the old PreparedText and draw it as usual.
// On redraw, inspect worker.is_finished(); do not join a running worker.
// When complete: let mut upload = renderer.begin_glyph_upload(worker.join().unwrap()?)?;
// Each later redraw performs one bounded commit, while still drawing the old text:
fn advance(renderer: &mut TextRenderer, upload: &mut astrelis::TextUpload)
    -> Result<Option<PreparedText>, TextRenderError> {
    let progress = renderer.commit_glyphs(upload,
        GlyphUploadBudget { max_glyphs: 4, max_bytes: 64 * 1024 })?;
    if progress.remaining != 0 { return Ok(None); }
    // Explicit upper bound for the final geometry upload, 48 bytes per quad.
    Ok(Some(renderer.finish_glyph_upload(upload, 1024 * 1024)?))
}
// Replace the old text only after successful finish; drop the upload to release
// its extra leases. The application chooses when to request another redraw.
```

Generation deduplicates complete keys: source-scoped font/face identity, selected
weight, synthetic italic, representation/density/range, physical size, hinting
and phase. Intrinsic artwork keeps zero phase. MTSDF classification and its
size-dependent raster fallback are separate entries. Requests deliberately do
not snapshot the GPU cache: concurrent requests may generate duplicate images,
which commit reuses if still cached. Prewarm representative text in advance;
retain the finished `PreparedText` for repeated draws, or drop it to leave an
evictable warm cache. Layout/settings changes require a new request; applications
must discard obsolete results themselves. Immutable older layouts remain valid.

`GlyphPreparationLimits` bounds unique entries (including blank/classification
entries) and retained image bytes. It excludes layout/font storage, hash metadata
and one glyph's generator/decoder scratch. Limit or generation errors discard
partial CPU output and do not mutate the renderer. Work cannot be interrupted
inside a glyph; applications can split large workloads into smaller requests.

`begin_glyph_upload` binds results to their originating renderer. `commit_glyphs`
processes at most `max_glyphs` entries and `max_bytes` of actual queue image writes
per call. Hits and blanks count as entries but consume zero upload bytes. There
is no oversized-first-image exception: a small byte budget can return zero work;
use `upload.next_image_bytes()` to choose a budget that fits the next image.
These are work bounds, not time guarantees:
allocating a page, driver work and amortized cache eviction can still vary.
No geometry upload happens during commit. `finish_glyph_upload` rejects pending
entries and applies an explicit conservative geometry byte bound before building
geometry. Decorations may initialize one shared four-byte solid texture. Finish
never generates missing glyphs. No API submits a frame or polls/waits for completion.

Committed entries lease their atlas pages independently of the cache, even with a
one-entry cache. Finish transfers independent ownership to the prepared text;
dropping the upload releases its extra leases. Existing prepared texts remain
drawable during generation, upload, cache eviction and cache clearing.
`AtlasFull` leaves the failing entry pending; earlier commits stay valid. Retry
after releasing other leases/completing frames, trimming or raising `max_pages`.
An upload can itself occupy the page budget: split requests or raise the budget
if all its required pages cannot coexist. Drop requests/results/uploads to cancel
unused work. `clear_cache` invalidates outstanding requests and uploads with
`StalePreparation`, including generation completed after clear; results from a
different renderer are rejected too. Ordinary eviction/trim does not invalidate
jobs. There is no automatic retry, device polling or unbounded upload fallback.

Run `cargo bench -p astrelis --bench glyph_preparation` for a headless, copyable
worker/draw/commit loop with pixel-parity checks and separate CPU timing for each
stage. See [measurements](performance/glyph-preparation.md) for its limits/results.
