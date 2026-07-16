# astrelis-paint

Backend-independent paths, immutable images, retained text layouts, semantic
display lists, and the `Painter` recording API.

This crate deliberately has no platform or GPU dependency. Display-list
generation, validation, inspection, and snapshot testing can run headlessly.

```sh
cargo run -p astrelis-paint --example headless_display_list
```
