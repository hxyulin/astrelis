//! Text shaping pipeline for deferred/async text processing.
//!
//! This module implements a two-tier text pipeline.
//! It provides a worker-ready abstraction for text shaping that can be executed
//! synchronously now and moved to worker threads later without API changes.

use crate::cache::ShapeKey;
use crate::ShapedTextResult as BaseShapedTextResult;
use astrelis_core::alloc::HashMap;

use std::sync::Arc;

/// Unique identifier for a text shaping request.
pub type RequestId = u64;

/// Request for text shaping with all necessary parameters.
///
/// Uses owned data (String) instead of references to enable Send+Sync
/// for future worker thread compatibility.
#[derive(Debug, Clone)]
pub struct TextShapeRequest {
    /// Unique ID for this request
    pub id: RequestId,
    /// Text content to shape (owned for Send)
    pub text: String,
    /// Font identifier from font system
    pub font_id: u32,
    /// Font size in pixels
    pub font_size: f32,
    /// Optional wrap width for text layout
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
        ShapeKey::new(
            self.font_id,
            self.font_size,
            self.text.as_str(),
            self.wrap_width,
        )
    }
}

/// Result of text shaping with metadata for pipeline management.
///
/// Wraps astrelis_text::ShapedTextResult with request tracking and cache stats.
#[derive(Debug, Clone)]
pub struct ShapedTextResult {
    /// Original request ID
    pub request_id: RequestId,
    /// Inner shaped text data from astrelis-text
    pub inner: BaseShapedTextResult,
    /// Text version this was shaped for
    /// Number of times this shaped result has been rendered
    pub render_count: u64,
}

impl ShapedTextResult {
    /// Create a new shaped text result.
    pub fn new(request_id: RequestId, inner: BaseShapedTextResult) -> Self {
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

    /// Increment render count for cache statistics.
    pub fn increment_render_count(&mut self) {
        self.render_count = self.render_count.saturating_add(1);
    }
}

/// Trait for text shaping implementations.
///
/// This abstraction allows swapping between sync and async implementations
/// without changing the API. Currently synchronous, but designed for future
/// worker thread execution.
pub trait TextShaper: Send + Sync {
    /// Shape text according to the request parameters.
    fn shape(&mut self, request: TextShapeRequest) -> ShapedTextResult;
}

/// Synchronous text shaper using a callback for measurement.
///
/// This is the initial implementation that performs shaping on the calling thread.
/// Since FontRenderer isn't Send+Sync, we don't implement TextShaper trait here.
pub struct SyncTextShaper;

impl Default for SyncTextShaper {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncTextShaper {
    /// Create a new synchronous text shaper.
    pub fn new() -> Self {
        Self { }
    }

    /// Shape text using the provided shaping function.
    ///
    /// The shaping function should call astrelis_text::shape_text and return
    /// the BaseShapedTextResult with actual glyph data.
    pub fn shape_with_measurer<F>(request: &TextShapeRequest, shape_fn: F) -> ShapedTextResult
    where
        F: FnOnce(&str, f32, Option<f32>) -> BaseShapedTextResult,
    {
        let inner = shape_fn(&request.text, request.font_size, request.wrap_width);

        ShapedTextResult::new(request.id, inner)
    }
}

/// Text shaping pipeline managing requests and results.
///
/// Coordinates text shaping operations with caching and request management.
/// Currently processes synchronously but designed for async execution.
pub struct TextPipeline {
    /// Pending requests waiting to be processed
    pending: HashMap<RequestId, TextShapeRequest>,
    /// Completed results ready for pickup
    completed: HashMap<RequestId, Arc<ShapedTextResult>>,
    /// Next request ID to allocate
    next_request_id: RequestId,
    /// Cache of shaped results by shape key
    cache: HashMap<ShapeKey, Arc<ShapedTextResult>>,
    /// Statistics
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub total_requests: u64,
}

impl TextPipeline {
    /// Create a new text pipeline.
    pub fn new() -> Self {
        Self {
            pending: HashMap::with_capacity(64),
            completed: HashMap::with_capacity(64),
            next_request_id: 1,
            cache: HashMap::with_capacity(256),
            cache_hits: 0,
            cache_misses: 0,
            total_requests: 0,
        }
    }

    /// Request text shaping, returns request ID.
    ///
    /// If the text is already cached with matching parameters, the cached result
    /// is immediately available. Otherwise, it's queued for processing.
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

        // Check cache first
        if let Some(cached) = self.cache.get(&shape_key).cloned() {
            self.cache_hits += 1;
            self.completed.insert(request_id, cached);
        } else {
            self.cache_misses += 1;
            self.pending.insert(request_id, request);
        }

        request_id
    }

    /// Process all pending shape requests using the provided shaping function.
    ///
    /// The shaping function should perform actual text shaping via astrelis_text::shape_text.
    /// Currently synchronous, but the API allows future async implementations
    /// where this would dispatch to workers and poll for completion.
    pub fn process_pending<F>(&mut self, shape_fn: F)
    where
        F: Fn(&str, f32, Option<f32>) -> BaseShapedTextResult,
    {
        if self.pending.is_empty() {
            return;
        }

        let mut completed_requests = Vec::new();

        for (_request_id, request) in self.pending.drain() {
            let result = SyncTextShaper::shape_with_measurer(&request, &shape_fn);
            let result_arc = Arc::new(result);

            // Cache by shape key
            let shape_key = request.shape_key();
            self.cache.insert(shape_key, result_arc.clone());

            completed_requests.push((request.id, result_arc));
        }

        for (request_id, result) in completed_requests {
            self.completed.insert(request_id, result);
        }
    }

    /// Take a completed result by request ID.
    ///
    /// Returns None if the request hasn't completed yet or doesn't exist.
    pub fn take_completed(&mut self, request_id: RequestId) -> Option<Arc<ShapedTextResult>> {
        self.completed.remove(&request_id)
    }

    /// Get a completed result by request ID without removing it.
    pub fn get_completed(&self, request_id: RequestId) -> Option<Arc<ShapedTextResult>> {
        self.completed.get(&request_id).cloned()
    }

    /// Check if a request is still pending.
    pub fn is_pending(&self, request_id: RequestId) -> bool {
        self.pending.contains_key(&request_id)
    }

    /// Get cache statistics.
    pub fn cache_stats(&self) -> (u64, u64, usize) {
        (self.cache_hits, self.cache_misses, self.cache.len())
    }

    /// Get cache hit rate as a percentage.
    pub fn cache_hit_rate(&self) -> f32 {
        if self.total_requests == 0 {
            return 0.0;
        }
        (self.cache_hits as f32 / self.total_requests as f32) * 100.0
    }

    /// Clear the cache.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
        self.cache_hits = 0;
        self.cache_misses = 0;
    }

    /// Prune cache entries that haven't been used recently.
    ///
    /// Removes entries with low render counts to keep memory usage bounded.
    pub fn prune_cache(&mut self, min_render_count: u64) {
        self.cache.retain(|_, result| {
            // Keep if render count is high enough or if there are multiple references
            Arc::strong_count(result) > 1 || result.render_count >= min_render_count
        });
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

    // Mock shaping function for testing
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

        // First request - cache miss
        let req_id1 = pipeline.request_shape("Hello".to_string(), 0, 16.0, None);
        pipeline.process_pending(mock_shape);
        let _ = pipeline.take_completed(req_id1);

        assert_eq!(pipeline.cache_hits, 0);
        assert_eq!(pipeline.cache_misses, 1);

        // Second request with same parameters - cache hit
        let req_id2 = pipeline.request_shape("Hello".to_string(), 0, 16.0, None);

        assert_eq!(pipeline.cache_hits, 1);
        assert_eq!(pipeline.cache_misses, 1);
        assert!(!pipeline.is_pending(req_id2));

        let result = pipeline.take_completed(req_id2);
        assert!(result.is_some());
    }

    #[test]
    fn test_content_invalidation() {
        let mut pipeline = TextPipeline::new();

        // Shape "Hello"
        let req_id1 = pipeline.request_shape("Hello".to_string(), 0, 16.0, None);
        pipeline.process_pending(mock_shape);
        let _ = pipeline.take_completed(req_id1);

        assert_eq!(pipeline.cache_misses, 1);

        // Shape "Hello World" - should be cache miss (different content)
        let req_id2 = pipeline.request_shape("Hello World".to_string(), 0, 16.0, None);

        assert_eq!(pipeline.cache_misses, 2);
        assert!(pipeline.is_pending(req_id2));
    }

    #[test]
    fn test_width_bucketing() {
        let mut pipeline = TextPipeline::new();

        // Shape at width 402
        let req_id1 = pipeline.request_shape("Hello".to_string(), 0, 16.0, Some(402.0));
        pipeline.process_pending(mock_shape);
        let _ = pipeline.take_completed(req_id1);

        // Shape at width 404 - should hit cache due to bucketing
        let _req_id2 = pipeline.request_shape("Hello".to_string(), 0, 16.0, Some(404.0));

        assert_eq!(
            pipeline.cache_hits, 1,
            "Width bucketing should allow cache hit"
        );
    }

    #[test]
    fn test_cache_prune() {
        let mut pipeline = TextPipeline::new();

        // Add multiple entries
        for i in 0..5 {
            let req_id = pipeline.request_shape(format!("Text {}", i), 0, 16.0, None);
            pipeline.process_pending(mock_shape);
            let _ = pipeline.take_completed(req_id);
        }

        assert_eq!(pipeline.cache.len(), 5);

        // Prune entries with low render count
        pipeline.prune_cache(10);

        assert_eq!(pipeline.cache.len(), 0, "All entries should be pruned");
    }

    #[test]
    fn test_hit_rate_calculation() {
        let mut pipeline = TextPipeline::new();

        let req_id = pipeline.request_shape("A".to_string(), 0, 16.0, None);
        pipeline.process_pending(mock_shape);
        let _ = pipeline.take_completed(req_id);

        // One miss
        assert_eq!(pipeline.cache_hit_rate(), 0.0);

        // One hit
        let req_id2 = pipeline.request_shape("A".to_string(), 0, 16.0, None);
        let _ = pipeline.take_completed(req_id2);

        assert_eq!(pipeline.cache_hit_rate(), 50.0);
    }
}
