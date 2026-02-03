//! Indirect draw buffer support for GPU-driven rendering.
//!
//! This module provides type-safe wrappers for indirect draw commands and buffers.
//! Indirect drawing allows the GPU to control draw parameters, enabling techniques
//! like GPU culling and dynamic batching.
//!
//! # Feature Requirements
//!
//! - `INDIRECT_FIRST_INSTANCE`: Required for using `first_instance` in indirect commands.
//! - `multi_draw_indirect()`: Available on all desktop GPUs (requires `DownlevelFlags::INDIRECT_EXECUTION`).
//! - `MULTI_DRAW_INDIRECT_COUNT`: Required for GPU-driven draw count variant.

use std::marker::PhantomData;

use bytemuck::{Pod, Zeroable};

use crate::context::GraphicsContext;
use crate::features::GpuFeatures;

/// Indirect draw command for non-indexed geometry.
///
/// This matches the layout expected by `wgpu::RenderPass::draw_indirect`.
///
/// # Fields
///
/// * `vertex_count` - Number of vertices to draw
/// * `instance_count` - Number of instances to draw
/// * `first_vertex` - Index of the first vertex to draw
/// * `first_instance` - Instance ID of the first instance (requires INDIRECT_FIRST_INSTANCE)
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct DrawIndirect {
    pub vertex_count: u32,
    pub instance_count: u32,
    pub first_vertex: u32,
    pub first_instance: u32,
}

// SAFETY: DrawIndirect is a repr(C) struct of u32s with no padding
unsafe impl Pod for DrawIndirect {}
unsafe impl Zeroable for DrawIndirect {}

impl DrawIndirect {
    /// Create a new indirect draw command.
    pub const fn new(
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    ) -> Self {
        Self {
            vertex_count,
            instance_count,
            first_vertex,
            first_instance,
        }
    }

    /// Create a simple draw command for a single instance.
    pub const fn single(vertex_count: u32) -> Self {
        Self::new(vertex_count, 1, 0, 0)
    }

    /// Create a draw command for multiple instances.
    pub const fn instanced(vertex_count: u32, instance_count: u32) -> Self {
        Self::new(vertex_count, instance_count, 0, 0)
    }

    /// Size of the command in bytes.
    pub const fn size() -> u64 {
        std::mem::size_of::<Self>() as u64
    }
}

/// Indirect draw command for indexed geometry.
///
/// This matches the layout expected by `wgpu::RenderPass::draw_indexed_indirect`.
///
/// # Fields
///
/// * `index_count` - Number of indices to draw
/// * `instance_count` - Number of instances to draw
/// * `first_index` - Index of the first index to draw
/// * `base_vertex` - Value added to each index before indexing into the vertex buffer
/// * `first_instance` - Instance ID of the first instance (requires INDIRECT_FIRST_INSTANCE)
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct DrawIndexedIndirect {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    pub base_vertex: i32,
    pub first_instance: u32,
}

// SAFETY: DrawIndexedIndirect is a repr(C) struct with no padding
unsafe impl Pod for DrawIndexedIndirect {}
unsafe impl Zeroable for DrawIndexedIndirect {}

impl DrawIndexedIndirect {
    /// Create a new indexed indirect draw command.
    pub const fn new(
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        base_vertex: i32,
        first_instance: u32,
    ) -> Self {
        Self {
            index_count,
            instance_count,
            first_index,
            base_vertex,
            first_instance,
        }
    }

    /// Create a simple indexed draw command for a single instance.
    pub const fn single(index_count: u32) -> Self {
        Self::new(index_count, 1, 0, 0, 0)
    }

    /// Create an indexed draw command for multiple instances.
    pub const fn instanced(index_count: u32, instance_count: u32) -> Self {
        Self::new(index_count, instance_count, 0, 0, 0)
    }

    /// Size of the command in bytes.
    pub const fn size() -> u64 {
        std::mem::size_of::<Self>() as u64
    }
}

/// Marker trait for indirect draw command types.
pub trait IndirectCommand: Pod + Zeroable + Default {
    /// Size of a single command in bytes.
    const SIZE: u64;
}

impl IndirectCommand for DrawIndirect {
    const SIZE: u64 = std::mem::size_of::<Self>() as u64;
}

impl IndirectCommand for DrawIndexedIndirect {
    const SIZE: u64 = std::mem::size_of::<Self>() as u64;
}

/// A type-safe GPU buffer for indirect draw commands.
///
/// This wrapper ensures type safety and provides convenient methods for
/// writing and using indirect draw commands.
///
/// # Type Parameters
///
/// * `T` - The type of indirect command (either `DrawIndirect` or `DrawIndexedIndirect`)
///
/// # Example
///
/// ```ignore
/// use astrelis_render::{IndirectBuffer, DrawIndexedIndirect, Renderer};
///
/// // Create an indirect buffer for 100 indexed draw commands
/// let indirect_buffer = IndirectBuffer::<DrawIndexedIndirect>::new(
///     context,
///     Some("My Indirect Buffer"),
///     100,
/// );
///
/// // Write commands
/// let commands = vec![
///     DrawIndexedIndirect::single(36),  // Draw 36 indices
///     DrawIndexedIndirect::instanced(36, 10),  // Draw 36 indices, 10 instances
/// ];
/// indirect_buffer.write(&context.queue, &commands);
///
/// // In render pass
/// render_pass.draw_indexed_indirect(indirect_buffer.buffer(), 0);
/// ```
pub struct IndirectBuffer<T: IndirectCommand> {
    buffer: wgpu::Buffer,
    capacity: usize,
    _marker: PhantomData<T>,
}

impl<T: IndirectCommand> IndirectBuffer<T> {
    /// Create a new indirect buffer with the specified capacity.
    ///
    /// # Arguments
    ///
    /// * `context` - The graphics context
    /// * `label` - Optional debug label
    /// * `capacity` - Maximum number of commands the buffer can hold
    ///
    /// # Panics
    ///
    /// Panics if `INDIRECT_FIRST_INSTANCE` feature is not enabled on the context.
    pub fn new(
        context: &GraphicsContext,
        label: Option<&str>,
        capacity: usize,
    ) -> Self {
        // Check that required feature is available
        context.require_feature(GpuFeatures::INDIRECT_FIRST_INSTANCE);

        let buffer = context.device().create_buffer(&wgpu::BufferDescriptor {
            label,
            size: T::SIZE * capacity as u64,
            usage: wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        Self {
            buffer,
            capacity,
            _marker: PhantomData,
        }
    }

    /// Create a new indirect buffer initialized with commands.
    ///
    /// # Arguments
    ///
    /// * `context` - The graphics context
    /// * `label` - Optional debug label
    /// * `commands` - Initial commands to write to the buffer
    ///
    /// # Panics
    ///
    /// Panics if `INDIRECT_FIRST_INSTANCE` feature is not enabled on the context.
    pub fn new_init(
        context: &GraphicsContext,
        label: Option<&str>,
        commands: &[T],
    ) -> Self {
        context.require_feature(GpuFeatures::INDIRECT_FIRST_INSTANCE);

        let buffer = context.device().create_buffer(&wgpu::BufferDescriptor {
            label,
            size: T::SIZE * commands.len() as u64,
            usage: wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        context
            .queue()
            .write_buffer(&buffer, 0, bytemuck::cast_slice(commands));

        Self {
            buffer,
            capacity: commands.len(),
            _marker: PhantomData,
        }
    }

    /// Get the underlying wgpu buffer.
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    /// Get the capacity (maximum number of commands).
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get the size of the buffer in bytes.
    pub fn size_bytes(&self) -> u64 {
        T::SIZE * self.capacity as u64
    }

    /// Get the byte offset of a command at the given index.
    pub fn offset_of(&self, index: usize) -> u64 {
        T::SIZE * index as u64
    }

    /// Write commands to the buffer starting at the given index.
    ///
    /// # Arguments
    ///
    /// * `queue` - The command queue to use for the write
    /// * `start_index` - Index of the first command to write
    /// * `commands` - Commands to write
    ///
    /// # Panics
    ///
    /// Panics if the write would exceed the buffer capacity.
    pub fn write_at(&self, queue: &wgpu::Queue, start_index: usize, commands: &[T]) {
        assert!(
            start_index + commands.len() <= self.capacity,
            "Indirect buffer write would exceed capacity: {} + {} > {}",
            start_index,
            commands.len(),
            self.capacity
        );

        let offset = T::SIZE * start_index as u64;
        queue.write_buffer(&self.buffer, offset, bytemuck::cast_slice(commands));
    }

    /// Write commands to the buffer starting at index 0.
    ///
    /// # Arguments
    ///
    /// * `queue` - The command queue to use for the write
    /// * `commands` - Commands to write
    ///
    /// # Panics
    ///
    /// Panics if the write would exceed the buffer capacity.
    pub fn write(&self, queue: &wgpu::Queue, commands: &[T]) {
        self.write_at(queue, 0, commands);
    }

    /// Clear the buffer by writing zeros.
    pub fn clear(&self, queue: &wgpu::Queue) {
        let zeros = vec![T::default(); self.capacity];
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&zeros));
    }
}

/// Extension trait for render passes to use indirect buffers.
pub trait RenderPassIndirectExt<'a> {
    /// Draw non-indexed geometry using an indirect buffer.
    ///
    /// # Arguments
    ///
    /// * `indirect_buffer` - Buffer containing draw commands
    /// * `index` - Index of the command to execute
    fn draw_indirect_at(&mut self, indirect_buffer: &'a IndirectBuffer<DrawIndirect>, index: usize);

    /// Draw indexed geometry using an indirect buffer.
    ///
    /// # Arguments
    ///
    /// * `indirect_buffer` - Buffer containing draw commands
    /// * `index` - Index of the command to execute
    fn draw_indexed_indirect_at(
        &mut self,
        indirect_buffer: &'a IndirectBuffer<DrawIndexedIndirect>,
        index: usize,
    );
}

impl<'a> RenderPassIndirectExt<'a> for wgpu::RenderPass<'a> {
    fn draw_indirect_at(
        &mut self,
        indirect_buffer: &'a IndirectBuffer<DrawIndirect>,
        index: usize,
    ) {
        let offset = indirect_buffer.offset_of(index);
        self.draw_indirect(indirect_buffer.buffer(), offset);
    }

    fn draw_indexed_indirect_at(
        &mut self,
        indirect_buffer: &'a IndirectBuffer<DrawIndexedIndirect>,
        index: usize,
    ) {
        let offset = indirect_buffer.offset_of(index);
        self.draw_indexed_indirect(indirect_buffer.buffer(), offset);
    }
}

/// Extension trait for multi-draw indirect operations.
///
/// Requires `DownlevelFlags::INDIRECT_EXECUTION` (available on all desktop GPUs).
pub trait RenderPassMultiDrawIndirectExt<'a> {
    /// Draw non-indexed geometry multiple times using an indirect buffer.
    ///
    /// # Arguments
    ///
    /// * `indirect_buffer` - Buffer containing draw commands
    /// * `start_index` - Index of the first command to execute
    /// * `count` - Number of commands to execute
    ///
    /// # Panics
    ///
    /// Requires `DownlevelFlags::INDIRECT_EXECUTION`.
    fn multi_draw_indirect(
        &mut self,
        indirect_buffer: &'a IndirectBuffer<DrawIndirect>,
        start_index: usize,
        count: u32,
    );

    /// Draw indexed geometry multiple times using an indirect buffer.
    ///
    /// # Arguments
    ///
    /// * `indirect_buffer` - Buffer containing draw commands
    /// * `start_index` - Index of the first command to execute
    /// * `count` - Number of commands to execute
    ///
    /// # Panics
    ///
    /// Requires `DownlevelFlags::INDIRECT_EXECUTION`.
    fn multi_draw_indexed_indirect(
        &mut self,
        indirect_buffer: &'a IndirectBuffer<DrawIndexedIndirect>,
        start_index: usize,
        count: u32,
    );
}

impl<'a> RenderPassMultiDrawIndirectExt<'a> for wgpu::RenderPass<'a> {
    fn multi_draw_indirect(
        &mut self,
        indirect_buffer: &'a IndirectBuffer<DrawIndirect>,
        start_index: usize,
        count: u32,
    ) {
        let offset = indirect_buffer.offset_of(start_index);
        self.multi_draw_indirect(indirect_buffer.buffer(), offset, count);
    }

    fn multi_draw_indexed_indirect(
        &mut self,
        indirect_buffer: &'a IndirectBuffer<DrawIndexedIndirect>,
        start_index: usize,
        count: u32,
    ) {
        let offset = indirect_buffer.offset_of(start_index);
        self.multi_draw_indexed_indirect(indirect_buffer.buffer(), offset, count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_draw_indirect_size() {
        // Verify the struct matches wgpu's expected layout
        assert_eq!(DrawIndirect::size(), 16); // 4 u32s = 16 bytes
        assert_eq!(DrawIndirect::SIZE, 16);
    }

    #[test]
    fn test_draw_indexed_indirect_size() {
        // Verify the struct matches wgpu's expected layout
        assert_eq!(DrawIndexedIndirect::size(), 20); // 4 u32s + 1 i32 = 20 bytes
        assert_eq!(DrawIndexedIndirect::SIZE, 20);
    }

    #[test]
    fn test_draw_indirect_single() {
        let cmd = DrawIndirect::single(36);
        assert_eq!(cmd.vertex_count, 36);
        assert_eq!(cmd.instance_count, 1);
        assert_eq!(cmd.first_vertex, 0);
        assert_eq!(cmd.first_instance, 0);
    }

    #[test]
    fn test_draw_indexed_indirect_instanced() {
        let cmd = DrawIndexedIndirect::instanced(36, 100);
        assert_eq!(cmd.index_count, 36);
        assert_eq!(cmd.instance_count, 100);
        assert_eq!(cmd.first_index, 0);
        assert_eq!(cmd.base_vertex, 0);
        assert_eq!(cmd.first_instance, 0);
    }
}
