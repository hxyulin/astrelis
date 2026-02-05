//! Window and surface management for rendering.
//!
//! This module provides [`RenderWindow`], which wraps a [`Window`] and manages
//! its GPU surface for rendering. It handles surface configuration, frame presentation,
//! and surface loss recovery.
//!
//! # Lifecycle
//!
//! 1. Create with [`RenderWindow::builder()`] or [`RenderWindow::new()`]
//! 2. Call [`begin_frame()`](RenderWindow::begin_frame) to start a frame
//! 3. Use the returned [`Frame`] for rendering
//! 4. Drop the frame to submit commands and present
//!
//! # Example
//!
//! ```rust,no_run
//! use astrelis_render::{GraphicsContext, RenderWindow, Color};
//! use astrelis_winit::window::Window;
//! # use std::sync::Arc;
//!
//! # fn example(window: Window, graphics: Arc<GraphicsContext>) {
//! let mut window = RenderWindow::builder()
//!     .with_depth_default()
//!     .build(window, graphics)
//!     .expect("Failed to create render window");
//!
//! // In render loop:
//! if let Some(frame) = window.begin_frame() {
//!     let mut pass = frame.render_pass()
//!         .clear_color(Color::BLACK)
//!         .with_window_depth()
//!         .clear_depth(0.0)
//!         .build();
//!     // Render commands...
//! } // Frame auto-submits and presents on drop
//! # }
//! ```
//!
//! # Surface Loss
//!
//! The surface can be lost due to window minimization, GPU driver resets, or other
//! platform events. [`RenderWindow`] handles this automatically by recreating
//! the surface when [`begin_frame()`](RenderWindow::begin_frame) is called.

use astrelis_core::{
    geometry::{LogicalSize, PhysicalPosition, PhysicalSize, ScaleFactor},
    profiling::profile_function,
};
use astrelis_winit::{
    WindowId,
    window::{Window, WindowBackend},
};
use std::cell::{Cell, RefCell};
use std::sync::Arc;

use crate::{
    context::{GraphicsContext, GraphicsError},
    depth::{DEFAULT_DEPTH_FORMAT, DepthTexture},
    frame::{AtomicFrameStats, Frame, Surface},
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
    /// Whether to create a depth texture for this window.
    ///
    /// When enabled, the window will maintain an auto-resizing depth texture
    /// that can be accessed via [`Frame::depth_view()`].
    pub with_depth: bool,
    /// The depth texture format. Defaults to `Depth32Float` if not specified.
    ///
    /// Only used when `with_depth` is `true`.
    pub depth_format: Option<wgpu::TextureFormat>,
}

pub(crate) struct PendingReconfigure {
    pub(crate) resize: Option<PhysicalSize<u32>>,
}

impl PendingReconfigure {
    const fn new() -> Self {
        Self { resize: None }
    }
}

/// Internal surface management for a window.
struct WindowSurface {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
}

/// Manages a wgpu [`Surface`](wgpu::Surface) and its configuration for a single window.
///
/// Handles surface creation, reconfiguration on resize, and frame acquisition.
/// Most users should interact with [`RenderWindow`] instead, which wraps
/// this type and adds convenience methods.
pub struct WindowContext {
    pub(crate) window: Window,
    pub(crate) context: Arc<GraphicsContext>,
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) reconfigure: PendingReconfigure,
    /// Optional depth texture for this window, auto-resizes with window.
    pub(crate) depth_texture: Option<DepthTexture>,
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
            .ok_or_else(|| {
                GraphicsError::SurfaceConfigurationFailed(
                    "No suitable surface configuration found".to_string(),
                )
            })?;

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

        // Create depth texture if requested
        let depth_texture = if descriptor.with_depth {
            let depth_format = descriptor.depth_format.unwrap_or(DEFAULT_DEPTH_FORMAT);
            Some(DepthTexture::with_label(
                context.device(),
                physical_size.width,
                physical_size.height,
                depth_format,
                "Window Depth Texture",
            ))
        } else {
            None
        };

        Ok(Self {
            window,
            surface,
            config,
            reconfigure: PendingReconfigure::new(),
            context,
            depth_texture,
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

        // Resize depth texture if present
        if let Some(ref mut depth) = self.depth_texture
            && depth.needs_resize(new_size.width, new_size.height)
        {
            depth.resize(self.context.device(), new_size.width, new_size.height);
        }
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

    /// Check if this window has a depth texture.
    pub fn has_depth(&self) -> bool {
        self.depth_texture.is_some()
    }

    /// Get the depth texture view if available.
    ///
    /// Returns an Arc-wrapped view for cheap, lifetime-free sharing.
    pub fn depth_view(&self) -> Option<std::sync::Arc<wgpu::TextureView>> {
        self.depth_texture.as_ref().map(|d| d.view())
    }

    /// Get the depth texture format, if depth is enabled.
    ///
    /// Returns the format used for the depth texture, or None if depth is not enabled.
    /// Use this to configure renderers that need to match the depth format.
    pub fn depth_format(&self) -> Option<wgpu::TextureFormat> {
        self.depth_texture.as_ref().map(|d| d.format())
    }

    /// Ensure a depth texture exists for this window.
    ///
    /// If no depth texture exists, creates one with the given format.
    /// If a depth texture already exists, this is a no-op.
    pub fn ensure_depth(&mut self, format: wgpu::TextureFormat) {
        if self.depth_texture.is_none() {
            self.depth_texture = Some(DepthTexture::with_label(
                self.context.device(),
                self.config.width,
                self.config.height,
                format,
                "Window Depth Texture",
            ));
        }
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
                return Err(GraphicsError::SurfaceTextureAcquisitionFailed(
                    e.to_string(),
                ));
            }
        }

        // Second attempt after reconfiguration
        match self.surface.get_current_texture() {
            Ok(frame) => Ok(frame),
            Err(wgpu::SurfaceError::Lost) => Err(GraphicsError::SurfaceLost),
            Err(wgpu::SurfaceError::Outdated) => Err(GraphicsError::SurfaceOutdated),
            Err(wgpu::SurfaceError::OutOfMemory) => Err(GraphicsError::SurfaceOutOfMemory),
            Err(wgpu::SurfaceError::Timeout) => Err(GraphicsError::SurfaceTimeout),
            Err(e) => Err(GraphicsError::SurfaceTextureAcquisitionFailed(
                e.to_string(),
            )),
        }
    }
}

// ============================================================================
// RenderWindow - Main window type
// ============================================================================

/// Builder for configuring a [`RenderWindow`].
///
/// # Example
///
/// ```rust,no_run
/// # use astrelis_render::{RenderWindow, GraphicsContext};
/// # use astrelis_winit::window::Window;
/// # use std::sync::Arc;
/// # let window: Window = todo!();
/// # let graphics: Arc<GraphicsContext> = todo!();
/// let render_window = RenderWindow::builder()
///     .present_mode(wgpu::PresentMode::Fifo)
///     .with_depth_default()
///     .with_profiling(true)
///     .build(window, graphics)
///     .expect("Failed to create window");
/// ```
#[derive(Default)]
pub struct RenderWindowBuilder {
    present_mode: Option<wgpu::PresentMode>,
    color_format: Option<wgpu::TextureFormat>,
    alpha_mode: Option<wgpu::CompositeAlphaMode>,
    depth_format: Option<wgpu::TextureFormat>,
    enable_profiling: bool,
}

impl RenderWindowBuilder {
    /// Create a new builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the present mode for vsync behavior.
    pub fn present_mode(mut self, mode: wgpu::PresentMode) -> Self {
        self.present_mode = Some(mode);
        self
    }

    /// Set the color format for the surface.
    pub fn color_format(mut self, format: wgpu::TextureFormat) -> Self {
        self.color_format = Some(format);
        self
    }

    /// Set the alpha compositing mode.
    pub fn alpha_mode(mut self, mode: wgpu::CompositeAlphaMode) -> Self {
        self.alpha_mode = Some(mode);
        self
    }

    /// Enable depth buffer with the specified format.
    pub fn with_depth(mut self, format: wgpu::TextureFormat) -> Self {
        self.depth_format = Some(format);
        self
    }

    /// Enable depth buffer with default format (Depth32Float).
    pub fn with_depth_default(mut self) -> Self {
        self.depth_format = Some(DEFAULT_DEPTH_FORMAT);
        self
    }

    /// Enable GPU profiling for this window.
    pub fn with_profiling(mut self, enabled: bool) -> Self {
        self.enable_profiling = enabled;
        self
    }

    /// Build the render window.
    pub fn build(
        self,
        window: Window,
        graphics: Arc<GraphicsContext>,
    ) -> Result<RenderWindow, GraphicsError> {
        let descriptor = WindowContextDescriptor {
            format: self.color_format,
            present_mode: self.present_mode,
            alpha_mode: self.alpha_mode,
            with_depth: self.depth_format.is_some(),
            depth_format: self.depth_format,
        };

        let context = WindowContext::new(window, graphics.clone(), descriptor)?;

        let gpu_profiler = if self.enable_profiling {
            Some(Arc::new(GpuFrameProfiler::new(&graphics)?))
        } else {
            None
        };

        Ok(RenderWindow {
            context,
            gpu_profiler,
        })
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
/// [`begin_frame`](Self::begin_frame) will include GPU profiling.
///
/// # Example
///
/// ```rust,no_run
/// # use astrelis_render::{RenderWindow, GraphicsContext, Color};
/// # use astrelis_winit::window::Window;
/// # use std::sync::Arc;
/// # fn example(window: Window, graphics: Arc<GraphicsContext>) {
/// let mut render_window = RenderWindow::builder()
///     .with_depth_default()
///     .build(window, graphics)
///     .expect("Failed to create window");
///
/// // In render loop:
/// let frame = render_window.begin_frame().expect("Surface available");
/// {
///     let mut pass = frame.render_pass()
///         .clear_color(Color::BLACK)
///         .with_window_depth()
///         .clear_depth(0.0)
///         .build();
///     // Render commands...
/// }
/// // Frame auto-submits on drop
/// # }
/// ```
pub struct RenderWindow {
    pub(crate) context: WindowContext,
    pub(crate) gpu_profiler: Option<Arc<GpuFrameProfiler>>,
}

impl RenderWindow {
    /// Create a new builder for configuring a render window.
    pub fn builder() -> RenderWindowBuilder {
        RenderWindowBuilder::new()
    }

    /// Create a new renderable window with default settings.
    pub fn new(window: Window, context: Arc<GraphicsContext>) -> Result<Self, GraphicsError> {
        Self::new_with_descriptor(window, context, WindowContextDescriptor::default())
    }

    /// Create a new renderable window with an auto-resizing depth texture.
    ///
    /// This is equivalent to using [`builder()`](Self::builder) with `with_depth_default()`.
    pub fn new_with_depth(
        window: Window,
        context: Arc<GraphicsContext>,
    ) -> Result<Self, GraphicsError> {
        Self::builder().with_depth_default().build(window, context)
    }

    /// Create a new renderable window with a descriptor.
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

    /// Begin a new frame for rendering.
    ///
    /// Returns `Some(Frame)` if the surface is available, or `None` if the
    /// surface is temporarily unavailable (e.g., window minimized).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use astrelis_render::{RenderWindow, Color};
    /// # let mut window: RenderWindow = todo!();
    /// if let Some(frame) = window.begin_frame() {
    ///     let mut pass = frame.render_pass()
    ///         .clear_color(Color::BLACK)
    ///         .build();
    ///     // Render commands...
    /// }
    /// ```
    pub fn begin_frame(&mut self) -> Option<Frame<'_>> {
        self.try_begin_frame().ok()
    }

    /// Try to begin a new frame, returning an error on failure.
    ///
    /// Unlike [`begin_frame`](Self::begin_frame), this returns the actual error
    /// for debugging or error handling.
    pub fn try_begin_frame(&mut self) -> Result<Frame<'_>, GraphicsError> {
        profile_function!();

        // Handle pending resize
        let mut configure_needed = false;
        if let Some(new_size) = self.context.reconfigure.resize.take() {
            self.context.config.width = new_size.width;
            self.context.config.height = new_size.height;
            configure_needed = true;

            // Resize depth texture if present
            if let Some(ref mut depth) = self.context.depth_texture
                && depth.needs_resize(new_size.width, new_size.height)
            {
                depth.resize(
                    self.context.context.device(),
                    new_size.width,
                    new_size.height,
                );
            }
        }

        if configure_needed {
            self.context
                .surface
                .configure(self.context.context.device(), &self.context.config);
        }

        // Acquire surface texture
        let surface_texture = self.context.try_acquire_surface_texture()?;
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        Ok(Frame {
            window: self,
            surface: Some(Surface {
                texture: surface_texture,
                view,
            }),
            command_buffers: RefCell::new(Vec::new()),
            stats: Arc::new(AtomicFrameStats::new()),
            submitted: Cell::new(false),
            surface_format: self.context.config.format,
            gpu_profiler: self.gpu_profiler.clone(),
            winit_window: self.context.window.window.clone(),
        })
    }

    /// Get the window ID.
    pub fn id(&self) -> WindowId {
        self.context.window.id()
    }

    /// Get the underlying window.
    pub fn window(&self) -> &Window {
        &self.context.window
    }

    /// Get the window context.
    pub fn context(&self) -> &WindowContext {
        &self.context
    }

    /// Get mutable access to the window context.
    pub fn context_mut(&mut self) -> &mut WindowContext {
        &mut self.context
    }

    /// Get the graphics context.
    pub fn graphics(&self) -> &GraphicsContext {
        &self.context.context
    }

    /// Get the Arc-wrapped graphics context.
    pub fn graphics_arc(&self) -> &Arc<GraphicsContext> {
        &self.context.context
    }

    /// Get the surface texture format.
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

    /// Get the surface size in pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.context.config.width, self.context.config.height)
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

    /// Attach a GPU profiler to this window.
    pub fn set_gpu_profiler(&mut self, profiler: Arc<GpuFrameProfiler>) {
        self.gpu_profiler = Some(profiler);
    }

    /// Remove the GPU profiler from this window.
    pub fn remove_gpu_profiler(&mut self) -> Option<Arc<GpuFrameProfiler>> {
        self.gpu_profiler.take()
    }

    /// Get a reference to the GPU profiler, if attached.
    pub fn gpu_profiler(&self) -> Option<&Arc<GpuFrameProfiler>> {
        self.gpu_profiler.as_ref()
    }

    /// Check if this window has a depth texture.
    pub fn has_depth(&self) -> bool {
        self.context.has_depth()
    }

    /// Get the depth texture view if available (Arc-wrapped).
    pub fn depth_view(&self) -> Option<Arc<wgpu::TextureView>> {
        self.context.depth_view()
    }

    /// Get a reference to the depth texture view (without Arc).
    pub fn depth_view_ref(&self) -> Option<&wgpu::TextureView> {
        self.context.depth_texture.as_ref().map(|d| d.view_ref())
    }

    /// Get the depth texture format, if depth is enabled.
    ///
    /// Returns the format used for the depth texture, or None if depth is not enabled.
    /// Use this to configure renderers that need to match the depth format.
    pub fn depth_format(&self) -> Option<wgpu::TextureFormat> {
        self.context.depth_format()
    }

    /// Ensure a depth texture exists for this window.
    pub fn ensure_depth(&mut self, format: wgpu::TextureFormat) {
        self.context.ensure_depth(format);
    }
}

impl std::ops::Deref for RenderWindow {
    type Target = WindowContext;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}

impl std::ops::DerefMut for RenderWindow {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.context
    }
}

// ============================================================================
// WindowBackend implementation for compatibility
// ============================================================================

impl WindowBackend for RenderWindow {
    type FrameContext = Frame<'static>;
    type Error = GraphicsError;

    fn try_begin_drawing(&mut self) -> Result<Self::FrameContext, Self::Error> {
        // This is a compatibility shim - the new API uses begin_frame()
        // We can't actually return Frame<'static> safely, so this will need
        // to be updated in the WindowBackend trait
        unimplemented!(
            "Use RenderWindow::begin_frame() instead of WindowBackend::try_begin_drawing()"
        )
    }
}

// ============================================================================
// Backwards Compatibility Aliases
// ============================================================================

/// Deprecated alias for [`RenderWindow`].
#[deprecated(since = "0.2.0", note = "Use RenderWindow instead")]
pub type RenderableWindow = RenderWindow;
