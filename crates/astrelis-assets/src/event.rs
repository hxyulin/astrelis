//! Load states and asset events.
//!
//! [`LoadState`] tracks the lifecycle of an asset from request to completion.
//! [`AssetEvent`] reports state changes that occurred during an
//! [`AssetServer::update`](crate::AssetServer::update) call, allowing consuming
//! systems to react to asset load completions, failures, and hot-reload changes.

use crate::handle::UntypedHandle;

/// The current load state of an asset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadState {
    /// The asset has not been requested for loading.
    NotLoaded,
    /// The asset is currently being loaded in the background.
    Loading,
    /// The asset has been successfully loaded and is ready for use.
    Loaded,
    /// The asset failed to load. Contains the error message.
    Failed(String),
}

/// An event emitted by the asset server during
/// [`AssetServer::update`](crate::AssetServer::update).
///
/// Events are collected per-frame and returned as a `Vec<AssetEvent>` from
/// the update call. Systems can inspect these events to trigger follow-up
/// work (e.g., creating GPU resources when a texture finishes loading).
#[derive(Debug, Clone)]
pub enum AssetEvent {
    /// A new asset was successfully loaded for the first time.
    Created {
        /// Handle to the newly loaded asset.
        handle: UntypedHandle,
    },
    /// An existing asset was reloaded (e.g., via hot-reload).
    Modified {
        /// Handle to the modified asset.
        handle: UntypedHandle,
    },
    /// An asset was removed (all strong handles dropped).
    Removed {
        /// Handle to the removed asset.
        handle: UntypedHandle,
    },
    /// An asset failed to load.
    Failed {
        /// Handle to the asset that failed.
        handle: UntypedHandle,
        /// Description of the error.
        error: String,
    },
}
