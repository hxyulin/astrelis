//! Frame lifecycle and RAII rendering context.
//!
//! This module provides [`Frame`], which manages the lifecycle of a single
//! rendering frame using RAII patterns. The frame automatically submits GPU commands
//! and presents the surface when dropped.
//!
//! # Architecture
//!
//! The render system follows a clear ownership hierarchy:
//!
//! ```text
//! GraphicsContext (Global, Arc<Self>)
//!     └─▶ RenderWindow (Per-window, persistent)
//!             └─▶ Frame (Per-frame, temporary)
//!                     └─▶ RenderPass (Per-pass, temporary, owns encoder)
//! ```
//!
//! Key design decisions:
//! - **Each pass owns its encoder** - No encoder movement, no borrow conflicts
//! - **Frame collects command buffers** - Via `RefCell<Vec<CommandBuffer>>`
//! - **Immutable frame reference** - RenderPass takes `&'f Frame`, not `&'f mut Frame`
//! - **Atomic stats** - Thread-safe counting via `Arc<AtomicFrameStats>`
//! - **No unsafe code** - Clean ownership, no pointer casts
//!
//! # RAII Pattern
//!
//! ```rust,no_run
//! # use astrelis_render::RenderWindow;
//! # let mut window: RenderWindow = todo!();
//! // New API - each pass owns its encoder
//! let frame = window.begin_frame().expect("Surface available");
//! {
//!     let mut pass = frame.render_pass()
//!         .clear_color(astrelis_render::Color::BLACK)
//!         .clear_depth(0.0)
//!         .label("main")
//!         .build();
//!
//!     // Render commands here
//!     // pass.wgpu_pass().draw(...);
//! } // pass drops: ends pass → finishes encoder → pushes command buffer to frame
//!
//! frame.submit(); // Or let it drop - auto-submits
//! ```
//!
//! # Important
//!
//! - Render passes own their encoder and push command buffers to the frame on drop
//! - Multiple passes can be created sequentially within a frame
//! - Frame auto-submits on drop if not explicitly submitted

use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use astrelis_core::profiling::{profile_function, profile_scope};
use astrelis_winit::window::WinitWindow;

use crate::Color;
use crate::context::GraphicsContext;
use crate::framebuffer::Framebuffer;
use crate::gpu_profiling::GpuFrameProfiler;
use crate::target::RenderTarget;

/// Per-frame rendering statistics.
///
/// Tracks the number of render passes and draw calls executed during a single frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameStats {
    /// Number of render passes begun this frame.
    pub passes: usize,
    /// Total number of draw calls issued across all passes.
    pub draw_calls: usize,
}

/// Thread-safe atomic frame statistics.
///
/// Used to eliminate borrow conflicts in GPU profiling code by allowing
/// stats updates through an Arc without needing mutable access to Frame.
pub struct AtomicFrameStats {
    passes: AtomicU32,
    draw_calls: AtomicU32,
}

impl AtomicFrameStats {
    /// Create new atomic stats initialized to zero.
    pub fn new() -> Self {
        Self {
            passes: AtomicU32::new(0),
            draw_calls: AtomicU32::new(0),
        }
    }

    /// Increment the pass count.
    pub fn increment_passes(&self) {
        self.passes.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment the draw call count.
    pub fn increment_draw_calls(&self) {
        self.draw_calls.fetch_add(1, Ordering::Relaxed);
    }

    /// Get the current pass count.
    pub fn passes(&self) -> u32 {
        self.passes.load(Ordering::Relaxed)
    }

    /// Get the current draw call count.
    pub fn draw_calls(&self) -> u32 {
        self.draw_calls.load(Ordering::Relaxed)
    }

    /// Convert to non-atomic FrameStats for final reporting.
    pub fn to_frame_stats(&self) -> FrameStats {
        FrameStats {
            passes: self.passes() as usize,
            draw_calls: self.draw_calls() as usize,
        }
    }
}

impl Default for AtomicFrameStats {
    fn default() -> Self {
        Self::new()
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
    /// Get the underlying texture.
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture.texture
    }

    /// Get the texture view.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}

/// Context for a single frame of rendering.
///
/// Frame represents a single frame being rendered. It holds the acquired surface
/// texture and collects command buffers from render passes. When dropped, it
/// automatically submits all command buffers and presents the surface.
///
/// # Key Design Points
///
/// - **Immutable reference**: RenderPasses take `&Frame`, not `&mut Frame`
/// - **RefCell for command buffers**: Allows multiple passes without mutable borrow
/// - **Atomic stats**: Thread-safe pass/draw counting
/// - **RAII cleanup**: Drop handles submit and present
///
/// # Example
///
/// ```rust,no_run
/// # use astrelis_render::{RenderWindow, Color};
/// # let mut window: RenderWindow = todo!();
/// let frame = window.begin_frame().expect("Surface available");
///
/// // Create first pass
/// {
///     let mut pass = frame.render_pass()
///         .clear_color(Color::BLACK)
///         .build();
///     // Render background
/// }
///
/// // Create second pass (different encoder)
/// {
///     let mut pass = frame.render_pass()
///         .load_color()
///         .build();
///     // Render UI overlay
/// }
///
/// // Auto-submits on drop
/// ```
pub struct Frame<'w> {
    /// Reference to the window (provides graphics context, depth view, etc.)
    pub(crate) window: &'w crate::window::RenderWindow,
    /// Acquired surface texture for this frame.
    pub(crate) surface: Option<Surface>,
    /// Collected command buffers from render passes.
    pub(crate) command_buffers: RefCell<Vec<wgpu::CommandBuffer>>,
    /// Atomic stats for thread-safe counting.
    pub(crate) stats: Arc<AtomicFrameStats>,
    /// Whether submit has been called.
    pub(crate) submitted: Cell<bool>,
    /// Surface texture format.
    pub(crate) surface_format: wgpu::TextureFormat,
    /// Optional GPU profiler.
    pub(crate) gpu_profiler: Option<Arc<GpuFrameProfiler>>,
    /// Window handle for redraw requests.
    pub(crate) winit_window: Arc<WinitWindow>,
}

impl<'w> Frame<'w> {
    /// Get the surface texture view for this frame.
    ///
    /// # Panics
    /// Panics if the surface has been consumed. Use `try_surface_view()` for fallible access.
    pub fn surface_view(&self) -> &wgpu::TextureView {
        self.surface
            .as_ref()
            .expect("Surface already consumed")
            .view()
    }

    /// Try to get the surface texture view for this frame.
    pub fn try_surface_view(&self) -> Option<&wgpu::TextureView> {
        self.surface.as_ref().map(|s| s.view())
    }

    /// Get the window's depth texture view, if the window was created with depth.
    ///
    /// This provides access to the window-owned depth buffer for render passes
    /// that need depth testing.
    pub fn depth_view(&self) -> Option<&wgpu::TextureView> {
        self.window.depth_view_ref()
    }

    /// Get the surface texture format.
    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.surface_format
    }

    /// Get the frame size in physical pixels.
    pub fn size(&self) -> (u32, u32) {
        self.window.size()
    }

    /// Get the graphics context.
    pub fn graphics(&self) -> &GraphicsContext {
        self.window.graphics()
    }

    /// Get the wgpu device.
    pub fn device(&self) -> &wgpu::Device {
        self.window.graphics().device()
    }

    /// Get the wgpu queue.
    pub fn queue(&self) -> &wgpu::Queue {
        self.window.graphics().queue()
    }

    /// Get frame statistics.
    pub fn stats(&self) -> FrameStats {
        self.stats.to_frame_stats()
    }

    /// Get the atomic stats for direct access (used by RenderPass).
    pub(crate) fn atomic_stats(&self) -> &Arc<AtomicFrameStats> {
        &self.stats
    }

    /// Get the GPU profiler if attached.
    pub fn gpu_profiler(&self) -> Option<&GpuFrameProfiler> {
        self.gpu_profiler.as_deref()
    }

    /// Check if GPU profiling is active.
    pub fn has_gpu_profiler(&self) -> bool {
        self.gpu_profiler.is_some()
    }

    /// Create a command encoder for custom command recording.
    ///
    /// Use this for operations that don't fit the render pass model,
    /// like buffer copies or texture uploads.
    pub fn create_encoder(&self, label: Option<&str>) -> wgpu::CommandEncoder {
        self.device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label })
    }

    /// Add a pre-built command buffer to the frame.
    ///
    /// Use this when you have custom command recording logic.
    pub fn add_command_buffer(&self, buffer: wgpu::CommandBuffer) {
        self.command_buffers.borrow_mut().push(buffer);
    }

    /// Start building a render pass.
    ///
    /// Returns a builder that can be configured with target, clear operations,
    /// and depth settings before building the actual pass.
    pub fn render_pass(&self) -> RenderPassBuilder<'_, 'w> {
        RenderPassBuilder::new(self)
    }

    /// Start building a compute pass.
    pub fn compute_pass(&self) -> crate::compute::ComputePassBuilder<'_, 'w> {
        crate::compute::ComputePassBuilder::new(self)
    }

    /// Submit all collected command buffers and present the surface.
    ///
    /// This is called automatically on drop, but can be called explicitly
    /// for more control over timing.
    pub fn submit(self) {
        // Move self to trigger drop which handles submission
        drop(self);
    }

    /// Internal submit implementation called by Drop.
    fn submit_inner(&self) {
        profile_function!();

        if self.stats.passes() == 0 {
            tracing::warn!("No render passes were executed for this frame");
        }

        // Resolve GPU profiler queries before submitting
        if let Some(ref profiler) = self.gpu_profiler {
            // Create a dedicated encoder for query resolution
            let mut resolve_encoder = self.create_encoder(Some("Profiler Resolve"));
            profiler.resolve_queries(&mut resolve_encoder);
            self.command_buffers
                .borrow_mut()
                .push(resolve_encoder.finish());
        }

        // Take all command buffers
        let buffers = std::mem::take(&mut *self.command_buffers.borrow_mut());

        if !buffers.is_empty() {
            profile_scope!("submit_commands");
            self.queue().submit(buffers);
        }

        // Present surface
        if let Some(surface) = self.surface.as_ref() {
            profile_scope!("present_surface");
            // Note: We can't take() the surface since self is borrowed, but present
            // doesn't consume it - it just signals we're done with this frame
        }

        // End GPU profiler frame
        if let Some(ref profiler) = self.gpu_profiler
            && let Err(e) = profiler.end_frame()
        {
            tracing::warn!("GPU profiler end_frame error: {e:?}");
        }
    }

    // =========================================================================
    // Backwards Compatibility Methods
    // =========================================================================

    /// Convenience method to clear to a color and execute rendering commands.
    ///
    /// This is the most common pattern - clear the surface and render.
    ///
    /// # Deprecated
    ///
    /// Prefer using the builder pattern:
    /// ```ignore
    /// let mut pass = frame.render_pass()
    ///     .clear_color(Color::BLACK)
    ///     .build();
    /// // render
    /// ```
    #[deprecated(
        since = "0.2.0",
        note = "Use frame.render_pass().clear_color().build() instead"
    )]
    pub fn clear_and_render<F>(&self, target: RenderTarget<'_>, clear_color: Color, f: F)
    where
        F: FnOnce(&mut RenderPass<'_>),
    {
        profile_scope!("clear_and_render");
        let mut pass = self
            .render_pass()
            .target(target)
            .clear_color(clear_color)
            .label("main_pass")
            .build();
        f(&mut pass);
    }

    /// Clear the target and render with depth testing enabled.
    ///
    /// # Deprecated
    ///
    /// Prefer using the builder pattern:
    /// ```ignore
    /// let mut pass = frame.render_pass()
    ///     .clear_color(Color::BLACK)
    ///     .clear_depth(0.0)
    ///     .build();
    /// ```
    #[deprecated(
        since = "0.2.0",
        note = "Use frame.render_pass().clear_color().clear_depth().build() instead"
    )]
    pub fn clear_and_render_with_depth<'a, F>(
        &'a self,
        target: RenderTarget<'a>,
        clear_color: Color,
        depth_view: &'a wgpu::TextureView,
        depth_clear_value: f32,
        f: F,
    ) where
        F: FnOnce(&mut RenderPass<'a>),
    {
        profile_scope!("clear_and_render_with_depth");
        let mut pass = self
            .render_pass()
            .target(target)
            .clear_color(clear_color)
            .depth_attachment(depth_view)
            .clear_depth(depth_clear_value)
            .label("main_pass_with_depth")
            .build();
        f(&mut pass);
    }
}

impl Drop for Frame<'_> {
    fn drop(&mut self) {
        if !self.submitted.get() {
            self.submitted.set(true);
            self.submit_inner();
        }

        // Present surface
        if let Some(surface) = self.surface.take() {
            profile_scope!("present_surface");
            surface.texture.present();
        }

        // Request redraw
        self.winit_window.request_redraw();
    }
}

// ============================================================================
// RenderPassBuilder
// ============================================================================

/// Target for color attachment in render passes.
#[derive(Debug, Clone, Copy, Default)]
pub enum ColorTarget<'a> {
    /// Render to the window surface.
    #[default]
    Surface,
    /// Render to a custom texture view.
    Custom(&'a wgpu::TextureView),
    /// Render to a framebuffer.
    Framebuffer(&'a Framebuffer),
}

/// Color operation for render pass.
#[derive(Debug, Clone, Copy, Default)]
pub enum ColorOp {
    /// Clear to the specified color.
    Clear(wgpu::Color),
    /// Load existing contents.
    #[default]
    Load,
}

impl From<Color> for ColorOp {
    fn from(color: Color) -> Self {
        Self::Clear(color.to_wgpu())
    }
}

impl From<wgpu::Color> for ColorOp {
    fn from(color: wgpu::Color) -> Self {
        Self::Clear(color)
    }
}

/// Depth operation for render pass.
#[derive(Debug, Clone, Copy)]
pub enum DepthOp {
    /// Clear to the specified value.
    Clear(f32),
    /// Load existing values.
    Load,
    /// Read-only depth (no writes).
    ReadOnly,
}

impl Default for DepthOp {
    fn default() -> Self {
        Self::Clear(1.0)
    }
}

/// Builder for creating render passes with fluent API.
///
/// # Example
///
/// ```rust,no_run
/// # use astrelis_render::{Frame, Color};
/// # let frame: &Frame = todo!();
/// let mut pass = frame.render_pass()
///     .clear_color(Color::BLACK)
///     .clear_depth(0.0)
///     .label("main")
///     .build();
///
/// // Use pass.wgpu_pass() for rendering
/// ```
pub struct RenderPassBuilder<'f, 'w> {
    frame: &'f Frame<'w>,
    color_target: ColorTarget<'f>,
    color_op: ColorOp,
    depth_view: Option<&'f wgpu::TextureView>,
    depth_op: DepthOp,
    label: Option<String>,
}

impl<'f, 'w> RenderPassBuilder<'f, 'w> {
    /// Create a new render pass builder.
    pub(crate) fn new(frame: &'f Frame<'w>) -> Self {
        Self {
            frame,
            color_target: ColorTarget::Surface,
            color_op: ColorOp::Load,
            depth_view: None,
            depth_op: DepthOp::default(),
            label: None,
        }
    }

    /// Set the render target (for backwards compatibility).
    pub fn target(mut self, target: RenderTarget<'f>) -> Self {
        match target {
            RenderTarget::Surface => {
                self.color_target = ColorTarget::Surface;
            }
            RenderTarget::SurfaceWithDepth {
                depth_view,
                clear_value,
            } => {
                self.color_target = ColorTarget::Surface;
                self.depth_view = Some(depth_view);
                if let Some(v) = clear_value {
                    self.depth_op = DepthOp::Clear(v);
                } else {
                    self.depth_op = DepthOp::Load;
                }
            }
            RenderTarget::Framebuffer(fb) => {
                self.color_target = ColorTarget::Framebuffer(fb);
                if let Some(dv) = fb.depth_view() {
                    self.depth_view = Some(dv);
                }
            }
        }
        self
    }

    /// Render to the window surface (default).
    pub fn to_surface(mut self) -> Self {
        self.color_target = ColorTarget::Surface;
        self
    }

    /// Render to a framebuffer.
    pub fn to_framebuffer(mut self, fb: &'f Framebuffer) -> Self {
        self.color_target = ColorTarget::Framebuffer(fb);
        if let Some(dv) = fb.depth_view() {
            self.depth_view = Some(dv);
        }
        self
    }

    /// Render to a custom texture view.
    pub fn to_texture(mut self, view: &'f wgpu::TextureView) -> Self {
        self.color_target = ColorTarget::Custom(view);
        self
    }

    /// Clear the color target to the specified color.
    pub fn clear_color(mut self, color: impl Into<ColorOp>) -> Self {
        self.color_op = color.into();
        self
    }

    /// Load existing color contents (default).
    pub fn load_color(mut self) -> Self {
        self.color_op = ColorOp::Load;
        self
    }

    /// Set the depth attachment.
    pub fn depth_attachment(mut self, view: &'f wgpu::TextureView) -> Self {
        self.depth_view = Some(view);
        self
    }

    /// Use the window's depth buffer automatically.
    ///
    /// # Panics
    /// Panics if the window doesn't have a depth buffer.
    pub fn with_window_depth(mut self) -> Self {
        self.depth_view = Some(
            self.frame
                .depth_view()
                .expect("Window must have depth buffer for with_window_depth()"),
        );
        self
    }

    /// Use the window's depth buffer if available.
    pub fn with_window_depth_if_available(mut self) -> Self {
        if let Some(dv) = self.frame.depth_view() {
            self.depth_view = Some(dv);
        }
        self
    }

    /// Clear the depth buffer to the specified value.
    pub fn clear_depth(mut self, value: f32) -> Self {
        self.depth_op = DepthOp::Clear(value);
        self
    }

    /// Load existing depth values.
    pub fn load_depth(mut self) -> Self {
        self.depth_op = DepthOp::Load;
        self
    }

    /// Use depth in read-only mode (no writes).
    pub fn depth_readonly(mut self) -> Self {
        self.depth_op = DepthOp::ReadOnly;
        self
    }

    /// Set a debug label for the render pass.
    pub fn label(mut self, name: impl Into<String>) -> Self {
        self.label = Some(name.into());
        self
    }

    /// Build and return the render pass.
    ///
    /// The pass owns its encoder. When dropped, it ends the pass,
    /// finishes the encoder, and adds the command buffer to the frame.
    pub fn build(self) -> RenderPass<'f> {
        profile_function!();

        let label = self.label.clone();
        let label_str = label.as_deref();

        // Create encoder for this pass
        let encoder = self
            .frame
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: label_str });

        // Build color attachment
        let color_view = match self.color_target {
            ColorTarget::Surface => self.frame.surface_view(),
            ColorTarget::Custom(v) => v,
            ColorTarget::Framebuffer(fb) => fb.render_view(),
        };

        let color_ops = match self.color_op {
            ColorOp::Clear(color) => wgpu::Operations {
                load: wgpu::LoadOp::Clear(color),
                store: wgpu::StoreOp::Store,
            },
            ColorOp::Load => wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        };

        let resolve_target = match self.color_target {
            ColorTarget::Framebuffer(fb) => fb.resolve_target(),
            _ => None,
        };

        let color_attachments = [Some(wgpu::RenderPassColorAttachment {
            view: color_view,
            resolve_target,
            ops: color_ops,
            depth_slice: None,
        })];

        // Build depth attachment
        let depth_attachment = self.depth_view.map(|view| {
            let (depth_ops, read_only) = match self.depth_op {
                DepthOp::Clear(value) => (
                    Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(value),
                        store: wgpu::StoreOp::Store,
                    }),
                    false,
                ),
                DepthOp::Load => (
                    Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    false,
                ),
                DepthOp::ReadOnly => (
                    Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Discard,
                    }),
                    true,
                ),
            };

            wgpu::RenderPassDepthStencilAttachment {
                view,
                depth_ops: if read_only { None } else { depth_ops },
                stencil_ops: None,
            }
        });

        // Increment pass count
        self.frame.stats.increment_passes();

        // Create the wgpu render pass
        // We need to keep encoder alive, so we create pass from a separate borrowed encoder
        let mut encoder = encoder;
        let pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: label_str,
                color_attachments: &color_attachments,
                depth_stencil_attachment: depth_attachment,
                timestamp_writes: None,
                occlusion_query_set: None,
            })
            .forget_lifetime();

        RenderPass {
            frame: self.frame,
            encoder: Some(encoder),
            pass: Some(pass),
            stats: self.frame.stats.clone(),
            #[cfg(feature = "gpu-profiling")]
            profiler_scope: None,
        }
    }
}

// ============================================================================
// RenderPass
// ============================================================================

/// A render pass that owns its encoder.
///
/// When dropped, the render pass:
/// 1. Ends the wgpu render pass
/// 2. Finishes the command encoder
/// 3. Pushes the command buffer to the frame
///
/// This design eliminates encoder movement and borrow conflicts.
pub struct RenderPass<'f> {
    /// Reference to the frame (for pushing command buffer on drop).
    frame: &'f Frame<'f>,
    /// The command encoder (owned by this pass).
    encoder: Option<wgpu::CommandEncoder>,
    /// The active wgpu render pass.
    pass: Option<wgpu::RenderPass<'static>>,
    /// Atomic stats for draw call counting.
    stats: Arc<AtomicFrameStats>,
    /// GPU profiler scope (when gpu-profiling feature is enabled).
    #[cfg(feature = "gpu-profiling")]
    profiler_scope: Option<wgpu_profiler::scope::OwningScope>,
}

impl<'f> RenderPass<'f> {
    /// Get the underlying wgpu RenderPass (mutable).
    ///
    /// # Panics
    /// Panics if the render pass has already been consumed.
    pub fn wgpu_pass(&mut self) -> &mut wgpu::RenderPass<'static> {
        self.pass.as_mut().expect("RenderPass already consumed")
    }

    /// Get the underlying wgpu RenderPass (immutable).
    ///
    /// # Panics
    /// Panics if the render pass has already been consumed.
    pub fn wgpu_pass_ref(&self) -> &wgpu::RenderPass<'static> {
        self.pass.as_ref().expect("RenderPass already consumed")
    }

    /// Try to get the underlying wgpu RenderPass.
    pub fn try_wgpu_pass(&mut self) -> Option<&mut wgpu::RenderPass<'static>> {
        self.pass.as_mut()
    }

    /// Check if the render pass is still valid.
    pub fn is_valid(&self) -> bool {
        self.pass.is_some()
    }

    /// Get raw access to the pass (alias for wgpu_pass).
    pub fn raw_pass(&mut self) -> &mut wgpu::RenderPass<'static> {
        self.wgpu_pass()
    }

    /// Get the command encoder.
    pub fn encoder(&self) -> Option<&wgpu::CommandEncoder> {
        self.encoder.as_ref()
    }

    /// Get mutable access to the command encoder.
    pub fn encoder_mut(&mut self) -> Option<&mut wgpu::CommandEncoder> {
        self.encoder.as_mut()
    }

    /// Get the graphics context.
    pub fn graphics(&self) -> &GraphicsContext {
        self.frame.graphics()
    }

    /// Record a draw call for statistics.
    pub fn record_draw_call(&self) {
        self.stats.increment_draw_calls();
    }

    /// Consume the pass early and return the encoder for further use.
    ///
    /// This ends the render pass but allows the encoder to be used
    /// for additional commands before submission.
    pub fn into_encoder(mut self) -> wgpu::CommandEncoder {
        // End the render pass
        drop(self.pass.take());

        // Take and return the encoder (skip normal Drop logic)
        self.encoder.take().expect("Encoder already taken")
    }

    /// Finish the render pass (called automatically on drop).
    pub fn finish(self) {
        drop(self);
    }

    // =========================================================================
    // Viewport/Scissor Methods
    // =========================================================================

    /// Set the viewport using physical coordinates.
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

    /// Set the viewport using logical coordinates.
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
    pub fn set_scissor_physical(&mut self, rect: astrelis_core::geometry::PhysicalRect<u32>) {
        self.wgpu_pass()
            .set_scissor_rect(rect.x, rect.y, rect.width, rect.height);
    }

    /// Set the scissor rectangle using logical coordinates.
    pub fn set_scissor_logical(
        &mut self,
        rect: astrelis_core::geometry::LogicalRect<f32>,
        scale: astrelis_core::geometry::ScaleFactor,
    ) {
        let physical = rect.to_physical(scale);
        self.set_scissor_physical(physical);
    }

    // =========================================================================
    // Drawing Methods
    // =========================================================================

    /// Set the pipeline.
    pub fn set_pipeline(&mut self, pipeline: &wgpu::RenderPipeline) {
        self.wgpu_pass().set_pipeline(pipeline);
    }

    /// Set a bind group.
    pub fn set_bind_group(&mut self, index: u32, bind_group: &wgpu::BindGroup, offsets: &[u32]) {
        self.wgpu_pass().set_bind_group(index, bind_group, offsets);
    }

    /// Set a vertex buffer.
    pub fn set_vertex_buffer(&mut self, slot: u32, buffer_slice: wgpu::BufferSlice<'_>) {
        self.wgpu_pass().set_vertex_buffer(slot, buffer_slice);
    }

    /// Set the index buffer.
    pub fn set_index_buffer(
        &mut self,
        buffer_slice: wgpu::BufferSlice<'_>,
        format: wgpu::IndexFormat,
    ) {
        self.wgpu_pass().set_index_buffer(buffer_slice, format);
    }

    /// Draw primitives.
    pub fn draw(&mut self, vertices: std::ops::Range<u32>, instances: std::ops::Range<u32>) {
        self.wgpu_pass().draw(vertices, instances);
        self.stats.increment_draw_calls();
    }

    /// Draw indexed primitives.
    pub fn draw_indexed(
        &mut self,
        indices: std::ops::Range<u32>,
        base_vertex: i32,
        instances: std::ops::Range<u32>,
    ) {
        self.wgpu_pass()
            .draw_indexed(indices, base_vertex, instances);
        self.stats.increment_draw_calls();
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

    /// Set push constants.
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
    pub fn set_push_constants_raw(&mut self, stages: wgpu::ShaderStages, offset: u32, data: &[u8]) {
        self.wgpu_pass().set_push_constants(stages, offset, data);
    }
}

impl Drop for RenderPass<'_> {
    fn drop(&mut self) {
        profile_function!();

        // Drop GPU profiler scope first (ends timing)
        #[cfg(feature = "gpu-profiling")]
        drop(self.profiler_scope.take());

        // End the render pass
        drop(self.pass.take());

        // Finish encoder and push command buffer to frame
        if let Some(encoder) = self.encoder.take() {
            let command_buffer = encoder.finish();
            self.frame.command_buffers.borrow_mut().push(command_buffer);
        }
    }
}

// ============================================================================
// Backwards Compatibility Types
// ============================================================================

/// Clear operation for a render pass.
#[derive(Debug, Clone, Copy, Default)]
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

impl From<Color> for ClearOp {
    fn from(color: Color) -> Self {
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

// ============================================================================
// Legacy Compatibility
// ============================================================================

/// Deprecated alias for backwards compatibility.
#[deprecated(since = "0.2.0", note = "Use Frame instead")]
pub type FrameContext = Frame<'static>;
