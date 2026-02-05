//! Structured logging for Astrelis using the `tracing` crate.
//!
//! This module provides initialization functions for structured logging throughout the engine.
//! All Astrelis crates use `tracing` macros for logging instead of `println!`.
//!
//! # Quick Start
//!
//! Call [`init()`] once at the start of your application:
//!
//! ```no_run
//! use astrelis_core::logging;
//!
//! fn main() {
//!     logging::init();
//!     tracing::info!("Application started");
//!     // ... your code ...
//! }
//! ```
//!
//! # Logging Macros
//!
//! Use these macros throughout your code:
//!
//! ```no_run
//! use tracing::{trace, debug, info, warn, error};
//!
//! trace!("Very detailed information for debugging");
//! debug!("Debugging information");
//! info!("Informational messages");
//! warn!("Warning messages");
//! error!("Error messages");
//!
//! // With structured fields
//! info!(width = 800, height = 600, "Window created");
//! ```
//!
//! # Log Levels
//!
//! By default, [`init()`] filters logs as follows:
//! - **Astrelis crates**: `TRACE` level (all logs)
//! - **External crates** (`wgpu`, `winit`, etc.): `INFO` level (reduces noise)
//!
//! Override with the `RUST_LOG` environment variable:
//!
//! ```bash
//! # Show all debug logs
//! RUST_LOG=debug cargo run
//!
//! # Show only warnings and errors
//! RUST_LOG=warn cargo run
//!
//! # Custom filter for specific modules
//! RUST_LOG=astrelis_ui=trace,wgpu=warn cargo run
//! ```
//!
//! # Best Practices
//!
//! 1. **Use tracing, not println!**: Structured logs can be filtered and formatted
//! 2. **Choose appropriate levels**: Use `trace` for hot paths, `info` for important events
//! 3. **Add structured fields**: `info!(count = 42, "Counter updated")` is better than `info!("Counter: {}", 42)`
//! 4. **Avoid logging in hot loops**: Use `trace!` sparingly in per-frame code
//!
//! # Performance
//!
//! `tracing` is designed for high-performance logging:
//! - **Minimal overhead** when disabled via log levels
//! - **Zero-cost** if logs are compiled out
//! - **Structured data** without string formatting overhead
//!
//! For performance-critical code, use `trace!` and filter it out in release builds:
//!
//! ```bash
//! # Development: all logs
//! cargo run
//!
//! # Release: only info and above
//! RUST_LOG=info cargo run --release
//! ```

/// Initializes the tracing subscriber with default filters.
///
/// This function sets up structured logging for the entire application. It configures:
/// - **Output format**: Human-readable with timestamps
/// - **Log levels**: `TRACE` for Astrelis crates, `INFO` for external dependencies
/// - **Filters**: Reduces noise from WGPU, winit, naga, and cosmic-text
///
/// # Panics
///
/// This function will panic if called more than once. Call it exactly once at the start
/// of your `main()` function.
///
/// # Examples
///
/// ```no_run
/// use astrelis_core::logging;
///
/// fn main() {
///     // Initialize logging once
///     logging::init();
///
///     // Now you can use tracing macros
///     tracing::info!("Application started");
/// }
/// ```
///
/// # Environment Variables
///
/// Override the default log filter with `RUST_LOG`:
///
/// ```bash
/// RUST_LOG=debug cargo run
/// ```
///
/// # See Also
///
/// - [`tracing`](https://docs.rs/tracing) - Structured logging macros
/// - [`tracing-subscriber`](https://docs.rs/tracing-subscriber) - Subscriber implementation
pub fn init() {
    tracing_subscriber::fmt()
        .with_env_filter("trace,wgpu_core=info,winit=info,cosmic_text=info,naga=info,wgpu_hal=info")
        .init();
}
