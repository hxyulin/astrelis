//! Astrelis Assets - Type-safe asset management system
//!
//! This crate provides a comprehensive asset management system with:
//! - Typed handles with generational IDs for O(1) access and use-after-free protection
//! - Multiple asset sources (disk, memory, raw bytes)
//! - Pluggable asset loaders
//! - Async/background loading
//! - Hot-reload support (disk and memory)
//! - GPU resource integration hooks
//! - Event system for change detection
//!
//! # Architecture Overview
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                        AssetServer                               │
//! │  - Coordinates all asset operations                             │
//! │  - Manages type-erased storage                                  │
//! │  - Dispatches to loaders                                        │
//! └───────────────────────────┬─────────────────────────────────────┘
//!                             │
//!          ┌──────────────────┼──────────────────┐
//!          ▼                  ▼                  ▼
//!   ┌─────────────┐   ┌─────────────┐   ┌─────────────┐
//!   │ Assets<Tex> │   │Assets<Shader│   │Assets<Audio>│
//!   │  SparseSet  │   │  SparseSet  │   │  SparseSet  │
//!   └──────┬──────┘   └──────┬──────┘   └──────┬──────┘
//!          │                 │                 │
//!          ▼                 ▼                 ▼
//!   ┌─────────────┐   ┌─────────────┐   ┌─────────────┐
//!   │AssetEntry<T>│   │AssetEntry<T>│   │AssetEntry<T>│
//!   │ - state     │   │ - state     │   │ - state     │
//!   │ - version   │   │ - version   │   │ - version   │
//!   │ - refcount  │   │ - refcount  │   │ - refcount  │
//!   └─────────────┘   └─────────────┘   └─────────────┘
//! ```
//!
//! # Quick Start
//!
//! ```ignore
//! use astrelis_assets::prelude::*;
//!
//! // Create the asset server
//! let mut server = AssetServer::new();
//!
//! // Register a loader for textures
//! server.register_loader::<Texture>(TextureLoader::new());
//!
//! // Load an asset from disk
//! let handle: Handle<Texture> = server.load("textures/player.png");
//!
//! // Check if ready and use
//! if let Some(texture) = server.get(&handle) {
//!     // Use the texture...
//! }
//!
//! // Poll for events
//! for event in server.drain_events() {
//!     match event {
//!         AssetEvent::Created { handle, .. } => { /* ... */ }
//!         AssetEvent::Modified { handle, .. } => { /* ... */ }
//!         AssetEvent::Removed { handle, .. } => { /* ... */ }
//!     }
//! }
//! ```

pub mod error;
pub mod event;
pub mod handle;
pub mod io;
pub mod loader;
pub mod server;
pub mod source;
pub mod state;
pub mod storage;

// Re-export core types
pub use error::*;
pub use event::*;
pub use handle::*;
pub use loader::*;
pub use server::*;
pub use source::*;
pub use state::*;
pub use storage::*;

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::{
        Asset, AssetError, AssetEvent, AssetLoader, AssetServer, AssetSource, AssetState, Assets,
        Handle, LoadContext, StrongHandle, UntypedHandle, WeakHandle,
    };
}

use std::any::Any;

/// Marker trait for types that can be managed as assets.
///
/// This trait combines `Any` (for type erasure) with `Send + Sync` (for thread safety).
/// Types implementing this trait can be loaded, stored, and hot-reloaded by the asset system.
///
/// # Example
///
/// ```ignore
/// use astrelis_assets::Asset;
///
/// #[derive(Debug)]
/// pub struct Texture {
///     pub width: u32,
///     pub height: u32,
///     pub data: Vec<u8>,
/// }
///
/// impl Asset for Texture {
///     fn type_name() -> &'static str {
///         "Texture"
///     }
/// }
/// ```
pub trait Asset: Any + Send + Sync + 'static {
    /// Returns a human-readable name for this asset type.
    /// Used for logging and debugging.
    fn type_name() -> &'static str
    where
        Self: Sized;
}

// Implement Asset for common types that might be useful
impl Asset for String {
    fn type_name() -> &'static str {
        "String"
    }
}

impl Asset for Vec<u8> {
    fn type_name() -> &'static str {
        "Bytes"
    }
}
