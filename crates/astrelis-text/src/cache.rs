//! Text shaping cache for performance optimization.
//!
//! Caches shaped text results to avoid expensive reshaping every frame.
//! Uses version-based keys with width bucketing for stable cache hits.

use std::collections::HashMap;
use std::sync::Arc;

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

/// Cached shaped text data.
#[derive(Debug, Clone)]
pub struct ShapedTextData {
    /// Text content that was shaped.
    pub content: String,
    /// Measured bounds `(width, height)`.
    pub bounds: (f32, f32),
    /// Version the text was shaped at.
    pub shaped_at_version: u32,
    /// Render count for this cached entry.
    pub render_count: u64,
}

impl ShapedTextData {
    /// Create new shaped text data.
    pub fn new(content: String, bounds: (f32, f32), version: u32) -> Self {
        Self {
            content,
            bounds,
            shaped_at_version: version,
            render_count: 0,
        }
    }
}

/// Cache for shaped text results.
///
/// Uses version-based keys to invalidate cached data when text content,
/// font properties, or layout constraints change.
pub struct TextShapeCache {
    cache: HashMap<ShapeKey, Arc<ShapedTextData>>,
    /// Total cache hits.
    pub hits: u64,
    /// Total cache misses.
    pub misses: u64,
}

impl TextShapeCache {
    /// Create a new empty text shape cache.
    pub fn new() -> Self {
        Self {
            cache: HashMap::with_capacity(256),
            hits: 0,
            misses: 0,
        }
    }

    /// Get cached shaped text or compute it.
    pub fn get_or_shape<F>(&mut self, key: ShapeKey, shape_fn: F) -> Arc<ShapedTextData>
    where
        F: FnOnce() -> ShapedTextData,
    {
        if let Some(cached) = self.cache.get_mut(&key) {
            self.hits += 1;
            if let Some(data) = Arc::get_mut(cached) {
                data.render_count += 1;
            }
            return cached.clone();
        }

        self.misses += 1;
        let shaped = Arc::new(shape_fn());
        self.cache.insert(key, shaped.clone());
        shaped
    }

    /// Get cached data without computing if missing.
    pub fn get(&mut self, key: &ShapeKey) -> Option<Arc<ShapedTextData>> {
        let result = self.cache.get_mut(key).map(|cached| {
            if let Some(data) = Arc::get_mut(cached) {
                data.render_count += 1;
            }
            cached.clone()
        });
        if result.is_some() {
            self.hits += 1;
        } else {
            self.misses += 1;
        }
        result
    }

    /// Insert shaped data into the cache.
    pub fn insert(&mut self, key: ShapeKey, data: ShapedTextData) -> Arc<ShapedTextData> {
        let arc_data = Arc::new(data);
        self.cache.insert(key, arc_data.clone());
        arc_data
    }

    /// Clear the cache.
    pub fn clear(&mut self) {
        self.cache.clear();
        self.hits = 0;
        self.misses = 0;
    }

    /// Remove entries older than a certain version.
    pub fn prune_old_versions(&mut self, min_version: u32) {
        self.cache
            .retain(|_key, data| data.shaped_at_version >= min_version);
    }

    /// Get cache hit rate as a fraction (0.0 to 1.0).
    pub fn hit_rate(&self) -> f32 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f32 / total as f32
        }
    }

    /// Get the number of cached entries.
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Get cache statistics as a formatted string.
    pub fn stats_string(&self) -> String {
        let total_renders: u64 = self.cache.values().map(|arc| arc.render_count).sum();
        format!(
            "TextCache: {} entries, {:.1}% hit rate ({} hits, {} misses), {} total renders",
            self.len(),
            self.hit_rate() * 100.0,
            self.hits,
            self.misses,
            total_renders
        )
    }
}

impl Default for TextShapeCache {
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

    #[test]
    fn test_cache_hit_miss() {
        let mut cache = TextShapeCache::new();
        let key = ShapeKey::new(0, 16.0, "Hello", None);

        // Miss
        assert!(cache.get(&key).is_none());
        assert_eq!(cache.misses, 1);

        // Insert
        cache.insert(key, ShapedTextData::new("Hello".into(), (100.0, 20.0), 1));

        // Hit
        assert!(cache.get(&key).is_some());
        assert_eq!(cache.hits, 1);
    }

    #[test]
    fn test_get_or_shape() {
        let mut cache = TextShapeCache::new();
        let key = ShapeKey::new(0, 16.0, "Hello", None);

        let data1 = cache.get_or_shape(key, || {
            ShapedTextData::new("Hello".into(), (100.0, 20.0), 1)
        });
        assert_eq!(cache.misses, 1);

        let data2 = cache.get_or_shape(key, || {
            panic!("should not be called");
        });
        assert_eq!(cache.hits, 1);
        assert_eq!(data1.bounds, data2.bounds);
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = TextShapeCache::new();
        let key = ShapeKey::new(0, 16.0, "Hello", None);
        cache.insert(key, ShapedTextData::new("Hello".into(), (100.0, 20.0), 1));
        assert_eq!(cache.len(), 1);

        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.hits, 0);
        assert_eq!(cache.misses, 0);
    }

    #[test]
    fn test_prune_old_versions() {
        let mut cache = TextShapeCache::new();

        cache.insert(
            ShapeKey::new(0, 16.0, "old", None),
            ShapedTextData::new("old".into(), (50.0, 20.0), 1),
        );
        cache.insert(
            ShapeKey::new(0, 16.0, "new", None),
            ShapedTextData::new("new".into(), (60.0, 20.0), 5),
        );

        cache.prune_old_versions(3);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_hit_rate() {
        let mut cache = TextShapeCache::new();
        assert_eq!(cache.hit_rate(), 0.0);

        let key = ShapeKey::new(0, 16.0, "x", None);
        cache.insert(key, ShapedTextData::new("x".into(), (10.0, 10.0), 1));

        let _ = cache.get(&key); // hit
        let _ = cache.get(&ShapeKey::new(0, 16.0, "y", None)); // miss

        assert_eq!(cache.hit_rate(), 0.5);
    }
}
