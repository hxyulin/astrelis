//! GPU instance buffer management for geometry rendering.
//!
//! Manages instance buffers with efficient partial updates.

use crate::dirty_ranges::DirtyRanges;
use astrelis_render::wgpu;
use bytemuck::Pod;

/// GPU instance buffer with partial update support.
///
/// Maintains a CPU-side buffer and GPU buffer, tracking dirty ranges.
pub struct InstanceBuffer<T: Pod> {
    /// GPU buffer for instance data
    buffer: wgpu::Buffer,
    /// CPU-side instance data
    instances: Vec<T>,
    /// Current capacity
    capacity: usize,
    /// Ranges that need GPU upload
    dirty_ranges: DirtyRanges,
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
        }
    }

    /// Get the GPU buffer.
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    /// Set instances, replacing all existing data.
    pub fn set_instances(&mut self, device: &wgpu::Device, instances: Vec<T>) {
        let new_len = instances.len();

        if new_len > self.capacity {
            self.reallocate(device, new_len.next_power_of_two());
        }

        self.instances = instances;

        if !self.instances.is_empty() {
            self.dirty_ranges.mark_dirty(0, self.instances.len());
        }
    }

    /// Upload all dirty ranges to the GPU.
    pub fn upload_dirty(&mut self, queue: &wgpu::Queue) {
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
        }

        self.dirty_ranges.clear();
    }

    /// Reallocate the GPU buffer with a new capacity.
    fn reallocate(&mut self, device: &wgpu::Device, new_capacity: usize) {
        self.buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Geometry Instance Buffer (Reallocated)"),
            size: (new_capacity * std::mem::size_of::<T>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.capacity = new_capacity;

        if !self.instances.is_empty() {
            self.dirty_ranges.mark_dirty(0, self.instances.len());
        }
    }
}
