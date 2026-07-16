//! Interned string table used for scope names, counter names, thread
//! names, and categories.
//!
//! Insertion takes a write lock; lookups of an already-interned string
//! take a read lock. Intern once, reuse the `StringId` forever.
//! The table is append-only: strings are never removed. Ids remain
//! valid for the entire process lifetime.

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::RwLock;

use crate::data::StringId;

/// Interned string table.
pub struct StringTable {
    inner: RwLock<Inner>,
}

struct Inner {
    strings: Vec<String>,
    index: HashMap<String, StringId>,
}

impl StringTable {
    /// Creates an empty string table.
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(Inner {
                strings: Vec::new(),
                index: HashMap::new(),
            }),
        }
    }

    /// Interns `s` and returns its [`StringId`]. If `s` is already
    /// interned, returns the existing id without inserting.
    ///
    /// Hot path for scope names: a fast-path read-lock lookup catches
    /// the common case. Only previously-unseen strings touch the
    /// write lock.
    pub fn intern(&self, s: &str) -> StringId {
        {
            let inner = self.inner.read().unwrap();
            if let Some(&id) = inner.index.get(s) {
                return id;
            }
        }
        let mut inner = self.inner.write().unwrap();
        if let Some(&id) = inner.index.get(s) {
            return id;
        }
        let idx = inner.strings.len() as u32;
        // +1 so that the NonZeroU32 invariant holds; 0 is reserved.
        let id = StringId(NonZeroU32::new(idx + 1).expect("string table overflow"));
        inner.strings.push(s.to_owned());
        inner.index.insert(s.to_owned(), id);
        id
    }

    /// Looks up a string by id. Returns an owned copy because the
    /// read lock cannot be held across the return value.
    pub fn get(&self, id: StringId) -> Option<String> {
        let inner = self.inner.read().unwrap();
        let idx = (id.0.get() - 1) as usize;
        inner.strings.get(idx).cloned()
    }

    /// Number of unique strings interned.
    pub fn len(&self) -> usize {
        self.inner.read().unwrap().strings.len()
    }

    /// Returns `true` if no strings are interned.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for StringTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_returns_same_id_for_same_string() {
        let t = StringTable::new();
        let a = t.intern("hello");
        let b = t.intern("hello");
        assert_eq!(a, b);
    }

    #[test]
    fn intern_returns_different_ids_for_different_strings() {
        let t = StringTable::new();
        let a = t.intern("hello");
        let b = t.intern("world");
        assert_ne!(a, b);
    }

    #[test]
    fn get_returns_interned_string() {
        let t = StringTable::new();
        let id = t.intern("foo");
        assert_eq!(t.get(id).as_deref(), Some("foo"));
    }
}
