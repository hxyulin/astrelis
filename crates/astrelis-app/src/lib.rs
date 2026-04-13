//! Application framework for the Astrelis engine.
//!
//! This crate provides a plugin-based application framework with
//! phase-ordered system execution, a type-map resource container,
//! and typed event channels.
//!
//! # Architecture
//!
//! - [`App`] — builder and runner; configures plugins, systems, and
//!   resources, then enters the event loop
//! - [`Plugin`] — modular extension trait for registering functionality
//! - [`Resources`] — type-map with runtime borrow checking for shared state
//! - [`Phase`] — fixed execution phases (PreUpdate → FixedUpdate → Update →
//!   PostUpdate → Render → Present)
//! - [`Events<T>`] — double-buffered typed event queues
//! - [`Time`] — frame timing and fixed-timestep accumulator
//!
//! # Example
//!
//! ```ignore
//! use astrelis_app::*;
//! use astrelis_input::InputState;
//! use astrelis_window::keyboard::KeyCode;
//!
//! struct MyGame;
//!
//! impl Plugin for MyGame {
//!     fn build(&self, app: &mut App) {
//!         app.add_system(Phase::Update, |resources| {
//!             let input = resources.get::<InputState>();
//!             let time = resources.get::<Time>();
//!             if input.is_key_pressed(KeyCode::Space) {
//!                 tracing::info!("Space held at {:.2}s", time.elapsed_secs());
//!             }
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

pub mod app;
pub mod events;
pub mod phase;
pub mod plugin;
pub mod plugins;
pub mod resources;
pub mod time;

pub use app::App;
pub use events::Events;
pub use phase::Phase;
pub use plugin::Plugin;
pub use resources::{Ref, RefMut, Resources};
pub use time::Time;
