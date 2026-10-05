# Distance-field fill: quality and CPU cost

Measured on Apple M3 Pro / Metal with wgpu 30.0.1 and bymsdfgen-core 0.1.1.
This is the uncommitted implementation on base fff3604; metadata records exact
source hashes. Three sequential release runs use 20 measured samples after four
warmups. No other cargo jobs, capture binaries, or example windows were active
during these runs. Each table reports the range of the three run medians; raw CSVs
preserve per-run p95 values.

## Boundaries and workloads

The `text_distance_fields` benchmark compares hinted native-size coverage with
unhinted MTSDF fields at font size 20, on a 1920×1080 RGBA8Unorm framebuffer, 1x MSAA.
No shaping, pipeline creation, mapping, or GPU completion wait is included in
preparation or recording timings. CPU frame time starts before acquisition and
ends after finish/submission; it is not GPU execution or presentation time.

- `repeated_1000`: 1,000 input scalars, 851 drawable glyphs, 31 cold image/blank keys,
  Latin/CFF and Arabic/variable-TTF fallback. All prepared image bounds fit the viewport.
- `unique_256`: 256 nominally distinct Source Sans 3 glyphs separated by spaces,
  512 input scalars, 256 drawable glyphs, and 257 cold keys including blank space.
- Cold preparation clears the atlas/cache each sample, generates/uploads images,
  and creates geometry. Queue upload flushes and completion waits occur afterward.
- Warm preparation reuses image keys and creates new immutable geometry.
  An additional DPI-change preparation outside timing asserts field-image reuse.
- Immediate post-preparation recording and CPU frame totals are retained in the
  CSV separately from steady drawing. The steady stages retain one PreparedText
  with no new preparation between frames. For 100 calls, identical visible
  paragraphs intentionally overlap at the same origin; this is a CPU submission
  workload, not a realistic GPU text-overdraw recommendation.
- Counter assertions require zero cache misses, atlas writes, and geometry uploads
  during drawing, and 48 placement/color bytes per nonempty call. Glyph geometry
  is 48 bytes per drawable glyph in both modes. Bounds checks establish visibility;
  separate GPU captures and pixel tests verify rendering.

## Preparation and steady drawing

| Input / representation | Cold preparation (ms) | Warm new geometry (µs) | Steady one-call recording (µs) | Steady one-call CPU frame (µs) |
| --- | ---: | ---: | ---: | ---: |
| Repeated / coverage | 0.299–0.305 | 72.729–74.374 | 0.417–0.583 | 28.395–33.938 |
| Repeated / MTSDF 32, range .25 | 18.735–19.740 | 74.645–80.333 | 0.541–0.542 | 28.709–29.605 |
| Repeated / MTSDF 64, range .25 | 51.965–54.060 | 79.874–83.854 | 0.520–0.584 | 30.167–31.250 |
| Repeated / MTSDF 96, range .25 | 102.627–104.770 | 80.583–81.604 | 0.584–0.667 | 30.375–36.355 |
| Repeated / MTSDF 64, range .125 | 39.415–41.104 | 78.499–79.562 | 0.542–0.583 | 30.188–32.584 |
| Repeated / MTSDF 64, range .5 | 86.221–89.416 | 80.750–84.208 | 0.541–0.584 | 28.896–31.959 |
| Unique / coverage | 1.897–1.947 | 44.438–46.667 | 0.541–0.625 | 29.395–31.625 |
| Unique / MTSDF 64, range .25 | 555.292–564.762 | 61.584–62.958 | 0.583–0.688 | 31.355–32.125 |

| Retained 1,000-scalar paragraph, 100 draw calls | Recording (µs) | CPU frame (µs) |
| --- | ---: | ---: |
| Coverage | 9.959–10.937 | 56.875–64.895 |
| MTSDF 64, range .25 | 9.916–17.437 | 57.917–66.375 |

Steady recording remains small, with unchanged upload structure. Cold generation
is substantial and grows with density, represented range/padding, and the number
and complexity of outlines. Warm geometry preparation is somewhat more expensive
than coverage in this workload. A repeated paragraph's glyph vocabulary is not a
substitute for testing cold unique text: preparing 256 distinct fields takes well
over half a second here. Keep coverage as the default and prepare fields ahead of
use. These results support caller-controlled staged preparation or a future worker/
prebaked resource path before using fields for arbitrary changing text.

## Logical atlas memory

The repeated coverage case uses one 1 MiB R8 page and uploads 2,983 payload bytes.
At 32/64/96 texels per EM with range .25, fields use one 4 MiB RGBA8 page and upload
77,912 / 276,256 / 597,736 bytes. At density 64, range .125/.5 changes payload to
192,288 / 490,272 bytes. The unique case uploads 33,669 coverage bytes versus
2,869,852 field bytes; each still fits one page of its respective kind.

Atlas bytes count logical texture allocations and exclude driver overhead,
immutable geometry, upload buffers, and backend staging. The quality scene retains
coverage, intrinsic-color, and field pages together: three pages totaling 9 MiB.
Prepared/recording/completion leases and explicit AtlasFull recovery are unchanged.

## Fill quality

The standalone `text_distance_fields` example owns its complete window lifecycle.
Space toggles 1x/4x MSAA; Z toggles 2x/4x magnification. It compares:

- Hinted coverage and 64-texel/EM fields at 9, 12, and 16 logical units, including
  fractional field placement.
- Final-size coverage against a magnified field without regeneration.
- Density 32/64/96 and full ranges .125/.25/.5 EM.
- Rotation, nonuniform affine scaling, combining marks/ligatures, Arabic fallback,
  and mixed monochrome/COLR/PNG intrinsic artwork.

These captures use the actual example scene on RGBA8UnormSrgb framebuffers,
with opaque background and zoom 4. RGB PNG encoding does not modify the render.
All four captures passed a GPU validation error scope and were visually inspected:

| DPI | 1x MSAA | 4x MSAA |
| --- | --- | --- |
| 1 | [1120×960](text-distance-fields-dpi-1-1x.png) | [1120×960](text-distance-fields-dpi-1-4x.png) |
| 2 | [2240×1920](text-distance-fields-dpi-2-1x.png) | [2240×1920](text-distance-fields-dpi-2-4x.png) |

The tested magnified fields preserve edges/corners close to final-size coverage;
small hinted coverage remains a useful alternative. Density/range changes are
visible around fine details and filtering boundaries. This is a visual comparison,
not a universal font quality score. Automated tests separately compare unhinted
coverage references with fields for CFF contours/holes, ligatures/combining marks,
synthetic italic, Arabic variable weights, rotation, and nonuniform scaling. Other
pixel tests cover color/opacity/order, scissor, MSAA/depth, cross-renderer Painter
use, invalid options, source reuse, budgets, and completion leases.

## Reproduction and limits

```sh
cargo run -p astrelis --example text_distance_fields
cargo bench -p astrelis --bench text_distance_fields
python3 docs/performance/capture-text-distance-fields.py OUTPUT_DIRECTORY
```

The capture helper copies scene functions into an exclusive temporary headless
binary, runs it, and deletes it afterward. No capture flags or shared support module
are added to the window example. It requires a GPU and 4x MSAA, uses only Python's
standard library for PNG output, and records source/image hashes and font provenance.

GPU execution is not measured. The earlier local Metal timestamp diagnostic returned
incomplete/stale samples; see [text workloads](text-workloads.md). CPU submission or
completion waits must not be interpreted as field shader execution cost. Browser
rendering, Windows/Linux GPU runs, very large magnification, and comprehensive font
format/outline parity have not been measured. The chosen generator is a young pinned
Rust dependency; the API does not expose its types.

Raw runs: [run 1](text-distance-fields-m3-pro-run-1.csv),
[run 2](text-distance-fields-m3-pro-run-2.csv),
[run 3](text-distance-fields-m3-pro-run-3.csv).
[Benchmark metadata](text-distance-fields-m3-pro-metadata.json) and
[capture metadata](text-distance-fields-metadata.json) record exact inputs.
See the [public API contract](../text-distance-fields.md) for defaults, bounds,
color fallback, lifetime, and the separate future effect contract.
