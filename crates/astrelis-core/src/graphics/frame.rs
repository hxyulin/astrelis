//! Frame-based rendering API for single-encoder workflow.
//!
//! This module provides:
//! - `FrameContext`: Owns command encoder for entire frame
//! - Scoped render pass API that avoids lifetime issues
//! - Statistics tracking

use std::cell::RefCell;

use crate::graphics::{GraphicsContext, RenderTarget};

/// Statistics collected during frame rendering
#[derive(Debug, Default, Clone)]
pub struct FrameStats {
    pub render_passes: u32,
    pub compute_passes: u32,
    pub draw_calls: u32,
    pub triangles: u64,
}

/// Context for a single frame, owning the command encoder.
///
/// Automatically submits and presents on drop.
pub struct FrameContext<'frame> {
    context: &'frame mut GraphicsContext,
    encoder: RefCell<Option<wgpu::CommandEncoder>>,
    stats: RefCell<FrameStats>,
}

impl<'frame> FrameContext<'frame> {
    /// Create a new frame context (internal use only)
    pub(crate) fn new(context: &'frame mut GraphicsContext) -> Self {
        let encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Frame Encoder"),
            });

        Self {
            context,
            encoder: RefCell::new(Some(encoder)),
            stats: RefCell::new(FrameStats::default()),
        }
    }

    /// Begin configuring a render pass
    pub fn render_pass(&mut self) -> RenderPassBuilder<'_, 'frame> {
        RenderPassBuilder::new(self)
    }

    /// Begin a compute pass with a callback
    pub fn compute_pass<F>(&mut self, label: Option<&str>, f: F)
    where
        F: FnOnce(&mut wgpu::ComputePass),
    {
        self.stats.borrow_mut().compute_passes += 1;

        let mut encoder = self.encoder.borrow_mut();
        let encoder = encoder.as_mut().unwrap();

        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label,
            ..Default::default()
        });

        f(&mut pass);
    }

    /// Write data to a buffer (convenience method)
    pub fn write_buffer(&self, buffer: &wgpu::Buffer, offset: u64, data: &[u8]) {
        self.context.queue.write_buffer(buffer, offset, data);
    }

    /// Get reference to the device
    pub fn device(&self) -> &wgpu::Device {
        &self.context.device
    }

    /// Get reference to the queue
    pub fn queue(&self) -> &wgpu::Queue {
        &self.context.queue
    }

    /// Get reference to the graphics context
    pub fn context(&self) -> &GraphicsContext {
        self.context
    }

    /// Get reference to the frame statistics
    pub fn stats(&self) -> FrameStats {
        self.stats.borrow().clone()
    }

    /// Get access to the encoder for advanced use cases.
    ///
    /// WARNING: This is an advanced API. Most code should use `render_pass()` or `compute_pass()`.
    /// Direct encoder access is provided for special cases like egui buffer updates.
    pub fn encoder(&self) -> std::cell::RefMut<'_, Option<wgpu::CommandEncoder>> {
        self.encoder.borrow_mut()
    }

    /// Get mutable access to the encoder for advanced use cases (deprecated, use encoder()).
    #[deprecated(note = "Use encoder() instead - Arc<Mutex<>> doesn't need &mut self")]
    pub fn encoder_mut(&mut self) -> std::cell::RefMut<'_, Option<wgpu::CommandEncoder>> {
        self.encoder.borrow_mut()
    }

    /// Record a draw call (internal)
    pub(crate) fn record_draw(&self, vertex_count: u32, instance_count: u32) {
        let mut stats = self.stats.borrow_mut();
        stats.draw_calls += 1;
        stats.triangles += (vertex_count / 3 * instance_count) as u64;
    }
}

impl Drop for FrameContext<'_> {
    fn drop(&mut self) {
        crate::profiling::profile_scope!("FrameContext::drop");

        // Finish and submit encoder
        if let Some(encoder) = self.encoder.borrow_mut().take() {
            let command_buffer = encoder.finish();
            self.context.queue.submit(Some(command_buffer));
        }

        // Present the frame
        self.context.end_render();

        let stats = self.stats.borrow();
        tracing::trace!(
            render_passes = stats.render_passes,
            compute_passes = stats.compute_passes,
            draw_calls = stats.draw_calls,
            triangles = stats.triangles,
            "Frame complete"
        );
    }
}

/// Load operation for attachments
#[derive(Debug, Clone, Copy)]
pub enum LoadOp {
    Clear,
    Load,
}

/// Store operation for attachments
#[derive(Debug, Clone, Copy)]
pub enum StoreOp {
    Store,
    Discard,
}

/// Builder for configuring a render pass
pub struct RenderPassBuilder<'pass, 'frame> {
    frame: &'pass mut FrameContext<'frame>,
    label: Option<String>,
    color_attachment: Option<RenderPassColorAttachment>,
    depth_attachment: Option<RenderPassDepthAttachment>,
}

/// Color attachment configuration
#[derive(Clone)]
pub struct RenderPassColorAttachment {
    pub target: RenderTarget,
    pub load_op: LoadOp,
    pub store_op: StoreOp,
    pub clear_color: Option<wgpu::Color>,
}

/// Depth attachment configuration
#[derive(Clone)]
pub struct RenderPassDepthAttachment {
    pub target: RenderTarget,
    pub depth_load_op: LoadOp,
    pub depth_store_op: StoreOp,
    pub clear_depth: Option<f32>,
    pub stencil_load_op: LoadOp,
    pub stencil_store_op: StoreOp,
    pub clear_stencil: Option<u32>,
}

impl<'pass, 'frame> RenderPassBuilder<'pass, 'frame> {
    fn new(frame: &'pass mut FrameContext<'frame>) -> Self {
        Self {
            frame,
            label: None,
            color_attachment: None,
            depth_attachment: None,
        }
    }

    /// Set a label for the render pass
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the color attachment target
    pub fn color_attachment(mut self, target: RenderTarget) -> Self {
        self.color_attachment = Some(RenderPassColorAttachment {
            target,
            load_op: LoadOp::Clear,
            store_op: StoreOp::Store,
            clear_color: Some(wgpu::Color::BLACK),
        });
        self
    }

    /// Configure color load operation
    pub fn color_load(mut self, load_op: LoadOp, clear_color: Option<wgpu::Color>) -> Self {
        if let Some(ref mut attachment) = self.color_attachment {
            attachment.load_op = load_op;
            attachment.clear_color = clear_color;
        }
        self
    }

    /// Configure color store operation
    pub fn color_store(mut self, store_op: StoreOp) -> Self {
        if let Some(ref mut attachment) = self.color_attachment {
            attachment.store_op = store_op;
        }
        self
    }

    /// Set the depth attachment
    pub fn depth_attachment(mut self, target: RenderTarget, clear_depth: f32) -> Self {
        self.depth_attachment = Some(RenderPassDepthAttachment {
            target,
            depth_load_op: LoadOp::Clear,
            depth_store_op: StoreOp::Store,
            clear_depth: Some(clear_depth),
            stencil_load_op: LoadOp::Load,
            stencil_store_op: StoreOp::Discard,
            clear_stencil: None,
        });
        self
    }

    /// Configure depth load operation
    pub fn depth_load(mut self, load_op: LoadOp, clear: Option<f32>) -> Self {
        if let Some(ref mut attachment) = self.depth_attachment {
            attachment.depth_load_op = load_op;
            attachment.clear_depth = clear;
        }
        self
    }

    /// Configure depth store operation
    pub fn depth_store(mut self, store_op: StoreOp) -> Self {
        if let Some(ref mut attachment) = self.depth_attachment {
            attachment.depth_store_op = store_op;
        }
        self
    }

    /// Begin the render pass, returning a recorder
    pub fn begin(self) -> RenderPassRecorder<'pass, 'frame> {
        self.frame.stats.borrow_mut().render_passes += 1;

        // Increment pass count for old API compatibility
        if let Some(ref mut frame) = self.frame.context.frame {
            frame.passes += 1;
        }

        let color_attachment = self.color_attachment.as_ref();
        let depth_attachment = self.depth_attachment.as_ref();

        let color_view = color_attachment.map(|att| att.target.get_color(self.frame.context));
        let depth_view = depth_attachment.and_then(|att| att.target.get_depth(self.frame.context));

        // Build wgpu render pass descriptor
        let wgpu_color_attachment = color_attachment.map(|att| wgpu::RenderPassColorAttachment {
            view: color_view.unwrap(),
            resolve_target: None,
            ops: wgpu::Operations {
                load: match att.load_op {
                    LoadOp::Clear => {
                        wgpu::LoadOp::Clear(att.clear_color.unwrap_or(wgpu::Color::BLACK))
                    }
                    LoadOp::Load => wgpu::LoadOp::Load,
                },
                store: match att.store_op {
                    StoreOp::Store => wgpu::StoreOp::Store,
                    StoreOp::Discard => wgpu::StoreOp::Discard,
                },
            },
        });

        let wgpu_depth_attachment =
            depth_attachment.map(|att| wgpu::RenderPassDepthStencilAttachment {
                view: depth_view.unwrap(),
                depth_ops: Some(wgpu::Operations {
                    load: match att.depth_load_op {
                        LoadOp::Clear => wgpu::LoadOp::Clear(att.clear_depth.unwrap_or(1.0)),
                        LoadOp::Load => wgpu::LoadOp::Load,
                    },
                    store: match att.depth_store_op {
                        StoreOp::Store => wgpu::StoreOp::Store,
                        StoreOp::Discard => wgpu::StoreOp::Discard,
                    },
                }),
                stencil_ops: Some(wgpu::Operations {
                    load: match att.stencil_load_op {
                        LoadOp::Clear => wgpu::LoadOp::Clear(att.clear_stencil.unwrap_or(0)),
                        LoadOp::Load => wgpu::LoadOp::Load,
                    },
                    store: match att.stencil_store_op {
                        StoreOp::Store => wgpu::StoreOp::Store,
                        StoreOp::Discard => wgpu::StoreOp::Discard,
                    },
                }),
            });

        let descriptor = wgpu::RenderPassDescriptor {
            label: self.label.as_deref(),
            color_attachments: &[wgpu_color_attachment],
            depth_stencil_attachment: wgpu_depth_attachment,
            ..Default::default()
        };

        // Create the pass with forgotten lifetime
        let mut encoder = self.frame.encoder.borrow_mut();
        let pass = encoder.as_mut().unwrap().begin_render_pass(&descriptor);
        let pass_static = pass.forget_lifetime();
        drop(encoder); // Explicitly drop the lock

        RenderPassRecorder {
            pass: Some(pass_static),
            frame: self.frame,
        }
    }
}

/// Recorder for a render pass with helper methods
///
/// The pass is lazily created with 'static lifetime (using forget_lifetime).
/// When dropped, the pass is dropped first, then frame can be accessed again.
pub struct RenderPassRecorder<'pass, 'frame> {
    pass: Option<wgpu::RenderPass<'static>>,
    frame: &'pass mut FrameContext<'frame>,
}

impl<'pass, 'frame> RenderPassRecorder<'pass, 'frame> {
    /// Get mutable reference to the underlying wgpu render pass
    pub fn pass(&mut self) -> &mut wgpu::RenderPass<'static> {
        self.pass.as_mut().expect("pass already taken")
    }

    /// Get reference to the frame context
    pub fn frame(&self) -> &FrameContext<'frame> {
        self.frame
    }

    /// Set the render pipeline
    pub fn set_pipeline(&mut self, pipeline: &wgpu::RenderPipeline) {
        self.pass().set_pipeline(pipeline);
    }

    /// Set a bind group
    pub fn set_bind_group(
        &mut self,
        index: u32,
        bind_group: &wgpu::BindGroup,
        offsets: &[wgpu::DynamicOffset],
    ) {
        self.pass().set_bind_group(index, bind_group, offsets);
    }

    /// Set vertex buffer
    pub fn set_vertex_buffer(&mut self, slot: u32, buffer_slice: wgpu::BufferSlice<'_>) {
        self.pass().set_vertex_buffer(slot, buffer_slice);
    }

    /// Set index buffer
    pub fn set_index_buffer(
        &mut self,
        buffer_slice: wgpu::BufferSlice<'_>,
        index_format: wgpu::IndexFormat,
    ) {
        self.pass().set_index_buffer(buffer_slice, index_format);
    }

    /// Draw primitives
    pub fn draw(&mut self, vertices: std::ops::Range<u32>, instances: std::ops::Range<u32>) {
        self.pass().draw(vertices, instances);
    }

    /// Draw indexed primitives
    pub fn draw_indexed(
        &mut self,
        indices: std::ops::Range<u32>,
        base_vertex: i32,
        instances: std::ops::Range<u32>,
    ) {
        self.pass().draw_indexed(indices, base_vertex, instances);
    }

    /// Set push constants
    pub fn set_push_constants(&mut self, stages: wgpu::ShaderStages, offset: u32, data: &[u8]) {
        self.pass().set_push_constants(stages, offset, data);
    }

    /// Set viewport
    pub fn set_viewport(&mut self, x: f32, y: f32, w: f32, h: f32, min_depth: f32, max_depth: f32) {
        self.pass().set_viewport(x, y, w, h, min_depth, max_depth);
    }

    /// Set scissor rect
    pub fn set_scissor_rect(&mut self, x: u32, y: u32, width: u32, height: u32) {
        self.pass().set_scissor_rect(x, y, width, height);
    }
}

impl Drop for RenderPassRecorder<'_, '_> {
    fn drop(&mut self) {
        // Drop the pass first to end the borrow of the encoder
        self.pass.take();
    }
}
