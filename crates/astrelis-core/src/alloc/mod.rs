//! Optimized allocation and collection types for Astrelis.
//!
//! This module provides:
//! - Re-exports of optimized hash collections using AHash
//! - SparseSet data structure for generational indices
//! - Common allocation utilities

pub mod sparse_set;

// Re-export optimized hash collections
pub use ahash::{AHashMap as HashMap, AHashSet as HashSet, RandomState};

/// Type alias for the standard HashMap with AHash for better performance.
pub type AHashMap<K, V> = ahash::AHashMap<K, V>;

/// Type alias for the standard HashSet with AHash for better performance.
pub type AHashSet<T> = ahash::AHashSet<T>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hashmap_ahash() {
        let mut map = HashMap::new();
        map.insert("key", "value");
        assert_eq!(map.get("key"), Some(&"value"));
    }

    #[test]
    fn test_hashset_ahash() {
        let mut set = HashSet::new();
        set.insert(42);
        assert!(set.contains(&42));
    }
}
