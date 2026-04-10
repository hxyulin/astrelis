//! Text shaping pipeline for deferred/async text processing.
//!
//! Provides a worker-ready abstraction for text shaping that can be executed
//! synchronously now and moved to worker threads later without API changes.

use crate::cache::{HashMapShapeCache, ShapeCache, ShapeKey};
use crate::shaping::ShapedTextResult as BaseShapedTextResult;
use std::collections::HashMap;
use std::sync::Arc;

/// Unique identifier for a text shaping request.
pub type RequestId = u64;

/// Request for text shaping with all necessary parameters.
///
/// Uses owned data (`String`) instead of references for `Send + Sync` compatibility.
#[derive(Debug, Clone)]
pub struct TextShapeRequest {
    /// Unique ID for this request.
    pub id: RequestId,
    /// Text content to shape.
    pub text: String,
    /// Font identifier from font system.
    pub font_id: u32,
    /// Font size in pixels.
    pub font_size: f32,
    /// Optional wrap width for text layout.
    pub wrap_width: Option<f32>,
}

impl TextShapeRequest {
    /// Create a new text shape request.
    pub fn new(
        id: RequestId,
        text: String,
        font_id: u32,
        font_size: f32,
        wrap_width: Option<f32>,
    ) -> Self {
        Self {
            id,
            text,
            font_id,
            font_size,
            wrap_width,
        }
    }

    /// Create a shape key for caching.
    pub fn shape_key(&self) -> ShapeKey {
        ShapeKey::new(self.font_id, self.font_size, &self.text, self.wrap_width)
    }
}

/// Result of text shaping with pipeline metadata.
#[derive(Debug, Clone)]
pub struct ShapedTextResult {
    /// Original request ID.
    pub request_id: RequestId,
    /// Shared shaped text data (zero-copy with cache).
    pub inner: Arc<BaseShapedTextResult>,
    /// Number of times this result has been rendered.
    pub render_count: u64,
}

impl ShapedTextResult {
    /// Create a new shaped text result.
    pub fn new(request_id: RequestId, inner: Arc<BaseShapedTextResult>) -> Self {
        Self {
            request_id,
            inner,
            render_count: 0,
        }
    }

    /// Get the bounds of the shaped text.
    pub fn bounds(&self) -> (f32, f32) {
        self.inner.bounds
    }

    /// Increment render count.
    pub fn increment_render_count(&mut self) {
        self.render_count = self.render_count.saturating_add(1);
    }
}

/// Trait for text shaping implementations.
///
/// Allows swapping between sync and async implementations.
pub trait TextShaper: Send + Sync {
    /// Shape text according to the request parameters.
    fn shape(&mut self, request: TextShapeRequest) -> ShapedTextResult;
}

/// Synchronous text shaper using a callback for measurement.
pub struct SyncTextShaper;

impl Default for SyncTextShaper {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncTextShaper {
    /// Create a new synchronous text shaper.
    pub fn new() -> Self {
        Self {}
    }

    /// Shape text using the provided shaping function.
    pub fn shape_with_measurer<F>(request: &TextShapeRequest, shape_fn: F) -> ShapedTextResult
    where
        F: FnOnce(&str, f32, Option<f32>) -> BaseShapedTextResult,
    {
        let inner = shape_fn(&request.text, request.font_size, request.wrap_width);
        ShapedTextResult::new(request.id, Arc::new(inner))
    }
}

/// Text shaping pipeline managing requests, results, and caching.
///
/// # Caching
///
/// The pipeline delegates caching to a [`ShapeCache`] implementation:
///
/// ```
/// use astrelis_text::TextPipeline;
///
/// // Default (HashMap-backed cache)
/// let pipeline = TextPipeline::new();
///
/// // No caching — every request is shaped from scratch
/// let pipeline = TextPipeline::without_cache();
/// ```
///
/// For a custom cache, implement [`ShapeCache`] and pass it via
/// [`TextPipeline::with_cache`].
pub struct TextPipeline {
    pending: HashMap<RequestId, TextShapeRequest>,
    completed: HashMap<RequestId, Arc<ShapedTextResult>>,
    next_request_id: RequestId,
    cache: Option<Box<dyn ShapeCache>>,
    /// Total cache hits.
    pub cache_hits: u64,
    /// Total cache misses.
    pub cache_misses: u64,
    /// Total requests processed.
    pub total_requests: u64,
}

impl TextPipeline {
    /// Create a pipeline with the default [`HashMapShapeCache`].
    pub fn new() -> Self {
        Self::with_cache(Some(Box::new(HashMapShapeCache::new())))
    }

    /// Create a pipeline with no caching.
    ///
    /// Every request will be shaped from scratch. Useful for benchmarking,
    /// debugging, or when the caller manages its own external cache.
    pub fn without_cache() -> Self {
        Self::with_cache(None)
    }

    /// Create a pipeline with a custom cache implementation.
    ///
    /// Pass `None` to disable caching, or `Some(Box::new(my_cache))`
    /// for a user-provided [`ShapeCache`].
    pub fn with_cache(cache: Option<Box<dyn ShapeCache>>) -> Self {
        Self {
            pending: HashMap::with_capacity(64),
            completed: HashMap::with_capacity(64),
            next_request_id: 1,
            cache,
            cache_hits: 0,
            cache_misses: 0,
            total_requests: 0,
        }
    }

    /// Replace the cache implementation at runtime.
    ///
    /// Resets hit/miss counters. Useful for switching strategies between
    /// scenes or game states.
    pub fn set_cache(&mut self, cache: Option<Box<dyn ShapeCache>>) {
        self.cache = cache;
        self.cache_hits = 0;
        self.cache_misses = 0;
    }

    /// Get a reference to the current cache, if any.
    pub fn cache(&self) -> Option<&dyn ShapeCache> {
        self.cache.as_deref()
    }

    /// Request text shaping, returns request ID.
    ///
    /// If a matching result is found in the cache, it is immediately
    /// available via [`get_completed`](Self::get_completed).
    pub fn request_shape(
        &mut self,
        text: String,
        font_id: u32,
        font_size: f32,
        wrap_width: Option<f32>,
    ) -> RequestId {
        self.total_requests += 1;
        let request_id = self.next_request_id;
        self.next_request_id += 1;

        let request = TextShapeRequest::new(request_id, text, font_id, font_size, wrap_width);
        let shape_key = request.shape_key();

        if let Some(cache) = &self.cache {
            if let Some(cached) = cache.get(&shape_key) {
                self.cache_hits += 1;
                let result = ShapedTextResult::new(request_id, cached);
                self.completed.insert(request_id, Arc::new(result));
                return request_id;
            }
        }

        self.cache_misses += 1;
        self.pending.insert(request_id, request);
        request_id
    }

    /// Process all pending requests using the provided shaping function.
    pub fn process_pending<F>(&mut self, shape_fn: F)
    where
        F: Fn(&str, f32, Option<f32>) -> BaseShapedTextResult,
    {
        if self.pending.is_empty() {
            return;
        }

        let mut completed_requests = Vec::new();

        for (_request_id, request) in self.pending.drain() {
            let inner = Arc::new(shape_fn(&request.text, request.font_size, request.wrap_width));

            if let Some(cache) = &self.cache {
                cache.insert(request.shape_key(), inner.clone());
            }

            let result = ShapedTextResult::new(request.id, inner);
            completed_requests.push((request.id, Arc::new(result)));
        }

        for (request_id, result) in completed_requests {
            self.completed.insert(request_id, result);
        }
    }

    /// Take a completed result by request ID.
    pub fn take_completed(&mut self, request_id: RequestId) -> Option<Arc<ShapedTextResult>> {
        self.completed.remove(&request_id)
    }

    /// Get a completed result without removing it.
    pub fn get_completed(&self, request_id: RequestId) -> Option<Arc<ShapedTextResult>> {
        self.completed.get(&request_id).cloned()
    }

    /// Check if a request is still pending.
    pub fn is_pending(&self, request_id: RequestId) -> bool {
        self.pending.contains_key(&request_id)
    }

    /// Get cache statistics `(hits, misses, entries)`.
    ///
    /// The entries count is `0` if no cache is configured.
    pub fn cache_stats(&self) -> (u64, u64, usize) {
        let entries = self.cache.as_ref().map_or(0, |c| c.len());
        (self.cache_hits, self.cache_misses, entries)
    }

    /// Get cache hit rate as a percentage.
    pub fn cache_hit_rate(&self) -> f32 {
        if self.total_requests == 0 {
            return 0.0;
        }
        (self.cache_hits as f32 / self.total_requests as f32) * 100.0
    }

    /// Clear the cache, if any.
    pub fn clear_cache(&mut self) {
        if let Some(cache) = &self.cache {
            cache.clear();
        }
        self.cache_hits = 0;
        self.cache_misses = 0;
    }
}

impl Default for TextPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_shape(_text: &str, _font_size: f32, _wrap_width: Option<f32>) -> BaseShapedTextResult {
        BaseShapedTextResult::new((100.0, 20.0), Vec::new())
    }

    #[test]
    fn test_request_and_process() {
        let mut pipeline = TextPipeline::new();

        let req_id = pipeline.request_shape("Hello".to_string(), 0, 16.0, None);
        assert!(pipeline.is_pending(req_id));

        pipeline.process_pending(mock_shape);

        assert!(!pipeline.is_pending(req_id));
        let result = pipeline.take_completed(req_id);
        assert!(result.is_some());
        assert_eq!(result.unwrap().bounds(), (100.0, 20.0));
    }

    #[test]
    fn test_cache_hit() {
        let mut pipeline = TextPipeline::new();

        let req_id1 = pipeline.request_shape("Hello".to_string(), 0, 16.0, None);
        pipeline.process_pending(mock_shape);
        let _ = pipeline.take_completed(req_id1);

        assert_eq!(pipeline.cache_hits, 0);
        assert_eq!(pipeline.cache_misses, 1);

        let req_id2 = pipeline.request_shape("Hello".to_string(), 0, 16.0, None);
        assert_eq!(pipeline.cache_hits, 1);
        assert!(!pipeline.is_pending(req_id2));

        let result = pipeline.take_completed(req_id2);
        assert!(result.is_some());
    }

    #[test]
    fn test_content_invalidation() {
        let mut pipeline = TextPipeline::new();

        let req_id1 = pipeline.request_shape("Hello".to_string(), 0, 16.0, None);
        pipeline.process_pending(mock_shape);
        let _ = pipeline.take_completed(req_id1);

        let req_id2 = pipeline.request_shape("Hello World".to_string(), 0, 16.0, None);
        assert_eq!(pipeline.cache_misses, 2);
        assert!(pipeline.is_pending(req_id2));
    }

    #[test]
    fn test_width_bucketing() {
        let mut pipeline = TextPipeline::new();

        let req_id1 = pipeline.request_shape("Hello".to_string(), 0, 16.0, Some(402.0));
        pipeline.process_pending(mock_shape);
        let _ = pipeline.take_completed(req_id1);

        let _req_id2 = pipeline.request_shape("Hello".to_string(), 0, 16.0, Some(404.0));
        assert_eq!(pipeline.cache_hits, 1);
    }

    #[test]
    fn test_hit_rate_calculation() {
        let mut pipeline = TextPipeline::new();

        let req_id = pipeline.request_shape("A".to_string(), 0, 16.0, None);
        pipeline.process_pending(mock_shape);
        let _ = pipeline.take_completed(req_id);

        assert_eq!(pipeline.cache_hit_rate(), 0.0);

        let req_id2 = pipeline.request_shape("A".to_string(), 0, 16.0, None);
        let _ = pipeline.take_completed(req_id2);

        assert_eq!(pipeline.cache_hit_rate(), 50.0);
    }

    #[test]
    fn test_without_cache_always_pending() {
        let mut pipeline = TextPipeline::without_cache();

        let req_id1 = pipeline.request_shape("Hello".to_string(), 0, 16.0, None);
        pipeline.process_pending(mock_shape);
        let _ = pipeline.take_completed(req_id1);

        // Same text again — no cache, so it goes to pending
        let req_id2 = pipeline.request_shape("Hello".to_string(), 0, 16.0, None);
        assert!(pipeline.is_pending(req_id2));
        assert_eq!(pipeline.cache_hits, 0);
        assert_eq!(pipeline.cache_misses, 2);
    }

    #[test]
    fn test_set_cache_resets_stats() {
        let mut pipeline = TextPipeline::new();

        let req_id = pipeline.request_shape("A".to_string(), 0, 16.0, None);
        pipeline.process_pending(mock_shape);
        let _ = pipeline.take_completed(req_id);

        assert_eq!(pipeline.cache_misses, 1);

        pipeline.set_cache(Some(Box::new(HashMapShapeCache::new())));
        assert_eq!(pipeline.cache_hits, 0);
        assert_eq!(pipeline.cache_misses, 0);
    }

    #[test]
    fn test_cache_stats_no_cache() {
        let pipeline = TextPipeline::without_cache();
        let (hits, misses, entries) = pipeline.cache_stats();
        assert_eq!(hits, 0);
        assert_eq!(misses, 0);
        assert_eq!(entries, 0);
    }

    #[test]
    fn test_custom_cache() {
        use std::sync::Mutex;

        /// A mock cache that only stores the last entry.
        struct LastEntryCache {
            inner: Mutex<Option<(ShapeKey, Arc<BaseShapedTextResult>)>>,
        }

        impl LastEntryCache {
            fn new() -> Self {
                Self {
                    inner: Mutex::new(None),
                }
            }
        }

        impl ShapeCache for LastEntryCache {
            fn get(&self, key: &ShapeKey) -> Option<Arc<BaseShapedTextResult>> {
                let guard = self.inner.lock().unwrap();
                guard
                    .as_ref()
                    .filter(|(k, _)| k == key)
                    .map(|(_, v)| v.clone())
            }

            fn insert(&self, key: ShapeKey, value: Arc<BaseShapedTextResult>) {
                *self.inner.lock().unwrap() = Some((key, value));
            }

            fn clear(&self) {
                *self.inner.lock().unwrap() = None;
            }

            fn len(&self) -> usize {
                if self.inner.lock().unwrap().is_some() {
                    1
                } else {
                    0
                }
            }
        }

        let mut pipeline =
            TextPipeline::with_cache(Some(Box::new(LastEntryCache::new())));

        // First request — miss
        let req1 = pipeline.request_shape("A".to_string(), 0, 16.0, None);
        pipeline.process_pending(mock_shape);
        let _ = pipeline.take_completed(req1);

        // Same text — hit from custom cache
        let req2 = pipeline.request_shape("A".to_string(), 0, 16.0, None);
        assert_eq!(pipeline.cache_hits, 1);
        assert!(!pipeline.is_pending(req2));

        // Different text — miss, and evicts "A" from last-entry cache
        let req3 = pipeline.request_shape("B".to_string(), 0, 16.0, None);
        pipeline.process_pending(mock_shape);
        let _ = pipeline.take_completed(req3);

        // "A" again — miss because last-entry cache only holds "B"
        let req4 = pipeline.request_shape("A".to_string(), 0, 16.0, None);
        assert!(pipeline.is_pending(req4));
        assert_eq!(pipeline.cache_hits, 1);
        assert_eq!(pipeline.cache_misses, 3);
    }
}
