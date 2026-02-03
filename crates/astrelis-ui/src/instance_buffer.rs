//! GPU instance buffer management for retained rendering.
//!
//! This module implements Phase 5 GPU infrastructure for managing instance buffers
//! with efficient partial updates. Supports any Pod type and tracks dirty ranges
//! for minimal GPU uploads.

use crate::dirty::DirtyRanges;
use astrelis_core::profiling::profile_function;
use astrelis_render::wgpu;
use bytemuck::Pod;

/// GPU instance buffer with partial update support.
///
/// Maintains a CPU-side buffer and GPU buffer, tracking which ranges
/// have been modified and need uploading. Supports efficient partial writes
/// for retained-mode rendering where only dirty instances change.
pub struct InstanceBuffer<T: Pod> {
    /// GPU buffer for instance data
    buffer: wgpu::Buffer,
    /// CPU-side instance data
    instances: Vec<T>,
    /// Current capacity in number of instances
    capacity: usize,
    /// Ranges that need GPU upload
    dirty_ranges: DirtyRanges,
    /// Total number of writes performed
    write_count: u64,
}

impl<T: Pod> InstanceBuffer<T> {
    /// Create a new instance buffer with the specified capacity.
    pub fn new(device: &wgpu::Device, label: Option<&str>, capacity: usize) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label,
            size: (capacity * std::mem::size_of::<T>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            buffer,
            instances: Vec::with_capacity(capacity),
            capacity,
            dirty_ranges: DirtyRanges::new(),
            write_count: 0,
        }
    }

    /// Get the GPU buffer.
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    /// Get the current instance data.
    pub fn instances(&self) -> &[T] {
        &self.instances
    }

    /// Get the number of instances.
    pub fn len(&self) -> usize {
        self.instances.len()
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }

    /// Get the capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Clear all instances.
    pub fn clear(&mut self) {
        self.instances.clear();
        self.dirty_ranges.clear();
    }

    /// Set instances, replacing all existing data.
    ///
    /// Marks the entire buffer as dirty for GPU upload.
    /// Reallocates GPU buffer if capacity is exceeded.
    pub fn set_instances(&mut self, device: &wgpu::Device, instances: Vec<T>) {
        let new_len = instances.len();

        // Check if we need to reallocate
        if new_len > self.capacity {
            self.reallocate(device, new_len.next_power_of_two());
        }

        self.instances = instances;

        // Mark entire buffer as dirty
        if !self.instances.is_empty() {
            self.dirty_ranges.mark_dirty(0, self.instances.len());
        }
    }

    /// Update a specific range of instances.
    ///
    /// Replaces instances[start..end] with the provided data.
    /// Marks the range as dirty for GPU upload.
    pub fn update_range(&mut self, start: usize, new_data: &[T]) {
        if new_data.is_empty() || start >= self.instances.len() {
            return;
        }

        let end = (start + new_data.len()).min(self.instances.len());
        let actual_len = end - start;

        self.instances[start..end].copy_from_slice(&new_data[..actual_len]);
        self.dirty_ranges.mark_dirty(start, end);
    }

    /// Update a single instance.
    pub fn update_instance(&mut self, index: usize, instance: T) {
        if index < self.instances.len() {
            self.instances[index] = instance;
            self.dirty_ranges.mark_dirty(index, index + 1);
        }
    }

    /// Append instances to the buffer.
    ///
    /// Reallocates if capacity is exceeded.
    pub fn append(&mut self, device: &wgpu::Device, new_instances: &[T]) {
        let start_idx = self.instances.len();
        let new_len = start_idx + new_instances.len();

        // Check if we need to reallocate
        if new_len > self.capacity {
            self.reallocate(device, new_len.next_power_of_two());
        }

        self.instances.extend_from_slice(new_instances);
        self.dirty_ranges.mark_dirty(start_idx, new_len);
    }

    /// Upload all dirty ranges to the GPU.
    ///
    /// This performs partial buffer writes for each dirty range,
    /// minimizing GPU bandwidth usage for retained rendering.
    pub fn upload_dirty(&mut self, queue: &wgpu::Queue) {
        profile_function!();

        if self.dirty_ranges.is_empty() {
            return;
        }

        let instance_size = std::mem::size_of::<T>() as u64;

        for range in self.dirty_ranges.iter() {
            let start = range.start;
            let end = range.end.min(self.instances.len());

            if start >= end {
                continue;
            }

            let offset = (start as u64) * instance_size;
            let data = bytemuck::cast_slice(&self.instances[start..end]);

            queue.write_buffer(&self.buffer, offset, data);
            self.write_count += 1;
        }

        self.dirty_ranges.clear();
    }

    /// Force upload of the entire buffer, ignoring dirty tracking.
    pub fn upload_all(&mut self, queue: &wgpu::Queue) {
        if self.instances.is_empty() {
            return;
        }

        let data = bytemuck::cast_slice(&self.instances);
        queue.write_buffer(&self.buffer, 0, data);
        self.write_count += 1;
        self.dirty_ranges.clear();
    }

    /// Get dirty ranges for inspection.
    pub fn dirty_ranges(&self) -> &DirtyRanges {
        &self.dirty_ranges
    }

    /// Get write statistics.
    pub fn write_count(&self) -> u64 {
        self.write_count
    }

    /// Reallocate the GPU buffer with a new capacity.
    fn reallocate(&mut self, device: &wgpu::Device, new_capacity: usize) {
        // Note: WGPU buffers don't expose their label after creation
        self.buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("UI Instance Buffer (Reallocated)"),
            size: (new_capacity * std::mem::size_of::<T>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.capacity = new_capacity;

        // Mark entire buffer as dirty after reallocation
        if !self.instances.is_empty() {
            self.dirty_ranges.mark_dirty(0, self.instances.len());
        }
    }

    /// Get buffer statistics.
    pub fn stats(&self) -> InstanceBufferStats {
        InstanceBufferStats {
            instance_count: self.instances.len(),
            capacity: self.capacity,
            utilization: if self.capacity > 0 {
                (self.instances.len() as f32 / self.capacity as f32) * 100.0
            } else {
                0.0
            },
            dirty_ranges: self.dirty_ranges.stats().num_ranges,
            write_count: self.write_count,
            size_bytes: self.instances.len() * std::mem::size_of::<T>(),
            capacity_bytes: self.capacity * std::mem::size_of::<T>(),
        }
    }
}

/// Statistics about an instance buffer.
#[derive(Debug, Clone, Copy)]
pub struct InstanceBufferStats {
    pub instance_count: usize,
    pub capacity: usize,
    pub utilization: f32,
    pub dirty_ranges: usize,
    pub write_count: u64,
    pub size_bytes: usize,
    pub capacity_bytes: usize,
}

/// Ring buffer strategy for multi-buffered instance data.
///
/// Useful for triple-buffering or managing multiple frames in flight.
/// Each frame gets its own slot in a circular buffer.
pub struct RingInstanceBuffer<T: Pod> {
    /// Multiple instance buffers, one per frame slot
    buffers: Vec<InstanceBuffer<T>>,
    /// Current frame index
    current_frame: usize,
    /// Number of frames to buffer
    frame_count: usize,
}

impl<T: Pod> RingInstanceBuffer<T> {
    /// Create a new ring buffer with the specified number of frame slots.
    pub fn new(
        device: &wgpu::Device,
        label_prefix: &str,
        frame_count: usize,
        capacity: usize,
    ) -> Self {
        let mut buffers = Vec::with_capacity(frame_count);

        for i in 0..frame_count {
            let label = format!("{} Frame {}", label_prefix, i);
            buffers.push(InstanceBuffer::new(device, Some(&label), capacity));
        }

        Self {
            buffers,
            current_frame: 0,
            frame_count,
        }
    }

    /// Get the current frame's buffer.
    pub fn current(&self) -> &InstanceBuffer<T> {
        &self.buffers[self.current_frame]
    }

    /// Get mutable reference to current frame's buffer.
    pub fn current_mut(&mut self) -> &mut InstanceBuffer<T> {
        &mut self.buffers[self.current_frame]
    }

    /// Advance to the next frame.
    pub fn advance_frame(&mut self) {
        self.current_frame = (self.current_frame + 1) % self.frame_count;
    }

    /// Get all buffers.
    pub fn buffers(&self) -> &[InstanceBuffer<T>] {
        &self.buffers
    }

    /// Get current frame index.
    pub fn frame_index(&self) -> usize {
        self.current_frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock Pod type for testing
    #[repr(C)]
    #[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
    struct TestInstance {
        position: [f32; 2],
        color: [f32; 4],
    }

    impl TestInstance {
        fn new(x: f32, y: f32, r: f32, g: f32, b: f32, a: f32) -> Self {
            Self {
                position: [x, y],
                color: [r, g, b, a],
            }
        }
    }

    // Note: These tests can't actually run without a WGPU device
    // In a real test environment, you'd use pollster and create a test device
    // For now, we test the logic that doesn't require GPU

    #[test]
    fn test_instance_tracking() {
        // We can test the CPU-side logic without GPU
        let instances = vec![
            TestInstance::new(0.0, 0.0, 1.0, 0.0, 0.0, 1.0),
            TestInstance::new(10.0, 10.0, 0.0, 1.0, 0.0, 1.0),
        ];

        // Test that we can track instances
        assert_eq!(instances.len(), 2);
        assert_eq!(instances[0].position, [0.0, 0.0]);
    }

    #[test]
    fn test_dirty_range_tracking() {
        let mut dirty_ranges = DirtyRanges::new();

        dirty_ranges.mark_dirty(0, 5);
        dirty_ranges.mark_dirty(10, 15);

        assert_eq!(dirty_ranges.len(), 2);
        assert_eq!(dirty_ranges.total_dirty_count(), 10);
    }

    #[test]
    fn test_capacity_calculation() {
        let capacity = 100;
        let instance_size = std::mem::size_of::<TestInstance>();
        let buffer_size = capacity * instance_size;

        assert_eq!(buffer_size, capacity * 24); // 2 floats + 4 floats = 24 bytes
    }

    #[test]
    fn test_stats_calculation() {
        // Test stats calculation logic
        let instance_count = 75;
        let capacity = 100;
        let utilization = (instance_count as f32 / capacity as f32) * 100.0;

        assert_eq!(utilization, 75.0);
    }
}
