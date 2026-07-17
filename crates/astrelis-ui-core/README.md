# astrelis-ui-core

Retained, backend-independent UI tree, layout, routed input, semantics, and
paint generation for Astrelis.

The crate owns no native windows or GPU objects. Taffy is an internal layout
implementation detail, and output is an `astrelis-paint` display list.

The initial vertical slice includes labels, buttons, rows, columns, padding,
and a Unicode-aware single-line text field.

```text
cargo run -p astrelis-ui-core --example settings_window
```

The same settings example runs in a browser using WebGPU. Install the target
and the CLI version matching the workspace lockfile, then build bindings:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.126 --locked
cargo build -p astrelis-ui-core --release \
  --target wasm32-unknown-unknown --example settings_window
wasm-bindgen --target web --out-name astrelis_settings \
  --out-dir crates/astrelis-ui-core/web/pkg \
  target/wasm32-unknown-unknown/release/examples/settings_window.wasm
python3 -m http.server --directory crates/astrelis-ui-core/web 8000
```

Open `http://localhost:8000`. This first browser slice requires WebGPU, owns
one supplied canvas, and deliberately leaves permission-gated browser
clipboard operations unavailable. The bundled Noto Sans font is licensed
under the SIL Open Font License in `assets/OFL.txt`.
