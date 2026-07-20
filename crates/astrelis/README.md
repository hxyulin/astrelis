# Astrelis

Umbrella façade for the rewritten modular Astrelis engine.

```toml
[dependencies]
astrelis = "=0.3.0-rc.1"
```

The façade exposes each component crate as a named module. The default features
include the wgpu/winit backends, 2D and 3D renderers, and retained UI. Test
backends are available through `features = ["testing"]`. Applications may
disable defaults or depend directly on individual `astrelis-*` crates to reduce
their dependency graph.

Astrelis 0.3 is a breaking rewrite of the pre-rewrite 0.2 releases; see the
repository migration guide before upgrading.
