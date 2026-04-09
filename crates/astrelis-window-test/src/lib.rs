//! Mock windowing backend for headless testing of the Astrelis engine.
//!
//! This crate provides [`MockBackend`], a scriptable windowing backend that
//! injects events synchronously and returns a [`RunResult`] for assertions —
//! no display server, no real windows, no manual interaction.
//!
//! # Usage
//!
//! Add as a dev-dependency:
//!
//! ```toml
//! [dev-dependencies]
//! astrelis-window-test = { workspace = true }
//! ```
//!
//! Then write tests:
//!
//! ```
//! use astrelis_window::backend::{AppHandler, EventLoopContext};
//! use astrelis_window::control_flow::ControlFlow;
//! use astrelis_window::event::WindowEvent;
//! use astrelis_window::lifecycle::AppLifecycle;
//! use astrelis_window::types::LogicalInnerSize;
//! use astrelis_window::window_id::WindowId;
//! use astrelis_window::WindowBuilder;
//! use astrelis_window_test::MockBackend;
//!
//! struct MyApp;
//!
//! impl AppHandler for MyApp {
//!     fn on_lifecycle(&mut self, ctx: &mut dyn EventLoopContext, state: AppLifecycle) {
//!         if state == AppLifecycle::Resumed {
//!             let attrs = WindowBuilder::new()
//!                 .with_title("Test")
//!                 .with_inner_size(LogicalInnerSize::new(800.0, 600.0))
//!                 .build();
//!             ctx.create_window(attrs).unwrap();
//!             ctx.set_control_flow(ControlFlow::Poll);
//!         }
//!     }
//!
//!     fn on_window_event(&mut self, ctx: &mut dyn EventLoopContext, _: WindowId, event: WindowEvent) {
//!         if matches!(event, WindowEvent::CloseRequested) {
//!             ctx.exit();
//!         }
//!     }
//!
//!     fn on_events_cleared(&mut self, _: &mut dyn EventLoopContext) {}
//! }
//!
//! let mut backend = MockBackend::new();
//! backend.push_lifecycle(AppLifecycle::Resumed);
//!
//! let result = backend.run_test(&mut MyApp);
//! assert_eq!(result.created_window_ids.len(), 1);
//! assert_eq!(result.control_flow, ControlFlow::Poll);
//! assert!(!result.exit_requested);
//! ```

mod backend;
mod context;
mod event_queue;
pub mod window;

pub use backend::{MockBackend, RunResult};
pub use event_queue::ScriptedEvent;
