# astrelis-ui-widgets

Reusable retained controls composed from the public `astrelis-ui-core` API.

The first Milestone 11 slice provides generic in-process drag sources and
painted drop targets. Split panes, overlay controls, and virtual lists follow
in subsequent slices.

Run the native gallery with:

```text
cargo run -p astrelis-ui-widgets --example widget_gallery
```

Build the same source for WebGPU/WASM with:

```text
cargo build -p astrelis-ui-widgets --release \
  --target wasm32-unknown-unknown --example widget_gallery
wasm-bindgen --target web --out-name astrelis_widgets \
  --out-dir crates/astrelis-ui-widgets/web/pkg \
  target/wasm32-unknown-unknown/release/examples/widget_gallery.wasm
python3 -m http.server --directory crates/astrelis-ui-widgets/web 8000
```
