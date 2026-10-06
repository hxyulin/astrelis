# Retained text interaction

Astrelis now provides CPU geometry for selectable/editable text on the same immutable
`TextLayout` used for drawing. RXUI's isolated `prototypes/next` workspace exercises
it in a desktop application with controlled editing, IME preedit/commit, focus,
selection, virtualization, accessibility, and a custom live chart.

## API and ownership

```rust
layout.prepare_interaction()?; // Optional ahead-of-input warmup; CPU only.
let position = layout.hit_test([local_x, local_y]);
let caret = layout.caret(TextPosition::new(byte_offset));
for rect in layout.selection_rects(selection_start..selection_end)? {
    // Draw the local-unit line-box rectangle behind the retained text.
}
```

Positions are whole-buffer UTF-8 bytes at extended-grapheme boundaries. CRLF/LFCR
breaks are atomic. Affinity chooses the preceding/following logical grapheme at a
soft wrap or bidi edge. Applications retain anchor/focus, undo drawing transforms
before hit-testing, remap positions after edits, and choose caret thickness and
end-of-line selection indicators. Mixed bidi ranges can produce disjoint rectangles.
Blank lines and omitted wrap whitespace retain caret stops without inventing ink.
Ligature interior carets divide advance evenly; font GDEF caret positions remain a
quality follow-up. This layer does not implement editing, focus, or a text widget.

The index is lazy, boxed, and owned by the immutable snapshot. Ordinary labels
allocate no index vectors. First interaction constructs sorted grapheme/cluster and
visual-line lookup tables; subsequent hit/caret/boundary queries use binary searches.
Selection visits visual cells in affected rows, yielding rectangles without allocating.
Selection-only changes require no reshaping, glyph rasterization, atlas upload, or
prepared-glyph geometry upload. Drawing the selection/caret still records ordinary
primitive draws and their parameters; it is not zero rendering work.

## CPU measurements

Measured 2026-10-06 on Apple M3 Pro, macOS 27.0.1, Rust 1.98.1, release, in three
sequential runs. Values below are ranges of per-run medians in microseconds; memory
is retained interaction storage in KiB. This benchmark initializes no GPU. The
native validation separately uses Metal.

```sh
cargo bench -p astrelis --bench text_interaction
```

| Glyphs | Lines | Cold index µs | Hit µs | Caret µs | Short selection µs | Whole selection µs | Index KiB |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 30 | 1 | 2.292–2.375 | 0.023–0.025 | 0.011–0.012 | 0.043–0.051 | 0.040–0.043 | 2.1 |
| 956 | 7 | 65.833–66.333 | 0.083–0.083 | 0.024–0.024 | 0.149–0.150 | 1.002–1.003 | 61.3 |
| 7,644 | 55 | 551.167–552.791 | 0.112–0.124 | 0.043–0.046 | 0.161–0.164 | 8.122–8.229 | 489.8 |
| 30,574 | 220 | 2297.958–2309.500 | 0.230–0.285 | 0.103–0.124 | 0.175–0.182 | 31.777–32.356 | 1959.0 |

The corpus repeats Latin ligatures, a combining mark, Arabic, and numbers with
SourceSans3/NotoSansArabic at size 16, line height 24, width 800/801. Each cold sample
uses a fresh layout snapshot and times index initialization only; layout/shaping
is excluded and fonts/shaped runs are already warm. Four warmup samples and fifteen
measured samples are used. Warm queries use one retained snapshot: each sample
performs 1,000 operations and reports average cost per operation before the sample
median. Hit points span rows; caret offsets stride across the full source. Short
selection covers up to twenty graphemes near the source midpoint; whole selection
iterates all rectangles. Assertions verify fresh cold indexes and valid caret/hit
results. No end-to-end input latency or GPU execution timing is claimed.

The largest case has 37,888 UTF-8 bytes and 30,574 glyphs. Its cold index costs
2.30–2.31 ms and retains about 1.91 MiB. Warm distributed hit/caret query
medians range from 0.230–0.285/0.103–0.124 µs; enumerating the whole
selection takes 31.78–32.36 µs. These figures
support retaining and warming an index for interactive content. Very large editor
documents still need viewport/incremental indexing rather than eagerly indexing
an entire document. Labels remain on the existing layout/render path.

[Metadata](text-interaction/metadata.json) records commands, source/font/lockfile
hashes and timing boundaries. The three CSV/log pairs retain the measurements.

## Desktop validation

`selectable_text` is a standalone window/event-loop example. A temporary instrumented
copy presented six native frames at 2x DPI: selecting all, moving selection, switching
1x to 4x MSAA, resizing/reflowing, and moving the caret again. Atlas/geometry counters
stayed fixed through selection and MSAA; reflow uploaded replacement glyph geometry,
then caret changes reused it. Duplicate native resize/DPI notifications now reuse
unchanged snapshots and prepared text. No test flags/modes were added to examples.

The RXUI prototype's native macOS window was visually checked for multiline selection,
Arabic, combining marks, installed emoji fallback, list clipping, and custom chart
rendering. Native tooling inspected AccessKit names/roles/focus/value/list hierarchy,
changed an editable value, selected a list option, scrolled the virtual list, and
exercised chart Run/Pause and window zoom/resize. Ordinary native typing while IME
support was enabled produced the controlled value. An instrumented view log confirmed
unchanged atlas/geometry counters for selection-only changes and unchanged text
preparation during chart ticks. Pausing leaves the event loop waiting for input.

Automated checks cover grapheme/ligature/bidi/wrap/blank-line geometry, immutable
snapshot reuse, prepared glyph-buffer identity, selection-only upload counters,
queued controlled edits, grapheme deletion, focus/activation, IME clearing followed
by commit, native byte/grapheme selection conversion and AccessKit consumer text
ranges, virtualized resource bounds, and clipping restoration. In the ten-row
virtualization test a 100,000-row data set retains at most thirteen controls,
including the viewport and edge rows; the desktop example retains more rows when
its viewport is taller. Both libraries also compile for wasm32-unknown-unknown.

The prototype uses explicit rectangles, a single-line field, and direct model
mutation through actions. It leaves general layout/style, richer editor behavior,
and reactivity architecture open. Accessibility lacks per-character geometry/rich
attributes and offscreen row realization; extremely long graphemes beyond AccessKit's
255-byte selectable-unit encoding expose only value. Full VoiceOver sessions and
Chinese/Japanese IME candidate flows have not been validated. Rust tests validate
preedit/commit state transitions without switching the user's native input source.
