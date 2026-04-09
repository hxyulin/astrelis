//! Asset handles with generational IDs and reference counting.
//!
//! [`Handle<T>`] is a strong reference that keeps an asset alive.
//! [`WeakHandle<T>`] is a weak reference that does not prevent unloading.
//! [`UntypedHandle`] is a type-erased handle for heterogeneous collections.

use std::any::TypeId;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::sync::{Arc, Weak};

use crate::Asset;

/// Internal generational identifier for an asset slot.
///
/// The `index` identifies the slot, and the `generation` detects reuse
/// (incremented each time a slot is recycled). This prevents use-after-free
/// bugs: a stale handle's generation won't match the current slot generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct AssetId {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

/// A strong reference to a loaded asset.
///
/// Cloning a handle increments an internal reference count. The asset remains
/// alive as long as at least one `Handle` (or [`UntypedHandle`]) exists.
/// Handles are `Send + Sync` and can be freely shared across threads.
///
/// Handles are **not** `Copy` — the reference count must be explicitly managed.
pub struct Handle<T: Asset> {
    pub(crate) id: AssetId,
    pub(crate) ref_count: Arc<()>,
    _marker: PhantomData<fn() -> T>,
}

impl<T: Asset> Handle<T> {
    /// Creates a new handle with the given ID and a fresh reference count.
    pub(crate) fn new(id: AssetId, ref_count: Arc<()>) -> Self {
        Self {
            id,
            ref_count,
            _marker: PhantomData,
        }
    }

    /// Downgrades this handle to a [`WeakHandle`] that does not prevent
    /// the asset from being unloaded.
    pub fn downgrade(&self) -> WeakHandle<T> {
        WeakHandle {
            id: self.id,
            ref_count: Arc::downgrade(&self.ref_count),
            _marker: PhantomData,
        }
    }

    /// Converts this typed handle into an [`UntypedHandle`].
    pub fn untyped(&self) -> UntypedHandle {
        UntypedHandle {
            id: self.id,
            type_id: TypeId::of::<T>(),
            ref_count: Arc::clone(&self.ref_count),
        }
    }
}

impl<T: Asset> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            ref_count: Arc::clone(&self.ref_count),
            _marker: PhantomData,
        }
    }
}

impl<T: Asset> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T: Asset> Eq for Handle<T> {}

impl<T: Asset> Hash for Handle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<T: Asset> std::fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Handle")
            .field("index", &self.id.index)
            .field("generation", &self.id.generation)
            .field("type", &T::type_name())
            .finish()
    }
}

// SAFETY: Handle contains Arc<()> (Send+Sync) and PhantomData (no actual T).
unsafe impl<T: Asset> Send for Handle<T> {}
unsafe impl<T: Asset> Sync for Handle<T> {}

/// A weak reference to an asset that does not prevent unloading.
///
/// Use [`WeakHandle::upgrade`] to attempt to obtain a strong [`Handle`].
/// Returns `None` if all strong handles have been dropped.
pub struct WeakHandle<T: Asset> {
    pub(crate) id: AssetId,
    ref_count: Weak<()>,
    _marker: PhantomData<fn() -> T>,
}

impl<T: Asset> WeakHandle<T> {
    /// Attempts to upgrade to a strong [`Handle`].
    ///
    /// Returns `None` if all strong references have been dropped.
    pub fn upgrade(&self) -> Option<Handle<T>> {
        self.ref_count.upgrade().map(|rc| Handle {
            id: self.id,
            ref_count: rc,
            _marker: PhantomData,
        })
    }
}

impl<T: Asset> Clone for WeakHandle<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            ref_count: self.ref_count.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T: Asset> std::fmt::Debug for WeakHandle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WeakHandle")
            .field("index", &self.id.index)
            .field("generation", &self.id.generation)
            .field("type", &T::type_name())
            .finish()
    }
}

// SAFETY: WeakHandle contains Weak<()> (Send+Sync) and PhantomData (no actual T).
unsafe impl<T: Asset> Send for WeakHandle<T> {}
unsafe impl<T: Asset> Sync for WeakHandle<T> {}

/// A type-erased handle for storing mixed-type handles together.
///
/// Retains a strong reference to the asset. Can be used in heterogeneous
/// collections (e.g., event queues) where the concrete asset type is not
/// known at compile time.
#[derive(Clone)]
pub struct UntypedHandle {
    pub(crate) id: AssetId,
    pub(crate) type_id: TypeId,
    pub(crate) ref_count: Arc<()>,
}

impl UntypedHandle {
    /// Returns the [`TypeId`] of the asset this handle refers to.
    pub fn asset_type_id(&self) -> TypeId {
        self.type_id
    }

    /// Attempts to convert this untyped handle into a typed [`Handle<T>`].
    ///
    /// Returns `None` if the type does not match.
    pub fn typed<T: Asset>(self) -> Option<Handle<T>> {
        if self.type_id == TypeId::of::<T>() {
            Some(Handle {
                id: self.id,
                ref_count: self.ref_count,
                _marker: PhantomData,
            })
        } else {
            None
        }
    }
}

impl PartialEq for UntypedHandle {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.type_id == other.type_id
    }
}

impl Eq for UntypedHandle {}

impl Hash for UntypedHandle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.type_id.hash(state);
    }
}

impl std::fmt::Debug for UntypedHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UntypedHandle")
            .field("index", &self.id.index)
            .field("generation", &self.id.generation)
            .field("type_id", &self.type_id)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestAsset;
    impl Asset for TestAsset {
        fn type_name() -> &'static str {
            "TestAsset"
        }
    }

    struct OtherAsset;
    impl Asset for OtherAsset {
        fn type_name() -> &'static str {
            "OtherAsset"
        }
    }

    #[test]
    fn clone_bumps_refcount() {
        let id = AssetId {
            index: 0,
            generation: 1,
        };
        let handle = Handle::<TestAsset>::new(id, Arc::new(()));
        assert_eq!(Arc::strong_count(&handle.ref_count), 1);

        let cloned = handle.clone();
        assert_eq!(Arc::strong_count(&handle.ref_count), 2);
        assert_eq!(handle, cloned);
    }

    #[test]
    fn drop_decrements_refcount() {
        let id = AssetId {
            index: 0,
            generation: 1,
        };
        let rc = Arc::new(());
        let handle = Handle::<TestAsset>::new(id, Arc::clone(&rc));
        assert_eq!(Arc::strong_count(&rc), 2);

        drop(handle);
        assert_eq!(Arc::strong_count(&rc), 1);
    }

    #[test]
    fn weak_upgrade_succeeds_while_strong_alive() {
        let id = AssetId {
            index: 0,
            generation: 1,
        };
        let handle = Handle::<TestAsset>::new(id, Arc::new(()));
        let weak = handle.downgrade();

        let upgraded = weak.upgrade();
        assert!(upgraded.is_some());
    }

    #[test]
    fn weak_upgrade_fails_after_strong_dropped() {
        let id = AssetId {
            index: 0,
            generation: 1,
        };
        let handle = Handle::<TestAsset>::new(id, Arc::new(()));
        let weak = handle.downgrade();

        drop(handle);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn untyped_roundtrip() {
        let id = AssetId {
            index: 0,
            generation: 1,
        };
        let handle = Handle::<TestAsset>::new(id, Arc::new(()));
        let untyped = handle.untyped();

        assert_eq!(untyped.asset_type_id(), TypeId::of::<TestAsset>());

        let typed = untyped.typed::<TestAsset>();
        assert!(typed.is_some());
    }

    #[test]
    fn untyped_wrong_type_returns_none() {
        let id = AssetId {
            index: 0,
            generation: 1,
        };
        let handle = Handle::<TestAsset>::new(id, Arc::new(()));
        let untyped = handle.untyped();

        let typed = untyped.typed::<OtherAsset>();
        assert!(typed.is_none());
    }
}
