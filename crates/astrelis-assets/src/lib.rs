//! Type-safe async asset management for the Astrelis engine.
//!
//! This crate provides a generic, type-safe asset management system. Assets are
//! loaded asynchronously in the background, tracked by handle, and optionally
//! hot-reloaded from disk.
//!
//! The system is type-erased at the storage layer but type-safe at the API layer.
//! Asset loaders for specific types (textures, fonts, etc.) are implemented by
//! consuming crates, not here.
//!
//! # Architecture
//!
//! - **[`Asset`]** — Trait marking a type as loadable.
//! - **[`AssetLoader`]** — Trait for types that can load assets from raw bytes.
//! - **[`Handle<T>`]** — Strong reference keeping an asset alive.
//! - **[`WeakHandle<T>`]** — Weak reference that doesn't prevent unloading.
//! - **[`AssetServer`]** — The central coordinator that manages loading,
//!   storage, and hot-reload.
//!
//! # Example
//!
//! ```no_run
//! use astrelis_assets::{Asset, AssetLoader, AssetServer, AssetLoadError};
//! use std::path::Path;
//! use std::sync::Arc;
//!
//! struct TextAsset {
//!     content: String,
//! }
//!
//! impl Asset for TextAsset {
//!     fn type_name() -> &'static str { "TextAsset" }
//! }
//!
//! struct TextLoader;
//!
//! impl AssetLoader for TextLoader {
//!     type Asset = TextAsset;
//!
//!     fn extensions(&self) -> &[&str] { &["txt"] }
//!
//!     fn load(&self, bytes: &[u8], _path: &Path) -> Result<Self::Asset, AssetLoadError> {
//!         let content = String::from_utf8(bytes.to_vec())
//!             .map_err(|e| AssetLoadError::Parse(e.to_string()))?;
//!         Ok(TextAsset { content })
//!     }
//! }
//!
//! let mut server = AssetServer::new("assets");
//! server.add_loader(TextLoader);
//!
//! let handle = server.load::<TextAsset>("hello.txt");
//!
//! // Call update() each frame to process completed loads.
//! let events = server.update();
//!
//! // Once loaded, get a reference to the asset.
//! if let Some(asset) = server.get(&handle) {
//!     println!("{}", asset.content);
//! }
//! ```

#![warn(missing_docs)]

pub mod event;
pub mod handle;
pub mod loader;
pub mod server;
mod storage;

pub use event::{AssetEvent, LoadState};
pub use handle::{Handle, UntypedHandle, WeakHandle};
pub use loader::AssetLoader;
pub use server::AssetServer;

use std::fmt;

/// Trait implemented by any loadable asset type.
///
/// Types that implement `Asset` can be loaded and managed by the [`AssetServer`].
/// The only requirement is that the type is `Send + Sync + 'static`, enabling
/// it to be loaded on a background thread and shared across the engine.
pub trait Asset: Send + Sync + 'static {
    /// Human-readable type name for debugging and error messages.
    fn type_name() -> &'static str;
}

/// Errors that can occur during asset loading.
#[derive(Debug, Clone)]
pub enum AssetLoadError {
    /// An I/O error occurred while reading the asset file.
    Io(String),
    /// The asset data could not be parsed or decoded.
    Parse(String),
    /// No loader is registered for the given file extension.
    NoLoader {
        /// The file extension that had no matching loader.
        extension: String,
    },
    /// The asset file was not found at the expected path.
    NotFound {
        /// The path that was not found.
        path: String,
    },
}

impl fmt::Display for AssetLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssetLoadError::Io(msg) => write!(f, "I/O error: {msg}"),
            AssetLoadError::Parse(msg) => write!(f, "parse error: {msg}"),
            AssetLoadError::NoLoader { extension } => {
                write!(f, "no loader registered for extension: {extension}")
            }
            AssetLoadError::NotFound { path } => write!(f, "asset not found: {path}"),
        }
    }
}

impl std::error::Error for AssetLoadError {}
