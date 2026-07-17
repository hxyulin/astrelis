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
