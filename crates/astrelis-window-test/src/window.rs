//! Mock window that records all state changes.

use astrelis_core::geometry::{Logical, Physical, Point, Size};
use astrelis_window::cursor::{CursorGrabMode, CursorIcon};
use astrelis_window::error::WindowError;
use astrelis_window::fullscreen::FullscreenMode;
use astrelis_window::monitor::MonitorInfo;
use astrelis_window::theme::Theme;
use astrelis_window::types::{
    InnerPosition, InnerSize, LogicalInnerSize, LogicalOuterPosition, OuterPosition, OuterSize,
};
use astrelis_window::window::{ResizeDirection, Window};
use astrelis_window::window_id::WindowId;
use astrelis_window::window_level::WindowLevel;

/// A mock window that stores all state in plain fields for test assertions.
///
/// Every setter records the value so tests can inspect what the handler did.
#[derive(Debug)]
pub struct MockWindow {
    id: WindowId,
    /// Current window title.
    pub title: String,
    /// Current inner size in physical pixels.
    pub inner_size: Size<Physical>,
    /// Whether the window is visible.
    pub visible: bool,
    /// Whether the window is minimized.
    pub minimized: bool,
    /// Whether the window is maximized.
    pub maximized: bool,
    /// Whether the window has decorations.
    pub decorations: bool,
    /// Whether the window is resizable.
    pub resizable: bool,
    /// Whether the window has focus.
    pub focused: bool,
    /// Current cursor icon.
    pub cursor_icon: CursorIcon,
    /// Whether the cursor is visible.
    pub cursor_visible: bool,
    /// Current cursor grab mode.
    pub cursor_grab_mode: CursorGrabMode,
    /// Current window level.
    pub window_level: WindowLevel,
    /// Current opacity.
    pub opacity: f32,
    /// Current theme.
    pub theme: Option<Theme>,
    /// Whether content protection is enabled.
    pub content_protected: bool,
    /// Current fullscreen mode.
    pub fullscreen: Option<FullscreenMode>,
    /// Number of times `request_redraw` was called.
    pub redraw_request_count: u32,
    /// Scale factor for this mock window.
    pub scale_factor: f32,
}

impl MockWindow {
    /// Creates a new mock window with the given ID and defaults.
    pub(crate) fn new(id: WindowId, title: String, inner_size: Size<Physical>) -> Self {
        Self {
            id,
            title,
            inner_size,
            visible: true,
            minimized: false,
            maximized: false,
            decorations: true,
            resizable: true,
            focused: false,
            cursor_icon: CursorIcon::Default,
            cursor_visible: true,
            cursor_grab_mode: CursorGrabMode::None,
            window_level: WindowLevel::Normal,
            opacity: 1.0,
            theme: None,
            content_protected: false,
            fullscreen: None,
            redraw_request_count: 0,
            scale_factor: 1.0,
        }
    }
}

impl Window for MockWindow {
    fn id(&self) -> WindowId {
        self.id
    }

    fn inner_size(&self) -> InnerSize {
        InnerSize::new(self.inner_size.width, self.inner_size.height)
    }

    fn outer_size(&self) -> OuterSize {
        // Mock: outer = inner + 20px for decorations.
        OuterSize::new(self.inner_size.width + 20.0, self.inner_size.height + 40.0)
    }

    fn inner_position(&self) -> Result<InnerPosition, WindowError> {
        Ok(InnerPosition::new(10.0, 30.0))
    }

    fn outer_position(&self) -> Result<OuterPosition, WindowError> {
        Ok(OuterPosition::new(0.0, 0.0))
    }

    fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    fn request_inner_size(&self, _size: LogicalInnerSize) {
        // MockWindow fields are not &self-mutable; state is set directly in tests.
    }

    fn set_min_inner_size(&self, _size: Option<LogicalInnerSize>) {}
    fn set_max_inner_size(&self, _size: Option<LogicalInnerSize>) {}
    fn set_outer_position(&self, _position: LogicalOuterPosition) {}

    fn set_title(&self, _title: &str) {
        // Can't mutate through &self; see window_mut() for mutable access.
    }

    fn title(&self) -> String {
        self.title.clone()
    }

    fn set_visible(&self, _visible: bool) {}

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_minimized(&self, _minimized: bool) {}

    fn is_minimized(&self) -> bool {
        self.minimized
    }

    fn set_maximized(&self, _maximized: bool) {}

    fn is_maximized(&self) -> bool {
        self.maximized
    }

    fn set_fullscreen(&self, _mode: Option<FullscreenMode>) {}

    fn fullscreen(&self) -> Option<FullscreenMode> {
        self.fullscreen.clone()
    }

    fn set_decorations(&self, _decorations: bool) {}

    fn has_decorations(&self) -> bool {
        self.decorations
    }

    fn set_opacity(&self, _opacity: f32) {}

    fn set_window_level(&self, _level: WindowLevel) {}

    fn set_resizable(&self, _resizable: bool) {}

    fn is_resizable(&self) -> bool {
        self.resizable
    }

    fn focus(&self) {}

    fn has_focus(&self) -> bool {
        self.focused
    }

    fn set_cursor_icon(&self, _icon: CursorIcon) {}

    fn set_cursor_visible(&self, _visible: bool) {}

    fn set_cursor_grab(&self, _mode: CursorGrabMode) -> Result<(), WindowError> {
        Ok(())
    }

    fn set_cursor_position(&self, _position: Point<Logical>) -> Result<(), WindowError> {
        Ok(())
    }

    fn request_redraw(&self) {
        // Can't increment through &self; tracked via redraw_request_count on &mut path.
    }

    fn current_monitor(&self) -> Option<MonitorInfo> {
        None
    }

    fn set_content_protected(&self, _protected: bool) {}

    fn theme(&self) -> Option<Theme> {
        self.theme
    }

    fn set_theme(&self, _theme: Option<Theme>) {}

    fn drag_window(&self) -> Result<(), WindowError> {
        Ok(())
    }

    fn drag_resize_window(&self, _direction: ResizeDirection) -> Result<(), WindowError> {
        Ok(())
    }
}

impl raw_window_handle::HasWindowHandle for MockWindow {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        // SAFETY: tests never use this handle for actual GPU operations.
        // We return a null-like handle that satisfies the trait.
        #[cfg(target_os = "macos")]
        {
            use raw_window_handle::RawWindowHandle;
            let handle = raw_window_handle::AppKitWindowHandle::new(
                std::ptr::NonNull::dangling(),
            );
            // SAFETY: This is a test-only dummy handle, never dereferenced.
            Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(RawWindowHandle::AppKit(handle)) })
        }
        #[cfg(target_os = "linux")]
        {
            use raw_window_handle::RawWindowHandle;
            let handle = raw_window_handle::XlibWindowHandle::new(0);
            // SAFETY: This is a test-only dummy handle, never dereferenced.
            Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(RawWindowHandle::Xlib(handle)) })
        }
        #[cfg(target_os = "windows")]
        {
            use raw_window_handle::RawWindowHandle;
            let handle = raw_window_handle::Win32WindowHandle::new(
                std::num::NonZero::new(1).unwrap(),
            );
            // SAFETY: This is a test-only dummy handle, never dereferenced.
            Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(RawWindowHandle::Win32(handle)) })
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            Err(raw_window_handle::HandleError::Unavailable)
        }
    }
}

impl raw_window_handle::HasDisplayHandle for MockWindow {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        // SAFETY: test-only dummy handle.
        #[cfg(target_os = "macos")]
        {
            use raw_window_handle::RawDisplayHandle;
            let handle = raw_window_handle::AppKitDisplayHandle::new();
            Ok(unsafe { raw_window_handle::DisplayHandle::borrow_raw(RawDisplayHandle::AppKit(handle)) })
        }
        #[cfg(target_os = "linux")]
        {
            use raw_window_handle::RawDisplayHandle;
            let handle = raw_window_handle::XlibDisplayHandle::new(None, 0);
            Ok(unsafe { raw_window_handle::DisplayHandle::borrow_raw(RawDisplayHandle::Xlib(handle)) })
        }
        #[cfg(target_os = "windows")]
        {
            use raw_window_handle::RawDisplayHandle;
            let handle = raw_window_handle::WindowsDisplayHandle::new();
            Ok(unsafe { raw_window_handle::DisplayHandle::borrow_raw(RawDisplayHandle::Windows(handle)) })
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            Err(raw_window_handle::HandleError::Unavailable)
        }
    }
}
