//! Type-map resource container with runtime borrow checking.
//!
//! [`Resources`] stores shared state keyed by [`TypeId`]. Systems access
//! resources through [`Ref<T>`] and [`RefMut<T>`] guards that enforce
//! single-writer / multiple-reader semantics at runtime.

use std::any::{Any, TypeId, type_name};
use std::cell::{Cell, UnsafeCell};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

/// Borrow state for a single resource slot.
///
/// Tracks the number of active borrows:
/// - `0` means no borrows
/// - positive means that many shared borrows
/// - `-1` means one exclusive borrow
struct BorrowFlag {
    flag: Cell<isize>,
}

impl BorrowFlag {
    fn new() -> Self {
        Self {
            flag: Cell::new(0),
        }
    }

    fn borrow(&self) -> bool {
        let val = self.flag.get();
        if val < 0 {
            return false;
        }
        self.flag.set(val + 1);
        true
    }

    fn unborrow(&self) {
        let val = self.flag.get();
        debug_assert!(val > 0);
        self.flag.set(val - 1);
    }

    fn borrow_mut(&self) -> bool {
        if self.flag.get() != 0 {
            return false;
        }
        self.flag.set(-1);
        true
    }

    fn unborrow_mut(&self) {
        debug_assert_eq!(self.flag.get(), -1);
        self.flag.set(0);
    }
}

/// A single resource slot holding the value and its borrow flag.
struct ResourceSlot {
    value: UnsafeCell<Box<dyn Any>>,
    borrow: BorrowFlag,
}

/// A type-map container for shared engine state.
///
/// Resources are inserted by type and accessed through runtime-checked
/// borrow guards. This is the primary mechanism for inter-system
/// communication in the application framework.
///
/// # Panics
///
/// [`get`](Resources::get) and [`get_mut`](Resources::get_mut) panic if
/// the resource is missing or if the borrow would violate single-writer /
/// multiple-reader rules. Use [`try_get`](Resources::try_get) and
/// [`try_get_mut`](Resources::try_get_mut) for non-panicking variants.
pub struct Resources {
    slots: HashMap<TypeId, ResourceSlot>,
}

impl Resources {
    /// Creates an empty resource container.
    pub fn new() -> Self {
        Self {
            slots: HashMap::new(),
        }
    }

    /// Inserts a resource, replacing any previous value of the same type.
    pub fn insert<T: 'static>(&mut self, value: T) {
        let id = TypeId::of::<T>();
        self.slots.insert(
            id,
            ResourceSlot {
                value: UnsafeCell::new(Box::new(value)),
                borrow: BorrowFlag::new(),
            },
        );
    }

    /// Removes a resource and returns it, or `None` if not present.
    pub fn remove<T: 'static>(&mut self) -> Option<T> {
        let id = TypeId::of::<T>();
        self.slots.remove(&id).map(|slot| {
            *slot.value.into_inner().downcast::<T>().expect("type mismatch")
        })
    }

    /// Returns `true` if a resource of type `T` is present.
    pub fn contains<T: 'static>(&self) -> bool {
        self.slots.contains_key(&TypeId::of::<T>())
    }

    /// Borrows a resource immutably.
    ///
    /// # Panics
    ///
    /// Panics if the resource is missing or already mutably borrowed.
    pub fn get<T: 'static>(&self) -> Ref<'_, T> {
        self.try_get::<T>().unwrap_or_else(|| {
            panic!(
                "Resource `{}` is not available (missing or already mutably borrowed)",
                type_name::<T>()
            )
        })
    }

    /// Borrows a resource mutably.
    ///
    /// # Panics
    ///
    /// Panics if the resource is missing or already borrowed.
    pub fn get_mut<T: 'static>(&self) -> RefMut<'_, T> {
        self.try_get_mut::<T>().unwrap_or_else(|| {
            panic!(
                "Resource `{}` is not available (missing or already borrowed)",
                type_name::<T>()
            )
        })
    }

    /// Tries to borrow a resource immutably.
    ///
    /// Returns `None` if the resource is missing or already mutably borrowed.
    pub fn try_get<T: 'static>(&self) -> Option<Ref<'_, T>> {
        let id = TypeId::of::<T>();
        let slot = self.slots.get(&id)?;
        if !slot.borrow.borrow() {
            return None;
        }
        // SAFETY: We just acquired a shared borrow via the flag, and the
        // value's type is guaranteed by the TypeId key.
        let ptr = unsafe { &*slot.value.get() };
        let value = ptr.downcast_ref::<T>().expect("type mismatch");
        Some(Ref {
            value,
            borrow: &slot.borrow,
            _marker: PhantomData,
        })
    }

    /// Tries to borrow a resource mutably.
    ///
    /// Returns `None` if the resource is missing or already borrowed.
    pub fn try_get_mut<T: 'static>(&self) -> Option<RefMut<'_, T>> {
        let id = TypeId::of::<T>();
        let slot = self.slots.get(&id)?;
        if !slot.borrow.borrow_mut() {
            return None;
        }
        // SAFETY: We just acquired an exclusive borrow via the flag, and the
        // value's type is guaranteed by the TypeId key.
        let ptr = unsafe { &mut *slot.value.get() };
        let value = ptr.downcast_mut::<T>().expect("type mismatch");
        Some(RefMut {
            value,
            borrow: &slot.borrow,
            _marker: PhantomData,
        })
    }
}

impl Default for Resources {
    fn default() -> Self {
        Self::new()
    }
}

/// Immutable borrow guard for a resource.
pub struct Ref<'a, T: 'static> {
    value: &'a T,
    borrow: &'a BorrowFlag,
    _marker: PhantomData<&'a T>,
}

impl<T: 'static> Deref for Ref<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.value
    }
}

impl<T: 'static> Drop for Ref<'_, T> {
    fn drop(&mut self) {
        self.borrow.unborrow();
    }
}

/// Mutable borrow guard for a resource.
pub struct RefMut<'a, T: 'static> {
    value: &'a mut T,
    borrow: &'a BorrowFlag,
    _marker: PhantomData<&'a mut T>,
}

impl<T: 'static> Deref for RefMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.value
    }
}

impl<T: 'static> DerefMut for RefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.value
    }
}

impl<T: 'static> Drop for RefMut<'_, T> {
    fn drop(&mut self) {
        self.borrow.unborrow_mut();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_get() {
        let mut res = Resources::new();
        res.insert(42u32);
        assert_eq!(*res.get::<u32>(), 42);
    }

    #[test]
    fn get_mut_modifies() {
        let mut res = Resources::new();
        res.insert(10i32);
        {
            let mut val = res.get_mut::<i32>();
            *val += 5;
        }
        assert_eq!(*res.get::<i32>(), 15);
    }

    #[test]
    fn multiple_shared_borrows() {
        let mut res = Resources::new();
        res.insert(String::from("hello"));
        let a = res.get::<String>();
        let b = res.get::<String>();
        assert_eq!(&*a, "hello");
        assert_eq!(&*b, "hello");
    }

    #[test]
    fn try_get_mut_fails_during_shared_borrow() {
        let mut res = Resources::new();
        res.insert(42u32);
        let _shared = res.get::<u32>();
        assert!(res.try_get_mut::<u32>().is_none());
    }

    #[test]
    fn try_get_fails_during_exclusive_borrow() {
        let mut res = Resources::new();
        res.insert(42u32);
        let _excl = res.get_mut::<u32>();
        assert!(res.try_get::<u32>().is_none());
    }

    #[test]
    fn contains_and_remove() {
        let mut res = Resources::new();
        assert!(!res.contains::<u32>());
        res.insert(42u32);
        assert!(res.contains::<u32>());
        let val = res.remove::<u32>();
        assert_eq!(val, Some(42));
        assert!(!res.contains::<u32>());
    }

    #[test]
    fn different_types_independent() {
        let mut res = Resources::new();
        res.insert(42u32);
        res.insert(String::from("test"));
        let _num = res.get_mut::<u32>();
        let _str = res.get::<String>();
        // Different types can be borrowed independently.
    }
}
