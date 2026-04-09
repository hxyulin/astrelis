//! Backend-agnostic windowing traits and types for the Astrelis engine.
//!
//! This crate defines platform-independent abstractions. Concrete
//! implementations (e.g., `astrelis-window-winit`) live in separate crates.
//!
//! # Architecture
//!
//! - [`WindowBackend`] — top-level entry point; creates the event loop
//! - [`AppHandler`] — user-implemented callback trait for receiving events
//! - [`EventLoopContext`] — provided during callbacks; create/access windows
//! - [`Window`] — trait for manipulating a single window
//! - [`WindowBuilder`] — fluent API for configuring new windows
//!
//! # Example
//!
//! ```ignore
//! use astrelis_window::*;
//! use astrelis_window::types::LogicalInnerSize;
//!
//! struct MyApp;
//!
//! impl AppHandler for MyApp {
//!     fn on_lifecycle(&mut self, ctx: &mut dyn EventLoopContext, state: AppLifecycle) {
//!         if state == AppLifecycle::Resumed {
//!             let attrs = WindowBuilder::new()
//!                 .with_title("My Game")
//!                 .with_inner_size(LogicalInnerSize::new(1920.0, 1080.0))
//!                 .build();
//!             ctx.create_window(attrs).unwrap();
//!             ctx.set_control_flow(ControlFlow::Poll);
//!         }
//!     }
//!
//!     fn on_window_event(
//!         &mut self,
//!         ctx: &mut dyn EventLoopContext,
//!         _window_id: WindowId,
//!         event: WindowEvent,
//!     ) {
//!         if matches!(event, WindowEvent::CloseRequested) {
//!             ctx.exit();
//!         }
//!     }
//!
//!     fn on_events_cleared(&mut self, _ctx: &mut dyn EventLoopContext) {}
//! }
//! ```

pub mod backend;
pub mod builder;
pub mod capability;
pub mod control_flow;
pub mod cursor;
pub mod error;
pub mod event;
pub mod fullscreen;
pub mod keyboard;
pub mod lifecycle;
pub mod monitor;
pub mod mouse;
pub mod theme;
pub mod types;
pub mod window;
pub mod window_id;
pub mod window_level;

// Convenience re-exports.
pub use backend::{AppHandler, EventLoopContext, WindowBackend};
pub use builder::{WindowAttributes, WindowBuilder};
pub use capability::{Capabilities, Capability};
pub use control_flow::ControlFlow;
pub use error::WindowError;
pub use event::{DeviceEvent, ElementState, WindowEvent};
pub use lifecycle::AppLifecycle;
pub use window::{ResizeDirection, Window};
pub use window_id::WindowId;
