//! Compute pass management with ergonomic builder pattern.
//!
//! This module provides a `ComputePassBuilder` that mirrors the ergonomics of
//! `RenderPassBuilder` for compute shader operations.

use astrelis_core::profiling::profile_function;

use crate::frame::FrameContext;

/// Builder for creating compute passes.
///
/// # Example
///
/// ```ignore
/// let mut compute_pass = ComputePassBuilder::new()
///     .label("My Compute Pass")
///     .build(&mut frame);
///
/// compute_pass.set_pipeline(&pipeline);
/// compute_pass.set_bind_group(0, &bind_group, &[]);
/// compute_pass.dispatch_workgroups(64, 64, 1);
/// ```
pub struct ComputePassBuilder<'a> {
    label: Option<&'a str>,
}

impl<'a> ComputePassBuilder<'a> {
    /// Create a new compute pass builder.
    pub fn new() -> Self {
        Self { label: None }
    }

    /// Set a debug label for the compute pass.
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Build the compute pass.
    ///
    /// This takes the command encoder from the FrameContext and returns it
    /// when the ComputePass is dropped.
    pub fn build(self, frame_context: &'a mut FrameContext) -> ComputePass<'a> {
        let mut encoder = frame_context.encoder.take().unwrap();

        let compute_pass = encoder
            .begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: self.label,
                timestamp_writes: None,
            })
            .forget_lifetime();

        ComputePass {
            context: frame_context,
            encoder: Some(encoder),
            pass: Some(compute_pass),
        }
    }
}

impl Default for ComputePassBuilder<'_> {
    fn default() -> Self {
        Self::new()
    }
}

/// A compute pass wrapper that automatically returns the encoder to the frame context.
///
/// This struct mirrors `RenderPass` in its lifecycle management - it takes the
/// encoder from `FrameContext` and returns it when dropped.
pub struct ComputePass<'a> {
    pub(crate) context: &'a mut FrameContext,
    pub(crate) encoder: Option<wgpu::CommandEncoder>,
    pub(crate) pass: Option<wgpu::ComputePass<'static>>,
}

impl<'a> ComputePass<'a> {
    /// Get the underlying wgpu compute pass.
    pub fn pass(&mut self) -> &mut wgpu::ComputePass<'static> {
        self.pass.as_mut().unwrap()
    }

    /// Set the compute pipeline to use.
    pub fn set_pipeline(&mut self, pipeline: &'a wgpu::ComputePipeline) {
        self.pass().set_pipeline(pipeline);
    }

    /// Set a bind group.
    pub fn set_bind_group(
        &mut self,
        index: u32,
        bind_group: &'a wgpu::BindGroup,
        offsets: &[u32],
    ) {
        self.pass().set_bind_group(index, bind_group, offsets);
    }

    /// Dispatch workgroups.
    ///
    /// # Arguments
    ///
    /// * `x` - Number of workgroups in the X dimension
    /// * `y` - Number of workgroups in the Y dimension
    /// * `z` - Number of workgroups in the Z dimension
    pub fn dispatch_workgroups(&mut self, x: u32, y: u32, z: u32) {
        self.pass().dispatch_workgroups(x, y, z);
    }

    /// Dispatch workgroups with a 1D configuration.
    ///
    /// Equivalent to `dispatch_workgroups(x, 1, 1)`.
    pub fn dispatch_workgroups_1d(&mut self, x: u32) {
        self.dispatch_workgroups(x, 1, 1);
    }

    /// Dispatch workgroups with a 2D configuration.
    ///
    /// Equivalent to `dispatch_workgroups(x, y, 1)`.
    pub fn dispatch_workgroups_2d(&mut self, x: u32, y: u32) {
        self.dispatch_workgroups(x, y, 1);
    }

    /// Dispatch workgroups indirectly from a buffer.
    ///
    /// The buffer should contain a `DispatchIndirect` struct:
    /// ```ignore
    /// #[repr(C)]
    /// struct DispatchIndirect {
    ///     x: u32,
    ///     y: u32,
    ///     z: u32,
    /// }
    /// ```
    pub fn dispatch_workgroups_indirect(&mut self, buffer: &'a wgpu::Buffer, offset: u64) {
        self.pass().dispatch_workgroups_indirect(buffer, offset);
    }

    /// Insert a debug marker.
    pub fn insert_debug_marker(&mut self, label: &str) {
        self.pass().insert_debug_marker(label);
    }

    /// Push a debug group.
    pub fn push_debug_group(&mut self, label: &str) {
        self.pass().push_debug_group(label);
    }

    /// Pop a debug group.
    pub fn pop_debug_group(&mut self) {
        self.pass().pop_debug_group();
    }

    /// Set push constants for the compute shader.
    ///
    /// Push constants are a fast way to pass small amounts of data to shaders
    /// without using uniform buffers. They require the `PUSH_CONSTANTS` feature
    /// to be enabled on the device.
    ///
    /// # Arguments
    ///
    /// * `offset` - Byte offset into the push constant range
    /// * `data` - Data to set (must be `Pod` for safe byte casting)
    ///
    /// # Example
    ///
    /// ```ignore
    /// #[repr(C)]
    /// #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    /// struct ComputeConstants {
    ///     workgroup_count: u32,
    ///     time: f32,
    /// }
    ///
    /// let constants = ComputeConstants {
    ///     workgroup_count: 64,
    ///     time: 1.5,
    /// };
    ///
    /// pass.set_push_constants(0, &constants);
    /// ```
    pub fn set_push_constants<T: bytemuck::Pod>(&mut self, offset: u32, data: &T) {
        self.pass()
            .set_push_constants(offset, bytemuck::bytes_of(data));
    }

    /// Set push constants from raw bytes.
    ///
    /// Use this when you need more control over the data layout.
    pub fn set_push_constants_raw(&mut self, offset: u32, data: &[u8]) {
        self.pass().set_push_constants(offset, data);
    }

    /// Finish the compute pass, returning the encoder to the frame context.
    pub fn finish(self) {
        drop(self);
    }
}

impl Drop for ComputePass<'_> {
    fn drop(&mut self) {
        profile_function!();

        // End the compute pass
        drop(self.pass.take());

        // Return the encoder to the frame context
        self.context.encoder = self.encoder.take();
    }
}

/// Indirect dispatch command.
///
/// This matches the layout expected by `wgpu::ComputePass::dispatch_workgroups_indirect`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct DispatchIndirect {
    /// Number of workgroups in the X dimension.
    pub x: u32,
    /// Number of workgroups in the Y dimension.
    pub y: u32,
    /// Number of workgroups in the Z dimension.
    pub z: u32,
}

// SAFETY: DispatchIndirect is a repr(C) struct of u32s with no padding
unsafe impl bytemuck::Pod for DispatchIndirect {}
unsafe impl bytemuck::Zeroable for DispatchIndirect {}

impl DispatchIndirect {
    /// Create a new dispatch command.
    pub const fn new(x: u32, y: u32, z: u32) -> Self {
        Self { x, y, z }
    }

    /// Create a 1D dispatch command.
    pub const fn new_1d(x: u32) -> Self {
        Self::new(x, 1, 1)
    }

    /// Create a 2D dispatch command.
    pub const fn new_2d(x: u32, y: u32) -> Self {
        Self::new(x, y, 1)
    }

    /// Size of the command in bytes.
    pub const fn size() -> u64 {
        std::mem::size_of::<Self>() as u64
    }
}

/// Helper trait for creating compute passes from FrameContext.
pub trait ComputePassExt {
    /// Create a compute pass with a label.
    fn compute_pass<'a>(&'a mut self, label: &'a str) -> ComputePass<'a>;

    /// Create a compute pass without a label.
    fn compute_pass_unlabeled(&mut self) -> ComputePass<'_>;
}

impl ComputePassExt for FrameContext {
    fn compute_pass<'a>(&'a mut self, label: &'a str) -> ComputePass<'a> {
        ComputePassBuilder::new().label(label).build(self)
    }

    fn compute_pass_unlabeled(&mut self) -> ComputePass<'_> {
        ComputePassBuilder::new().build(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dispatch_indirect_size() {
        // Verify the struct matches wgpu's expected layout
        assert_eq!(DispatchIndirect::size(), 12); // 3 u32s = 12 bytes
    }

    #[test]
    fn test_dispatch_indirect_1d() {
        let cmd = DispatchIndirect::new_1d(64);
        assert_eq!(cmd.x, 64);
        assert_eq!(cmd.y, 1);
        assert_eq!(cmd.z, 1);
    }

    #[test]
    fn test_dispatch_indirect_2d() {
        let cmd = DispatchIndirect::new_2d(32, 32);
        assert_eq!(cmd.x, 32);
        assert_eq!(cmd.y, 32);
        assert_eq!(cmd.z, 1);
    }
}
