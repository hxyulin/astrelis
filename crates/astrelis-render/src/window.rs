//! Window and surface management for rendering.
//!
//! This module provides [`RenderableWindow`], which wraps a [`Window`] and manages
//! its GPU surface for rendering. It handles surface configuration, frame presentation,
//! and surface loss recovery.
//!
//! # Lifecycle
//!
//! 1. Create with [`RenderableWindow::new()`] passing a window and graphics context
//! 2. Call [`begin_drawing()`](RenderableWindow::begin_drawing) to start a frame
//! 3. Use the returned [`FrameContext`] for rendering
//! 4. Drop the frame context to submit commands and present
//!
//! # Example
//!
//! ```rust,no_run
//! use astrelis_render::{GraphicsContext, RenderableWindow, Color};
//! use astrelis_winit::window::WindowBackend;
//! # use std::sync::Arc;
//!
//! # fn example(window: astrelis_winit::window::Window, graphics: Arc<GraphicsContext>) {
//! let mut renderable = RenderableWindow::new(window, graphics)
//!     .expect("Failed to create renderable window");
//!
//! // In render loop:
//! let mut frame = renderable.begin_drawing();
//! frame.clear_and_render(
//!     astrelis_render::RenderTarget::Surface,
//!     Color::BLACK,
//!     |pass| {
//!         // Rendering commands
//!     },
//! );
//! frame.finish();
//! # }
//! ```
//!
//! # Surface Loss
//!
//! The surface can be lost due to window minimization, GPU driver resets, or other
//! platform events. [`RenderableWindow`] handles this automatically by recreating
//! the surface when [`begin_drawing()`](RenderableWindow::begin_drawing) is called.

use astrelis_core::{
    geometry::{LogicalSize, PhysicalPosition, PhysicalSize, ScaleFactor},
    profiling::profile_function,
};
use astrelis_winit::{
    WindowId,
    window::{Window, WindowBackend},
};
use std::sync::Arc;

use crate::{
    context::{GraphicsContext, GraphicsError},
    frame::{FrameContext, FrameStats, Surface},
    gpu_profiling::GpuFrameProfiler,
};

/// Viewport definition for rendering.
///
/// A viewport represents the renderable area of a window in physical coordinates,
/// along with the scale factor for coordinate conversions.
#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    /// Position in physical coordinates (pixels).
    pub position: PhysicalPosition<f32>,
    /// Size in physical coordinates (pixels).
    pub size: PhysicalSize<f32>,
    /// Scale factor for logical/physical conversion.
    pub scale_factor: ScaleFactor,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            position: PhysicalPosition::new(0.0, 0.0),
            size: PhysicalSize::new(800.0, 600.0),
            // it needs to be 1.0 to avoid division by zero and other issues
            scale_factor: ScaleFactor(1.0),
        }
    }
}

impl Viewport {
    /// Create a new viewport with the given physical size and scale factor.
    pub fn new(width: f32, height: f32, scale_factor: ScaleFactor) -> Self {
        Self {
            position: PhysicalPosition::new(0.0, 0.0),
            size: PhysicalSize::new(width, height),
            scale_factor,
        }
    }

    /// Create a viewport from physical size.
    pub fn from_physical_size(size: PhysicalSize<u32>, scale_factor: ScaleFactor) -> Self {
        Self {
            position: PhysicalPosition::new(0.0, 0.0),
            size: PhysicalSize::new(size.width as f32, size.height as f32),
            scale_factor,
        }
    }

    /// Check if the viewport is valid (has positive dimensions).
    pub fn is_valid(&self) -> bool {
        self.size.width > 0.0 && self.size.height > 0.0 && self.scale_factor.0 > 0.0
    }

    /// Get the size in logical pixels.
    pub fn to_logical(&self) -> LogicalSize<f32> {
        self.size.to_logical(self.scale_factor)
    }

    /// Get the width in physical pixels.
    pub fn width(&self) -> f32 {
        self.size.width
    }

    /// Get the height in physical pixels.
    pub fn height(&self) -> f32 {
        self.size.height
    }

    /// Get the x position in physical pixels.
    pub fn x(&self) -> f32 {
        self.position.x
    }

    /// Get the y position in physical pixels.
    pub fn y(&self) -> f32 {
        self.position.y
    }
}

/// Descriptor for configuring a window's rendering context.
#[derive(Default)]
pub struct WindowContextDescriptor {
    /// The surface texture format. If None, uses the default format for the surface.
    pub format: Option<wgpu::TextureFormat>,
    /// Present mode for the surface.
    pub present_mode: Option<wgpu::PresentMode>,
    /// Alpha mode for the surface.
    pub alpha_mode: Option<wgpu::CompositeAlphaMode>,
}

pub(crate) struct PendingReconfigure {
    pub(crate) resize: Option<PhysicalSize<u32>>,
}

impl PendingReconfigure {
    const fn new() -> Self {
        Self { resize: None }
    }
}

/// Manages a wgpu [`Surface`](wgpu::Surface) and its configuration for a single window.
///
/// Handles surface creation, reconfiguration on resize, and frame acquisition.
/// Most users should interact with [`RenderableWindow`] instead, which wraps
/// this type and adds convenience methods.
pub struct WindowContext {
    pub(crate) window: Window,
    pub(crate) context: Arc<GraphicsContext>,
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) reconfigure: PendingReconfigure,
}

impl WindowContext {
    pub fn new(
        window: Window,
        context: Arc<GraphicsContext>,
        descriptor: WindowContextDescriptor,
    ) -> Result<Self, GraphicsError> {
        profile_function!();
        let scale_factor = window.scale_factor();
        let logical_size = window.logical_size();
        let physical_size = logical_size.to_physical(scale_factor);

        let surface = context
            .instance()
            .create_surface(window.window.clone())
            .map_err(|e| GraphicsError::SurfaceCreationFailed(e.to_string()))?;

        let mut config = surface
            .get_default_config(context.adapter(), physical_size.width, physical_size.height)
            .ok_or_else(|| GraphicsError::SurfaceConfigurationFailed(
                "No suitable surface configuration found".to_string()
            ))?;

        if let Some(format) = descriptor.format {
            config.format = format;
        }
        if let Some(present_mode) = descriptor.present_mode {
            config.present_mode = present_mode;
        }
        if let Some(alpha_mode) = descriptor.alpha_mode {
            config.alpha_mode = alpha_mode;
        }

        surface.configure(context.device(), &config);

        Ok(Self {
            window,
            surface,
            config,
            reconfigure: PendingReconfigure::new(),
            context,
        })
    }

    /// Handle window resize event (logical size).
    pub fn resized(&mut self, new_size: LogicalSize<u32>) {
        let scale_factor = self.window.scale_factor();
        let physical_size = new_size.to_physical(scale_factor);
        self.reconfigure.resize = Some(physical_size);
    }

    /// Handle window resize event (physical size).
    pub fn resized_physical(&mut self, new_size: PhysicalSize<u32>) {
        self.reconfigure.resize = Some(new_size);
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn graphics_context(&self) -> &GraphicsContext {
        &self.context
    }

    pub fn surface(&self) -> &wgpu::Surface<'static> {
        &self.surface
    }

    pub fn surface_config(&self) -> &wgpu::SurfaceConfiguration {
        &self.config
    }

    /// Get the surface texture format.
    ///
    /// This is the format that render pipelines must use when rendering to this
    /// window's surface. Pass this to renderer constructors like
    /// [`LineRenderer::new`](crate::LineRenderer::new).
    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// Get the logical size of the window.
    pub fn logical_size(&self) -> LogicalSize<u32> {
        self.window.logical_size()
    }

    /// Get the physical size of the window.
    pub fn physical_size(&self) -> PhysicalSize<u32> {
        self.window.physical_size()
    }

    /// Get the logical size as f32.
    pub fn logical_size_f32(&self) -> LogicalSize<f32> {
        let size = self.logical_size();
        LogicalSize::new(size.width as f32, size.height as f32)
    }

    /// Get the physical size as f32.
    pub fn physical_size_f32(&self) -> PhysicalSize<f32> {
        let size = self.physical_size();
        PhysicalSize::new(size.width as f32, size.height as f32)
    }

    /// Reconfigure the surface with a new configuration.
    pub fn reconfigure_surface(&mut self, config: wgpu::SurfaceConfiguration) {
        self.config = config;
        self.surface.configure(self.context.device(), &self.config);
    }
}

impl WindowContext {
    /// Try to acquire a surface texture, handling recoverable errors by reconfiguring.
    ///
    /// This method will attempt to reconfigure the surface if it's lost or outdated,
    /// providing automatic recovery for common scenarios like window minimize/restore.
    fn try_acquire_surface_texture(&mut self) -> Result<wgpu::SurfaceTexture, GraphicsError> {
        // First attempt
        match self.surface.get_current_texture() {
            Ok(frame) => return Ok(frame),
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                // Surface needs reconfiguration - try to recover
                tracing::debug!("Surface lost/outdated, reconfiguring...");
                self.surface.configure(self.context.device(), &self.config);
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                return Err(GraphicsError::SurfaceOutOfMemory);
            }
            Err(wgpu::SurfaceError::Timeout) => {
                return Err(GraphicsError::SurfaceTimeout);
            }
            Err(e) => {
                return Err(GraphicsError::SurfaceTextureAcquisitionFailed(e.to_string()));
            }
        }

        // Second attempt after reconfiguration
        match self.surface.get_current_texture() {
            Ok(frame) => Ok(frame),
            Err(wgpu::SurfaceError::Lost) => Err(GraphicsError::SurfaceLost),
            Err(wgpu::SurfaceError::Outdated) => Err(GraphicsError::SurfaceOutdated),
            Err(wgpu::SurfaceError::OutOfMemory) => Err(GraphicsError::SurfaceOutOfMemory),
            Err(wgpu::SurfaceError::Timeout) => Err(GraphicsError::SurfaceTimeout),
            Err(e) => Err(GraphicsError::SurfaceTextureAcquisitionFailed(e.to_string())),
        }
    }
}

impl WindowContext {
    /// Begin drawing a frame, optionally with a GPU profiler attached.
    ///
    /// This is the internal implementation used by both `WindowBackend::try_begin_drawing`
    /// and `RenderableWindow::begin_drawing`.
    pub(crate) fn try_begin_drawing_with_profiler(
        &mut self,
        gpu_profiler: Option<Arc<GpuFrameProfiler>>,
    ) -> Result<FrameContext, GraphicsError> {
        profile_function!();

        let mut configure_needed = false;
        if let Some(new_size) = self.reconfigure.resize.take() {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            configure_needed = true;
        }

        if configure_needed {
            self.surface.configure(self.context.device(), &self.config);
        }

        let frame = self.try_acquire_surface_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let encoder = self
            .context
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Frame Encoder"),
            });

        Ok(FrameContext {
            surface: Some(Surface {
                texture: frame,
                view,
            }),
            encoder: Some(encoder),
            context: self.context.clone(),
            stats: FrameStats::new(),
            window: self.window.window.clone(),
            surface_format: self.config.format,
            gpu_profiler,
        })
    }
}

impl WindowBackend for WindowContext {
    type FrameContext = FrameContext;
    type Error = GraphicsError;

    fn try_begin_drawing(&mut self) -> Result<Self::FrameContext, Self::Error> {
        self.try_begin_drawing_with_profiler(None)
    }
}

/// A renderable window that combines a winit [`Window`] with a [`WindowContext`].
///
/// This is the primary type for rendering to a window. It implements
/// `Deref<Target = WindowContext>`, so all `WindowContext` methods are
/// available directly.
///
/// # GPU Profiling
///
/// Attach a [`GpuFrameProfiler`] via [`set_gpu_profiler`](Self::set_gpu_profiler)
/// to automatically profile render passes. Once attached, all frames created via
/// [`begin_drawing`](WindowBackend::begin_drawing) will include GPU profiling:
/// - Render passes in `with_pass()` / `clear_and_render()` get automatic GPU scopes
/// - Queries are resolved and the profiler frame is ended in `FrameContext::Drop`
pub struct RenderableWindow {
    pub(crate) context: WindowContext,
    pub(crate) gpu_profiler: Option<Arc<GpuFrameProfiler>>,
}

impl RenderableWindow {
    pub fn new(window: Window, context: Arc<GraphicsContext>) -> Result<Self, GraphicsError> {
        Self::new_with_descriptor(window, context, WindowContextDescriptor::default())
    }

    pub fn new_with_descriptor(
        window: Window,
        context: Arc<GraphicsContext>,
        descriptor: WindowContextDescriptor,
    ) -> Result<Self, GraphicsError> {
        profile_function!();
        let context = WindowContext::new(window, context, descriptor)?;
        Ok(Self {
            context,
            gpu_profiler: None,
        })
    }

    pub fn id(&self) -> WindowId {
        self.context.window.id()
    }

    pub fn window(&self) -> &Window {
        &self.context.window
    }

    pub fn context(&self) -> &WindowContext {
        &self.context
    }

    pub fn context_mut(&mut self) -> &mut WindowContext {
        &mut self.context
    }

    /// Get the surface texture format.
    ///
    /// This is the format that render pipelines must use when rendering to this
    /// window's surface. Pass this to renderer constructors like
    /// [`LineRenderer::new`](crate::LineRenderer::new).
    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.context.surface_format()
    }

    /// Handle window resize event (logical size).
    pub fn resized(&mut self, new_size: LogicalSize<u32>) {
        self.context.resized(new_size);
    }

    /// Handle window resize event (physical size).
    pub fn resized_physical(&mut self, new_size: PhysicalSize<u32>) {
        self.context.resized_physical(new_size);
    }

    /// Get the physical size of the window.
    pub fn physical_size(&self) -> PhysicalSize<u32> {
        self.context.physical_size()
    }

    /// Get the scale factor.
    pub fn scale_factor(&self) -> ScaleFactor {
        self.window().scale_factor()
    }

    /// Get the viewport for this window.
    pub fn viewport(&self) -> Viewport {
        let physical_size = self.physical_size();
        let scale_factor = self.scale_factor();

        Viewport {
            position: PhysicalPosition::new(0.0, 0.0),
            size: PhysicalSize::new(physical_size.width as f32, physical_size.height as f32),
            scale_factor,
        }
    }

    /// Attach a GPU profiler to this window for automatic render pass profiling.
    ///
    /// Once set, all frames created via [`begin_drawing`](WindowBackend::begin_drawing)
    /// will automatically:
    /// - Create GPU profiling scopes around render passes
    /// - Resolve timestamp queries before command submission
    /// - End the profiler frame after queue submit
    ///
    /// # Example
    ///
    /// ```ignore
    /// let profiler = Arc::new(GpuFrameProfiler::new(&graphics_ctx)?);
    /// window.set_gpu_profiler(profiler);
    ///
    /// // Now all frames are automatically profiled:
    /// let mut frame = window.begin_drawing();
    /// frame.clear_and_render(RenderTarget::Surface, Color::BLACK, |pass| {
    ///     // GPU scope "main_pass" is automatically active
    /// });
    /// frame.finish(); // auto: resolve_queries -> submit -> end_frame
    /// ```
    pub fn set_gpu_profiler(&mut self, profiler: Arc<GpuFrameProfiler>) {
        self.gpu_profiler = Some(profiler);
    }

    /// Remove the GPU profiler from this window.
    ///
    /// Returns the profiler if one was attached.
    pub fn remove_gpu_profiler(&mut self) -> Option<Arc<GpuFrameProfiler>> {
        self.gpu_profiler.take()
    }

    /// Get a reference to the GPU profiler, if one is attached.
    pub fn gpu_profiler(&self) -> Option<&Arc<GpuFrameProfiler>> {
        self.gpu_profiler.as_ref()
    }
}

impl std::ops::Deref for RenderableWindow {
    type Target = WindowContext;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}

impl std::ops::DerefMut for RenderableWindow {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.context
    }
}

impl WindowBackend for RenderableWindow {
    type FrameContext = FrameContext;
    type Error = GraphicsError;

    fn try_begin_drawing(&mut self) -> Result<Self::FrameContext, Self::Error> {
        self.context
            .try_begin_drawing_with_profiler(self.gpu_profiler.clone())
    }
}
