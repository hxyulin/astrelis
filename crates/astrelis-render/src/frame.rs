use std::sync::Arc;

use astrelis_core::profiling::{profile_function, profile_scope};
use astrelis_winit::window::WinitWindow;

use crate::context::GraphicsContext;
use crate::target::RenderTarget;

/// Statistics for a rendered frame.
pub struct FrameStats {
    pub passes: usize,
    pub draw_calls: usize,
}

impl FrameStats {
    pub(crate) fn new() -> Self {
        Self {
            passes: 0,
            draw_calls: 0,
        }
    }
}

/// Surface texture and view for rendering.
pub struct Surface {
    pub(crate) texture: wgpu::SurfaceTexture,
    pub(crate) view: wgpu::TextureView,
}

impl Surface {
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture.texture
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}

/// Context for a single frame of rendering.
pub struct FrameContext {
    pub(crate) stats: FrameStats,
    pub(crate) surface: Option<Surface>,
    pub(crate) encoder: Option<wgpu::CommandEncoder>,
    pub context: Arc<GraphicsContext>,
    pub(crate) window: Arc<WinitWindow>,
    pub(crate) surface_format: wgpu::TextureFormat,
}

impl FrameContext {
    /// Get the surface for this frame.
    ///
    /// # Panics
    /// Panics if the surface has already been consumed. Use `try_surface()` for fallible access.
    pub fn surface(&self) -> &Surface {
        self.surface.as_ref().expect("Surface already consumed or not acquired")
    }

    /// Try to get the surface for this frame.
    ///
    /// Returns `None` if the surface has already been consumed.
    pub fn try_surface(&self) -> Option<&Surface> {
        self.surface.as_ref()
    }

    /// Check if the surface is available.
    pub fn has_surface(&self) -> bool {
        self.surface.is_some()
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.surface_format
    }

    pub fn increment_passes(&mut self) {
        self.stats.passes += 1;
    }

    pub fn increment_draw_calls(&mut self) {
        self.stats.draw_calls += 1;
    }

    pub fn stats(&self) -> &FrameStats {
        &self.stats
    }

    pub fn graphics_context(&self) -> &GraphicsContext {
        &self.context
    }

    /// Get the command encoder for this frame.
    ///
    /// # Panics
    /// Panics if the encoder has already been taken. Use `try_encoder()` for fallible access.
    pub fn encoder(&mut self) -> &mut wgpu::CommandEncoder {
        self.encoder.as_mut().expect("Encoder already taken")
    }

    /// Try to get the command encoder for this frame.
    ///
    /// Returns `None` if the encoder has already been taken.
    pub fn try_encoder(&mut self) -> Option<&mut wgpu::CommandEncoder> {
        self.encoder.as_mut()
    }

    /// Check if the encoder is available.
    pub fn has_encoder(&self) -> bool {
        self.encoder.is_some()
    }

    /// Get the encoder and surface together.
    ///
    /// # Panics
    /// Panics if either the encoder or surface has been consumed.
    /// Use `try_encoder_and_surface()` for fallible access.
    pub fn encoder_and_surface(&mut self) -> (&mut wgpu::CommandEncoder, &Surface) {
        (
            self.encoder.as_mut().expect("Encoder already taken"),
            self.surface.as_ref().expect("Surface already consumed"),
        )
    }

    /// Try to get the encoder and surface together.
    ///
    /// Returns `None` if either has been consumed.
    pub fn try_encoder_and_surface(&mut self) -> Option<(&mut wgpu::CommandEncoder, &Surface)> {
        match (self.encoder.as_mut(), self.surface.as_ref()) {
            (Some(encoder), Some(surface)) => Some((encoder, surface)),
            _ => None,
        }
    }

    pub fn finish(self) {
        drop(self);
    }

    /// Execute a closure with a render pass, automatically handling scoping.
    ///
    /// This is the ergonomic RAII pattern that eliminates the need for manual `{ }` blocks.
    /// The render pass is automatically dropped after the closure completes.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use astrelis_render::*;
    /// # let mut frame: FrameContext = todo!();
    /// frame.with_pass(
    ///     RenderPassBuilder::new()
    ///         .target(RenderTarget::Surface)
    ///         .clear_color(Color::BLACK),
    ///     |pass| {
    ///         // Render commands here
    ///         // pass automatically drops when closure ends
    ///     }
    /// );
    /// frame.finish();
    /// ```
    pub fn with_pass<'a, F>(&'a mut self, builder: RenderPassBuilder<'a>, f: F)
    where
        F: FnOnce(&mut RenderPass<'a>),
    {
        let mut pass = builder.build(self);
        f(&mut pass);
        // pass drops here automatically
    }

    /// Convenience method to clear to a color and execute rendering commands.
    ///
    /// This is the most common pattern - clear the surface and render.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use astrelis_render::*;
    /// # let mut frame: FrameContext = todo!();
    /// frame.clear_and_render(
    ///     RenderTarget::Surface,
    ///     Color::BLACK,
    ///     |pass| {
    ///         // Render your content here
    ///         // Example: ui.render(pass.descriptor());
    ///     }
    /// );
    /// frame.finish();
    /// ```
    pub fn clear_and_render<'a, F>(
        &'a mut self,
        target: RenderTarget<'a>,
        clear_color: impl Into<crate::Color>,
        f: F,
    ) where
        F: FnOnce(&mut RenderPass<'a>),
    {
        self.with_pass(
            RenderPassBuilder::new()
                .target(target)
                .clear_color(clear_color.into()),
            f,
        );
    }
}

impl Drop for FrameContext {
    fn drop(&mut self) {
        profile_function!();

        if self.stats.passes == 0 {
            tracing::error!("No render passes were executed for this frame!");
            return;
        }

        if let Some(encoder) = self.encoder.take() {
            profile_scope!("submit_commands");
            self.context.queue.submit(std::iter::once(encoder.finish()));
        }

        if let Some(surface) = self.surface.take() {
            profile_scope!("present_surface");
            surface.texture.present();
        }

        // Request redraw for next frame
        self.window.request_redraw();
    }
}

/// Clear operation for a render pass.
#[derive(Debug, Clone, Copy)]
#[derive(Default)]
pub enum ClearOp {
    /// Load existing contents (no clear).
    #[default]
    Load,
    /// Clear to the specified color.
    Clear(wgpu::Color),
}


impl From<wgpu::Color> for ClearOp {
    fn from(color: wgpu::Color) -> Self {
        ClearOp::Clear(color)
    }
}

impl From<crate::Color> for ClearOp {
    fn from(color: crate::Color) -> Self {
        ClearOp::Clear(color.to_wgpu())
    }
}

/// Depth clear operation for a render pass.
#[derive(Debug, Clone, Copy)]
pub enum DepthClearOp {
    /// Load existing depth values.
    Load,
    /// Clear to the specified depth value (typically 1.0).
    Clear(f32),
}

impl Default for DepthClearOp {
    fn default() -> Self {
        DepthClearOp::Clear(1.0)
    }
}

/// Builder for creating render passes.
pub struct RenderPassBuilder<'a> {
    label: Option<&'a str>,
    // New simplified API
    target: Option<RenderTarget<'a>>,
    clear_op: ClearOp,
    depth_clear_op: DepthClearOp,
    // Legacy API for advanced use
    color_attachments: Vec<Option<wgpu::RenderPassColorAttachment<'a>>>,
    surface_attachment_ops: Option<(wgpu::Operations<wgpu::Color>, Option<&'a wgpu::TextureView>)>,
    depth_stencil_attachment: Option<wgpu::RenderPassDepthStencilAttachment<'a>>,
}

impl<'a> RenderPassBuilder<'a> {
    pub fn new() -> Self {
        Self {
            label: None,
            target: None,
            clear_op: ClearOp::Load,
            depth_clear_op: DepthClearOp::default(),
            color_attachments: Vec::new(),
            surface_attachment_ops: None,
            depth_stencil_attachment: None,
        }
    }

    /// Set a debug label for the render pass.
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Set the render target (Surface or Framebuffer).
    ///
    /// This is the simplified API - use this instead of manual color_attachment calls.
    pub fn target(mut self, target: RenderTarget<'a>) -> Self {
        self.target = Some(target);
        self
    }

    /// Set clear color for the render target.
    ///
    /// Pass a wgpu::Color or use ClearOp::Load to preserve existing contents.
    pub fn clear_color(mut self, color: impl Into<ClearOp>) -> Self {
        self.clear_op = color.into();
        self
    }

    /// Set depth clear operation.
    pub fn clear_depth(mut self, depth: f32) -> Self {
        self.depth_clear_op = DepthClearOp::Clear(depth);
        self
    }

    /// Load existing depth values instead of clearing.
    pub fn load_depth(mut self) -> Self {
        self.depth_clear_op = DepthClearOp::Load;
        self
    }

    // Legacy API for advanced use cases

    /// Add a color attachment manually (advanced API).
    ///
    /// For most cases, use `.target()` instead.
    pub fn color_attachment(
        mut self,
        view: Option<&'a wgpu::TextureView>,
        resolve_target: Option<&'a wgpu::TextureView>,
        ops: wgpu::Operations<wgpu::Color>,
    ) -> Self {
        if let Some(view) = view {
            self.color_attachments
                .push(Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target,
                    ops,
                    depth_slice: None,
                }));
        } else {
            // Store ops for later - will be filled with surface view in build()
            self.surface_attachment_ops = Some((ops, resolve_target));
        }
        self
    }

    /// Add a depth-stencil attachment manually (advanced API).
    ///
    /// For framebuffers with depth, the depth attachment is handled automatically
    /// when using `.target()`.
    pub fn depth_stencil_attachment(
        mut self,
        view: &'a wgpu::TextureView,
        depth_ops: Option<wgpu::Operations<f32>>,
        stencil_ops: Option<wgpu::Operations<u32>>,
    ) -> Self {
        self.depth_stencil_attachment = Some(wgpu::RenderPassDepthStencilAttachment {
            view,
            depth_ops,
            stencil_ops,
        });
        self
    }

    /// Builds the render pass and begins it on the provided frame context.
    ///
    /// This takes ownership of the CommandEncoder from the FrameContext, and releases it
    /// back to the FrameContext when the RenderPass is dropped or [`finish`](RenderPass::finish)
    /// is called.
    pub fn build(self, frame_context: &'a mut FrameContext) -> RenderPass<'a> {
        let mut encoder = frame_context.encoder.take().unwrap();

        // Build color attachments based on target or legacy API
        let mut all_attachments = Vec::new();

        if let Some(target) = &self.target {
            // New simplified API
            let color_ops = match self.clear_op {
                ClearOp::Load => wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                ClearOp::Clear(color) => wgpu::Operations {
                    load: wgpu::LoadOp::Clear(color),
                    store: wgpu::StoreOp::Store,
                },
            };

            match target {
                RenderTarget::Surface => {
                    let surface_view = frame_context.surface().view();
                    all_attachments.push(Some(wgpu::RenderPassColorAttachment {
                        view: surface_view,
                        resolve_target: None,
                        ops: color_ops,
                        depth_slice: None,
                    }));
                }
                RenderTarget::Framebuffer(fb) => {
                    all_attachments.push(Some(wgpu::RenderPassColorAttachment {
                        view: fb.render_view(),
                        resolve_target: fb.resolve_target(),
                        ops: color_ops,
                        depth_slice: None,
                    }));
                }
            }
        } else {
            // Legacy API
            if let Some((ops, resolve_target)) = self.surface_attachment_ops {
                let surface_view = frame_context.surface().view();
                all_attachments.push(Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target,
                    ops,
                    depth_slice: None,
                }));
            }
            all_attachments.extend(self.color_attachments);
        }

        // Build depth attachment
        let depth_attachment = if let Some(attachment) = self.depth_stencil_attachment {
            Some(attachment)
        } else if let Some(RenderTarget::Framebuffer(fb)) = &self.target {
            fb.depth_view().map(|view| {
                let depth_ops = match self.depth_clear_op {
                    DepthClearOp::Load => wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    DepthClearOp::Clear(depth) => wgpu::Operations {
                        load: wgpu::LoadOp::Clear(depth),
                        store: wgpu::StoreOp::Store,
                    },
                };
                wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(depth_ops),
                    stencil_ops: None,
                }
            })
        } else {
            None
        };

        let descriptor = wgpu::RenderPassDescriptor {
            label: self.label,
            color_attachments: &all_attachments,
            depth_stencil_attachment: depth_attachment,
            occlusion_query_set: None,
            timestamp_writes: None,
        };

        let render_pass = encoder.begin_render_pass(&descriptor).forget_lifetime();

        frame_context.increment_passes();

        RenderPass {
            context: frame_context,
            encoder: Some(encoder),
            descriptor: Some(render_pass),
        }
    }
}

impl Default for RenderPassBuilder<'_> {
    fn default() -> Self {
        Self::new()
    }
}

/// A render pass wrapper that automatically returns the encoder to the frame context.
pub struct RenderPass<'a> {
    pub context: &'a mut FrameContext,
    pub(crate) encoder: Option<wgpu::CommandEncoder>,
    pub(crate) descriptor: Option<wgpu::RenderPass<'static>>,
}

impl<'a> RenderPass<'a> {
    /// Get the underlying wgpu RenderPass descriptor.
    ///
    /// # Panics
    /// Panics if the render pass has already been consumed (dropped or finished).
    /// Use `try_descriptor()` for fallible access.
    pub fn descriptor(&mut self) -> &mut wgpu::RenderPass<'static> {
        self.descriptor.as_mut()
            .expect("RenderPass already consumed - ensure it wasn't dropped or finished early")
    }

    /// Try to get the underlying wgpu RenderPass descriptor.
    ///
    /// Returns `None` if the render pass has already been consumed.
    pub fn try_descriptor(&mut self) -> Option<&mut wgpu::RenderPass<'static>> {
        self.descriptor.as_mut()
    }

    /// Check if the render pass is still valid and can be used.
    pub fn is_valid(&self) -> bool {
        self.descriptor.is_some()
    }

    pub fn finish(self) {
        drop(self);
    }

    // =========================================================================
    // Viewport/Scissor Methods
    // =========================================================================

    /// Set the viewport using physical coordinates.
    ///
    /// The viewport defines the transformation from normalized device coordinates
    /// to window coordinates.
    ///
    /// # Arguments
    ///
    /// * `rect` - The viewport rectangle in physical (pixel) coordinates
    /// * `min_depth` - Minimum depth value (typically 0.0)
    /// * `max_depth` - Maximum depth value (typically 1.0)
    pub fn set_viewport_physical(
        &mut self,
        rect: astrelis_core::geometry::PhysicalRect<f32>,
        min_depth: f32,
        max_depth: f32,
    ) {
        self.descriptor().set_viewport(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            min_depth,
            max_depth,
        );
    }

    /// Set the viewport using logical coordinates (converts with scale factor).
    ///
    /// # Arguments
    ///
    /// * `rect` - The viewport rectangle in logical coordinates
    /// * `min_depth` - Minimum depth value (typically 0.0)
    /// * `max_depth` - Maximum depth value (typically 1.0)
    /// * `scale` - Scale factor for logical to physical conversion
    pub fn set_viewport_logical(
        &mut self,
        rect: astrelis_core::geometry::LogicalRect<f32>,
        min_depth: f32,
        max_depth: f32,
        scale: astrelis_core::geometry::ScaleFactor,
    ) {
        let physical = rect.to_physical_f32(scale);
        self.set_viewport_physical(physical, min_depth, max_depth);
    }

    /// Set the viewport from a Viewport struct.
    ///
    /// Uses the viewport's position and size, with depth range 0.0 to 1.0.
    pub fn set_viewport(&mut self, viewport: &crate::Viewport) {
        self.descriptor().set_viewport(
            viewport.position.x,
            viewport.position.y,
            viewport.size.width,
            viewport.size.height,
            0.0,
            1.0,
        );
    }

    /// Set the scissor rectangle using physical coordinates.
    ///
    /// The scissor rectangle defines the area of the render target that
    /// can be modified by drawing commands.
    ///
    /// # Arguments
    ///
    /// * `rect` - The scissor rectangle in physical (pixel) coordinates
    pub fn set_scissor_physical(&mut self, rect: astrelis_core::geometry::PhysicalRect<u32>) {
        self.descriptor()
            .set_scissor_rect(rect.x, rect.y, rect.width, rect.height);
    }

    /// Set the scissor rectangle using logical coordinates.
    ///
    /// # Arguments
    ///
    /// * `rect` - The scissor rectangle in logical coordinates
    /// * `scale` - Scale factor for logical to physical conversion
    pub fn set_scissor_logical(
        &mut self,
        rect: astrelis_core::geometry::LogicalRect<f32>,
        scale: astrelis_core::geometry::ScaleFactor,
    ) {
        let physical = rect.to_physical(scale);
        self.set_scissor_physical(physical);
    }

    // =========================================================================
    // Convenience Drawing Methods
    // =========================================================================

    /// Set the pipeline for this render pass.
    pub fn set_pipeline(&mut self, pipeline: &'a wgpu::RenderPipeline) {
        self.descriptor().set_pipeline(pipeline);
    }

    /// Set a bind group for this render pass.
    pub fn set_bind_group(
        &mut self,
        index: u32,
        bind_group: &'a wgpu::BindGroup,
        offsets: &[u32],
    ) {
        self.descriptor().set_bind_group(index, bind_group, offsets);
    }

    /// Set the vertex buffer for this render pass.
    pub fn set_vertex_buffer(&mut self, slot: u32, buffer_slice: wgpu::BufferSlice<'a>) {
        self.descriptor().set_vertex_buffer(slot, buffer_slice);
    }

    /// Set the index buffer for this render pass.
    pub fn set_index_buffer(&mut self, buffer_slice: wgpu::BufferSlice<'a>, format: wgpu::IndexFormat) {
        self.descriptor().set_index_buffer(buffer_slice, format);
    }

    /// Draw primitives.
    pub fn draw(&mut self, vertices: std::ops::Range<u32>, instances: std::ops::Range<u32>) {
        self.descriptor().draw(vertices, instances);
        self.context.increment_draw_calls();
    }

    /// Draw indexed primitives.
    pub fn draw_indexed(
        &mut self,
        indices: std::ops::Range<u32>,
        base_vertex: i32,
        instances: std::ops::Range<u32>,
    ) {
        self.descriptor().draw_indexed(indices, base_vertex, instances);
        self.context.increment_draw_calls();
    }

    /// Insert a debug marker.
    pub fn insert_debug_marker(&mut self, label: &str) {
        self.descriptor().insert_debug_marker(label);
    }

    /// Push a debug group.
    pub fn push_debug_group(&mut self, label: &str) {
        self.descriptor().push_debug_group(label);
    }

    /// Pop a debug group.
    pub fn pop_debug_group(&mut self) {
        self.descriptor().pop_debug_group();
    }

    // =========================================================================
    // Push Constants
    // =========================================================================

    /// Set push constants for a range of shader stages.
    ///
    /// Push constants are a fast way to pass small amounts of data to shaders
    /// without the overhead of buffer updates. They are limited in size
    /// (typically 128-256 bytes depending on the GPU).
    ///
    /// **Requires the `PUSH_CONSTANTS` feature to be enabled.**
    ///
    /// # Arguments
    ///
    /// * `stages` - Which shader stages can access this data
    /// * `offset` - Byte offset within the push constant range
    /// * `data` - The data to set (must be Pod)
    ///
    /// # Example
    ///
    /// ```ignore
    /// #[repr(C)]
    /// #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    /// struct PushConstants {
    ///     transform: [[f32; 4]; 4],
    ///     color: [f32; 4],
    /// }
    ///
    /// let constants = PushConstants {
    ///     transform: /* ... */,
    ///     color: [1.0, 0.0, 0.0, 1.0],
    /// };
    ///
    /// pass.set_push_constants(
    ///     wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
    ///     0,
    ///     &constants,
    /// );
    /// ```
    pub fn set_push_constants<T: bytemuck::Pod>(
        &mut self,
        stages: wgpu::ShaderStages,
        offset: u32,
        data: &T,
    ) {
        self.descriptor()
            .set_push_constants(stages, offset, bytemuck::bytes_of(data));
    }

    /// Set push constants from raw bytes.
    ///
    /// Use this when you need more control over the data layout.
    pub fn set_push_constants_raw(
        &mut self,
        stages: wgpu::ShaderStages,
        offset: u32,
        data: &[u8],
    ) {
        self.descriptor().set_push_constants(stages, offset, data);
    }
}

impl Drop for RenderPass<'_> {
    fn drop(&mut self) {
        profile_function!();

        drop(self.descriptor.take());

        // Return the encoder to the frame context
        self.context.encoder = self.encoder.take();
    }
}

/// Helper trait for creating render passes with common configurations.
pub trait RenderPassExt {
    /// Create a render pass that clears to the given color.
    fn clear_pass<'a>(
        &'a mut self,
        target: RenderTarget<'a>,
        clear_color: wgpu::Color,
    ) -> RenderPass<'a>;

    /// Create a render pass that loads existing content.
    fn load_pass<'a>(&'a mut self, target: RenderTarget<'a>) -> RenderPass<'a>;
}

impl RenderPassExt for FrameContext {
    fn clear_pass<'a>(
        &'a mut self,
        target: RenderTarget<'a>,
        clear_color: wgpu::Color,
    ) -> RenderPass<'a> {
        RenderPassBuilder::new()
            .target(target)
            .clear_color(clear_color)
            .build(self)
    }

    fn load_pass<'a>(&'a mut self, target: RenderTarget<'a>) -> RenderPass<'a> {
        RenderPassBuilder::new().target(target).build(self)
    }
}
