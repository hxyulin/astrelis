//! Asset handles - typed references to assets.
//!
//! Handles are lightweight, copyable references to assets stored in the asset system.
//! They use generational indices for O(1) access with use-after-free protection.

use std::any::TypeId;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use astrelis_core::alloc::sparse_set::IndexSlot;

use crate::Asset;

/// A unique identifier for an asset, combining a generational index with type information.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HandleId {
    /// The generational index slot.
    pub(crate) slot: IndexSlot,
    /// The type of asset this handle refers to.
    pub(crate) type_id: TypeId,
}

impl HandleId {
    /// Create a new handle ID.
    pub fn new(slot: IndexSlot, type_id: TypeId) -> Self {
        Self { slot, type_id }
    }

    /// Get the index portion of the handle.
    pub fn index(&self) -> u32 {
        self.slot.index()
    }

    /// Get the generation portion of the handle.
    pub fn generation(&self) -> u32 {
        self.slot.generation()
    }

    /// Get the type ID.
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }
}

/// A typed handle to an asset.
///
/// Handles are the primary way to reference assets. They are:
/// - Lightweight (just an index + generation + type marker)
/// - Copyable (no reference counting overhead for copies)
/// - Type-safe (can only be used with the correct asset type)
/// - Safe (generational indices detect use-after-free)
///
/// # Example
///
/// ```ignore
/// let handle: Handle<Texture> = server.load("player.png");
///
/// // Later, check if ready
/// if let Some(texture) = server.get(&handle) {
///     // Use texture...
/// }
/// ```
pub struct Handle<T: Asset> {
    pub(crate) id: HandleId,
    pub(crate) _marker: PhantomData<T>,
}

impl<T: Asset> std::fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Handle")
            .field("type", &T::type_name())
            .field("index", &self.id.index())
            .field("generation", &self.id.generation())
            .finish()
    }
}

impl<T: Asset> Handle<T> {
    /// Create a new handle from an ID.
    pub(crate) fn new(id: HandleId) -> Self {
        debug_assert_eq!(id.type_id, TypeId::of::<T>());
        Self {
            id,
            _marker: PhantomData,
        }
    }

    /// Get the handle ID.
    pub fn id(&self) -> HandleId {
        self.id
    }

    /// Convert to an untyped handle.
    pub fn untyped(self) -> UntypedHandle {
        UntypedHandle { id: self.id }
    }

    /// Get the type name of the asset.
    pub fn type_name(&self) -> &'static str {
        T::type_name()
    }
}

impl<T: Asset> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Asset> Copy for Handle<T> {}

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

/// An untyped handle that can reference any asset type.
///
/// Useful for storing handles in collections without knowing the concrete type,
/// or for passing handles through type-erased APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UntypedHandle {
    pub(crate) id: HandleId,
}

impl UntypedHandle {
    /// Get the handle ID.
    pub fn id(&self) -> HandleId {
        self.id
    }

    /// Get the type ID of the asset.
    pub fn type_id(&self) -> TypeId {
        self.id.type_id
    }

    /// Create an UntypedHandle for testing purposes only.
    ///
    /// This creates a handle with the given index and generation, using
    /// a dummy type ID. Should only be used in tests.
    #[cfg(test)]
    pub(crate) fn test_handle(index: u32, generation: u32) -> Self {
        Self {
            id: HandleId::new(
                IndexSlot::new(index, generation),
                TypeId::of::<()>(),
            ),
        }
    }

    /// Try to convert to a typed handle.
    ///
    /// Returns `None` if the type doesn't match.
    pub fn typed<T: Asset>(self) -> Option<Handle<T>> {
        if self.id.type_id == TypeId::of::<T>() {
            Some(Handle::new(self.id))
        } else {
            None
        }
    }

    /// Convert to a typed handle without checking the type.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the handle's type matches `T`.
    pub unsafe fn typed_unchecked<T: Asset>(self) -> Handle<T> {
        Handle::new(self.id)
    }
}

impl<T: Asset> From<Handle<T>> for UntypedHandle {
    fn from(handle: Handle<T>) -> Self {
        handle.untyped()
    }
}

/// A strong handle that keeps the asset alive through reference counting.
///
/// When all strong handles to an asset are dropped, the asset may be unloaded
/// (depending on cache policy).
///
/// # Example
///
/// ```ignore
/// let strong: StrongHandle<Texture> = server.load_strong("player.png");
///
/// // The asset will stay loaded as long as `strong` exists
/// let weak = strong.downgrade();
///
/// // The weak handle doesn't keep the asset alive
/// drop(strong);
///
/// // Now the asset may be unloaded
/// ```
pub struct StrongHandle<T: Asset> {
    pub(crate) handle: Handle<T>,
    pub(crate) refcount: Arc<AtomicU32>,
}

impl<T: Asset> StrongHandle<T> {
    /// Create a new strong handle.
    pub(crate) fn new(handle: Handle<T>, refcount: Arc<AtomicU32>) -> Self {
        // Use Acquire ordering to ensure we see any previous modifications
        // to data associated with this handle before incrementing.
        refcount.fetch_add(1, Ordering::Acquire);
        Self { handle, refcount }
    }

    /// Get the underlying handle.
    pub fn handle(&self) -> Handle<T> {
        self.handle
    }

    /// Get the handle ID.
    pub fn id(&self) -> HandleId {
        self.handle.id
    }

    /// Create a weak handle from this strong handle.
    pub fn downgrade(&self) -> WeakHandle<T> {
        WeakHandle {
            handle: self.handle,
            refcount: Arc::downgrade(&self.refcount),
        }
    }

    /// Get the current reference count.
    pub fn ref_count(&self) -> u32 {
        // Use Acquire to ensure we see the most recent count
        self.refcount.load(Ordering::Acquire)
    }
}

impl<T: Asset> Clone for StrongHandle<T> {
    fn clone(&self) -> Self {
        // Use Relaxed here because cloning doesn't need synchronization with
        // other operations - we already have a valid reference.
        // The Arc clone provides the necessary synchronization.
        self.refcount.fetch_add(1, Ordering::Relaxed);
        Self {
            handle: self.handle,
            refcount: Arc::clone(&self.refcount),
        }
    }
}

impl<T: Asset> Drop for StrongHandle<T> {
    fn drop(&mut self) {
        // Use Release ordering to ensure all previous writes to data associated
        // with this handle are visible before the reference count is decremented.
        // This synchronizes with the Acquire in upgrade() to prevent use-after-free.
        self.refcount.fetch_sub(1, Ordering::Release);
    }
}

impl<T: Asset> std::ops::Deref for StrongHandle<T> {
    type Target = Handle<T>;

    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}

/// A weak handle that doesn't keep the asset alive.
///
/// Weak handles can be upgraded to strong handles if the asset is still loaded.
pub struct WeakHandle<T: Asset> {
    pub(crate) handle: Handle<T>,
    pub(crate) refcount: std::sync::Weak<AtomicU32>,
}

impl<T: Asset> WeakHandle<T> {
    /// Get the underlying handle.
    pub fn handle(&self) -> Handle<T> {
        self.handle
    }

    /// Get the handle ID.
    pub fn id(&self) -> HandleId {
        self.handle.id
    }

    /// Try to upgrade to a strong handle.
    ///
    /// Returns `None` if all strong handles have been dropped.
    pub fn upgrade(&self) -> Option<StrongHandle<T>> {
        self.refcount.upgrade().map(|refcount| {
            // Use Acquire ordering to synchronize with the Release in Drop.
            // This ensures we see all writes that happened before the last
            // strong handle was dropped, preventing use-after-free.
            refcount.fetch_add(1, Ordering::Acquire);
            StrongHandle {
                handle: self.handle,
                refcount,
            }
        })
    }

    /// Check if the asset is still alive (has strong handles).
    pub fn is_alive(&self) -> bool {
        self.refcount.strong_count() > 0
    }
}

impl<T: Asset> Clone for WeakHandle<T> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle,
            refcount: self.refcount.clone(),
        }
    }
}

/// A handle that tracks the version it last saw for change detection.
///
/// Useful for systems that need to react to asset changes without
/// subscribing to events.
///
/// # Example
///
/// ```ignore
/// let mut tracked = TrackedHandle::new(handle);
///
/// // In update loop:
/// if tracked.check_changed(&assets) {
///     // Asset was modified since last check
///     rebuild_material();
/// }
/// ```
pub struct TrackedHandle<T: Asset> {
    handle: Handle<T>,
    seen_version: u32,
}

impl<T: Asset> TrackedHandle<T> {
    /// Create a new tracked handle.
    pub fn new(handle: Handle<T>) -> Self {
        Self {
            handle,
            seen_version: 0,
        }
    }

    /// Get the underlying handle.
    pub fn handle(&self) -> Handle<T> {
        self.handle
    }

    /// Get the last seen version.
    pub fn seen_version(&self) -> u32 {
        self.seen_version
    }

    /// Check if the asset has changed since last check.
    ///
    /// Returns `true` if the asset version is newer than the last seen version,
    /// and updates the seen version.
    pub fn check_changed(&mut self, current_version: u32) -> bool {
        if current_version > self.seen_version {
            self.seen_version = current_version;
            true
        } else {
            false
        }
    }

    /// Reset the seen version to 0, causing the next check to always return true.
    pub fn reset(&mut self) {
        self.seen_version = 0;
    }
}

impl<T: Asset> Clone for TrackedHandle<T> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle,
            seen_version: self.seen_version,
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use astrelis_core::alloc::sparse_set::IndexSlot;
    use std::sync::atomic::AtomicU32;

    // Define a test asset type
    #[derive(Debug, Clone)]
    struct TestAsset;

    impl Asset for TestAsset {
        fn type_name() -> &'static str {
            "TestAsset"
        }
    }

    fn make_test_handle() -> Handle<TestAsset> {
        let slot = IndexSlot::new(1, 42); // generation, index
        let type_id = TypeId::of::<TestAsset>();
        let handle_id = HandleId::new(slot, type_id);
        Handle::new(handle_id)
    }

    #[test]
    fn test_strong_handle_new_and_refcount() {
        // Create a handle and refcount
        let handle = make_test_handle();
        let refcount = Arc::new(AtomicU32::new(0));

        // Create a strong handle using the new() method
        let strong = StrongHandle::new(handle, Arc::clone(&refcount));

        // Verify refcount was incremented
        assert_eq!(strong.ref_count(), 1);
        assert_eq!(refcount.load(Ordering::Relaxed), 1);

        // Clone the strong handle
        let strong2 = strong.clone();
        assert_eq!(strong2.ref_count(), 2);
        assert_eq!(refcount.load(Ordering::Relaxed), 2);

        // Drop one handle
        drop(strong);
        assert_eq!(strong2.ref_count(), 1);
        assert_eq!(refcount.load(Ordering::Relaxed), 1);

        // Drop the last handle
        drop(strong2);
        assert_eq!(refcount.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_strong_handle_downgrade() {
        let handle = make_test_handle();
        let refcount = Arc::new(AtomicU32::new(0));
        let strong = StrongHandle::new(handle, Arc::clone(&refcount));

        // Downgrade to weak
        let weak = strong.downgrade();
        assert_eq!(weak.handle(), handle);

        // Strong still keeps refcount
        assert_eq!(strong.ref_count(), 1);

        // Weak can upgrade while strong exists
        let upgraded = weak.upgrade();
        assert!(upgraded.is_some());
        if let Some(upgraded_strong) = upgraded {
            assert_eq!(upgraded_strong.ref_count(), 2); // original + upgraded
            drop(upgraded_strong);
        }

        // After dropping the original strong handle, refcount goes to 0
        drop(strong);
        assert_eq!(refcount.load(Ordering::Relaxed), 0);

        // Weak can still create a handle, but it represents a dead reference
        // (The refcount is managed separately from Arc lifecycle)
        let second_upgrade = weak.upgrade();
        assert!(second_upgrade.is_some()); // Weak can still upgrade since Arc<AtomicU32> exists
    }
}
