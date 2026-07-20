//! Umbrella façade for the modular Astrelis engine.
//!
//! Applications may depend on individual `astrelis-*` crates to minimize their
//! dependency graph. This crate keeps the complete engine discoverable from one
//! import root and is the successor to the pre-rewrite `astrelis` package.

#![warn(missing_docs)]

/// Application scheduling, invalidation, and runtime integration.
pub use astrelis_app as app;
/// Ordered UI and scene composition.
pub use astrelis_compositor as compositor;
/// Shared math, color, geometry, identifiers, and logging.
pub use astrelis_core as core;
/// Backend-neutral GPU API.
pub use astrelis_gpu as gpu;
/// Wgpu implementation of the GPU API.
#[cfg(feature = "wgpu")]
pub use astrelis_gpu_wgpu as gpu_wgpu;
/// Backend-independent display lists and painting.
pub use astrelis_paint as paint;
/// GPU display-list renderer.
pub use astrelis_paint_gpu as paint_gpu;
/// Backend-neutral window, lifecycle, input, and clipboard APIs.
pub use astrelis_platform as platform;
/// Deterministic platform backend for tests.
#[cfg(feature = "testing")]
pub use astrelis_platform_test as platform_test;
/// Native and browser-canvas winit platform backend.
#[cfg(feature = "winit")]
pub use astrelis_platform_winit as platform_winit;
/// CPU and GPU timeline profiling.
pub use astrelis_profiling as profiling;
/// Shared scene-rendering targets and frame vocabulary.
pub use astrelis_render as render;
/// Batched 2D scene rendering.
#[cfg(feature = "render-2d")]
pub use astrelis_render_2d as render_2d;
/// Lit 3D scene rendering.
#[cfg(feature = "render-3d")]
pub use astrelis_render_3d as render_3d;
/// Font discovery, shaping, and retained text layout.
pub use astrelis_text as text;
/// GPU glyph rasterization and atlas caching.
pub use astrelis_text_gpu as text_gpu;
/// Ergonomic retained UI façade.
#[cfg(feature = "ui")]
pub use astrelis_ui as ui;
/// Extensible retained UI core.
#[cfg(feature = "ui")]
pub use astrelis_ui_core as ui_core;
/// Serializable editor docking workspaces.
#[cfg(feature = "ui")]
pub use astrelis_ui_docking as ui_docking;
/// Cross-platform retained UI window hosting.
#[cfg(feature = "ui")]
pub use astrelis_ui_host as ui_host;
/// Deterministic semantic and display-list UI testing.
#[cfg(feature = "testing")]
pub use astrelis_ui_testing as ui_testing;
/// Reusable retained widget compositions.
#[cfg(feature = "ui")]
pub use astrelis_ui_widgets as ui_widgets;
