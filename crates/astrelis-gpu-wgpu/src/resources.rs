//! Thread-safe resource map for GPU object storage.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

use astrelis_core::id::Id;

/// Thread-safe map from typed [`Id`] handles to GPU resources.
///
/// Uses `RwLock<HashMap>` for interior mutability and an atomic counter
/// for ID generation.
pub(crate) struct ResourceMap<M, V> {
    map: RwLock<HashMap<u64, V>>,
    next_id: AtomicU64,
    _marker: std::marker::PhantomData<M>,
}

impl<M, V> ResourceMap<M, V> {
    /// Creates a new empty resource map. IDs start at 1.
    pub(crate) fn new() -> Self {
        Self {
            map: RwLock::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            _marker: std::marker::PhantomData,
        }
    }

    /// Inserts a resource and returns its typed handle.
    pub(crate) fn insert(&self, value: V) -> Id<M> {
        let raw = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.map.write().unwrap().insert(raw, value);
        Id::new(raw)
    }

    /// Removes and returns a resource by handle.
    pub(crate) fn remove(&self, id: Id<M>) -> Option<V> {
        self.map.write().unwrap().remove(&id.raw())
    }

    /// Provides read access to a resource via a callback.
    ///
    /// Returns `None` if the handle is invalid.
    pub(crate) fn get<R>(&self, id: Id<M>, f: impl FnOnce(&V) -> R) -> Option<R> {
        let guard = self.map.read().unwrap();
        guard.get(&id.raw()).map(f)
    }

    /// Returns a read guard to the internal map.
    ///
    /// Useful when multiple lookups are needed within a single operation
    /// (e.g., building a bind group with references to several resources).
    pub(crate) fn read_guard(&self) -> std::sync::RwLockReadGuard<'_, HashMap<u64, V>> {
        self.map.read().unwrap()
    }
}
