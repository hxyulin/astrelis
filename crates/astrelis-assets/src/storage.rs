//! Per-type asset storage with reference counting.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

use astrelis_core::alloc::sparse_set::{IndexSlot, SparseSet};

use crate::error::AssetError;
use crate::handle::{Handle, HandleId, UntypedHandle};
use crate::state::{AssetEntry, AssetState};
use crate::source::AssetSource;
use crate::Asset;

/// Per-type asset storage container.
///
/// Stores assets of a specific type using a `SparseSet` for O(1) access.
pub struct Assets<T: Asset> {
    /// The assets stored in a sparse set.
    entries: SparseSet<AssetEntry<T>>,
    /// Maps source paths/keys to handle IDs for deduplication.
    source_to_handle: HashMap<String, HandleId>,
    /// Track reference counts for handles (by slot index).
    ref_counts: HashMap<u32, u32>,
}

impl<T: Asset> Default for Assets<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Asset> Assets<T> {
    /// Create a new empty asset storage.
    pub fn new() -> Self {
        Self {
            entries: SparseSet::new(),
            source_to_handle: HashMap::new(),
            ref_counts: HashMap::new(),
        }
    }

    /// Insert a new asset and return a handle to it.
    pub fn insert(&mut self, source: AssetSource, asset: T) -> Handle<T> {
        // Check if already exists using &str (no allocation)
        let key_ref = source.key();
        if let Some(&handle_id) = self.source_to_handle.get(key_ref) {
            // Update existing
            if let Some(entry) = self.entries.try_get_mut(handle_id.slot) {
                entry.state = AssetState::Ready(Arc::new(asset));
                entry.version.increment();
            }
            return Handle::new(handle_id);
        }

        // Only allocate String if we need to insert a new entry
        let key = key_ref.to_string();

        // Insert new entry
        let entry = AssetEntry::with_asset(source.clone(), asset);
        let slot = self.entries.push(entry);
        let handle_id = HandleId::new(slot, TypeId::of::<T>());

        self.source_to_handle.insert(key, handle_id);
        self.ref_counts.insert(slot.index(), 1);

        Handle::new(handle_id)
    }

    /// Reserve a handle for an asset that will be loaded later.
    pub fn reserve(&mut self, source: AssetSource) -> Handle<T> {
        // Check if already exists using &str (no allocation)
        let key_ref = source.key();
        if let Some(&handle_id) = self.source_to_handle.get(key_ref) {
            return Handle::new(handle_id);
        }

        // Only allocate String if we need to insert a new entry
        let key = key_ref.to_string();

        // Insert placeholder entry
        let entry = AssetEntry::new(source.clone());
        let slot = self.entries.push(entry);
        let handle_id = HandleId::new(slot, TypeId::of::<T>());

        self.source_to_handle.insert(key, handle_id);
        self.ref_counts.insert(slot.index(), 1);

        Handle::new(handle_id)
    }

    /// Set an asset to loading state.
    pub fn set_loading(&mut self, handle: &Handle<T>) {
        if let Some(entry) = self.entries.try_get_mut(handle.id().slot) {
            entry.state = AssetState::Loading;
        }
    }

    /// Complete loading by setting the asset data.
    pub fn set_loaded(&mut self, handle: &Handle<T>, asset: T) {
        if let Some(entry) = self.entries.try_get_mut(handle.id().slot) {
            entry.state = AssetState::Ready(Arc::new(asset));
            entry.version.increment();
        }
    }

    /// Set an asset to failed state.
    pub fn set_failed(&mut self, handle: &Handle<T>, error: AssetError) {
        if let Some(entry) = self.entries.try_get_mut(handle.id().slot) {
            entry.state = AssetState::Failed(Arc::new(error));
        }
    }

    /// Get an asset by handle.
    pub fn get(&self, handle: &Handle<T>) -> Option<&Arc<T>> {
        self.entries
            .try_get(handle.id().slot)
            .and_then(|entry| entry.asset())
    }

    /// Get a mutable reference to the asset entry.
    pub fn get_entry(&self, handle: &Handle<T>) -> Option<&AssetEntry<T>> {
        self.entries.try_get(handle.id().slot)
    }

    /// Get the asset state.
    pub fn state(&self, handle: &Handle<T>) -> Option<&AssetState<T>> {
        self.entries.try_get(handle.id().slot).map(|e| &e.state)
    }

    /// Check if an asset is ready.
    pub fn is_ready(&self, handle: &Handle<T>) -> bool {
        self.entries
            .try_get(handle.id().slot)
            .map(|e| e.is_ready())
            .unwrap_or(false)
    }

    /// Check if an asset is loading.
    pub fn is_loading(&self, handle: &Handle<T>) -> bool {
        self.entries
            .try_get(handle.id().slot)
            .map(|e| e.is_loading())
            .unwrap_or(false)
    }

    /// Get the version of an asset.
    pub fn version(&self, handle: &Handle<T>) -> Option<u32> {
        self.entries.try_get(handle.id().slot).map(|e| e.version.get())
    }

    /// Find a handle by source.
    pub fn find_by_source(&self, source: &AssetSource) -> Option<Handle<T>> {
        let key = source.key();
        self.source_to_handle.get(key).map(|&id| Handle::new(id))
    }

    /// Remove an asset by handle.
    pub fn remove(&mut self, handle: &Handle<T>) -> Option<AssetEntry<T>> {
        let slot = handle.id().slot;
        // Check if the entry exists and generation matches
        if self.entries.try_get(slot).is_some() {
            let entry = self.entries.remove(slot);
            // Use &str directly - no allocation needed (HashMap implements Borrow<str>)
            self.source_to_handle.remove(entry.source.key());
            self.ref_counts.remove(&slot.index());
            Some(entry)
        } else {
            None
        }
    }

    /// Increment reference count for a handle.
    pub fn add_ref(&mut self, handle: &Handle<T>) {
        let idx = handle.id().slot.index();
        if let Some(count) = self.ref_counts.get_mut(&idx) {
            *count = count.saturating_add(1);
        }
    }

    /// Decrement reference count for a handle.
    /// Returns true if the asset should be removed (ref count reached 0).
    pub fn release(&mut self, handle: &Handle<T>) -> bool {
        let idx = handle.id().slot.index();
        if let Some(count) = self.ref_counts.get_mut(&idx) {
            *count = count.saturating_sub(1);
            *count == 0
        } else {
            false
        }
    }

    /// Get the current reference count for a handle.
    pub fn ref_count(&self, handle: &Handle<T>) -> u32 {
        let idx = handle.id().slot.index();
        self.ref_counts.get(&idx).copied().unwrap_or(0)
    }

    /// Iterate over all assets.
    pub fn iter(&self) -> impl Iterator<Item = &AssetEntry<T>> {
        self.entries.iter()
    }

    /// Get the number of stored assets.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if storage is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Type-erased asset storage for dynamic dispatch.
pub trait ErasedAssets: Send + Sync {
    /// Get the type ID of assets in this storage.
    fn asset_type_id(&self) -> TypeId;

    /// Check if an asset is ready.
    fn is_ready_untyped(&self, slot: IndexSlot) -> bool;

    /// Get the source for an asset by its slot.
    fn source_for_slot(&self, slot: IndexSlot) -> Option<&AssetSource>;

    /// Set a loaded asset from a type-erased value.
    ///
    /// Returns `true` if the asset was successfully stored, `false` if the type doesn't match
    /// or the slot doesn't exist.
    fn set_loaded_erased(&mut self, slot: IndexSlot, asset: Box<dyn Any + Send + Sync>) -> bool;

    /// Set an asset to failed state.
    fn set_failed_erased(&mut self, slot: IndexSlot, error: AssetError);

    /// Get the version of an asset by slot.
    fn version_for_slot(&self, slot: IndexSlot) -> Option<u32>;

    /// Get as Any for downcasting.
    fn as_any(&self) -> &dyn Any;

    /// Get as mutable Any for downcasting.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Asset> ErasedAssets for Assets<T> {
    fn asset_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }

    fn is_ready_untyped(&self, slot: IndexSlot) -> bool {
        self.entries
            .try_get(slot)
            .map(|e| e.is_ready())
            .unwrap_or(false)
    }

    fn source_for_slot(&self, slot: IndexSlot) -> Option<&AssetSource> {
        self.entries.try_get(slot).map(|e| &e.source)
    }

    fn set_loaded_erased(&mut self, slot: IndexSlot, asset: Box<dyn Any + Send + Sync>) -> bool {
        // Try to downcast the asset to the expected type
        let Some(typed_asset) = asset.downcast::<T>().ok() else {
            tracing::error!(
                "Type mismatch storing asset: expected {}, got different type",
                T::type_name()
            );
            return false;
        };

        // Get the entry and update it
        let Some(entry) = self.entries.try_get_mut(slot) else {
            tracing::error!("Slot not found for asset: {:?}", slot);
            return false;
        };

        entry.state = AssetState::Ready(Arc::new(*typed_asset));
        entry.version.increment();
        true
    }

    fn set_failed_erased(&mut self, slot: IndexSlot, error: AssetError) {
        if let Some(entry) = self.entries.try_get_mut(slot) {
            entry.state = AssetState::Failed(Arc::new(error));
        }
    }

    fn version_for_slot(&self, slot: IndexSlot) -> Option<u32> {
        self.entries.try_get(slot).map(|e| e.version.get())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Registry of all asset storages, keyed by type.
#[derive(Default)]
pub struct AssetStorages {
    storages: HashMap<TypeId, Box<dyn ErasedAssets>>,
}

impl AssetStorages {
    /// Create a new empty storage registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get or create storage for a type.
    pub fn get_or_create<T: Asset>(&mut self) -> &mut Assets<T> {
        let type_id = TypeId::of::<T>();

        self.storages.entry(type_id).or_insert_with(|| Box::new(Assets::<T>::new()));

        self.storages
            .get_mut(&type_id)
            .and_then(|s| s.as_any_mut().downcast_mut())
            .expect("type mismatch in storage registry")
    }

    /// Get storage for a type.
    pub fn get<T: Asset>(&self) -> Option<&Assets<T>> {
        let type_id = TypeId::of::<T>();
        self.storages
            .get(&type_id)
            .and_then(|s| s.as_any().downcast_ref())
    }

    /// Get mutable storage for a type.
    pub fn get_mut<T: Asset>(&mut self) -> Option<&mut Assets<T>> {
        let type_id = TypeId::of::<T>();
        self.storages
            .get_mut(&type_id)
            .and_then(|s| s.as_any_mut().downcast_mut())
    }

    /// Check if storage exists for a type.
    pub fn has<T: Asset>(&self) -> bool {
        self.storages.contains_key(&TypeId::of::<T>())
    }

    /// Find the source for an untyped handle by searching all storages.
    ///
    /// This is used for hot reload to find which file to watch for changes.
    pub fn find_source(&self, handle: &UntypedHandle) -> Option<&AssetSource> {
        // Try to find the storage for this handle's type
        let storage = self.storages.get(&handle.type_id())?;

        // Get the source using the slot from the handle
        storage.source_for_slot(handle.id().slot)
    }

    /// Set a loaded asset using a type-erased value and untyped handle.
    ///
    /// This is used by the async loading system to store assets without knowing
    /// the concrete type at compile time.
    ///
    /// Returns `true` if the asset was successfully stored, `false` if the storage
    /// doesn't exist or the type doesn't match.
    pub fn set_loaded_erased(
        &mut self,
        handle: &UntypedHandle,
        asset: Box<dyn Any + Send + Sync>,
    ) -> bool {
        let Some(storage) = self.storages.get_mut(&handle.type_id()) else {
            tracing::error!(
                "No storage found for type {:?}",
                handle.type_id()
            );
            return false;
        };

        storage.set_loaded_erased(handle.id().slot, asset)
    }

    /// Set an asset to failed state using an untyped handle.
    pub fn set_failed_erased(&mut self, handle: &UntypedHandle, error: AssetError) {
        if let Some(storage) = self.storages.get_mut(&handle.type_id()) {
            storage.set_failed_erased(handle.id().slot, error);
        }
    }

    /// Get the version of an asset using an untyped handle.
    pub fn version_erased(&self, handle: &UntypedHandle) -> Option<u32> {
        let storage = self.storages.get(&handle.type_id())?;
        storage.version_for_slot(handle.id().slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_get() {
        let mut assets: Assets<String> = Assets::new();

        let source = AssetSource::memory("test.txt");
        let handle = assets.insert(source, "Hello, World!".to_string());

        assert!(assets.is_ready(&handle));
        let asset = assets.get(&handle).unwrap();
        assert_eq!(**asset, "Hello, World!");
    }

    #[test]
    fn test_reserve_and_load() {
        let mut assets: Assets<String> = Assets::new();

        let source = AssetSource::memory("test.txt");
        let handle = assets.reserve(source);

        assert!(!assets.is_ready(&handle));

        assets.set_loading(&handle);
        assert!(assets.is_loading(&handle));

        assets.set_loaded(&handle, "Loaded!".to_string());
        assert!(assets.is_ready(&handle));

        let asset = assets.get(&handle).unwrap();
        assert_eq!(**asset, "Loaded!");
    }

    #[test]
    fn test_find_by_source() {
        let mut assets: Assets<String> = Assets::new();

        let source = AssetSource::memory("test.txt");
        let handle = assets.insert(source.clone(), "Test".to_string());

        let found = assets.find_by_source(&source).unwrap();
        assert_eq!(found.id(), handle.id());
    }

    #[test]
    fn test_ref_counting() {
        let mut assets: Assets<String> = Assets::new();

        let source = AssetSource::memory("test.txt");
        let handle = assets.insert(source, "Test".to_string());

        assert_eq!(assets.ref_count(&handle), 1);

        assets.add_ref(&handle);
        assert_eq!(assets.ref_count(&handle), 2);

        let should_remove = assets.release(&handle);
        assert!(!should_remove);
        assert_eq!(assets.ref_count(&handle), 1);

        let should_remove = assets.release(&handle);
        assert!(should_remove);
        assert_eq!(assets.ref_count(&handle), 0);
    }
}
