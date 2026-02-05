//! Astrelis - A modular game engine
//!
//! Astrelis is a Rust game engine built around a flexible plugin system.
//! It provides core functionality for game development including:
//!
//! - **Plugin System**: Extensible architecture for adding features
//! - **Asset Management**: Type-safe asset loading and caching
//! - **Rendering**: GPU-accelerated rendering with wgpu
//! - **Text Rendering**: High-quality text with cosmic-text
//! - **Input Handling**: Keyboard, mouse, and gamepad input
//! - **Windowing**: Cross-platform window management
//!
//! # Quick Start
//!
//! ```ignore
//! use astrelis::prelude::*;
//!
//! struct MyGame {
//!     engine: Engine,
//! }
//!
//! impl App for MyGame {
//!     fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
//!         // Game logic here
//!     }
//! }
//!
//! fn main() {
//!     run_app(|ctx| {
//!         let window = ctx.create_window(WindowDescriptor::default()).unwrap();
//!         
//!         let engine = Engine::new()
//!             .add_plugin(AssetPlugin)
//!             .add_plugin(RenderPlugin)
//!             .build();
//!
//!         Box::new(MyGame { engine })
//!     });
//! }
//! ```
//!
//! # Architecture
//!
//! The engine is built around the `Plugin` trait, which allows adding
//! functionality in a modular way. Plugins can:
//!
//! - Register resources with the engine
//! - Hook into the update and render lifecycle
//! - Depend on other plugins
//!
//! # Default Plugins
//!
//! - `AssetPlugin` - Asset loading and management
//! - `RenderPlugin` - Graphics context and rendering (requires window)
//! - `TextPlugin` - Text rendering capabilities
//! - `InputPlugin` - Input state management

pub mod application;
pub mod engine;
pub mod plugin;
pub mod resource;
pub mod task_pool;
pub mod time;

#[cfg(feature = "assets")]
pub mod plugins;

// Re-export core types
pub use astrelis_core as core;
pub use astrelis_core::math;

// Re-export sub-crates based on features
#[cfg(feature = "winit")]
pub use astrelis_winit as winit;
#[cfg(feature = "winit")]
pub use astrelis_winit::{
    WindowId,
    app::{App, AppCtx, AppFactory, run_app},
    event::{Event, EventBatch, HandleStatus},
    window::{Window, WindowDescriptor},
};

#[cfg(feature = "assets")]
pub use astrelis_assets as assets;

#[cfg(feature = "render")]
pub use astrelis_render as render;

#[cfg(feature = "text")]
pub use astrelis_text as text;

#[cfg(feature = "input")]
pub use astrelis_input as input;

#[cfg(feature = "ui")]
pub use astrelis_ui as ui;

#[cfg(feature = "egui")]
pub use astrelis_egui as egui;

#[cfg(feature = "audio")]
pub use astrelis_audio as audio;

// Re-export engine types
pub use application::ApplicationBuilder;
pub use engine::{Engine, EngineBuilder, EngineError};
pub use plugin::{FnPlugin, Plugin, PluginGroup};
pub use resource::{Resource, Resources};
pub use task_pool::TaskPool;
pub use time::Time;

// Re-export plugin types when available
#[cfg(feature = "assets")]
pub use plugins::AssetPlugin;

#[cfg(feature = "assets")]
pub use plugins::AsyncRuntimePlugin;

#[cfg(all(feature = "render", feature = "winit"))]
pub use plugins::{RenderContexts, RenderPlugin};

#[cfg(feature = "text")]
pub use plugins::TextPlugin;

#[cfg(feature = "input")]
pub use plugins::InputPlugin;

#[cfg(feature = "assets")]
pub use plugins::TimePlugin;

#[cfg(feature = "assets")]
pub use plugins::{DefaultPlugins, MinimalPlugins};

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::application::ApplicationBuilder;
    pub use crate::engine::{Engine, EngineBuilder, EngineError};
    pub use crate::plugin::{FnPlugin, Plugin};
    pub use crate::resource::{Resource, Resources};
    pub use crate::task_pool::TaskPool;
    pub use crate::time::Time;

    // Core math types
    pub use astrelis_core::math::{Mat4, Vec2, Vec3, Vec4};

    // Winit types
    #[cfg(feature = "winit")]
    pub use astrelis_winit::{
        WindowId,
        app::{App, AppCtx, run_app},
        event::{Event, EventBatch, HandleStatus},
        window::{Window, WindowDescriptor},
    };

    // Asset types
    #[cfg(feature = "assets")]
    pub use astrelis_assets::{Asset, AssetEvent, AssetLoader, AssetServer, AssetSource, Handle};

    // Render types
    #[cfg(feature = "render")]
    pub use astrelis_render::{
        Color, ComputePassBuilder, Frame, GraphicsContext, RenderPassBuilder, WindowContext,
        WindowManager,
    };

    // Text types
    #[cfg(feature = "text")]
    pub use astrelis_text::{FontRenderer, FontSystem, Text, TextAlign};

    // Plugin types
    #[cfg(feature = "assets")]
    pub use crate::plugins::{AssetPlugin, AsyncRuntimePlugin, DefaultPlugins, MinimalPlugins};

    #[cfg(all(feature = "render", feature = "winit"))]
    pub use crate::plugins::{RenderContexts, RenderPlugin};

    #[cfg(feature = "input")]
    pub use crate::plugins::InputPlugin;

    #[cfg(feature = "text")]
    pub use crate::plugins::TextPlugin;

    #[cfg(feature = "assets")]
    pub use crate::plugins::TimePlugin;
}
