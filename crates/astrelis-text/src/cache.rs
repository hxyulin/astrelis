//! Pluggable text shaping cache.
//!
//! Provides a [`ShapeCache`] trait for user-defined caching strategies and a
//! built-in [`HashMapShapeCache`] backed by `RwLock<HashMap>`.
//!
//! # Caching modes
//!
//! The [`TextPipeline`](crate::pipeline::TextPipeline) accepts an
//! `Option<Box<dyn ShapeCache>>`:
//!
//! - **Default** — `TextPipeline::new()` uses [`HashMapShapeCache`].
//! - **Disabled** — `TextPipeline::without_cache()` passes `None`; every
//!   request is shaped from scratch (useful for benchmarking or when
//!   the caller manages its own cache).
//! - **Custom** — `TextPipeline::with_cache(Some(Box::new(my_cache)))`
//!   lets you supply an LRU cache, a bounded cache, a `DashMap`-backed
//!   concurrent cache, etc.
//!
//! # Implementing a custom cache
//!
//! ```ignore
//! use astrelis_text::{ShapeCache, ShapeKey, ShapedTextResult};
//! use std::sync::Arc;
//!
//! struct MyLruCache { /* ... */ }
//!
//! impl ShapeCache for MyLruCache {
//!     fn get(&self, key: &ShapeKey) -> Option<Arc<ShapedTextResult>> { todo!() }
//!     fn insert(&self, key: ShapeKey, value: Arc<ShapedTextResult>) { todo!() }
//!     fn clear(&self) { todo!() }
//!     fn len(&self) -> usize { todo!() }
//! }
//! ```

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::shaping::ShapedTextResult;

/// Key for caching shaped text results.
///
/// Uses bucketed dimensions (4px increments) for wrap width to increase
/// cache hit rate across similar layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShapeKey {
    /// Font ID from the font system.
    pub font_id: u32,
    /// Font size in pixels (rounded to nearest integer).
    pub font_size_px: u16,
    /// Text content hash.
    pub text_hash: u32,
    /// Wrap width bucketed to 4px increments (0 = no wrap).
    pub wrap_width_bucket: u16,
}

impl ShapeKey {
    /// Create a new shape key with width bucketing.
    pub fn new(font_id: u32, font_size: f32, text_content: &str, wrap_width: Option<f32>) -> Self {
        Self {
            font_id,
            font_size_px: font_size.round() as u16,
            text_hash: fxhash::hash32(text_content),
            wrap_width_bucket: wrap_width.map(Self::bucket_width).unwrap_or(0),
        }
    }

    /// Bucket width to 4px increments.
    fn bucket_width(width: f32) -> u16 {
        (width / 4.0).round() as u16
    }

    /// Get the actual bucketed width value.
    pub fn bucketed_width(&self) -> Option<f32> {
        if self.wrap_width_bucket == 0 {
            None
        } else {
            Some(self.wrap_width_bucket as f32 * 4.0)
        }
    }
}

/// Trait for text shape caching strategies.
///
/// Implementations must use interior mutability (`RwLock`, `DashMap`, etc.)
/// since methods take `&self`. This enables `Arc<dyn ShapeCache>` sharing
/// across threads when needed.
///
/// # Thread Safety
///
/// All implementations must be `Send + Sync`.
pub trait ShapeCache: Send + Sync {
    /// Look up a cached shaping result by key.
    fn get(&self, key: &ShapeKey) -> Option<Arc<ShapedTextResult>>;

    /// Store a shaping result in the cache.
    fn insert(&self, key: ShapeKey, value: Arc<ShapedTextResult>);

    /// Remove all entries from the cache.
    fn clear(&self);

    /// Number of entries currently in the cache.
    fn len(&self) -> usize;

    /// Whether the cache contains no entries.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Default hash-map-based shape cache.
///
/// Uses `RwLock<HashMap>` for thread safety. Lock contention is negligible
/// for typical single-threaded game loops; for highly concurrent scenarios
/// consider a `DashMap`-backed implementation.
pub struct HashMapShapeCache {
    inner: RwLock<HashMap<ShapeKey, Arc<ShapedTextResult>>>,
}

impl HashMapShapeCache {
    /// Create a new cache with default capacity (256 entries).
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::with_capacity(256)),
        }
    }

    /// Create a new cache with the specified initial capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: RwLock::new(HashMap::with_capacity(capacity)),
        }
    }

    /// Remove entries that are only referenced by the cache itself
    /// (strong reference count == 1).
    ///
    /// Useful for evicting entries that are no longer held by any
    /// pipeline result or user code.
    pub fn prune_unreferenced(&self) {
        let mut map = self.inner.write().unwrap();
        map.retain(|_, v| Arc::strong_count(v) > 1);
    }

    /// Retain only entries matching a predicate.
    ///
    /// This is an eviction hook specific to this implementation.
    /// Custom caches should expose their own eviction API.
    pub fn retain<F>(&self, f: F)
    where
        F: FnMut(&ShapeKey, &mut Arc<ShapedTextResult>) -> bool,
    {
        let mut map = self.inner.write().unwrap();
        map.retain(f);
    }
}

impl ShapeCache for HashMapShapeCache {
    fn get(&self, key: &ShapeKey) -> Option<Arc<ShapedTextResult>> {
        self.inner.read().unwrap().get(key).cloned()
    }

    fn insert(&self, key: ShapeKey, value: Arc<ShapedTextResult>) {
        self.inner.write().unwrap().insert(key, value);
    }

    fn clear(&self) {
        self.inner.write().unwrap().clear();
    }

    fn len(&self) -> usize {
        self.inner.read().unwrap().len()
    }
}

impl Default for HashMapShapeCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shape_key_creation() {
        let key = ShapeKey::new(0, 16.0, "Hello", None);
        assert_eq!(key.font_id, 0);
        assert_eq!(key.font_size_px, 16);
        assert_eq!(key.wrap_width_bucket, 0);
        assert_eq!(key.bucketed_width(), None);
    }

    #[test]
    fn test_shape_key_width_bucketing() {
        let key1 = ShapeKey::new(0, 16.0, "Hello", Some(402.0));
        let key2 = ShapeKey::new(0, 16.0, "Hello", Some(404.0));
        // Should bucket to same value (within 4px)
        assert_eq!(key1.wrap_width_bucket, key2.wrap_width_bucket);
    }

    fn make_result() -> ShapedTextResult {
        ShapedTextResult::new((100.0, 20.0), Vec::new())
    }

    #[test]
    fn test_hashmap_cache_get_insert() {
        let cache = HashMapShapeCache::new();
        let key = ShapeKey::new(0, 16.0, "Hello", None);

        assert!(cache.get(&key).is_none());
        assert!(cache.is_empty());

        cache.insert(key, Arc::new(make_result()));

        assert!(cache.get(&key).is_some());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_hashmap_cache_clear() {
        let cache = HashMapShapeCache::new();
        let key = ShapeKey::new(0, 16.0, "Hello", None);
        cache.insert(key, Arc::new(make_result()));
        assert_eq!(cache.len(), 1);

        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_hashmap_cache_prune_unreferenced() {
        let cache = HashMapShapeCache::new();

        let key1 = ShapeKey::new(0, 16.0, "held", None);
        let key2 = ShapeKey::new(0, 16.0, "dropped", None);

        let held = Arc::new(make_result());
        cache.insert(key1, held.clone()); // strong count = 2 (held + cache)
        cache.insert(key2, Arc::new(make_result())); // strong count = 1 (cache only)

        assert_eq!(cache.len(), 2);
        cache.prune_unreferenced();
        assert_eq!(cache.len(), 1);
        assert!(cache.get(&key1).is_some());
        assert!(cache.get(&key2).is_none());

        drop(held);
    }

    #[test]
    fn test_hashmap_cache_retain() {
        let cache = HashMapShapeCache::new();

        cache.insert(
            ShapeKey::new(0, 16.0, "small", None),
            Arc::new(ShapedTextResult::new((50.0, 10.0), Vec::new())),
        );
        cache.insert(
            ShapeKey::new(0, 16.0, "large", None),
            Arc::new(ShapedTextResult::new((200.0, 40.0), Vec::new())),
        );

        assert_eq!(cache.len(), 2);
        cache.retain(|_, v| v.bounds.0 > 100.0);
        assert_eq!(cache.len(), 1);
    }
}
