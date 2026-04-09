//! Type-safe generic ID handles.
//!
//! [`Id<T>`] is a lightweight wrapper over [`u64`] that uses a phantom type
//! parameter to prevent mixing IDs from different domains (e.g., window IDs
//! vs entity IDs).
//!
//! # Example
//!
//! ```
//! use astrelis_core::id::Id;
//!
//! struct Window;
//! struct Entity;
//!
//! let window_id: Id<Window> = Id::new(1);
//! let entity_id: Id<Entity> = Id::new(1);
//!
//! // Same raw value, but the type system prevents mixing them:
//! // window_id == entity_id  // compile error!
//! assert_eq!(window_id.raw(), entity_id.raw());
//! ```

use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

/// A type-safe identifier parameterized by domain type `T`.
pub struct Id<T> {
    raw: u64,
    _marker: PhantomData<fn() -> T>,
}

// Manual impls because derive would add bounds on T.

impl<T> Id<T> {
    /// Creates a new ID from a raw `u64` value.
    #[inline]
    pub const fn new(raw: u64) -> Self {
        Self {
            raw,
            _marker: PhantomData,
        }
    }

    /// Returns the underlying raw value.
    #[inline]
    pub const fn raw(self) -> u64 {
        self.raw
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Id<T> {}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl<T> Eq for Id<T> {}

impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id<{}>({})", std::any::type_name::<T>(), self.raw)
    }
}

impl<T> fmt::Display for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Window;
    struct Entity;

    #[test]
    fn same_raw_different_types() {
        let w: Id<Window> = Id::new(42);
        let e: Id<Entity> = Id::new(42);
        assert_eq!(w.raw(), e.raw());
        // They should NOT be comparable across types (enforced at compile time).
    }

    #[test]
    fn equality() {
        let a: Id<Window> = Id::new(1);
        let b: Id<Window> = Id::new(1);
        let c: Id<Window> = Id::new(2);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn debug_format() {
        let id: Id<Window> = Id::new(7);
        let dbg = format!("{id:?}");
        assert!(dbg.contains("Window"));
        assert!(dbg.contains("7"));
    }

    #[test]
    fn display_format() {
        let id: Id<Window> = Id::new(99);
        assert_eq!(format!("{id}"), "99");
    }
}
