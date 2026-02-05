//! Test utilities for Astrelis engine.
//!
//! This crate provides testing infrastructure for the Astrelis game engine,
//! including mock GPU contexts and render trait abstractions.
//!
//! # Overview
//!
//! The main components are:
//!
//! - [`RenderContext`] - Trait abstracting GPU operations
//! - `MockRenderContext` - Mock implementation for testing (requires `mock` feature)
//! - GPU wrapper types (`GpuBuffer`, `GpuTexture`, etc.) - Can be real or mock
//!
//! # Example
//!
//! ```rust
//! # #[cfg(feature = "mock")]
//! # {
//! use astrelis_test_utils::{MockRenderContext, RenderContext};
//! use wgpu::*;
//!
//! // Create a mock context for testing
//! let mock = MockRenderContext::new();
//!
//! // Use it like a real GPU context
//! let buffer = mock.create_buffer(&BufferDescriptor {
//!     label: Some("test_buffer"),
//!     size: 1024,
//!     usage: BufferUsages::VERTEX,
//!     mapped_at_creation: false,
//! });
//!
//! // Verify operations in tests
//! assert_eq!(mock.count_buffer_creates(), 1);
//! assert!(buffer.is_mock());
//! # }
//! ```
//!
//! # Design Philosophy
//!
//! This crate follows several key design principles:
//!
//! ## 1. No Lifetimes
//!
//! All GPU wrapper types are owned and use reference counting internally.
//! This eliminates lifetime parameters from propagating through the codebase.
//!
//! ## 2. Interior Mutability
//!
//! Mock implementations use `Mutex` for interior mutability, allowing `&self`
//! methods to record calls.
//!
//! ## 3. Object Safety
//!
//! The `RenderContext` trait is object-safe (`dyn RenderContext`), allowing
//! for polymorphic usage with both real and mock contexts.

pub mod gpu_types;
#[cfg(feature = "mock")]
pub mod mock_render;
pub mod render_context;

// Re-export main types at crate root
pub use gpu_types::*;
#[cfg(feature = "mock")]
pub use mock_render::*;
pub use render_context::*;
