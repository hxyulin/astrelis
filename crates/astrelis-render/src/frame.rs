//! Frame lifecycle and RAII rendering context.
//!
//! This module provides [`FrameContext`], which manages the lifecycle of a single
//! rendering frame using RAII patterns. The frame automatically submits GPU commands
//! and presents the surface when dropped.
//!
//! # RAII Pattern
//!
//! ```rust,no_run
//! # use astrelis_render::RenderableWindow;
//! # use astrelis_winit::window::WindowBackend;
//! # let mut renderable_window: RenderableWindow = todo!();
//! {
//!     let mut frame = renderable_window.begin_drawing();
//!
//!     // Render passes must be dropped before frame.finish()
//!     frame.clear_and_render(
//!         astrelis_render::RenderTarget::Surface,
//!         astrelis_render::Color::BLACK,
//!         |pass| {
//!             // Rendering commands
//!             // Pass is automatically dropped here
//!         },
//!     );
//!
//!     frame.finish(); // Submits commands and presents surface
//! } // FrameContext drops here if .finish() not called
//! ```
//!
//! # Important
//!
//! - Render passes MUST be dropped before calling `frame.finish()`
//! - Use `clear_and_render()` for automatic pass scoping
//! - Forgetting `frame.finish()` will still submit via Drop, but explicitly calling it is recommended

use std::sync::Arc;

use astrelis_core::profiling::{profile_function, profile_scope};
use astrelis_winit::window::WinitWindow;

use crate::context::GraphicsContext;
use crate::gpu_profiling::GpuFrameProfiler;
use crate::target::RenderTarget;

/// Per-frame rendering statistics.
///
/// Tracks the number of render passes and draw calls executed during a single frame.
pub struct FrameStats {
    /// Number of render passes begun this frame.
    pub passes: usize,
    /// Total number of draw calls issued across all passes.
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

/// The acquired surface texture and its view for the current frame.
///
/// Wraps a [`wgpu::SurfaceTexture`] together with a pre-created
/// [`wgpu::TextureView`] so that render passes can bind it directly.
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
///
/// When a [`GpuFrameProfiler`] is attached (via [`RenderableWindow::set_gpu_profiler`]),
/// GPU profiling scopes are automatically created around render passes in
/// [`with_pass`](Self::with_pass) and [`clear_and_render`](Self::clear_and_render).
/// Queries are resolved and the profiler frame is ended in the `Drop` implementation.
pub struct FrameContext {
    pub(crate) stats: FrameStats,
    pub(crate) surface: Option<Surface>,
    pub(crate) encoder: Option<wgpu::CommandEncoder>,
    pub(crate) context: Arc<GraphicsContext>,
    pub(crate) window: Arc<WinitWindow>,
    pub(crate) surface_format: wgpu::TextureFormat,
    /// Optional GPU profiler for automatic render pass profiling.
    pub(crate) gpu_profiler: Option<Arc<GpuFrameProfiler>>,
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

    /// Get a cloneable Arc reference to the graphics context.
    pub fn graphics_context_arc(&self) -> &Arc<GraphicsContext> {
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

    /// Get direct access to the command encoder (immutable).
    pub fn encoder_ref(&self) -> Option<&wgpu::CommandEncoder> {
        self.encoder.as_ref()
    }

    /// Get mutable access to the command encoder.
    pub fn encoder_mut(&mut self) -> Option<&mut wgpu::CommandEncoder> {
        self.encoder.as_mut()
    }

    /// Get the surface texture view for this frame.
    ///
    /// # Panics
    /// Panics if the surface has been consumed. Use `try_surface_view()` for fallible access.
    pub fn surface_view(&self) -> &wgpu::TextureView {
        self.surface().view()
    }

    /// Try to get the surface texture view for this frame.
    pub fn try_surface_view(&self) -> Option<&wgpu::TextureView> {
        self.try_surface().map(|s| s.view())
    }

    /// Get the surface texture for this frame.
    ///
    /// # Panics
    /// Panics if the surface has been consumed. Use `try_surface_texture()` for fallible access.
    pub fn surface_texture(&self) -> &wgpu::Texture {
        self.surface().texture()
    }

    /// Try to get the surface texture for this frame.
    pub fn try_surface_texture(&self) -> Option<&wgpu::Texture> {
        self.try_surface().map(|s| s.texture())
    }

    pub fn finish(self) {
        drop(self);
    }

    /// Get the GPU profiler attached to this frame, if any.
    pub fn gpu_profiler(&self) -> Option<&GpuFrameProfiler> {
        self.gpu_profiler.as_deref()
    }

    /// Check if GPU profiling is active for this frame.
    pub fn has_gpu_profiler(&self) -> bool {
        self.gpu_profiler.is_some()
    }

    /// Execute a closure with a render pass, automatically handling scoping.
    ///
    /// This is the ergonomic RAII pattern that eliminates the need for manual `{ }` blocks.
    /// The render pass is automatically dropped after the closure completes.
    ///
    /// When a GPU profiler is attached to this frame (via [`RenderableWindow::set_gpu_profiler`]),
    /// a GPU profiling scope is automatically created around the render pass, using the
    /// pass label (or `"render_pass"` as default) as the scope name.
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
        profile_scope!("with_pass");

        #[cfg(feature = "gpu-profiling")]
        {
            if self.gpu_profiler.is_some() {
                self.with_pass_profiled_inner(builder, f);
                return;
            }
        }

        let mut pass = builder.build(self);
        f(&mut pass);
        // pass drops here automatically
    }

    /// Internal: execute a render pass with GPU profiling scope.
    ///
    /// This method creates a GPU timing scope around the render pass using
    /// the profiler attached to this frame. The encoder is temporarily moved
    /// out, wrapped in a profiling scope, and returned after the closure completes.
    ///
    /// # Safety rationale for the unsafe block:
    ///
    /// The `RenderPass` created here borrows `self` (for draw call counting),
    /// but the encoder is held by the profiling scope, not by the `RenderPass`.
    /// After the closure and scope are dropped, we need to write the encoder back
    /// to `self.encoder`. The borrow checker cannot see that the `RenderPass` is
    /// fully dropped before the write, so we use a raw pointer to reborrow `self`.
    /// This is safe because:
    /// 1. The `RenderPass` has `encoder: None` - it never touches `self.encoder`
    /// 2. The `pass` (wgpu_pass) is taken before the `RenderPass` is dropped
    /// 3. The `RenderPass::Drop` returns early when `encoder` is `None`
    /// 4. The encoder write happens strictly after all borrows are released
    #[cfg(feature = "gpu-profiling")]
    fn with_pass_profiled_inner<'a, F>(&'a mut self, builder: RenderPassBuilder<'a>, f: F)
    where
        F: FnOnce(&mut RenderPass<'a>),
    {
        let label = builder.label_or("render_pass").to_string();
        let profiler = self.gpu_profiler.clone().unwrap();
        let mut encoder = self.encoder.take().expect("Encoder already taken");

        // Build attachments (borrows self immutably for surface view access).
        // We must finish using the attachments before mutating self.
        let (all_attachments, depth_attachment) = builder.build_attachments(self);

        {
            let mut scope = profiler.scope(&label, &mut encoder);

            let descriptor = wgpu::RenderPassDescriptor {
                label: Some(&label),
                color_attachments: &all_attachments,
                depth_stencil_attachment: depth_attachment,
                occlusion_query_set: None,
                timestamp_writes: None,
            };

            let wgpu_pass = scope.begin_render_pass(&descriptor).forget_lifetime();

            // SAFETY: We need to create a RenderPass that borrows self for 'a
            // (for draw call counting via frame.increment_draw_calls()), but
            // we also need to reassign self.encoder after the scope drops.
            // The RenderPass created here has encoder=None, so its Drop impl
            // will NOT write to self.encoder. We use a raw pointer to get a
            // second mutable reference, which is safe because:
            // - The RenderPass only accesses self.stats (via increment_draw_calls)
            // - The encoder reassignment only accesses self.encoder
            // - These are disjoint fields
            // - The RenderPass is fully dropped before the encoder reassignment
            let self_ptr = self as *mut FrameContext;
            let frame_ref: &'a mut FrameContext = unsafe { &mut *self_ptr };
            frame_ref.stats.passes += 1;

            let mut pass = RenderPass {
                frame: frame_ref,
                encoder: None, // encoder held by scope
                pass: Some(wgpu_pass),
            };

            f(&mut pass);

            // End the render pass before the scope closes.
            pass.pass.take();
            // RenderPass is dropped here - its Drop impl skips encoder return (encoder is None)
        }
        // scope dropped here -- end GPU timestamp

        // SAFETY: All borrows from the RenderPass and scope are now released.
        // The attachments from build_attachments are also dropped.
        self.encoder = Some(encoder);
    }

    /// Convenience method to clear to a color and execute rendering commands.
    ///
    /// This is the most common pattern - clear the surface and render.
    /// When a GPU profiler is attached, a scope named `"main_pass"` is automatically
    /// created around the render pass.
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
    ///         // Example: ui.render(pass.wgpu_pass());
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
        profile_scope!("clear_and_render");
        self.with_pass(
            RenderPassBuilder::new()
                .label("main_pass")
                .target(target)
                .clear_color(clear_color.into()),
            f,
        );
    }

    /// Clear the target and render with depth testing enabled.
    ///
    /// This is the same as `clear_and_render` but also attaches a depth buffer
    /// and clears it to the specified value (typically 0.0 for reverse-Z depth).
    ///
    /// # Arguments
    /// - `target`: The render target (Surface or Framebuffer)
    /// - `clear_color`: The color to clear to
    /// - `depth_view`: The depth texture view for depth testing
    /// - `depth_clear_value`: The value to clear the depth buffer to (0.0 for reverse-Z)
    /// - `f`: The closure to execute rendering commands
    ///
    /// # Example
    ///
    /// ```ignore
    /// let ui_depth_view = ui_renderer.depth_view();
    /// frame.clear_and_render_with_depth(
    ///     RenderTarget::Surface,
    ///     Color::BLACK,
    ///     ui_depth_view,
    ///     0.0, // Clear to 0.0 for reverse-Z
    ///     |pass| {
    ///         ui_system.render(pass.wgpu_pass());
    ///     },
    /// );
    /// ```
    pub fn clear_and_render_with_depth<'a, F>(
        &'a mut self,
        target: RenderTarget<'a>,
        clear_color: impl Into<crate::Color>,
        depth_view: &'a wgpu::TextureView,
        depth_clear_value: f32,
        f: F,
    ) where
        F: FnOnce(&mut RenderPass<'a>),
    {
        profile_scope!("clear_and_render_with_depth");
        self.with_pass(
            RenderPassBuilder::new()
                .label("main_pass_with_depth")
                .target(target)
                .clear_color(clear_color.into())
                .depth_stencil_attachment(
                    depth_view,
                    Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(depth_clear_value),
                        store: wgpu::StoreOp::Store,
                    }),
                    None,
                ),
            f,
        );
    }

    /// Create a render pass that clears to the given color.
    pub fn clear_pass<'a>(
        &'a mut self,
        target: RenderTarget<'a>,
        clear_color: wgpu::Color,
    ) -> RenderPass<'a> {
        RenderPassBuilder::new()
            .target(target)
            .clear_color(clear_color)
            .build(self)
    }

    /// Create a render pass that loads existing content.
    pub fn load_pass<'a>(&'a mut self, target: RenderTarget<'a>) -> RenderPass<'a> {
        RenderPassBuilder::new().target(target).build(self)
    }

    /// Execute a closure with a GPU profiling scope on the command encoder.
    ///
    /// If no GPU profiler is attached, the closure is called directly with the encoder.
    /// When a profiler is present, a GPU timing scope with the given label wraps the closure.
    ///
    /// This is useful for profiling non-render-pass work like buffer copies, texture uploads,
    /// or compute dispatches that happen outside of `with_pass()`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// frame.with_gpu_scope("upload_data", |encoder| {
    ///     encoder.copy_buffer_to_buffer(&src, 0, &dst, 0, size);
    /// });
    /// ```
    #[cfg(feature = "gpu-profiling")]
    pub fn with_gpu_scope<F>(&mut self, label: &str, f: F)
    where
        F: FnOnce(&mut wgpu::CommandEncoder),
    {
        if let Some(profiler) = self.gpu_profiler.clone() {
            let mut encoder = self.encoder.take().expect("Encoder already taken");
            {
                let mut scope = profiler.scope(label, &mut encoder);
                f(&mut scope);
            }
            self.encoder = Some(encoder);
        } else {
            f(self.encoder());
        }
    }

    /// Execute a closure with a GPU profiling scope on the command encoder.
    ///
    /// When `gpu-profiling` feature is disabled, this simply calls the closure with the encoder.
    #[cfg(not(feature = "gpu-profiling"))]
    pub fn with_gpu_scope<F>(&mut self, _label: &str, f: F)
    where
        F: FnOnce(&mut wgpu::CommandEncoder),
    {
        f(self.encoder());
    }
}

impl Drop for FrameContext {
    fn drop(&mut self) {
        profile_function!();

        if self.stats.passes == 0 {
            tracing::error!("No render passes were executed for this frame!");
            return;
        }

        // Resolve GPU profiler queries before submitting commands
        if let Some(ref profiler) = self.gpu_profiler
            && let Some(encoder) = self.encoder.as_mut() {
                profiler.resolve_queries(encoder);
            }

        if let Some(encoder) = self.encoder.take() {
            {
                profile_scope!("submit_commands");
                self.context.queue().submit(std::iter::once(encoder.finish()));
            }
        }

        if let Some(surface) = self.surface.take() {
            profile_scope!("present_surface");
            surface.texture.present();
        }

        // End GPU profiler frame (after submit, before next frame)
        if let Some(ref profiler) = self.gpu_profiler
            && let Err(e) = profiler.end_frame() {
                tracing::warn!("GPU profiler end_frame error: {e:?}");
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
    // GPU profiling support
    #[cfg(feature = "gpu-profiling")]
    timestamp_writes: Option<wgpu::RenderPassTimestampWrites<'a>>,
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
            #[cfg(feature = "gpu-profiling")]
            timestamp_writes: None,
        }
    }

    /// Set a debug label for the render pass.
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Get the label, or a default fallback.
    #[allow(dead_code)]
    pub(crate) fn label_or<'b>(&'b self, default: &'b str) -> &'b str {
        self.label.unwrap_or(default)
    }

    /// Set timestamp writes for GPU profiling.
    #[cfg(feature = "gpu-profiling")]
    #[allow(dead_code)]
    pub(crate) fn timestamp_writes(mut self, tw: wgpu::RenderPassTimestampWrites<'a>) -> Self {
        self.timestamp_writes = Some(tw);
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

    /// Build the color and depth attachments without creating the render pass.
    ///
    /// Used internally by `build()` and by `with_pass_profiled_inner()` to build
    /// attachments before creating the GPU profiling scope.
    pub(crate) fn build_attachments(
        &self,
        frame_context: &'a FrameContext,
    ) -> (
        Vec<Option<wgpu::RenderPassColorAttachment<'a>>>,
        Option<wgpu::RenderPassDepthStencilAttachment<'a>>,
    ) {
        let mut all_attachments = Vec::new();

        if let Some(target) = &self.target {
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
            if let Some((ops, resolve_target)) = self.surface_attachment_ops {
                let surface_view = frame_context.surface().view();
                all_attachments.push(Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target,
                    ops,
                    depth_slice: None,
                }));
            }
            all_attachments.extend(self.color_attachments.iter().cloned());
        }

        let depth_attachment = if let Some(ref attachment) = self.depth_stencil_attachment {
            Some(attachment.clone())
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

        (all_attachments, depth_attachment)
    }

    /// Builds the render pass and begins it on the provided frame context.
    ///
    /// This takes ownership of the CommandEncoder from the FrameContext, and releases it
    /// back to the FrameContext when the RenderPass is dropped or [`finish`](RenderPass::finish)
    /// is called.
    pub fn build(self, frame_context: &'a mut FrameContext) -> RenderPass<'a> {
        profile_function!();
        let mut encoder = frame_context.encoder.take().unwrap();

        let (all_attachments, depth_attachment) = self.build_attachments(frame_context);

        #[cfg(feature = "gpu-profiling")]
        let ts_writes = self.timestamp_writes;
        #[cfg(not(feature = "gpu-profiling"))]
        let ts_writes: Option<wgpu::RenderPassTimestampWrites<'_>> = None;

        let descriptor = wgpu::RenderPassDescriptor {
            label: self.label,
            color_attachments: &all_attachments,
            depth_stencil_attachment: depth_attachment,
            occlusion_query_set: None,
            timestamp_writes: ts_writes,
        };

        let render_pass = encoder.begin_render_pass(&descriptor).forget_lifetime();

        frame_context.increment_passes();

        RenderPass {
            frame: frame_context,
            encoder: Some(encoder),
            pass: Some(render_pass),
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
    pub(crate) frame: &'a mut FrameContext,
    pub(crate) encoder: Option<wgpu::CommandEncoder>,
    pub(crate) pass: Option<wgpu::RenderPass<'static>>,
}

impl<'a> RenderPass<'a> {
    /// Get the underlying wgpu RenderPass.
    ///
    /// # Panics
    /// Panics if the render pass has already been consumed (dropped or finished).
    /// Use `try_wgpu_pass()` for fallible access.
    pub fn wgpu_pass(&mut self) -> &mut wgpu::RenderPass<'static> {
        self.pass.as_mut()
            .expect("RenderPass already consumed - ensure it wasn't dropped or finished early")
    }

    /// Try to get the underlying wgpu RenderPass.
    ///
    /// Returns `None` if the render pass has already been consumed.
    pub fn try_wgpu_pass(&mut self) -> Option<&mut wgpu::RenderPass<'static>> {
        self.pass.as_mut()
    }

    /// Check if the render pass is still valid and can be used.
    pub fn is_valid(&self) -> bool {
        self.pass.is_some()
    }

    /// Get raw access to the underlying wgpu render pass.
    pub fn raw_pass(&mut self) -> &mut wgpu::RenderPass<'static> {
        self.pass.as_mut().unwrap()
    }

    /// Get the graphics context.
    pub fn graphics_context(&self) -> &GraphicsContext {
        &self.frame.context
    }

    /// Get the frame context.
    pub fn frame_context(&self) -> &FrameContext {
        self.frame
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
        self.wgpu_pass().set_viewport(
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
        self.wgpu_pass().set_viewport(
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
        self.wgpu_pass()
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
        self.wgpu_pass().set_pipeline(pipeline);
    }

    /// Set a bind group for this render pass.
    pub fn set_bind_group(
        &mut self,
        index: u32,
        bind_group: &'a wgpu::BindGroup,
        offsets: &[u32],
    ) {
        self.wgpu_pass().set_bind_group(index, bind_group, offsets);
    }

    /// Set the vertex buffer for this render pass.
    pub fn set_vertex_buffer(&mut self, slot: u32, buffer_slice: wgpu::BufferSlice<'a>) {
        self.wgpu_pass().set_vertex_buffer(slot, buffer_slice);
    }

    /// Set the index buffer for this render pass.
    pub fn set_index_buffer(&mut self, buffer_slice: wgpu::BufferSlice<'a>, format: wgpu::IndexFormat) {
        self.wgpu_pass().set_index_buffer(buffer_slice, format);
    }

    /// Draw primitives.
    pub fn draw(&mut self, vertices: std::ops::Range<u32>, instances: std::ops::Range<u32>) {
        self.wgpu_pass().draw(vertices, instances);
        self.frame.increment_draw_calls();
    }

    /// Draw indexed primitives.
    pub fn draw_indexed(
        &mut self,
        indices: std::ops::Range<u32>,
        base_vertex: i32,
        instances: std::ops::Range<u32>,
    ) {
        self.wgpu_pass().draw_indexed(indices, base_vertex, instances);
        self.frame.increment_draw_calls();
    }

    /// Insert a debug marker.
    pub fn insert_debug_marker(&mut self, label: &str) {
        self.wgpu_pass().insert_debug_marker(label);
    }

    /// Push a debug group.
    pub fn push_debug_group(&mut self, label: &str) {
        self.wgpu_pass().push_debug_group(label);
    }

    /// Pop a debug group.
    pub fn pop_debug_group(&mut self) {
        self.wgpu_pass().pop_debug_group();
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
        self.wgpu_pass()
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
        self.wgpu_pass().set_push_constants(stages, offset, data);
    }
}

impl Drop for RenderPass<'_> {
    fn drop(&mut self) {
        profile_function!();

        drop(self.pass.take());

        // Return the encoder to the frame context.
        // When used within a GPU profiling scope (with_pass_profiled_inner),
        // encoder is None because the encoder is held by the profiling scope — skip in that case.
        if let Some(encoder) = self.encoder.take() {
            self.frame.encoder = Some(encoder);
        }
    }
}

