//! Astrelis Core
//!
//! This crate provides the foundational functionality for the Astrelis game engine, including
//! math utilities, logging, profiling, geometry types, and custom allocators.
//!
//! # Overview
//!
//! The `astrelis-core` crate is dependency-free (except for re-exported external crates) and
//! serves as the foundation for all other Astrelis crates. It provides:
//!
//! - **Math**: Re-exports of `glam` types ([`Vec2`], [`Vec3`], [`Mat4`], etc.) for linear algebra
//! - **Logging**: Structured logging via `tracing` with [`logging::init()`]
//! - **Profiling**: Performance profiling integration with `puffin` via [`profiling`]
//! - **Geometry**: Common 2D geometry types (sizes, positions, coordinate spaces)
//! - **Allocators**: Custom allocators like `ahash` for fast hashing
//!
//! # Modules
//!
//! - [`math`]: Linear algebra types and utilities (re-exports `glam`)
//! - [`logging`]: Initialize and configure tracing-based logging
//! - [`profiling`]: Performance profiling with puffin integration
//! - [`geometry`]: 2D geometry primitives (rectangles, transforms, etc.)
//! - [`alloc`]: Custom allocators and hash functions
//!
//! # Quick Start
//!
//! ```no_run
//! use astrelis_core::{logging, math::Vec2};
//!
//! // Initialize logging (outputs to stdout with timestamps)
//! logging::init();
//!
//! // Use math types
//! let position = Vec2::new(10.0, 20.0);
//! let velocity = Vec2::new(1.0, 0.5);
//! let new_position = position + velocity * 0.016; // Delta time
//!
//! tracing::info!("New position: {:?}", new_position);
//! ```
//!
//! # Usage with Other Crates
//!
//! The `astrelis-core` crate is typically used as a foundation for higher-level crates:
//!
//! ```toml
//! [dependencies]
//! astrelis-core = "0.1"
//! astrelis-winit = "0.1"  # Window management (depends on core)
//! astrelis-render = "0.1" # Rendering (depends on core)
//! ```
//!
//! # Feature Flags
//!
//! - `profiling` (default): Enables puffin-based profiling. When disabled, all profiling
//!   macros and functions become zero-cost no-ops.
//! - `winit` (default): Enables winit window type re-exports.
//!
//! # See Also
//!
//! - [Getting Started Guide](https://docs.rs/astrelis/latest/astrelis/guides/getting-started/)
//! - [Architecture Overview](https://docs.rs/astrelis/latest/astrelis/guides/architecture/)
//!
//! [`Vec2`]: math::Vec2
//! [`Vec3`]: math::Vec3
//! [`Mat4`]: math::Mat4

pub mod alloc;
pub mod geometry;
pub mod logging;
pub mod math;
pub mod profiling;
