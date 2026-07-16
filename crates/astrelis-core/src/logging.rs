//! Tracing subscriber initialization for the Astrelis engine.
//!
//! Provides [`init_default`] to install a [`mod@tracing_subscriber::fmt`] subscriber
//! with sensible defaults:
//!
//! - `RUST_LOG` environment variable support via [`EnvFilter`](tracing_subscriber::EnvFilter)
//! - Default filter: `warn` globally, `info` for all `astrelis_*` crates
//! - Compact format with thread names
//!
//! # Examples
//!
//! ```no_run
//! astrelis_core::logging::init_default();
//! ```
//!
//! Override at runtime with `RUST_LOG`:
//!
//! ```bash
//! RUST_LOG=debug cargo run --example my_example
//! RUST_LOG=astrelis_gpu=trace cargo run --example my_example
//! ```

/// Installs the default tracing subscriber for the Astrelis engine.
///
/// Uses [`mod@tracing_subscriber::fmt`] with an [`EnvFilter`](tracing_subscriber::EnvFilter)
/// that defaults to `warn` for third-party crates and `info` for all `astrelis_*` crates.
/// The filter can be overridden via the `RUST_LOG` environment variable.
///
/// Safe to call multiple times — subsequent calls are silently ignored.
#[cfg(feature = "tracing-init")]
pub fn init_default() {
    use tracing_subscriber::{EnvFilter, fmt};

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(
            "info,\
             astrelis_core=trace",
        )
    });

    fmt().with_env_filter(filter).try_init().ok();
}
