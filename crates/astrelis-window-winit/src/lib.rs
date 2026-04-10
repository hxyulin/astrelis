//! Winit-based windowing backend for the Astrelis engine.
//!
//! This crate provides [`WinitBackend`], an implementation of
//! [`astrelis_window::backend::WindowBackend`] built on top of
//! [winit](https://docs.rs/winit). It handles event-loop creation,
//! window management, and input-event conversion.
//!
//! # Usage
//!
//! ```no_run
//! use astrelis_window::backend::WindowBackend;
//! use astrelis_window_winit::WinitBackend;
//!
//! let backend = WinitBackend::new().expect("failed to create backend");
//! // backend.run(&mut my_handler).expect("event loop error");
//! ```

mod backend;
mod capabilities;
mod convert;
mod window;

pub use backend::WinitBackend;
