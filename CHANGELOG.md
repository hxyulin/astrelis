# Changelog

All notable changes to the rewritten Astrelis engine are documented here.

## 0.3.0-rc.1 — Unreleased

This release candidate replaces the pre-rewrite `0.2.x` architecture. It is a
new modular native application, GPU, rendering, text, retained UI, and testing
stack rather than a source-compatible upgrade.

### Added

- Backend-neutral platform and GPU APIs with winit and wgpu implementations.
- Idle-aware application scheduling, invalidation, timers, and profiling.
- Backend-independent vector painting, text shaping, and GPU composition.
- Retained UI core with routed events, focus, IME, clipboard, semantics, drag
  and drop, overlays, virtualization, docking, host, and deterministic tests.
- Batched 2D and lit 3D renderers plus texture-backed UI render views.
- A new `astrelis` umbrella façade over the modular crate family.
- Native and browser WebGPU examples and validation paths.

### Changed

- Every rewritten crate now shares version `0.3.0-rc.1` and requires Rust 1.88.
- Public packages use exact prerelease requirements for other Astrelis crates.

### Compatibility

- The former assets, audio, ECS, egui, geometry, input, scene, test-utils, and
  `astrelis-winit` APIs are not carried forward by compatibility shims.
- Consumers must select the new modular crates and migrate their application
  lifecycle, rendering, and UI integration explicitly.
