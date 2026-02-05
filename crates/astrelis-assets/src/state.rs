//! Asset state machine and version tracking.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::error::AssetError;
use crate::source::AssetSource;

/// The current state of an asset in the loading pipeline.
#[derive(Debug, Clone, Default)]
pub enum AssetState<T> {
    /// The asset has not been loaded yet.
    #[default]
    Unloaded,

    /// The asset is currently being loaded.
    Loading,

    /// The asset has been successfully loaded and is ready for use.
    Ready(Arc<T>),

    /// The asset failed to load.
    Failed(Arc<AssetError>),
}

impl<T> AssetState<T> {
    /// Returns `true` if the asset is in the `Unloaded` state.
    pub fn is_unloaded(&self) -> bool {
        matches!(self, AssetState::Unloaded)
    }

    /// Returns `true` if the asset is currently loading.
    pub fn is_loading(&self) -> bool {
        matches!(self, AssetState::Loading)
    }

    /// Returns `true` if the asset is ready for use.
    pub fn is_ready(&self) -> bool {
        matches!(self, AssetState::Ready(_))
    }

    /// Returns `true` if the asset failed to load.
    pub fn is_failed(&self) -> bool {
        matches!(self, AssetState::Failed(_))
    }

    /// Get the asset if it's ready, or `None` otherwise.
    pub fn get(&self) -> Option<&Arc<T>> {
        match self {
            AssetState::Ready(asset) => Some(asset),
            _ => None,
        }
    }

    /// Get a clone of the asset Arc if ready.
    pub fn get_cloned(&self) -> Option<Arc<T>> {
        match self {
            AssetState::Ready(asset) => Some(Arc::clone(asset)),
            _ => None,
        }
    }

    /// Get the error if loading failed.
    pub fn error(&self) -> Option<&Arc<AssetError>> {
        match self {
            AssetState::Failed(err) => Some(err),
            _ => None,
        }
    }
}

/// Load state for tracking async loading progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadState {
    /// Not yet started loading.
    NotLoaded,

    /// Currently loading.
    Loading,

    /// Successfully loaded.
    Loaded,

    /// Failed to load.
    Failed,
}

impl LoadState {
    /// Returns true if loading has completed (successfully or not).
    pub fn is_done(&self) -> bool {
        matches!(self, LoadState::Loaded | LoadState::Failed)
    }
}

impl<T> From<&AssetState<T>> for LoadState {
    fn from(state: &AssetState<T>) -> Self {
        match state {
            AssetState::Unloaded => LoadState::NotLoaded,
            AssetState::Loading => LoadState::Loading,
            AssetState::Ready(_) => LoadState::Loaded,
            AssetState::Failed(_) => LoadState::Failed,
        }
    }
}

/// Version tracker for change detection.
///
/// Increments each time the asset is modified/reloaded.
#[derive(Debug)]
pub struct AssetVersion {
    value: AtomicU32,
}

impl AssetVersion {
    /// Create a new version starting at 1.
    pub fn new() -> Self {
        Self {
            value: AtomicU32::new(1),
        }
    }

    /// Get the current version number.
    pub fn get(&self) -> u32 {
        self.value.load(Ordering::Relaxed)
    }

    /// Increment the version and return the new value.
    pub fn increment(&self) -> u32 {
        self.value.fetch_add(1, Ordering::Relaxed) + 1
    }
}

impl Clone for AssetVersion {
    fn clone(&self) -> Self {
        Self {
            value: AtomicU32::new(self.value.load(Ordering::Relaxed)),
        }
    }
}

impl Default for AssetVersion {
    fn default() -> Self {
        Self::new()
    }
}

/// An entry in the asset storage containing state, version, and metadata.
#[derive(Debug)]
pub struct AssetEntry<T> {
    /// The source of the asset.
    pub source: AssetSource,
    /// The current state of the asset.
    pub state: AssetState<T>,
    /// The version for change detection.
    pub version: AssetVersion,
}

impl<T> AssetEntry<T> {
    /// Create a new entry with the given source.
    pub fn new(source: AssetSource) -> Self {
        Self {
            source,
            state: AssetState::Unloaded,
            version: AssetVersion::new(),
        }
    }

    /// Create an entry with an already-loaded asset.
    pub fn with_asset(source: AssetSource, asset: T) -> Self {
        Self {
            source,
            state: AssetState::Ready(Arc::new(asset)),
            version: AssetVersion::new(),
        }
    }

    /// Check if the asset is ready.
    pub fn is_ready(&self) -> bool {
        self.state.is_ready()
    }

    /// Check if the asset is loading.
    pub fn is_loading(&self) -> bool {
        self.state.is_loading()
    }

    /// Get the asset data if ready.
    pub fn asset(&self) -> Option<&Arc<T>> {
        self.state.get()
    }
}
