//! Type-erased asset storage.
//!
//! The storage layer uses a `HashMap<TypeId, Box<dyn Any>>` where each entry
//! holds a typed `Assets<T>` collection. This provides type-erasure at the
//! storage boundary while keeping per-type operations fully typed.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

use crate::event::LoadState;
use crate::handle::AssetId;
use crate::Asset;

/// A single asset slot in the storage.
pub(crate) struct AssetEntry<T> {
    /// The loaded asset data, or `None` while still loading.
    pub(crate) asset: Option<Arc<T>>,
    /// Current load state.
    pub(crate) state: LoadState,
    /// Version counter, bumped on hot-reload.
    pub(crate) version: u32,
    /// Strong reference kept by the server; handles clone from this.
    _ref_count: Arc<()>,
}

/// Per-type asset collection mapping [`AssetId`] to [`AssetEntry<T>`].
pub(crate) struct Assets<T: Asset> {
    entries: HashMap<AssetId, AssetEntry<T>>,
}

impl<T: Asset> Assets<T> {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Inserts a new entry in the `Loading` state.
    pub(crate) fn insert_loading(&mut self, id: AssetId, ref_count: Arc<()>) {
        self.entries.insert(
            id,
            AssetEntry {
                asset: None,
                state: LoadState::Loading,
                version: 0,
                _ref_count: ref_count,
            },
        );
    }

    /// Inserts an already-loaded entry.
    pub(crate) fn insert_loaded(&mut self, id: AssetId, asset: Arc<T>, ref_count: Arc<()>) {
        self.entries.insert(
            id,
            AssetEntry {
                asset: Some(asset),
                state: LoadState::Loaded,
                version: 0,
                _ref_count: ref_count,
            },
        );
    }

    /// Returns the asset `Arc` if loaded.
    pub(crate) fn get(&self, id: &AssetId) -> Option<Arc<T>> {
        self.entries
            .get(id)
            .and_then(|entry| entry.asset.as_ref().cloned())
    }

    /// Returns the current load state.
    pub(crate) fn load_state(&self, id: &AssetId) -> LoadState {
        self.entries
            .get(id)
            .map(|entry| entry.state.clone())
            .unwrap_or(LoadState::NotLoaded)
    }
}

/// Type-erased operations on an `Assets<T>` collection.
pub(crate) trait ErasedAssets: Any + Send + Sync {
    /// Update an entry with a loaded asset (type-erased).
    fn set_loaded(&mut self, id: &AssetId, asset: Box<dyn Any + Send + Sync>);

    /// Mark an entry as failed.
    fn set_failed(&mut self, id: &AssetId, error: String);

    /// Returns self as `&dyn Any` for downcasting.
    fn as_any(&self) -> &dyn Any;

    /// Returns self as `&mut dyn Any` for downcasting.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Asset> ErasedAssets for Assets<T> {
    fn set_loaded(&mut self, id: &AssetId, asset: Box<dyn Any + Send + Sync>) {
        if let Some(entry) = self.entries.get_mut(id)
            && let Ok(arc) = asset.downcast::<Arc<T>>()
        {
            entry.asset = Some(*arc);
            entry.state = LoadState::Loaded;
            entry.version += 1;
        }
    }

    fn set_failed(&mut self, id: &AssetId, error: String) {
        if let Some(entry) = self.entries.get_mut(id) {
            entry.state = LoadState::Failed(error);
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Top-level storage map holding one `Assets<T>` per asset type.
pub(crate) struct StorageMap {
    map: HashMap<TypeId, Box<dyn ErasedAssets>>,
}

impl StorageMap {
    /// Creates an empty storage map.
    pub(crate) fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Returns the typed `Assets<T>` for the given type, creating it if needed.
    pub(crate) fn get_or_create<T: Asset>(&mut self) -> &mut Assets<T> {
        self.map
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(Assets::<T>::new()))
            .as_any_mut()
            .downcast_mut::<Assets<T>>()
            .expect("type mismatch in storage map")
    }

    /// Returns the typed `Assets<T>` if it exists.
    pub(crate) fn get<T: Asset>(&self) -> Option<&Assets<T>> {
        self.map
            .get(&TypeId::of::<T>())
            .and_then(|boxed| boxed.as_any().downcast_ref::<Assets<T>>())
    }

    /// Returns the erased assets for a given type ID.
    pub(crate) fn get_erased_mut(&mut self, type_id: &TypeId) -> Option<&mut dyn ErasedAssets> {
        self.map.get_mut(type_id).map(|b| b.as_mut())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct TestAsset {
        value: u32,
    }
    impl Asset for TestAsset {
        fn type_name() -> &'static str {
            "TestAsset"
        }
    }

    #[test]
    fn insert_and_get_loaded() {
        let mut storage = StorageMap::new();
        let id = AssetId {
            index: 0,
            generation: 1,
        };
        let rc = Arc::new(());

        let assets = storage.get_or_create::<TestAsset>();
        assets.insert_loaded(id, Arc::new(TestAsset { value: 42 }), rc);

        let assets = storage.get::<TestAsset>().unwrap();
        let asset = assets.get(&id).unwrap();
        assert_eq!(asset.value, 42);
    }

    #[test]
    fn load_state_transitions() {
        let mut storage = StorageMap::new();
        let id = AssetId {
            index: 0,
            generation: 1,
        };
        let rc = Arc::new(());

        let assets = storage.get_or_create::<TestAsset>();
        assert_eq!(assets.load_state(&id), LoadState::NotLoaded);

        assets.insert_loading(id, rc);
        assert_eq!(assets.load_state(&id), LoadState::Loading);
    }

    #[test]
    fn erased_set_loaded() {
        let mut storage = StorageMap::new();
        let id = AssetId {
            index: 0,
            generation: 1,
        };
        let rc = Arc::new(());

        storage.get_or_create::<TestAsset>().insert_loading(id, rc);

        let erased = storage
            .get_erased_mut(&TypeId::of::<TestAsset>())
            .unwrap();
        let boxed: Box<dyn Any + Send + Sync> = Box::new(Arc::new(TestAsset { value: 99 }));
        erased.set_loaded(&id, boxed);

        let assets = storage.get::<TestAsset>().unwrap();
        let asset = assets.get(&id).unwrap();
        assert_eq!(asset.value, 99);
        assert_eq!(assets.load_state(&id), LoadState::Loaded);
    }
}
