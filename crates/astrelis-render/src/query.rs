//! GPU query and profiling support.
//!
//! This module provides wrappers for GPU queries (timestamps, occlusion) and
//! a high-level profiler for measuring GPU execution times.
//!
//! # Features Required
//!
//! - `TIMESTAMP_QUERY` - Required for timestamp queries and GPU profiling
//!
//! # Example
//!
//! ```ignore
//! use astrelis_render::{GpuProfiler, GraphicsContext, GraphicsContextExt};
//!
//! // Create profiler (requires TIMESTAMP_QUERY feature)
//! let mut profiler = GpuProfiler::new(context.clone(), 256);
//!
//! // In render loop:
//! profiler.begin_frame();
//!
//! {
//!     let region = profiler.begin_region(&mut encoder, "Shadow Pass");
//!     // ... render shadow pass ...
//!     profiler.end_region(&mut encoder, region);
//! }
//!
//! profiler.resolve(&mut encoder);
//!
//! // Later, read results
//! for (label, duration_ms) in profiler.read_results() {
//!     println!("{}: {:.2}ms", label, duration_ms);
//! }
//! ```

use std::sync::Arc;

use crate::context::GraphicsContext;
use crate::extension::GraphicsContextExt;

// =============================================================================
// Query Types
// =============================================================================

/// Types of GPU queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueryType {
    /// Timestamp query for measuring GPU execution time.
    /// Requires `TIMESTAMP_QUERY` feature.
    Timestamp,
    /// Occlusion query for counting visible fragments.
    Occlusion,
}

impl QueryType {
    /// Convert to wgpu query type.
    pub fn to_wgpu(self) -> wgpu::QueryType {
        match self {
            QueryType::Timestamp => wgpu::QueryType::Timestamp,
            QueryType::Occlusion => wgpu::QueryType::Occlusion,
        }
    }
}

// =============================================================================
// QuerySet
// =============================================================================

/// A wrapper around wgpu::QuerySet with metadata.
pub struct QuerySet {
    query_set: wgpu::QuerySet,
    query_type: QueryType,
    count: u32,
}

impl QuerySet {
    /// Create a new query set.
    ///
    /// # Arguments
    ///
    /// * `device` - The wgpu device
    /// * `label` - Optional debug label
    /// * `query_type` - Type of queries in this set
    /// * `count` - Number of queries in the set
    pub fn new(
        device: &wgpu::Device,
        label: Option<&str>,
        query_type: QueryType,
        count: u32,
    ) -> Self {
        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label,
            ty: query_type.to_wgpu(),
            count,
        });

        Self {
            query_set,
            query_type,
            count,
        }
    }

    /// Get the underlying wgpu query set.
    #[inline]
    pub fn query_set(&self) -> &wgpu::QuerySet {
        &self.query_set
    }

    /// Get the query type.
    #[inline]
    pub fn query_type(&self) -> QueryType {
        self.query_type
    }

    /// Get the number of queries in the set.
    #[inline]
    pub fn count(&self) -> u32 {
        self.count
    }
}

// =============================================================================
// QueryResultBuffer
// =============================================================================

/// Buffer for storing and reading query results.
pub struct QueryResultBuffer {
    resolve_buffer: wgpu::Buffer,
    read_buffer: wgpu::Buffer,
    count: u32,
}

impl QueryResultBuffer {
    /// Create a new query result buffer.
    ///
    /// # Arguments
    ///
    /// * `device` - The wgpu device
    /// * `label` - Optional debug label
    /// * `count` - Number of query results to store
    pub fn new(device: &wgpu::Device, label: Option<&str>, count: u32) -> Self {
        let size = (count as u64) * std::mem::size_of::<u64>() as u64;

        let resolve_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: label.map(|l| format!("{} Resolve", l)).as_deref(),
            size,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let read_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: label.map(|l| format!("{} Read", l)).as_deref(),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Self {
            resolve_buffer,
            read_buffer,
            count,
        }
    }

    /// Get the resolve buffer (used for query resolution).
    #[inline]
    pub fn resolve_buffer(&self) -> &wgpu::Buffer {
        &self.resolve_buffer
    }

    /// Get the read buffer (used for CPU readback).
    #[inline]
    pub fn read_buffer(&self) -> &wgpu::Buffer {
        &self.read_buffer
    }

    /// Get the number of results this buffer can hold.
    #[inline]
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Resolve queries from a query set into this buffer.
    pub fn resolve(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        query_set: &QuerySet,
        query_range: std::ops::Range<u32>,
        destination_offset: u32,
    ) {
        encoder.resolve_query_set(
            query_set.query_set(),
            query_range,
            &self.resolve_buffer,
            (destination_offset as u64) * std::mem::size_of::<u64>() as u64,
        );
    }

    /// Copy resolved results to the readable buffer.
    pub fn copy_to_readable(&self, encoder: &mut wgpu::CommandEncoder) {
        let size = (self.count as u64) * std::mem::size_of::<u64>() as u64;
        encoder.copy_buffer_to_buffer(&self.resolve_buffer, 0, &self.read_buffer, 0, size);
    }

    /// Map the read buffer for CPU access.
    ///
    /// Returns a future that completes when the buffer is mapped.
    pub fn map_async(&self) -> impl std::future::Future<Output = Result<(), wgpu::BufferAsyncError>> {
        let slice = self.read_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();

        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });

        async move { rx.recv().map_err(|_| wgpu::BufferAsyncError)? }
    }

    /// Read the query results (must be mapped first).
    ///
    /// Returns the raw u64 timestamps/occlusion counts.
    pub fn read_results(&self) -> Vec<u64> {
        let slice = self.read_buffer.slice(..);
        let data = slice.get_mapped_range();
        let results: Vec<u64> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        self.read_buffer.unmap();
        results
    }
}

// =============================================================================
// ProfileRegion
// =============================================================================

/// A handle to a profiling region.
///
/// Created by `GpuProfiler::begin_region` and consumed by `GpuProfiler::end_region`.
#[derive(Debug)]
pub struct ProfileRegion {
    label: String,
    start_query: u32,
}

// =============================================================================
// GpuProfiler
// =============================================================================

/// High-level GPU profiler for measuring execution times.
///
/// This profiler uses timestamp queries to measure GPU execution time
/// for different regions of your rendering code.
///
/// # Requirements
///
/// - Device must support `TIMESTAMP_QUERY` feature
/// - Must call `begin_frame()` at the start of each frame
/// - Must call `resolve()` before submitting commands
///
/// # Example
///
/// ```ignore
/// let mut profiler = GpuProfiler::new(context.clone(), 256);
///
/// // Each frame:
/// profiler.begin_frame();
///
/// let region = profiler.begin_region(&mut encoder, "My Pass");
/// // ... do rendering ...
/// profiler.end_region(&mut encoder, region);
///
/// profiler.resolve(&mut encoder);
///
/// // Read results (may be from previous frame)
/// for (label, duration_ms) in profiler.read_results() {
///     println!("{}: {:.2}ms", label, duration_ms);
/// }
/// ```
pub struct GpuProfiler {
    context: Arc<GraphicsContext>,
    query_set: QuerySet,
    result_buffer: QueryResultBuffer,
    /// Current query index for the frame
    current_query: u32,
    /// Maximum queries per frame
    max_queries: u32,
    /// Regions from the current frame (label, start_query, end_query)
    regions: Vec<(String, u32, u32)>,
    /// Cached results from the previous frame
    cached_results: Vec<(String, f64)>,
    /// Timestamp period in nanoseconds per tick
    timestamp_period: f32,
}

impl GpuProfiler {
    /// Create a new GPU profiler.
    ///
    /// # Arguments
    ///
    /// * `context` - Graphics context (must support TIMESTAMP_QUERY)
    /// * `max_queries` - Maximum number of timestamp queries per frame
    ///
    /// # Panics
    ///
    /// Panics if the device doesn't support timestamp queries.
    pub fn new(context: Arc<GraphicsContext>, max_queries: u32) -> Self {
        let timestamp_period = context.queue().get_timestamp_period();

        let query_set = QuerySet::new(
            context.device(),
            Some("GPU Profiler Queries"),
            QueryType::Timestamp,
            max_queries,
        );

        let result_buffer = QueryResultBuffer::new(
            context.device(),
            Some("GPU Profiler Results"),
            max_queries,
        );

        Self {
            context,
            query_set,
            result_buffer,
            current_query: 0,
            max_queries,
            regions: Vec::new(),
            cached_results: Vec::new(),
            timestamp_period,
        }
    }

    /// Begin a new frame.
    ///
    /// Call this at the start of each frame before recording any regions.
    pub fn begin_frame(&mut self) {
        self.current_query = 0;
        self.regions.clear();
    }

    /// Begin a profiling region.
    ///
    /// # Arguments
    ///
    /// * `encoder` - Command encoder to write the timestamp
    /// * `label` - Human-readable label for this region
    ///
    /// # Returns
    ///
    /// A `ProfileRegion` handle that must be passed to `end_region`.
    pub fn begin_region(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        label: &str,
    ) -> Option<ProfileRegion> {
        if self.current_query >= self.max_queries {
            return None;
        }

        let start_query = self.current_query;
        encoder.write_timestamp(&self.query_set.query_set, start_query);
        self.current_query += 1;

        Some(ProfileRegion {
            label: label.to_string(),
            start_query,
        })
    }

    /// End a profiling region.
    ///
    /// # Arguments
    ///
    /// * `encoder` - Command encoder to write the timestamp
    /// * `region` - The region handle from `begin_region`
    pub fn end_region(&mut self, encoder: &mut wgpu::CommandEncoder, region: ProfileRegion) {
        if self.current_query >= self.max_queries {
            return;
        }

        let end_query = self.current_query;
        encoder.write_timestamp(&self.query_set.query_set, end_query);
        self.current_query += 1;

        self.regions
            .push((region.label, region.start_query, end_query));
    }

    /// Resolve all queries from this frame.
    ///
    /// Call this after all regions have been recorded, before submitting commands.
    pub fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        if self.current_query == 0 {
            return;
        }

        self.result_buffer.resolve(
            encoder,
            &self.query_set,
            0..self.current_query,
            0,
        );
        self.result_buffer.copy_to_readable(encoder);
    }

    /// Read profiling results synchronously.
    ///
    /// This blocks until the results are available from the GPU.
    /// For non-blocking reads, consider using double-buffering or
    /// reading results from the previous frame.
    ///
    /// # Returns
    ///
    /// A vector of (label, duration_ms) pairs for each completed region.
    pub fn read_results(&mut self) -> &[(String, f64)] {
        if self.regions.is_empty() {
            return &self.cached_results;
        }

        let device = self.context.device();

        // Map the buffer
        let slice = self.result_buffer.read_buffer().slice(..);
        let (tx, rx) = std::sync::mpsc::channel();

        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });

        // Wait for the buffer to be mapped (blocking)
        let _ = device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });

        // Wait for the callback
        if rx.recv().is_ok() {
            let data = slice.get_mapped_range();
            let timestamps: &[u64] = bytemuck::cast_slice(&data);

            self.cached_results.clear();

            for (label, start, end) in &self.regions {
                let start_ts = timestamps.get(*start as usize).copied().unwrap_or(0);
                let end_ts = timestamps.get(*end as usize).copied().unwrap_or(0);

                // Convert ticks to milliseconds
                let duration_ns = (end_ts.saturating_sub(start_ts)) as f64
                    * self.timestamp_period as f64;
                let duration_ms = duration_ns / 1_000_000.0;

                self.cached_results.push((label.clone(), duration_ms));
            }

            drop(data);
            self.result_buffer.read_buffer().unmap();
        }

        &self.cached_results
    }

    /// Try to read profiling results without blocking.
    ///
    /// Returns None if the results are not yet available.
    /// This is useful when you want to display results from the previous frame.
    ///
    /// # Returns
    ///
    /// Some reference to the cached results if new data was read, or the existing cached results.
    pub fn try_read_results(&mut self) -> &[(String, f64)] {
        if self.regions.is_empty() {
            return &self.cached_results;
        }

        let device = self.context.device();

        // Try to map the buffer
        let slice = self.result_buffer.read_buffer().slice(..);
        let (tx, rx) = std::sync::mpsc::channel();

        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });

        // Non-blocking poll
        let _ = device.poll(wgpu::PollType::Poll);

        // Check if mapping succeeded
        if let Ok(Ok(())) = rx.try_recv() {
            let data = slice.get_mapped_range();
            let timestamps: &[u64] = bytemuck::cast_slice(&data);

            self.cached_results.clear();

            for (label, start, end) in &self.regions {
                let start_ts = timestamps.get(*start as usize).copied().unwrap_or(0);
                let end_ts = timestamps.get(*end as usize).copied().unwrap_or(0);

                // Convert ticks to milliseconds
                let duration_ns = (end_ts.saturating_sub(start_ts)) as f64
                    * self.timestamp_period as f64;
                let duration_ms = duration_ns / 1_000_000.0;

                self.cached_results.push((label.clone(), duration_ms));
            }

            drop(data);
            self.result_buffer.read_buffer().unmap();
        }

        &self.cached_results
    }

    /// Get the number of queries used this frame.
    #[inline]
    pub fn queries_used(&self) -> u32 {
        self.current_query
    }

    /// Get the maximum queries per frame.
    #[inline]
    pub fn max_queries(&self) -> u32 {
        self.max_queries
    }

    /// Get the timestamp period in nanoseconds per tick.
    #[inline]
    pub fn timestamp_period(&self) -> f32 {
        self.timestamp_period
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_type_conversion() {
        // Just verify conversion doesn't panic
        let _ = QueryType::Timestamp.to_wgpu();
        let _ = QueryType::Occlusion.to_wgpu();
    }

    #[test]
    fn test_profile_region_debug() {
        let region = ProfileRegion {
            label: "Test".to_string(),
            start_query: 0,
        };
        // Just ensure Debug is implemented
        let _ = format!("{:?}", region);
    }
}
