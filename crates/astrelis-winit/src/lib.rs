//! Window and event management for Astrelis engine.
//!
//! This crate provides the [`App`] trait for game loop implementation,
//! window creation/management, and event batching for efficient processing.
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use astrelis_winit::{app::{run_app, App, AppCtx}, FrameTime, WindowId, event::EventBatch};
//!
//! struct MyApp;
//!
//! impl App for MyApp {
//!     fn update(&mut self, _ctx: &mut AppCtx, _time: &FrameTime) {
//!         // Game logic update
//!     }
//!
//!     fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
//!         // Rendering logic per window
//!     }
//! }
//!
//! fn main() {
//!     run_app(|_ctx| Box::new(MyApp));
//! }
//! ```
//!
//! # App Lifecycle
//!
//! Methods are called in this order each frame:
//! 1. [`App::on_start()`] - Once at startup
//! 2. [`App::begin_frame()`] - Start of each frame
//! 3. [`App::update()`] - Game logic update
//! 4. [`App::fixed_update()`] - Physics/fixed timestep (repeated as needed)
//! 5. [`App::render()`] - Per-window rendering (called for each window)
//! 6. [`App::end_frame()`] - End of each frame
//! 7. [`App::on_exit()`] - Once at shutdown
//!
//! # Event Batching
//!
//! Events are collected and batched per-window using [`EventBatch`]. Access events
//! in the `render()` method to handle input for each window separately.
//!
//! # Features
//!
//! - Window creation and management via [`window::WindowBackend`]
//! - Event loop integration with winit
//! - Frame timing and fixed timestep support via [`FrameTime`]
//! - Window resizing, focus, and lifecycle events
//!
//! [`App`]: app::App
//! [`App::on_start()`]: app::App::on_start
//! [`App::begin_frame()`]: app::App::begin_frame
//! [`App::update()`]: app::App::update
//! [`App::fixed_update()`]: app::App::fixed_update
//! [`App::render()`]: app::App::render
//! [`App::end_frame()`]: app::App::end_frame
//! [`App::on_exit()`]: app::App::on_exit
//! [`EventBatch`]: event::EventBatch

pub mod app;
pub mod event;
pub mod time;
pub mod window;

// Re-export WindowId for convenience
pub use winit::window::WindowId;

// Re-export FrameTime for convenience
pub use time::FrameTime;
