# Viewport text snapshots

`cargo bench -p astrelis --bench text_viewport --locked` compares independent full
and viewport buffers over the same embedded-font content. It checks exact size,
visual-line count and the expected retained glyph count before timing. One warmup
is discarded, then three samples measure cold evaluation, twenty distinct scroll
positions and a middle-of-document insertion. Both multiline documents and long
single paragraphs are tested at 1,000 and 10,000 repeated units. This is CPU wall
time; GPU execution is not measured.

The [raw CSV](text-viewport-run.csv), [build/run log](text-viewport-run.log) and
[machine metadata](text-viewport-metadata.txt) are committed together. The run uses
an Apple M5 Max shared development host with other builds in flight. Treat times
as noisy observations, not frame-budget guarantees. The font system is shared and
already loaded; cold buffers do not mean cold font-file caches. The benchmark
always evaluates full before viewport, so it is not a randomized comparison.

The 10,000-line case has 360,000 full-snapshot glyphs and retains 756 glyphs for a
440-unit viewport. The wrapped single-paragraph case has 367,647 full glyphs and
retains 2,085. These counts describe Astrelis snapshots, not the backend's retained
shaped glyphs. Scrolling scans all line metrics and paragraph offsets, so its CPU
cost still grows with document size. An identical viewport returns a cached Arc;
the scrolling timing deliberately uses different ranges to avoid measuring that
cache hit.

For 10,000 multiline units, recorded medians were 1.35 s full / 1.16 s viewport
for cold evaluation, 0.181 ms per viewport scroll, and 32.7 ms full / 1.20 ms
viewport after the insertion. For 10,000 units in one paragraph, cold medians were
1.04 s / 0.928 s, scrolling 1.67 ms, and editing 912 ms / 1,088 ms. The three
single-paragraph scroll averages ranged from 0.089 to 3.15 ms on this shared host;
these figures do not establish a latency bound or a general speedup.

Cold viewport seeking still shapes/reflows the full document for exact scroll
extents. It is therefore comparable to full cold evaluation. A multiline edit
reuses unaffected shaped paragraphs and avoids rebuilding offscreen snapshot
glyphs. A long single-paragraph edit still reshapes that entire paragraph; the
benchmark deliberately exposes that limit rather than promising work proportional
to the visible text. Vertical culling cannot reduce an unwrapped single line's
geometry. See the [API contract](../text.md#visible-document-snapshots) for overscan,
selection and resource ownership.

The regression suite independently compares visible snapshots to full layout
geometry and multilingual interaction after scrolling and editing. Its headless
Metal readback also compares worker-prepared visible text against a full draw at
the same scroll translation, asserting substantially fewer prepared glyph quads.
No renderer implementation was changed for this API.
