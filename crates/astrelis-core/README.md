# astrelis-core

Foundational value types shared by Astrelis crates.

The crate deliberately contains no windowing, rendering, application-runtime,
or UI policy. It currently provides:

- linear algebra types re-exported from `glam`;
- packed GPU-transfer representations;
- linear RGBA colors;
- logical and physical geometry with type-safe coordinate spaces;
- typed identifiers;
- optional `tracing` subscriber initialization.

## Features

- `tracing-init` (default): enables `astrelis_core::logging::init_default`.

Disable default features when an application installs its own tracing
subscriber:

```toml
astrelis-core = { version = "0.0.0", default-features = false }
```

## Example

```rust
use astrelis_core::geometry::{Logical, Physical, Point};

let logical = Point::<Logical>::new(120.0, 80.0);
let physical: Point<Physical> = logical.to_physical(2.0);

assert_eq!(physical.x, 240.0);
assert_eq!(physical.y, 160.0);
```

## License

MIT
