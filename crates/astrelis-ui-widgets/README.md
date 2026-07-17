# astrelis-ui-widgets

Reusable retained controls composed from the public `astrelis-ui-core` API.

Milestone 12 adds `RenderView`, a retained texture-backed scene viewport with
rounded clipping, typed input, scheduling snapshots, and allocation hysteresis.

Run the native texture-backed example with:

```text
cargo run -p astrelis-ui-widgets --example render_view
```

The animated color scene responds to pointer position and focused keyboard
input. Its pause control cancels the window-scoped repeating timer so the
desktop runtime returns to waiting for events.

Milestone 11 provides generic in-process drag sources and drop
targets, resizable horizontal and vertical split panes, immediate tooltips,
popovers, keyboard-accessible menus and tabs, selectable retained lists, and
common labeled form compositions. Fixed-extent virtual lists retain only the
visible rows plus configured overscan; users call `sync` after input, viewport,
scroll, or data changes.

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
