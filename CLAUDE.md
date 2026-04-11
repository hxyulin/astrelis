# Astrelis

A modular Rust game engine built on wgpu.

## Collaboration Instructions

1. **Always ask for clarification rather than assuming intent.** If a
   request is vague, ambiguous, or appears incorrect, stop and ask
   before acting.
2. **Consider multiple approaches.** When responding to a request, think
   through alternatives and surface better options if they exist —
   explain the trade-offs rather than silently picking one.
3. **Be educational.** Offer suggestions, guidelines, and brief
   explanations of *why* an approach is preferred. Engine design has
   a lot of subtle trade-offs (incrementality, parallelism, cache
   invalidation, dependency resolution); treat each interaction as a
   chance to make those trade-offs explicit.
4. **Plan before you build.** Every feature — no matter how small it
   seems — must be planned thoroughly before any code is written.
   Discuss the design, edge cases, and integration points with the user
   first, and only begin implementation once the plan is agreed upon.

## Project Structure

All engine crates live under `crates/astrelis-{domain}`. One concern
per crate — never mix unrelated responsibilities in a single crate.

### Layered Architecture

```
Layer 4: astrelis                 (facade)
Layer 3: astrelis-app, astrelis-ui-*, astrelis-scene
Layer 2: astrelis-gpu, astrelis-window, astrelis-render-2d, astrelis-assets, ...
Layer 1: astrelis-profiling
Layer 0: astrelis-core
```

**Rule:** Crates may only depend on crates in equal or lower layers.
No circular dependencies.

### Backend Agnosticism

Windowing and GPU use concrete implementations (no trait indirection):
- `astrelis-window` wraps winit directly.
- `astrelis-gpu` wraps wgpu directly.
- `astrelis-profiling` is a custom in-engine profiler. CPU and GPU
  spans land on a single global timeline; the viewer
  (`astrelis-profiling-egui`) reads from it in-process. Always on —
  no feature flags, no external tool.
- Feature flags select optional dependencies, **not** architectural
  boundaries.

## Code Conventions

- **Edition:** 2024
- **Docs:** All public items must have doc comments.
  All crates use `#![warn(missing_docs)]`.
- **Lints:** Workspace-level clippy and rustc lints (see root
  `Cargo.toml`).
- **Dependencies:** All external dependencies declared in
  `[workspace.dependencies]` in the root `Cargo.toml`. Crate-level
  `Cargo.toml` files reference them with `{ workspace = true }`.
- **Profiling:** Use `astrelis_profiling::profile_function!()` and
  `astrelis_profiling::profile_scope!()`. For counters and plots,
  use `astrelis_profiling::profile_counter!()` and
  `astrelis_profiling::profile_plot!()`. Call `new_frame()` once
  per frame to drain buffers into the global timeline.
- **Commits:** Conventional commits — `feat:`, `fix:`, `chore:`,
  `docs:`, `refactor:`, `test:`.
