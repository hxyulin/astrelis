//! Text shaping cache for performance optimization.
//!
//! This module implements version-based text caching.
//! It caches shaped text results to avoid expensive reshaping operations every frame.

use astrelis_core::alloc::HashMap;
use std::sync::Arc;

/// Key for caching shaped text results.
///
/// Uses version numbers and bucketed dimensions to create stable cache keys
/// while allowing reasonable reuse across similar layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShapeKey {
    /// Font ID from the font system
    pub font_id: u32,
    /// Font size in pixels (rounded to nearest integer)
    pub font_size_px: u16,
    /// Text content version (from TextValue)
    pub text_hash: u32,
    /// Wrap width bucketed to 4px increments (0 = no wrap)
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

    /// Bucket width to 4px increments to increase cache hit rate.
    ///
    /// This allows text shaped at width 402px to be reused at 404px,
    /// trading minimal visual accuracy for better cache performance.
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
///
/// Stores the expensive results of text shaping so they can be reused
/// across multiple frames without reshaping.
#[derive(Debug, Clone)]
pub struct ShapedTextData {
    /// Text content that was shaped
    pub content: String,
    /// Measured bounds (width, height)
    pub bounds: (f32, f32),
    /// The shaped buffer from the font renderer
    /// Note: In a real implementation, this would contain the actual shaped runs,
    /// glyph positions, etc. For now we store measurement data.
    pub shaped_at_version: u32,
    /// Render count for this cached entry
    pub render_count: u64,
}

impl ShapedTextData {
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
    /// Statistics for monitoring cache performance
    pub hits: u64,
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
    ///
    /// Returns an Arc to the shaped data, allowing cheap cloning and sharing.
    /// The cache is invalidated automatically when the key changes (version bump).
    pub fn get_or_shape<F>(&mut self, key: ShapeKey, shape_fn: F) -> Arc<ShapedTextData>
    where
        F: FnOnce() -> ShapedTextData,
    {
        if let Some(cached) = self.cache.get_mut(&key) {
            self.hits += 1;
            // Increment render count to track cache effectiveness
            Arc::get_mut(cached).map(|data| data.render_count += 1);
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
            Arc::get_mut(cached).map(|data| data.render_count += 1);
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

    /// Clear the cache (useful when fonts are reloaded).
    pub fn clear(&mut self) {
        self.cache.clear();
        self.hits = 0;
        self.misses = 0;
    }

    /// Remove entries older than a certain version (garbage collection).
    pub fn prune_old_versions(&mut self, min_version: u32) {
        self.cache
            .retain(|_key, data| data.shaped_at_version >= min_version);
    }

    /// Get cache statistics.
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
        let total_renders: u64 = self
            .cache
            .values()
            .filter_map(|arc| Some(arc.render_count))
            .sum();
        format!(
            "TextCache: {} entries, {:.1}% hit rate ({} hits, {} misses), {} total renders",
            self.len(),
            self.hit_rate() * 100.0,
            self.hits,
            self.misses,
            total_renders
        )
    }

    /// Get average renders per cached entry (effectiveness metric).
    pub fn avg_renders_per_entry(&self) -> f32 {
        if self.cache.is_empty() {
            return 0.0;
        }
        let total_renders: u64 = self
            .cache
            .values()
            .filter_map(|arc| Some(arc.render_count))
            .sum();
        total_renders as f32 / self.cache.len() as f32
    }
}

impl Default for TextShapeCache {
    fn default() -> Self {
        Self::new()
    }
}
