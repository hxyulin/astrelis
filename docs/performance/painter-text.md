# Painter prepared-text recording baseline

Measured on 2026-10-05, Apple M3 Pro / Metal, macOS 27.0.1, Rust 1.98.1, release
build. Three sequential runs use sixteen warmups and eighty measured samples per
case and API. [Metadata](painter-text-m3-pro-metadata.json) records source hashes,
compiler/adapter details, font provenance, conditions, and original run logs. Raw
median/p95 measurements: [run 1](painter-text-m3-pro-run-1.csv),
[run 2](painter-text-m3-pro-run-2.csv), [run 3](painter-text-m3-pro-run-3.csv).

```sh
cargo bench -p astrelis --bench painter_text
```

Each case uses the same PreparedText, target, draw count, placement, color, and
resulting affine transform for direct TextRenderer and Painter calls. Text repeats
Latin ligatures/kerning, combining marks, Arabic, numbers, and spaces using bundled
Source Sans 3 and Noto Sans Arabic. Paragraph width is 400, font size 20, line height
28, raster scale one. Input sizes are Unicode scalar counts; drawable glyph counts
are 84, 851, and 8,537. Each prepared resource occupies one coverage atlas page,
producing one instanced GPU batch per text draw. Cases record one or one hundred
repeated placements into a 64×64 framebuffer at 1x MSAA. Most glyphs in longer
paragraphs lie outside that viewport; these are command-recording comparisons,
not a GPU throughput or visible-text rendering benchmark. Color atlases, many-page
text, interleaved workloads, and MSAA are outside these timing cases.

Identity cases enter a painting session and draw without a scope. Transformed cases
also enter one translated child scope; each Painter draw composes its affine
transform. Direct draws use an equivalent transform precomposed outside timing.
This gives a lower-level reference with placement work already done. Direct/Painter
order alternates each sample to reduce systematic first/second recording bias.
Pipeline creation, font loading, layout, rasterization, atlas and static geometry
uploads, pass creation, submission, and GPU completion waits are outside timing.
The timed interval includes Painter session/scope construction and drop.

Ranges of per-run recording medians in microseconds; values represent the entire
stream, including all one hundred calls in the larger cases:

| Input scalars | Draws | Child scope | Direct TextRenderer | Painter |
| ---: | ---: | :---: | ---: | ---: |
| 100 | 1 | No | 0.500–0.541 | 0.500–0.542 |
| 100 | 1 | Yes | 0.541–0.542 | 0.542–0.583 |
| 100 | 100 | No | 8.292–10.208 | 8.334–9.396 |
| 100 | 100 | Yes | 7.875–9.062 | 7.917–9.250 |
| 1,000 | 1 | No | 0.375–0.417 | 0.375–0.458 |
| 1,000 | 1 | Yes | 0.416–0.459 | 0.417–0.500 |
| 1,000 | 100 | No | 10.417–10.812 | 10.208–10.709 |
| 1,000 | 100 | Yes | 10.625–10.834 | 10.854–11.125 |
| 10,000 | 1 | No | 0.416–0.458 | 0.438–0.458 |
| 10,000 | 1 | Yes | 0.417–0.458 | 0.417–0.500 |
| 10,000 | 100 | No | 11.459–12.521 | 10.250–12.375 |
| 10,000 | 100 | Yes | 10.146–11.062 | 10.313–11.166 |

Single-call differences are small relative to timer granularity and run-to-run
noise. Some Painter medians are lower than their references; this does not establish
an optimization. The measurements show no substantial CPU recording cost from
this facade on this workload and machine. They do not establish a portable
performance threshold or measure GPU execution, startup, or glyph-cache pressure.

Runtime counter assertions require zero additional glyph-cache misses, atlas payload
writes, or static geometry uploads during drawing. Both APIs issue the same number
of text batches; Painter uploads exactly 48 bytes of draw parameters per text call.
Painter preserves the underlying renderer's page ownership/completion leases rather
than introducing a second cache or geometry buffer. Separate GPU readback tests
compare mixed image/primitive/text/custom-mesh output, including nested transforms,
DPI, clipping, viewports, color glyphs, and 1x/4x MSAA. Those tests establish rendering
behavior; they are outside these benchmark intervals.
