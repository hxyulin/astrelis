# Coverage text quality check

The standalone [text_quality example](../../crates/astrelis/examples/text_quality.rs)
owns its window, events, surface recovery, DPI preparation, and retained resources.
Space toggles supported 1x/4x MSAA; Z toggles 2x/4x geometric magnification. No test
or capture flags are added to the window example.

```sh
cargo run -p astrelis --example text_quality
```

The nine comparison rows cover 9/12/16-unit text at whole/fractional physical-pixel
origins, hinting on/off, preparing at actual DPI versus magnifying a 1x raster,
magnifying a native raster versus preparing at final size, a rotated native versus
denser raster, advanced shaping/fallback, and magnified coverage/COLR/PNG glyphs.
Caption/layout units scale explicitly with DPI; fractional offsets stay half a
physical pixel. Text cells clip independently, keeping large samples out of their
neighbors. Sources are the bundled Source Sans 3, Noto Sans Arabic, and original
TestColor fixture; color glyphs are geometric test marks rather than a general
emoji font. This is a coverage comparison, not a distance-field implementation.

## Captures and reproduction

Four offscreen captures render the actual example's preparation/drawing functions
at 1120×960 logical units, explicit DPI 1 or 2, zoom 4, and 1x/4x MSAA. The target is
RGBA8UnormSrgb with opaque backgrounds. Inspect them at native image size; browser
or image-viewer resizing applies another filter and obscures small-pixel differences.

| DPI | 1x MSAA | 4x MSAA |
| ---: | --- | --- |
| 1 | [1120×960 image](text-quality-dpi-1-1x.png) | [1120×960 image](text-quality-dpi-1-4x.png) |
| 2 | [2240×1920 image](text-quality-dpi-2-1x.png) | [2240×1920 image](text-quality-dpi-2-4x.png) |

[Metadata](text-quality-metadata.json) records source/fixture/image hashes, target
settings, adapter, and capture log. The helper generates a one-off headless binary
from the example, copies GPU readback into lossless PNGs using Python's standard
library, and removes the temporary Rust file. It creates the file exclusively and
will not overwrite an existing example with that name. It requires a working GPU
and 4x MSAA for the full set; no Python packages are installed.

```sh
python3 docs/performance/capture-text-quality.py /tmp/astrelis-quality
```

Captures were visually inspected for legible captions, correct comparisons, cell
clipping, and shaped multilingual/color output. They are review artifacts, not
backend-independent golden images or an automated perceptual-quality score.

## Findings

At 4x geometric magnification, the native coverage raster visibly softens and exposes
its source sampling; preparing at the final physical size retains much cleaner
edges. The difference persists in the 4x MSAA capture. MSAA does not recover the
outline detail absent from a small glyph image. The 2x-DPI capture likewise shows
why preparing at actual DPI differs from scaling a 1x image after preparation.

Fractional movement deliberately filters the same prepared raster. It can change
small-text contrast without a new glyph-cache key; this path uses zero raster phase.
Hinting, denser preparation, and rotation are visible comparison choices, not a
claim that one setting wins for every size/font. The present coverage path remains
useful for text prepared near its displayed physical size.

These results motivate an explicitly prepared scalable outline representation while
retaining coverage for small text and images for intrinsic color. Future distance
fields should be compared against these final-size coverage references at the same
layout, DPI, transform, and output size. Generation cost, atlas memory, sharp corners,
thin strokes, minification, and effect-range limits still need separate validation;
these captures do not demonstrate SDF/MSDF/MTSDF quality.
