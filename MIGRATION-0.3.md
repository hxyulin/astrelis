# Migrating from Astrelis 0.2 to 0.3

Astrelis 0.3 is a breaking rewrite. Treat it as a new engine API rather than a
routine SemVer upgrade.

## Package selection

The `astrelis` package is now a thin umbrella façade. New applications should
prefer the smallest relevant packages:

- application lifecycle and scheduling: `astrelis-app`;
- platform abstraction and native/browser backend: `astrelis-platform` and
  `astrelis-platform-winit`;
- GPU abstraction and backend: `astrelis-gpu` and `astrelis-gpu-wgpu`;
- painting and composition: `astrelis-paint`, `astrelis-paint-gpu`, and
  `astrelis-compositor`;
- scene rendering: `astrelis-render`, `astrelis-render-2d`, and
  `astrelis-render-3d`;
- retained interfaces: `astrelis-ui`, `astrelis-ui-widgets`,
  `astrelis-ui-docking`, and `astrelis-ui-host`;
- deterministic testing: `astrelis-platform-test` and `astrelis-ui-testing`.

During the release-candidate series, pin every direct Astrelis dependency:

```toml
astrelis-ui = "=0.3.0-rc.1"
```

## Removed pre-rewrite modules

The old assets, audio, ECS, egui, geometry, input, scene, test-utils, and
`astrelis-winit` packages have no direct 0.3 compatibility layer. Keep a 0.2
branch for applications that still require them, or migrate the relevant
functionality to the new platform/rendering model before updating.

## Application and UI migration

- Replace the former monolithic application entry point with `astrelis-app`
  plus a platform backend.
- Create windows through `astrelis-platform`; connect retained trees through
  `astrelis-ui-host` when using the supplied wgpu compositor path.
- Move UI code to retained `Ui<Message>` trees. Application state remains
  application-owned and receives typed messages from routed interactions.
- Replace renderer-specific UI integration with display lists, compositor
  views, or `RenderView` as appropriate.
- Rebuild automated coverage around the semantic action and normalized
  snapshot helpers in `astrelis-ui-testing`.

The `0.3.0-rc` series may still make breaking changes. Each later candidate
will document migrations in the changelog.
