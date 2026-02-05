//! Dynamic buffer management with ring buffers and staging pools.
//!
//! Provides efficient GPU buffer allocation patterns for streaming data.
//!
//! # Ring Buffer
//!
//! A ring buffer allows continuous writing without stalling by cycling through
//! buffer regions. Perfect for per-frame uniform data.
//!
//! ```ignore
//! use astrelis_render::*;
//!
//! let mut ring = RingBuffer::new(&ctx, 1024 * 1024, wgpu::BufferUsages::UNIFORM);
//!
//! // Each frame
//! if let Some(allocation) = ring.allocate(256, 256) {
//!     allocation.write(&data);
//!     // Use allocation.buffer() and allocation.offset() for binding
//! }
//!
//! // At frame end
//! ring.next_frame();
//! ```
//!
//! # Staging Buffer Pool
//!
//! A pool of staging buffers for efficient CPU-to-GPU transfers.
//!
//! ```ignore
//! use astrelis_render::*;
//!
//! let mut pool = StagingBufferPool::new();
//!
//! // Allocate staging buffer
//! let staging = pool.allocate(&ctx, 4096);
//! staging.write(&data);
//! staging.copy_to_buffer(&mut encoder, &target_buffer, 0);
//!
//! // Return to pool when done
//! pool.recycle(staging);
//! ```

use astrelis_core::profiling::profile_function;

use crate::GraphicsContext;
use std::sync::Arc;

/// Number of frames to buffer (triple buffering).
const RING_BUFFER_FRAMES: usize = 3;

/// A region allocated from a ring buffer.
pub struct RingBufferAllocation {
    /// The underlying buffer
    buffer: Arc<wgpu::Buffer>,
    /// Offset into the buffer
    offset: u64,
    /// Size of the allocation
    size: u64,
}

impl RingBufferAllocation {
    /// Get the buffer.
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    /// Get the offset into the buffer.
    pub fn offset(&self) -> u64 {
        self.offset
    }

    /// Get the size of the allocation.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Write data to this allocation.
    ///
    /// # Panics
    ///
    /// Panics if data size exceeds allocation size.
    pub fn write(&self, queue: &wgpu::Queue, data: &[u8]) {
        assert!(
            data.len() as u64 <= self.size,
            "Data size {} exceeds allocation size {}",
            data.len(),
            self.size
        );
        queue.write_buffer(&self.buffer, self.offset, data);
    }

    /// Get a binding resource for this allocation.
    pub fn as_binding(&self) -> wgpu::BindingResource<'_> {
        wgpu::BindingResource::Buffer(wgpu::BufferBinding {
            buffer: &self.buffer,
            offset: self.offset,
            size: Some(std::num::NonZeroU64::new(self.size).unwrap()),
        })
    }
}

/// A ring buffer for streaming per-frame data.
///
/// Ring buffers cycle through multiple frames worth of buffer space to avoid
/// stalling the GPU pipeline.
pub struct RingBuffer {
    /// The underlying GPU buffer
    buffer: Arc<wgpu::Buffer>,
    /// Total size of the buffer
    size: u64,
    /// Current write offset
    offset: u64,
    /// Current frame number
    frame: u64,
}

impl RingBuffer {
    /// Create a new ring buffer.
    ///
    /// # Arguments
    ///
    /// * `context` - Graphics context
    /// * `size` - Total size in bytes (will be multiplied by RING_BUFFER_FRAMES)
    /// * `usage` - Buffer usage flags (UNIFORM, STORAGE, etc.)
    pub fn new(context: Arc<GraphicsContext>, size: u64, usage: wgpu::BufferUsages) -> Self {
        let total_size = size * RING_BUFFER_FRAMES as u64;

        let buffer = context.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("Ring Buffer"),
            size: total_size,
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            buffer: Arc::new(buffer),
            size: total_size,
            offset: 0,
            frame: 0,
        }
    }

    /// Allocate a region from the ring buffer.
    ///
    /// # Arguments
    ///
    /// * `size` - Size in bytes to allocate
    /// * `alignment` - Required alignment (typically 256 for uniforms)
    ///
    /// # Returns
    ///
    /// Returns `Some(allocation)` if space is available, `None` if the buffer is full.
    pub fn allocate(&mut self, size: u64, alignment: u64) -> Option<RingBufferAllocation> {
        profile_function!();
        // Align offset
        let aligned_offset = if !self.offset.is_multiple_of(alignment) {
            self.offset + (alignment - (self.offset % alignment))
        } else {
            self.offset
        };

        // Check if we have space in current frame
        let frame_size = self.size / RING_BUFFER_FRAMES as u64;
        let frame_start = (self.frame % RING_BUFFER_FRAMES as u64) * frame_size;
        let frame_end = frame_start + frame_size;

        if aligned_offset + size > frame_end {
            return None;
        }

        let allocation = RingBufferAllocation {
            buffer: self.buffer.clone(),
            offset: aligned_offset,
            size,
        };

        self.offset = aligned_offset + size;

        Some(allocation)
    }

    /// Advance to the next frame.
    ///
    /// Call this at the beginning or end of each frame to reset the ring buffer
    /// for the next frame's allocations.
    pub fn next_frame(&mut self) {
        self.frame += 1;
        let frame_size = self.size / RING_BUFFER_FRAMES as u64;
        self.offset = (self.frame % RING_BUFFER_FRAMES as u64) * frame_size;
    }

    /// Reset the ring buffer (useful for testing or manual control).
    pub fn reset(&mut self) {
        self.frame = 0;
        self.offset = 0;
    }

    /// Get the current frame number.
    pub fn frame(&self) -> u64 {
        self.frame
    }

    /// Get the current offset.
    pub fn offset(&self) -> u64 {
        self.offset
    }

    /// Get the total size.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Get remaining space in current frame.
    pub fn remaining(&self) -> u64 {
        let frame_size = self.size / RING_BUFFER_FRAMES as u64;
        let frame_end = ((self.frame % RING_BUFFER_FRAMES as u64) + 1) * frame_size;
        frame_end.saturating_sub(self.offset)
    }
}

/// A staging buffer for CPU-to-GPU transfers.
pub struct StagingBuffer {
    /// The GPU buffer
    buffer: wgpu::Buffer,
    /// Size of the buffer
    size: u64,
}

impl StagingBuffer {
    /// Create a new staging buffer.
    fn new(context: &GraphicsContext, size: u64) -> Self {
        let buffer = context.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size,
            usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        Self { buffer, size }
    }

    /// Get the buffer.
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    /// Get the size.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Write data to the staging buffer.
    pub fn write(&self, queue: &wgpu::Queue, data: &[u8]) {
        assert!(
            data.len() as u64 <= self.size,
            "Data size {} exceeds buffer size {}",
            data.len(),
            self.size
        );
        queue.write_buffer(&self.buffer, 0, data);
    }

    /// Copy this staging buffer to a destination buffer.
    pub fn copy_to_buffer(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        dst: &wgpu::Buffer,
        dst_offset: u64,
    ) {
        encoder.copy_buffer_to_buffer(&self.buffer, 0, dst, dst_offset, self.size);
    }

    /// Copy a region of this staging buffer to a destination buffer.
    pub fn copy_region_to_buffer(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        src_offset: u64,
        dst: &wgpu::Buffer,
        dst_offset: u64,
        size: u64,
    ) {
        encoder.copy_buffer_to_buffer(&self.buffer, src_offset, dst, dst_offset, size);
    }
}

/// A pool of staging buffers for reuse.
pub struct StagingBufferPool {
    /// Available buffers, grouped by size
    available: Vec<StagingBuffer>,
}

impl StagingBufferPool {
    /// Create a new staging buffer pool.
    pub fn new() -> Self {
        Self {
            available: Vec::new(),
        }
    }

    /// Allocate a staging buffer from the pool.
    ///
    /// If a suitable buffer is available, it will be reused. Otherwise, a new
    /// buffer will be created.
    pub fn allocate(&mut self, context: &GraphicsContext, size: u64) -> StagingBuffer {
        profile_function!();
        // Try to find a buffer of suitable size
        // We look for a buffer that's >= size but not too much bigger
        let mut best_idx = None;
        let mut best_size = u64::MAX;

        for (idx, buffer) in self.available.iter().enumerate() {
            if buffer.size >= size && buffer.size < best_size {
                best_idx = Some(idx);
                best_size = buffer.size;
            }
        }

        if let Some(idx) = best_idx {
            self.available.swap_remove(idx)
        } else {
            // No suitable buffer found, create a new one
            // Round up to next power of 2 for better reuse
            let rounded_size = size.next_power_of_two();
            StagingBuffer::new(context, rounded_size)
        }
    }

    /// Return a staging buffer to the pool for reuse.
    pub fn recycle(&mut self, buffer: StagingBuffer) {
        self.available.push(buffer);
    }

    /// Clear all buffers from the pool.
    pub fn clear(&mut self) {
        self.available.clear();
    }

    /// Get the number of available buffers.
    pub fn available_count(&self) -> usize {
        self.available.len()
    }

    /// Get the total size of available buffers.
    pub fn total_available_size(&self) -> u64 {
        self.available.iter().map(|b| b.size).sum()
    }
}

impl Default for StagingBufferPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_allocation() {
        let ctx = GraphicsContext::new_owned_sync().expect("Failed to create graphics context");
        let mut ring = RingBuffer::new(ctx, 1024, wgpu::BufferUsages::UNIFORM);

        // Allocate some space
        let alloc1 = ring.allocate(256, 256);
        assert!(alloc1.is_some());
        let alloc1 = alloc1.unwrap();
        assert_eq!(alloc1.offset, 0);
        assert_eq!(alloc1.size, 256);

        // Allocate more
        let alloc2 = ring.allocate(256, 256);
        assert!(alloc2.is_some());
        let alloc2 = alloc2.unwrap();
        assert_eq!(alloc2.offset, 256);
        assert_eq!(alloc2.size, 256);
    }

    #[test]
    fn test_ring_buffer_frame_advance() {
        let ctx = GraphicsContext::new_owned_sync().expect("Failed to create graphics context");
        let mut ring = RingBuffer::new(ctx, 1024, wgpu::BufferUsages::UNIFORM);

        // Fill first frame
        let alloc1 = ring.allocate(512, 256);
        assert!(alloc1.is_some());

        // Advance to next frame
        ring.next_frame();
        assert_eq!(ring.frame(), 1);

        // Should be able to allocate in new frame
        let alloc2 = ring.allocate(512, 256);
        assert!(alloc2.is_some());
        let alloc2 = alloc2.unwrap();
        assert_eq!(alloc2.offset, 1024); // Second frame starts at 1024
    }

    #[test]
    fn test_staging_pool() {
        let ctx = GraphicsContext::new_owned_sync().expect("Failed to create graphics context");
        let mut pool = StagingBufferPool::new();

        // Allocate a buffer
        let buffer1 = pool.allocate(&ctx, 1024);
        assert_eq!(buffer1.size(), 1024);
        assert_eq!(pool.available_count(), 0);

        // Return it to pool
        pool.recycle(buffer1);
        assert_eq!(pool.available_count(), 1);

        // Allocate again - should reuse
        let buffer2 = pool.allocate(&ctx, 1024);
        assert_eq!(buffer2.size(), 1024);
        assert_eq!(pool.available_count(), 0);

        pool.recycle(buffer2);
    }

    #[test]
    fn test_staging_pool_size_matching() {
        let ctx = GraphicsContext::new_owned_sync().expect("Failed to create graphics context");
        let mut pool = StagingBufferPool::new();

        // Add buffers of different sizes
        pool.recycle(StagingBuffer::new(&ctx, 512));
        pool.recycle(StagingBuffer::new(&ctx, 1024));
        pool.recycle(StagingBuffer::new(&ctx, 2048));

        // Request 600 bytes - should get the 1024 buffer (smallest that fits)
        let buffer = pool.allocate(&ctx, 600);
        assert_eq!(buffer.size(), 1024);
        assert_eq!(pool.available_count(), 2);
    }
}
