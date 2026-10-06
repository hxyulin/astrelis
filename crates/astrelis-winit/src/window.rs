use crate::{SurfaceSettings, WindowError, WindowMetrics};
use astrelis::{Frame, FrameError, GraphicsContext, RenderFormat, RenderTarget};
use std::sync::Arc;
use winit::{
    event::WindowEvent,
    window::{Window, WindowId},
};

/// Changes from a native event, useful for invalidating an application-owned loop.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowUpdate {
    /// Physical size or logical scale changed.
    pub metrics_changed: bool,
    /// Presentation eligibility changed, for example after exposure.
    pub availability_changed: bool,
    /// Native geometry/exposure requests a probe even if cached state is unchanged.
    pub redraw_requested: bool,
}
impl WindowUpdate {
    /// Whether this update warrants a new draw when presentation is available.
    pub const fn needs_redraw(self) -> bool {
        self.metrics_changed || self.availability_changed || self.redraw_requested
    }
}

/// Native window and its recoverable Astrelis surface, independent of a runner.
///
/// GPU resources and requested attachments survive suspension. A surface is
/// recreated only after resume or loss; resize updates the existing target.
/// The target is exposed read-only so configuration cannot diverge from recovery
/// settings. Frame acquisition borrows this context until recording ends.
pub struct WindowContext {
    pub(crate) window: Arc<Window>,
    pub(crate) graphics: GraphicsContext,
    pub(crate) target: Option<RenderTarget<'static>>,
    pub(crate) format: RenderFormat,
    metrics: WindowMetrics,
    settings: SurfaceSettings,
    supported_samples: Vec<u32>,
    generation: u64,
    active: bool,
    visible: bool,
    occluded: bool,
    pub(crate) defer_show: bool,
    pub(crate) invalidated: bool,
    pub(crate) availability_invalidated: bool,
}
impl WindowContext {
    /// Selects a compatible adapter/device asynchronously for this first surface.
    /// Metrics are reconciled after GPU initialization, which may take time.
    pub async fn new(window: Arc<Window>, settings: SurfaceSettings) -> Result<Self, WindowError> {
        let metrics = WindowMetrics::from_window(&window)?;
        let (graphics, target) = GraphicsContext::with_surface(
            window.clone(),
            settings.surface_options(metrics.physical_size()),
        )
        .await?;
        Self::from_target(graphics, window, settings, target)
    }
    /// Creates another window surface on an explicitly selected/shared device.
    /// Incompatible surfaces fail rather than silently selecting another device.
    pub fn with_graphics(
        graphics: &GraphicsContext,
        window: Arc<Window>,
        settings: SurfaceSettings,
    ) -> Result<Self, WindowError> {
        let metrics = WindowMetrics::from_window(&window)?;
        let target = graphics.create_surface(
            window.clone(),
            settings.surface_options(metrics.physical_size()),
        )?;
        Self::from_target(graphics.clone(), window, settings, target)
    }
    fn from_target(
        graphics: GraphicsContext,
        window: Arc<Window>,
        settings: SurfaceSettings,
        mut target: RenderTarget<'static>,
    ) -> Result<Self, WindowError> {
        let metrics = WindowMetrics::from_window(&window)?;
        let size = metrics.physical_size();
        target.resize(size.width, size.height)?;
        let format = target.render_format();
        let supported_samples = target.supported_sample_counts().to_vec();
        let visible = window.is_visible().unwrap_or(true);
        Ok(Self {
            window,
            graphics,
            target: Some(target),
            format,
            metrics,
            settings,
            supported_samples,
            generation: 1,
            active: true,
            visible,
            occluded: false,
            defer_show: false,
            invalidated: true,
            availability_invalidated: false,
        })
    }
    /// Native handle for title, cursor, IME, accessibility, and platform APIs.
    pub fn window(&self) -> &Arc<Window> {
        &self.window
    }
    /// Device shared by this window's attachments and application resources.
    pub fn graphics(&self) -> &GraphicsContext {
        &self.graphics
    }
    /// Cached inner dimensions and density; performs no native query.
    pub fn metrics(&self) -> WindowMetrics {
        self.metrics
    }
    /// Requested settings retained across surface replacement.
    pub fn settings(&self) -> SurfaceSettings {
        self.settings
    }
    /// Live target, absent during suspension or failed recreation.
    pub fn target(&self) -> Option<&RenderTarget<'static>> {
        self.target.as_ref()
    }
    /// Cached live attachment compatibility; also available for zero-sized targets.
    pub fn render_format(&self) -> Option<&RenderFormat> {
        self.target.as_ref().map(|_| &self.format)
    }
    /// Increments on successful surface replacement, not resize or MSAA changes.
    pub fn surface_generation(&self) -> u64 {
        self.generation
    }
    /// Cached native eligibility, without attempting swapchain acquisition.
    pub fn can_present(&self) -> bool {
        self.active
            && self.visible
            && !self.occluded
            && self.metrics.has_area()
            && self.target.is_some()
    }
    /// Updates cached resize/DPI/occlusion state before forwarding a native event.
    /// Unchanged size notifications do not replace attachments.
    pub fn process_event(&mut self, event: &WindowEvent) -> Result<WindowUpdate, WindowError> {
        let before = self.can_present();
        let next = match event {
            WindowEvent::Resized(size) => {
                Some(WindowMetrics::new(*size, self.metrics.scale_factor())?)
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                Some(WindowMetrics::new(self.window.inner_size(), *scale_factor)?)
            }
            _ => None,
        };
        let metrics_changed = next.is_some_and(|m| m != self.metrics);
        if let Some(metrics) = next
            && metrics_changed
        {
            if let Some(target) = &mut self.target {
                let size = metrics.physical_size();
                target.resize(size.width, size.height)?;
            }
            self.metrics = metrics;
        }
        if let WindowEvent::Occluded(value) = event {
            self.occluded = *value;
        }
        let update = WindowUpdate {
            metrics_changed,
            availability_changed: before != self.can_present(),
            redraw_requested: matches!(
                event,
                WindowEvent::Occluded(false)
                    | WindowEvent::Resized(_)
                    | WindowEvent::ScaleFactorChanged { .. }
            ),
        };
        self.invalidated |= update.needs_redraw();
        self.availability_invalidated |= update.availability_changed;
        Ok(update)
    }
    /// Releases presentation resources while preserving device and settings.
    /// Repeated calls are harmless.
    pub fn suspend(&mut self) {
        self.active = false;
        self.target = None;
        self.invalidated = true;
        self.availability_invalidated = true;
    }
    /// Restores presentation after suspension; repeated active calls are harmless.
    pub fn resume(&mut self) -> Result<(), WindowError> {
        if self.target.is_none() {
            self.recreate()?;
        }
        self.active = true;
        Ok(())
    }
    pub(crate) fn recreate(&mut self) -> Result<(), WindowError> {
        // Drop first: some backends cannot create two swapchains for one window.
        self.target = None;
        let metrics = WindowMetrics::from_window(&self.window)?;
        let target = self.graphics.create_surface(
            self.window.clone(),
            self.settings.surface_options(metrics.physical_size()),
        )?;
        self.format = target.render_format();
        self.supported_samples = target.supported_sample_counts().to_vec();
        self.target = Some(target);
        self.metrics = metrics;
        self.generation = self.generation.saturating_add(1);
        self.invalidated = true;
        self.availability_invalidated = true;
        Ok(())
    }
    /// Changes MSAA now or queues a validated setting while suspended.
    /// Rejection leaves both live attachments and recovery settings unchanged.
    pub fn set_sample_count(&mut self, count: u32) -> Result<(), WindowError> {
        if let Some(target) = &mut self.target {
            target.set_sample_count(count)?;
        } else if !self.supported_samples.contains(&count) {
            return Err(astrelis::Error::UnsupportedSampleCount {
                format: self.format.colors[0].expect("surface color"),
                count,
            }
            .into());
        }
        if self
            .settings
            .surface_options(self.metrics.physical_size())
            .sample_count
            != count
        {
            self.settings = self.settings.sample_count(count);
            if let Some(target) = &self.target {
                self.format = target.render_format();
            }
            self.invalidated = true;
        }
        Ok(())
    }
    /// Sets managed visibility and invalidates on changes.
    /// In window_created, showing is deferred until that callback returns.
    pub fn set_visible(&mut self, visible: bool) {
        if self.visible != visible {
            self.visible = visible;
            self.invalidated = true;
            self.availability_invalidated = true;
        }
        if !self.defer_show {
            self.window.set_visible(visible);
        }
    }
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    pub(crate) fn finish_creation(&mut self) {
        if self.defer_show {
            self.defer_show = false;
            self.window.set_visible(self.visible);
        }
    }
    /// Acquires an independently configured core frame; no pass is opened.
    /// Custom loops must notify the native window before consuming Frame::finish.
    pub fn begin_frame(&mut self) -> Result<Frame<'_, 'static>, FrameError> {
        if !self.can_present() {
            return Err(FrameError::Suspended);
        }
        self.target.as_mut().expect("eligible target").begin_frame()
    }
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    pub(crate) fn info_and_frame(
        &mut self,
    ) -> Result<(WindowInfo<'_>, Frame<'_, 'static>), FrameError> {
        if !self.can_present() {
            return Err(FrameError::Suspended);
        }
        let info = WindowInfo {
            window: &self.window,
            graphics: &self.graphics,
            metrics: self.metrics,
            format: &self.format,
            generation: self.generation,
        };
        let frame = self
            .target
            .as_mut()
            .expect("eligible target")
            .begin_frame()?;
        Ok((info, frame))
    }
}

/// Immutable window information borrowed independently of its recording frame.
/// No Arc or attachment-format vector is cloned for a render callback.
#[derive(Clone, Copy)]
pub struct WindowInfo<'a> {
    window: &'a Window,
    graphics: &'a GraphicsContext,
    metrics: WindowMetrics,
    format: &'a RenderFormat,
    generation: u64,
}
impl WindowInfo<'_> {
    /// Native identity, also used by AppContext.
    pub fn id(&self) -> WindowId {
        self.window.id()
    }
    /// Native window operations remain available while recording.
    pub fn window(&self) -> &Window {
        self.window
    }
    /// Graphics device used by this window.
    pub fn graphics(&self) -> &GraphicsContext {
        self.graphics
    }
    /// Cached physical/logical dimensions and scale.
    pub fn metrics(&self) -> WindowMetrics {
        self.metrics
    }
    /// Cached attachment compatibility for preparing application renderers.
    pub fn render_format(&self) -> &RenderFormat {
        self.format
    }
    /// Surface replacement generation.
    pub fn surface_generation(&self) -> u64 {
        self.generation
    }
}
