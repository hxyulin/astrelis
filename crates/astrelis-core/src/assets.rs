use std::{
    any::{Any, TypeId},
    collections::HashMap,
    marker::PhantomData,
};

use crate::{
    alloc::{IndexSlot, SparseSet},
    profiling::profile_function,
};

/// A typed handle to an asset
/// Similar to Bevy's Handle<T>
///
/// Handle is Copy because it's just a typed wrapper around an ID.
/// This makes it ergonomic to pass by value.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Handle<T> {
    id: IndexSlot,
    _marker: PhantomData<T>,
}

// Manual implementations needed because PhantomData<T> isn't automatically Copy
impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Handle<T> {}

impl<T> Handle<T> {
    fn new(id: IndexSlot) -> Self {
        Self {
            id,
            _marker: PhantomData,
        }
    }

    pub fn id(&self) -> IndexSlot {
        self.id
    }
}

/// Trait for assets that can be managed by the asset system
pub trait Asset: 'static {}

/// Storage for a specific asset type
pub struct Assets<T: Asset> {
    storage: SparseSet<T>,
}

impl<T: Asset> Assets<T> {
    pub fn new() -> Self {
        Self {
            storage: SparseSet::new(),
        }
    }

    /// Add an asset and return a handle to it
    pub fn add(&mut self, asset: T) -> Handle<T> {
        profile_function!();
        let id = self.storage.push(asset);
        Handle::new(id)
    }

    /// Get an asset by handle
    pub fn get(&self, handle: Handle<T>) -> Option<&T> {
        profile_function!();
        if self.storage.contains(handle.id) {
            Some(self.storage.get(handle.id))
        } else {
            None
        }
    }

    /// Get a mutable reference to an asset by handle
    pub fn get_mut(&mut self, handle: Handle<T>) -> Option<&mut T> {
        profile_function!();
        if self.storage.contains(handle.id) {
            Some(self.storage.get_mut(handle.id))
        } else {
            None
        }
    }

    /// Remove an asset by handle
    pub fn remove(&mut self, handle: Handle<T>) -> Option<T> {
        profile_function!();
        if self.storage.contains(handle.id) {
            Some(self.storage.remove(handle.id))
        } else {
            None
        }
    }

    /// Check if a handle is valid
    pub fn contains(&self, handle: Handle<T>) -> bool {
        self.storage.contains(handle.id)
    }

    /// Get the number of assets
    pub fn len(&self) -> usize {
        self.storage.len()
    }

    /// Check if there are no assets
    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }

    /// Clear all assets
    pub fn clear(&mut self) {
        self.storage.clear();
    }

    /// Iterate over all assets with their handles
    pub fn iter(&self) -> impl Iterator<Item = (Handle<T>, &T)> {
        self.storage
            .iter()
            .map(|(id, asset)| (Handle::new(id), asset))
    }

    /// Iterate mutably over all assets with their handles
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Handle<T>, &mut T)> {
        self.storage
            .iter_mut()
            .map(|(id, asset)| (Handle::new(id), asset))
    }
}

impl<T: Asset> Default for Assets<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Central asset manager that holds all asset types
/// Similar to Bevy's App.world.resource::<Assets<T>>()
pub struct AssetManager {
    assets: HashMap<TypeId, Box<dyn Any>>,
}

impl AssetManager {
    pub fn new() -> Self {
        Self {
            assets: HashMap::new(),
        }
    }

    /// Get or create the Assets<T> storage for a specific asset type
    fn get_or_create_storage<T: Asset>(&mut self) -> &mut Assets<T> {
        profile_function!();
        let type_id = TypeId::of::<T>();
        self.assets
            .entry(type_id)
            .or_insert_with(|| Box::new(Assets::<T>::new()))
            .downcast_mut::<Assets<T>>()
            .expect("type mismatch in asset storage")
    }

    /// Get the Assets<T> storage for a specific asset type
    pub fn get_storage<T: Asset>(&self) -> Option<&Assets<T>> {
        profile_function!();
        let type_id = TypeId::of::<T>();
        self.assets
            .get(&type_id)
            .and_then(|any| any.downcast_ref::<Assets<T>>())
    }

    /// Get the mutable Assets<T> storage for a specific asset type
    pub fn get_storage_mut<T: Asset>(&mut self) -> Option<&mut Assets<T>> {
        profile_function!();
        let type_id = TypeId::of::<T>();
        self.assets
            .get_mut(&type_id)
            .and_then(|any| any.downcast_mut::<Assets<T>>())
    }

    /// Add an asset and return a handle
    pub fn add<T: Asset>(&mut self, asset: T) -> Handle<T> {
        profile_function!();
        self.get_or_create_storage::<T>().add(asset)
    }

    /// Get an asset by handle
    pub fn get<T: Asset>(&self, handle: Handle<T>) -> Option<&T> {
        profile_function!();
        self.get_storage::<T>()?.get(handle)
    }

    /// Get a mutable reference to an asset by handle
    pub fn get_mut<T: Asset>(&mut self, handle: Handle<T>) -> Option<&mut T> {
        profile_function!();
        self.get_storage_mut::<T>()?.get_mut(handle)
    }

    /// Remove an asset by handle
    pub fn remove<T: Asset>(&mut self, handle: Handle<T>) -> Option<T> {
        profile_function!();
        self.get_storage_mut::<T>()?.remove(handle)
    }

    /// Check if a handle is valid
    pub fn contains<T: Asset>(&self, handle: Handle<T>) -> bool {
        self.get_storage::<T>()
            .map(|storage| storage.contains(handle))
            .unwrap_or(false)
    }

    /// Clear all assets of a specific type
    pub fn clear<T: Asset>(&mut self) {
        if let Some(storage) = self.get_storage_mut::<T>() {
            storage.clear();
        }
    }

    /// Clear all assets of all types
    pub fn clear_all(&mut self) {
        self.assets.clear();
    }
}

impl Default for AssetManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct TestAsset {
        value: u32,
    }
    impl Asset for TestAsset {}

    #[derive(Debug, Clone, PartialEq)]
    struct OtherAsset {
        name: String,
    }
    impl Asset for OtherAsset {}

    #[test]
    fn test_assets_add_get() {
        let mut assets = Assets::<TestAsset>::new();
        let handle = assets.add(TestAsset { value: 42 });

        let asset = assets.get(handle).unwrap();
        assert_eq!(asset.value, 42);
    }

    #[test]
    fn test_assets_get_mut() {
        let mut assets = Assets::<TestAsset>::new();
        let handle = assets.add(TestAsset { value: 42 });

        {
            let asset = assets.get_mut(handle).unwrap();
            asset.value = 100;
        }

        let asset = assets.get(handle).unwrap();
        assert_eq!(asset.value, 100);
    }

    #[test]
    fn test_assets_remove() {
        let mut assets = Assets::<TestAsset>::new();
        let handle = assets.add(TestAsset { value: 42 });

        let removed = assets.remove(handle).unwrap();
        assert_eq!(removed.value, 42);
        assert!(assets.get(handle).is_none());
    }

    #[test]
    fn test_assets_contains() {
        let mut assets = Assets::<TestAsset>::new();
        let handle = assets.add(TestAsset { value: 42 });

        assert!(assets.contains(handle));
        assets.remove(handle);
        assert!(!assets.contains(handle));
    }

    #[test]
    fn test_asset_manager() {
        let mut manager = AssetManager::new();

        let handle1 = manager.add(TestAsset { value: 42 });
        let handle2 = manager.add(TestAsset { value: 100 });

        assert_eq!(manager.get(handle1).unwrap().value, 42);
        assert_eq!(manager.get(handle2).unwrap().value, 100);

        assert!(manager.contains(handle1));
        manager.remove(handle1);
        assert!(!manager.contains(handle1));
    }

    #[test]
    fn test_asset_manager_multiple_types() {
        let mut manager = AssetManager::new();

        let test_handle = manager.add(TestAsset { value: 42 });
        let other_handle = manager.add(OtherAsset {
            name: "test".to_string(),
        });

        assert_eq!(manager.get(test_handle).unwrap().value, 42);
        assert_eq!(manager.get(other_handle).unwrap().name, "test");
    }
}
