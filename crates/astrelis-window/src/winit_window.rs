//! Winit-backed window implementation.

use astrelis_core::geometry::{Logical, Point};

use crate::convert;
use crate::cursor::{CursorGrabMode, CursorIcon};
use crate::error::WindowError;
use crate::fullscreen::FullscreenMode;
use crate::monitor::MonitorInfo;
use crate::theme::Theme;
use crate::types::{
    InnerPosition, InnerSize, LogicalInnerSize, LogicalOuterPosition, OuterPosition, OuterSize,
};
use crate::window::{ResizeDirection, Window};
use crate::window_id::WindowId;
use crate::window_level::WindowLevel;

/// A window backed by winit.
pub(crate) struct WinitWindow {
    pub(crate) inner: winit::window::Window,
    pub(crate) astrelis_id: WindowId,
    pub(crate) title: String,
}

impl Window for WinitWindow {
    fn id(&self) -> WindowId {
        self.astrelis_id
    }

    fn inner_size(&self) -> InnerSize {
        let s = self.inner.inner_size();
        InnerSize::new(s.width as f32, s.height as f32)
    }

    fn outer_size(&self) -> OuterSize {
        let s = self.inner.outer_size();
        OuterSize::new(s.width as f32, s.height as f32)
    }

    fn inner_position(&self) -> Result<InnerPosition, WindowError> {
        // winit does not provide inner_position directly on all platforms;
        // fall back to outer_position as an approximation.
        let p = self
            .inner
            .outer_position()
            .map_err(|e| WindowError::EventLoopError(e.to_string()))?;
        Ok(InnerPosition::new(p.x as f32, p.y as f32))
    }

    fn outer_position(&self) -> Result<OuterPosition, WindowError> {
        let p = self
            .inner
            .outer_position()
            .map_err(|e| WindowError::EventLoopError(e.to_string()))?;
        Ok(OuterPosition::new(p.x as f32, p.y as f32))
    }

    fn scale_factor(&self) -> f32 {
        self.inner.scale_factor() as f32
    }

    fn request_inner_size(&self, size: LogicalInnerSize) {
        astrelis_profiling::profile_function!();
        let s = size.logical();
        let _ = self
            .inner
            .request_inner_size(winit::dpi::LogicalSize::new(s.width, s.height));
    }

    fn set_min_inner_size(&self, size: Option<LogicalInnerSize>) {
        astrelis_profiling::profile_function!();
        self.inner.set_min_inner_size(
            size.map(|s| {
                let s = s.logical();
                winit::dpi::LogicalSize::new(s.width, s.height)
            }),
        );
    }

    fn set_max_inner_size(&self, size: Option<LogicalInnerSize>) {
        astrelis_profiling::profile_function!();
        self.inner.set_max_inner_size(
            size.map(|s| {
                let s = s.logical();
                winit::dpi::LogicalSize::new(s.width, s.height)
            }),
        );
    }

    fn set_outer_position(&self, position: LogicalOuterPosition) {
        astrelis_profiling::profile_function!();
        let p = position.logical();
        self.inner
            .set_outer_position(winit::dpi::LogicalPosition::new(p.x, p.y));
    }

    fn set_title(&self, title: &str) {
        astrelis_profiling::profile_function!();
        self.inner.set_title(title);
    }

    fn title(&self) -> String {
        self.title.clone()
    }

    fn set_visible(&self, visible: bool) {
        astrelis_profiling::profile_function!();
        self.inner.set_visible(visible);
    }

    fn is_visible(&self) -> bool {
        self.inner.is_visible().unwrap_or(true)
    }

    fn set_minimized(&self, minimized: bool) {
        astrelis_profiling::profile_function!();
        self.inner.set_minimized(minimized);
    }

    fn is_minimized(&self) -> bool {
        self.inner.is_minimized().unwrap_or(false)
    }

    fn set_maximized(&self, maximized: bool) {
        astrelis_profiling::profile_function!();
        self.inner.set_maximized(maximized);
    }

    fn is_maximized(&self) -> bool {
        self.inner.is_maximized()
    }

    fn set_fullscreen(&self, mode: Option<FullscreenMode>) {
        astrelis_profiling::profile_function!();
        let winit_fs = mode.map(|m| match m {
            FullscreenMode::Borderless(_) => {
                winit::window::Fullscreen::Borderless(self.inner.current_monitor())
            }
            FullscreenMode::Exclusive { .. } => {
                // For exclusive, we'd need to look up the specific video mode.
                // For now, fall back to borderless.
                winit::window::Fullscreen::Borderless(self.inner.current_monitor())
            }
        });
        self.inner.set_fullscreen(winit_fs);
    }

    fn fullscreen(&self) -> Option<FullscreenMode> {
        self.inner
            .fullscreen()
            .map(|_| FullscreenMode::Borderless(None))
    }

    fn set_decorations(&self, decorations: bool) {
        astrelis_profiling::profile_function!();
        self.inner.set_decorations(decorations);
    }

    fn has_decorations(&self) -> bool {
        self.inner.is_decorated()
    }

    fn set_opacity(&self, _opacity: f32) {
        // winit 0.30 does not expose set_opacity on the public Window API.
        // This is a no-op; check capabilities before calling.
    }

    fn set_window_level(&self, level: WindowLevel) {
        astrelis_profiling::profile_function!();
        let winit_level = match level {
            WindowLevel::AlwaysOnBottom => winit::window::WindowLevel::AlwaysOnBottom,
            WindowLevel::Normal => winit::window::WindowLevel::Normal,
            WindowLevel::AlwaysOnTop => winit::window::WindowLevel::AlwaysOnTop,
        };
        self.inner.set_window_level(winit_level);
    }

    fn set_resizable(&self, resizable: bool) {
        astrelis_profiling::profile_function!();
        self.inner.set_resizable(resizable);
    }

    fn is_resizable(&self) -> bool {
        self.inner.is_resizable()
    }

    fn focus(&self) {
        astrelis_profiling::profile_function!();
        self.inner.focus_window();
    }

    fn has_focus(&self) -> bool {
        self.inner.has_focus()
    }

    fn set_cursor_icon(&self, icon: CursorIcon) {
        astrelis_profiling::profile_function!();
        self.inner
            .set_cursor(winit::window::Cursor::Icon(convert::cursor::to_winit_cursor(icon)));
    }

    fn set_cursor_visible(&self, visible: bool) {
        astrelis_profiling::profile_function!();
        self.inner.set_cursor_visible(visible);
    }

    fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), WindowError> {
        astrelis_profiling::profile_function!();
        let winit_mode = match mode {
            CursorGrabMode::None => winit::window::CursorGrabMode::None,
            CursorGrabMode::Confined => winit::window::CursorGrabMode::Confined,
            CursorGrabMode::Locked => winit::window::CursorGrabMode::Locked,
        };
        self.inner
            .set_cursor_grab(winit_mode)
            .map_err(|e| WindowError::EventLoopError(e.to_string()))
    }

    fn set_cursor_position(&self, position: Point<Logical>) -> Result<(), WindowError> {
        astrelis_profiling::profile_function!();
        self.inner
            .set_cursor_position(winit::dpi::LogicalPosition::new(position.x, position.y))
            .map_err(|e| WindowError::EventLoopError(e.to_string()))
    }

    fn request_redraw(&self) {
        astrelis_profiling::profile_function!();
        self.inner.request_redraw();
    }

    fn current_monitor(&self) -> Option<MonitorInfo> {
        self.inner
            .current_monitor()
            .map(|h| convert::monitor::convert_monitor(&h, 0))
    }

    fn set_content_protected(&self, protected: bool) {
        astrelis_profiling::profile_function!();
        self.inner.set_content_protected(protected);
    }

    fn theme(&self) -> Option<Theme> {
        self.inner.theme().map(|t| match t {
            winit::window::Theme::Light => Theme::Light,
            winit::window::Theme::Dark => Theme::Dark,
        })
    }

    fn set_theme(&self, theme: Option<Theme>) {
        astrelis_profiling::profile_function!();
        self.inner.set_theme(theme.map(|t| match t {
            Theme::Light => winit::window::Theme::Light,
            Theme::Dark => winit::window::Theme::Dark,
        }));
    }

    fn drag_window(&self) -> Result<(), WindowError> {
        astrelis_profiling::profile_function!();
        self.inner
            .drag_window()
            .map_err(|e| WindowError::EventLoopError(e.to_string()))
    }

    fn drag_resize_window(&self, direction: ResizeDirection) -> Result<(), WindowError> {
        astrelis_profiling::profile_function!();
        let winit_dir = match direction {
            ResizeDirection::North => winit::window::ResizeDirection::North,
            ResizeDirection::South => winit::window::ResizeDirection::South,
            ResizeDirection::East => winit::window::ResizeDirection::East,
            ResizeDirection::West => winit::window::ResizeDirection::West,
            ResizeDirection::NorthWest => winit::window::ResizeDirection::NorthWest,
            ResizeDirection::NorthEast => winit::window::ResizeDirection::NorthEast,
            ResizeDirection::SouthWest => winit::window::ResizeDirection::SouthWest,
            ResizeDirection::SouthEast => winit::window::ResizeDirection::SouthEast,
        };
        self.inner
            .drag_resize_window(winit_dir)
            .map_err(|e| WindowError::EventLoopError(e.to_string()))
    }
}

impl raw_window_handle::HasWindowHandle for WinitWindow {
    fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        self.inner.window_handle()
    }
}

impl raw_window_handle::HasDisplayHandle for WinitWindow {
    fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        self.inner.display_handle()
    }
}
