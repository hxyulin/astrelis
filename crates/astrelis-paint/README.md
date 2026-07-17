# astrelis-paint

Backend-independent paths, immutable images, retained text layouts, semantic
display lists, and the `Painter` recording API.

Paint sources include solid colors plus clamped linear and circular radial
gradients. The semantic display list supports filled and stroked paths,
rectangles, rounded rectangles, ellipses, images, text, affine transforms,
clips, and nested multiplicative opacity. Opacity applies to each draw rather
than creating an isolated compositing layer; filters, shadows, blend modes,
and isolated groups are intentionally deferred.

This crate deliberately has no platform or GPU dependency. Display-list
generation, validation, inspection, and snapshot testing can run headlessly.

```sh
cargo run -p astrelis-paint --example headless_display_list
```
