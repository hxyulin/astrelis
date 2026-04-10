//! GPU timestamp query pool management.
//!
//! Each [`QueryPool`] owns a wgpu `QuerySet` and associated buffers for
//! resolving and reading back timestamp results. Pools are fixed-capacity
//! and allocated on demand.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Number of timestamp slots per query pool.
const QUERIES_PER_POOL: u32 = 256;

/// A single query pool with its associated resolve and readback buffers.
pub(crate) struct QueryPool {
    query_set: wgpu::QuerySet,
    /// GPU-only buffer for `resolve_query_set` output.
    resolve_buffer: wgpu::Buffer,
    /// CPU-readable buffer for mapping results back.
    readback_buffer: wgpu::Buffer,
    /// Number of timestamp slots used in this pool.
    used: u32,
    /// Total capacity (number of timestamp slots).
    capacity: u32,
    /// Set to `true` by the `map_async` callback when the readback buffer is mapped.
    mapping_ready: Arc<AtomicBool>,
}

impl QueryPool {
    /// Creates a new query pool with the default capacity.
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        Self::with_capacity(device, QUERIES_PER_POOL)
    }

    /// Creates a new query pool with the specified capacity.
    fn with_capacity(device: &wgpu::Device, capacity: u32) -> Self {
        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("gpu_profiler_query_set"),
            ty: wgpu::QueryType::Timestamp,
            count: capacity,
        });

        let buffer_size = (capacity as u64) * std::mem::size_of::<u64>() as u64;

        let resolve_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_profiler_resolve"),
            size: buffer_size,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_profiler_readback"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Self {
            query_set,
            resolve_buffer,
            readback_buffer,
            used: 0,
            capacity,
            mapping_ready: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Allocates a pair of timestamp query indices (start, end).
    ///
    /// Returns `None` if the pool is full.
    pub(crate) fn allocate_pair(&mut self) -> Option<TimestampPair> {
        if self.used + 2 > self.capacity {
            return None;
        }
        let start = self.used;
        self.used += 2;
        Some(TimestampPair {
            start_index: start,
            end_index: start + 1,
        })
    }

    /// Returns the wgpu `QuerySet` for writing timestamps.
    pub(crate) fn query_set(&self) -> &wgpu::QuerySet {
        &self.query_set
    }

    /// Whether this pool has any used queries.
    pub(crate) fn is_empty(&self) -> bool {
        self.used == 0
    }

    /// Resets the pool for reuse, setting the used count back to zero.
    pub(crate) fn reset(&mut self) {
        self.used = 0;
        self.mapping_ready.store(false, Ordering::Release);
    }

    /// Adds resolve and copy commands to the encoder.
    ///
    /// Call this after all timestamps have been written and before submitting
    /// the command buffer. This resolves the query results into the resolve
    /// buffer, then copies to the readback buffer for CPU access.
    pub(crate) fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        if self.used == 0 {
            return;
        }
        encoder.resolve_query_set(&self.query_set, 0..self.used, &self.resolve_buffer, 0);
        encoder.copy_buffer_to_buffer(
            &self.resolve_buffer,
            0,
            &self.readback_buffer,
            0,
            (self.used as u64) * std::mem::size_of::<u64>() as u64,
        );
    }

    /// Initiates an async map of the readback buffer.
    ///
    /// Call after the command buffer containing the resolve has been submitted
    /// and the GPU has had time to execute it. The `mapping_ready` flag will
    /// be set to `true` when the mapping completes.
    pub(crate) fn map_readback(&self) {
        if self.used == 0 {
            return;
        }
        let size = (self.used as u64) * std::mem::size_of::<u64>() as u64;
        let ready = Arc::clone(&self.mapping_ready);
        self.readback_buffer
            .slice(..size)
            .map_async(wgpu::MapMode::Read, move |result| {
                if result.is_ok() {
                    ready.store(true, Ordering::Release);
                }
            });
    }

    /// Returns `true` if the readback buffer mapping has completed.
    pub(crate) fn is_mapping_ready(&self) -> bool {
        self.mapping_ready.load(Ordering::Acquire)
    }

    /// Reads the mapped readback buffer and returns the raw timestamps.
    ///
    /// Caller must ensure `is_mapping_ready()` returned `true` first.
    /// After this call, the buffer is unmapped.
    pub(crate) fn read_and_unmap(&self) -> Vec<u64> {
        if self.used == 0 {
            return Vec::new();
        }
        let size = (self.used as u64) * std::mem::size_of::<u64>() as u64;
        let slice = self.readback_buffer.slice(..size);
        let data = slice.get_mapped_range();
        let timestamps: Vec<u64> = data
            .chunks_exact(std::mem::size_of::<u64>())
            .map(|chunk| u64::from_ne_bytes(chunk.try_into().unwrap()))
            .collect();
        drop(data);
        self.readback_buffer.unmap();
        timestamps
    }
}

/// A pair of timestamp query indices representing a start and end measurement.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TimestampPair {
    /// Index of the start timestamp in the query set.
    pub(crate) start_index: u32,
    /// Index of the end timestamp in the query set.
    pub(crate) end_index: u32,
}
