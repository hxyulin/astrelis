//! Astrelis — a modular Rust game engine built on wgpu.
//!
//! This is the top-level facade crate that re-exports all engine
//! sub-crates for convenience. Use `astrelis::prelude::*` for the
//! most common types, or access specific modules via their namespaced
//! re-exports (e.g. `astrelis::gpu::Buffer`).
//!
//! # Example
//!
//! ```ignore
//! use astrelis::prelude::*;
//!
//! struct MyGame;
//!
//! impl Plugin for MyGame {
//!     fn build(&self, app: &mut App) {
//!         app.add_system(Phase::Update, |res| {
//!             let time = res.get::<Time>();
//!             let input = res.get::<InputState>();
//!             // game logic
//!         });
//!     }
//! }
//!
//! fn main() {
//!     App::new()
//!         .add_default_plugins()
//!         .add_plugin(MyGame)
//!         .run();
//! }
//! ```

#![warn(missing_docs)]

/// Application framework: plugins, systems, resources, events.
pub use astrelis_app as app;

/// Asset loading and management.
pub use astrelis_assets as assets;

/// Core types: math, color, geometry, IDs.
pub use astrelis_core as core;

/// GPU abstraction (wgpu wrappers).
pub use astrelis_gpu as gpu;

/// Polling-style input state.
pub use astrelis_input as input;

/// In-engine CPU/GPU profiling.
pub use astrelis_profiling as profiling;

/// 2D rendering: sprites, shapes, camera, batching.
pub use astrelis_render_2d as render_2d;

/// Scene tree with columnar component storage.
pub use astrelis_scene as scene;

/// Text shaping and font management.
pub use astrelis_text as text;

/// Windowing and event loop.
pub use astrelis_window as window;

/// Curated prelude for the 80% case.
///
/// Import with `use astrelis::prelude::*` to get the most commonly
/// used types without importing individual sub-crates.
pub mod prelude {
    // App framework.
    pub use astrelis_app::{App, Events, Phase, Plugin, Ref, RefMut, Resources, Time};

    // Core types.
    pub use astrelis_core::color::Color;
    pub use astrelis_core::math::*;

    // 2D rendering.
    pub use astrelis_render_2d::{Camera2D, Renderer2D, SpriteOptions, TextureHandle};

    // Scene tree.
    pub use astrelis_scene::{Component, NodeId, Scene, SceneError, ScenePlugin, Transform};

    // Input.
    pub use astrelis_input::InputState;

    // Assets.
    pub use astrelis_assets::{Asset, AssetLoader, AssetServer, Handle};

    // Window types commonly used in game code.
    pub use astrelis_window::keyboard::KeyCode;
    pub use astrelis_window::mouse::MouseButton;
}
