# astrelis-text-gpu

Device-bound Swash glyph rasterization and bounded R8/RGBA atlas caching for
`astrelis-text` layouts. The cache is consumed by `astrelis-paint-gpu` so text
shares display-list ordering, transforms, clipping, and blending.
