# Changelog

All notable changes to the rewritten Astrelis engine are documented here.

## 0.3.0-rc.2 — Unreleased

### Changed

- Routed scrolling now preserves both axes, line-versus-pixel granularity,
  gesture phase, and cursor position. Native trackpad pinch and pan gestures
  are routed to hovered widgets and exposed by render views.
- UI window hosts created from one graphics context share a configured GPU
  device and explicitly report device loss.
- Platform windows expose fallible command/query variants while preserving the
  existing convenience fallbacks.
- Generic image presentation and viewport navigation moved into the engine
  widget layer; editor docking policy moved to RXUI.

### Added

- Optional `astrelis-image` PNG, JPEG, and WebP decoding.
- Read-only retained UI invalidation reasons for host and devtools integration.
- A host-level accessibility adapter seam that publishes semantic trees and
  dispatches platform actions through retained semantic handling.
- `WindowHosts`, an application-owned native multi-window collection with
  shared graphics and per-window retained UI, event routing, and presentation.
- **WIP:** External drag routing APIs for carrying an in-process payload
  between independent retained UI trees hosted by different native windows.
  Native cross-window pointer/drag routing is not implemented yet.
- Client-area desktop-position queries for translating captured pointer input
  between native windows.

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
