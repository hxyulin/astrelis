//! Windows, monitors, creation attributes, and commands.

use std::{fmt, sync::Arc};

use astrelis_core::geometry::{Logical, Physical, Point, Rect, Size};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};

use crate::{ImePurpose, PlatformError, backend};

/// Stable window identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowId(pub u64);

/// Stable monitor identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MonitorId(pub u64);

/// Monitor information.
#[derive(Clone, Debug, PartialEq)]
pub struct Monitor {
    /// Stable identifier.
    pub id: MonitorId,
    /// Human-readable name.
    pub name: Option<String>,
    /// Physical desktop position.
    pub position: Point<Physical, i32>,
    /// Physical pixel size.
    pub size: Size<Physical, u32>,
    /// DPI scale.
    pub scale_factor: f64,
}

/// Light or dark appearance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Theme {
    /// Light appearance.
    Light,
    /// Dark appearance.
    Dark,
}

/// Stacking level for a window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WindowLevel {
    /// Below normal windows.
    AlwaysOnBottom,
    /// Normal stacking.
    #[default]
    Normal,
    /// Above normal windows.
    AlwaysOnTop,
}

/// Standard system cursor icon.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CursorIcon {
    /// Platform default.
    #[default]
    Default,
    /// Pointing hand.
    Pointer,
    /// Text insertion.
    Text,
    /// Crosshair.
    Crosshair,
    /// Busy indicator.
    Wait,
    /// Move indicator.
    Move,
    /// Horizontal resize.
    EwResize,
    /// Vertical resize.
    NsResize,
    /// Resize along the north-west to south-east diagonal.
    NwseResize,
    /// Resize along the north-east to south-west diagonal.
    NeswResize,
    /// Hidden or invalid action.
    NotAllowed,
}

/// Cursor confinement mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CursorGrabMode {
    /// Cursor is unrestricted.
    #[default]
    None,
    /// Cursor is confined to the window.
    Confined,
    /// Cursor is locked in place.
    Locked,
}

/// Edge used for interactive resize dragging.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResizeDirection {
    /// North.
    North,
    /// North-east.
    NorthEast,
    /// East.
    East,
    /// South-east.
    SouthEast,
    /// South.
    South,
    /// South-west.
    SouthWest,
    /// West.
    West,
    /// North-west.
    NorthWest,
}

/// Focused platform capability flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowCapabilities {
    /// IME controls are supported.
    pub ime: bool,
    /// Cursor confinement is supported.
    pub cursor_confined: bool,
    /// Cursor locking is supported.
    pub cursor_locked: bool,
    /// Transparent windows are supported.
    pub transparent: bool,
    /// Client-area window dragging is supported.
    pub drag_window: bool,
    /// Client-area resize dragging is supported.
    pub drag_resize_window: bool,
}

/// Window creation settings.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowAttributes {
    /// Window title.
    pub title: String,
    /// Initial logical client size.
    pub inner_size: Option<Size<Logical, f64>>,
    /// Minimum logical client size.
    pub min_inner_size: Option<Size<Logical, f64>>,
    /// Maximum logical client size.
    pub max_inner_size: Option<Size<Logical, f64>>,
    /// Initial logical outer position.
    pub position: Option<Point<Logical, f64>>,
    /// Initially visible.
    pub visible: bool,
    /// User-resizable.
    pub resizable: bool,
    /// Native decorations.
    pub decorations: bool,
    /// Transparent framebuffer.
    pub transparent: bool,
    /// Activate on creation.
    pub active: bool,
    /// Initially maximized.
    pub maximized: bool,
    /// Preferred appearance.
    pub theme: Option<Theme>,
    /// Stacking level.
    pub level: WindowLevel,
}

impl Default for WindowAttributes {
    fn default() -> Self {
        Self {
            title: "Astrelis".into(),
            inner_size: None,
            min_inner_size: None,
            max_inner_size: None,
            position: None,
            visible: true,
            resizable: true,
            decorations: true,
            transparent: false,
            active: true,
            maximized: false,
            theme: None,
            level: WindowLevel::Normal,
        }
    }
}

/// A backend command used by the stable [`Window`] wrapper.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum WindowCommand {
    /// Query inner size.
    InnerSize,
    /// Query the client area's desktop position.
    InnerPosition,
    /// Query outer position.
    OuterPosition,
    /// Query scale factor.
    ScaleFactor,
    /// Set title.
    SetTitle(String),
    /// Set visibility.
    SetVisible(bool),
    /// Request focus.
    Focus,
    /// Query focus.
    IsFocused,
    /// Set minimized.
    SetMinimized(bool),
    /// Set maximized.
    SetMaximized(bool),
    /// Query maximized state.
    IsMaximized,
    /// Set borderless fullscreen.
    SetFullscreen(bool),
    /// Set resizability.
    SetResizable(bool),
    /// Set decorations.
    SetDecorations(bool),
    /// Set cursor icon.
    SetCursorIcon(CursorIcon),
    /// Set cursor visibility.
    SetCursorVisible(bool),
    /// Set cursor grab.
    SetCursorGrab(CursorGrabMode),
    /// Set cursor physical position.
    SetCursorPosition(Point<Physical, f64>),
    /// Request redraw.
    RequestRedraw,
    /// Set IME enabled.
    SetImeAllowed(bool),
    /// Set IME purpose.
    SetImePurpose(ImePurpose),
    /// Set IME candidate cursor area in logical units.
    SetImeCursorArea(Rect<Logical, f64>),
    /// Begin native move dragging.
    DragWindow,
    /// Begin native resize dragging.
    DragResizeWindow(ResizeDirection),
    /// Query current theme.
    Theme,
    /// Query current monitor.
    CurrentMonitor,
}

/// Value returned by a backend command.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum WindowValue {
    /// Physical size.
    PhysicalSize(Size<Physical, u32>),
    /// Physical position.
    PhysicalPosition(Point<Physical, i32>),
    /// Floating-point value.
    Float(f64),
    /// Boolean value.
    Bool(bool),
    /// Theme value.
    Theme(Option<Theme>),
    /// Monitor value.
    Monitor(Option<Monitor>),
}

/// A clonable strong owner of a native window.
#[derive(Clone)]
pub struct Window {
    inner: Arc<dyn backend::Window>,
}

impl Window {
    /// Wraps backend window storage.
    pub fn from_backend(inner: Arc<dyn backend::Window>) -> Self {
        Self { inner }
    }
    /// Returns the stable identifier.
    pub fn id(&self) -> WindowId {
        self.inner.id()
    }
    /// Returns focused capability flags.
    pub fn capabilities(&self) -> WindowCapabilities {
        self.inner.capabilities()
    }
    fn command(&self, command: WindowCommand) -> Result<Option<WindowValue>, PlatformError> {
        self.inner.command(command)
    }
    /// Returns the framebuffer size.
    pub fn inner_size(&self) -> Result<Size<Physical, u32>, PlatformError> {
        match self.command(WindowCommand::InnerSize)? {
            Some(WindowValue::PhysicalSize(v)) => Ok(v),
            _ => Err(PlatformError::new("backend returned an invalid inner size")),
        }
    }
    /// Returns the client area's physical desktop position.
    pub fn inner_position(&self) -> Result<Point<Physical, i32>, PlatformError> {
        match self.command(WindowCommand::InnerPosition)? {
            Some(WindowValue::PhysicalPosition(v)) => Ok(v),
            _ => Err(PlatformError::new(
                "backend returned an invalid inner position",
            )),
        }
    }
    /// Returns the outer desktop position.
    pub fn outer_position(&self) -> Result<Point<Physical, i32>, PlatformError> {
        match self.command(WindowCommand::OuterPosition)? {
            Some(WindowValue::PhysicalPosition(v)) => Ok(v),
            _ => Err(PlatformError::new(
                "backend returned an invalid outer position",
            )),
        }
    }
    /// Returns the DPI scale.
    pub fn scale_factor(&self) -> f64 {
        self.try_scale_factor().unwrap_or(1.0)
    }
    /// Returns the DPI scale without discarding backend failures.
    pub fn try_scale_factor(&self) -> Result<f64, PlatformError> {
        match self.command(WindowCommand::ScaleFactor)? {
            Some(WindowValue::Float(value)) => Ok(value),
            _ => Err(PlatformError::new(
                "backend returned an invalid scale factor",
            )),
        }
    }
    /// Changes the title.
    pub fn set_title(&self, title: impl Into<String>) {
        let _ = self.try_set_title(title);
    }
    /// Changes the title without discarding backend failures.
    pub fn try_set_title(&self, title: impl Into<String>) -> Result<(), PlatformError> {
        self.command(WindowCommand::SetTitle(title.into()))
            .map(|_| ())
    }
    /// Changes visibility.
    pub fn set_visible(&self, visible: bool) {
        let _ = self.try_set_visible(visible);
    }
    /// Changes visibility without discarding backend failures.
    pub fn try_set_visible(&self, visible: bool) -> Result<(), PlatformError> {
        self.command(WindowCommand::SetVisible(visible)).map(|_| ())
    }
    /// Requests keyboard focus.
    pub fn focus(&self) {
        let _ = self.try_focus();
    }
    /// Requests keyboard focus without discarding backend failures.
    pub fn try_focus(&self) -> Result<(), PlatformError> {
        self.command(WindowCommand::Focus).map(|_| ())
    }
    /// Reports focus.
    pub fn is_focused(&self) -> bool {
        self.try_is_focused().unwrap_or(false)
    }
    /// Reports focus without discarding backend failures.
    pub fn try_is_focused(&self) -> Result<bool, PlatformError> {
        match self.command(WindowCommand::IsFocused)? {
            Some(WindowValue::Bool(value)) => Ok(value),
            _ => Err(PlatformError::new("backend returned invalid focus state")),
        }
    }
    /// Changes minimized state.
    pub fn set_minimized(&self, value: bool) {
        let _ = self.try_set_minimized(value);
    }
    /// Changes minimized state without discarding backend failures.
    pub fn try_set_minimized(&self, value: bool) -> Result<(), PlatformError> {
        self.command(WindowCommand::SetMinimized(value)).map(|_| ())
    }
    /// Changes maximized state.
    pub fn set_maximized(&self, value: bool) {
        let _ = self.try_set_maximized(value);
    }
    /// Changes maximized state without discarding backend failures.
    pub fn try_set_maximized(&self, value: bool) -> Result<(), PlatformError> {
        self.command(WindowCommand::SetMaximized(value)).map(|_| ())
    }
    /// Reports whether the window is maximized.
    pub fn is_maximized(&self) -> bool {
        self.try_is_maximized().unwrap_or(false)
    }
    /// Reports maximized state without discarding backend failures.
    pub fn try_is_maximized(&self) -> Result<bool, PlatformError> {
        match self.command(WindowCommand::IsMaximized)? {
            Some(WindowValue::Bool(value)) => Ok(value),
            _ => Err(PlatformError::new(
                "backend returned invalid maximized state",
            )),
        }
    }
    /// Enables borderless fullscreen on the current monitor.
    pub fn set_borderless_fullscreen(&self, value: bool) {
        let _ = self.try_set_borderless_fullscreen(value);
    }
    /// Changes borderless fullscreen without discarding backend failures.
    pub fn try_set_borderless_fullscreen(&self, value: bool) -> Result<(), PlatformError> {
        self.command(WindowCommand::SetFullscreen(value))
            .map(|_| ())
    }
    /// Changes resizability.
    pub fn set_resizable(&self, value: bool) {
        let _ = self.try_set_resizable(value);
    }
    /// Changes resizability without discarding backend failures.
    pub fn try_set_resizable(&self, value: bool) -> Result<(), PlatformError> {
        self.command(WindowCommand::SetResizable(value)).map(|_| ())
    }
    /// Changes native decorations.
    pub fn set_decorations(&self, value: bool) {
        let _ = self.try_set_decorations(value);
    }
    /// Changes native decorations without discarding backend failures.
    pub fn try_set_decorations(&self, value: bool) -> Result<(), PlatformError> {
        self.command(WindowCommand::SetDecorations(value))
            .map(|_| ())
    }
    /// Changes the standard cursor.
    pub fn set_cursor_icon(&self, value: CursorIcon) {
        let _ = self.try_set_cursor_icon(value);
    }
    /// Changes the standard cursor without discarding backend failures.
    pub fn try_set_cursor_icon(&self, value: CursorIcon) -> Result<(), PlatformError> {
        self.command(WindowCommand::SetCursorIcon(value))
            .map(|_| ())
    }
    /// Changes cursor visibility.
    pub fn set_cursor_visible(&self, value: bool) {
        let _ = self.try_set_cursor_visible(value);
    }
    /// Changes cursor visibility without discarding backend failures.
    pub fn try_set_cursor_visible(&self, value: bool) -> Result<(), PlatformError> {
        self.command(WindowCommand::SetCursorVisible(value))
            .map(|_| ())
    }
    /// Changes cursor confinement.
    pub fn set_cursor_grab(&self, value: CursorGrabMode) -> Result<(), PlatformError> {
        self.command(WindowCommand::SetCursorGrab(value))
            .map(|_| ())
    }
    /// Moves the cursor.
    pub fn set_cursor_position(&self, value: Point<Physical, f64>) -> Result<(), PlatformError> {
        self.command(WindowCommand::SetCursorPosition(value))
            .map(|_| ())
    }
    /// Schedules a redraw event.
    pub fn request_redraw(&self) {
        let _ = self.try_request_redraw();
    }
    /// Schedules a redraw event without discarding backend failures.
    pub fn try_request_redraw(&self) -> Result<(), PlatformError> {
        self.command(WindowCommand::RequestRedraw).map(|_| ())
    }
    /// Enables or disables IME.
    pub fn set_ime_allowed(&self, value: bool) {
        let _ = self.try_set_ime_allowed(value);
    }
    /// Enables or disables IME without discarding backend failures.
    pub fn try_set_ime_allowed(&self, value: bool) -> Result<(), PlatformError> {
        self.command(WindowCommand::SetImeAllowed(value))
            .map(|_| ())
    }
    /// Selects the IME purpose.
    pub fn set_ime_purpose(&self, value: ImePurpose) {
        let _ = self.try_set_ime_purpose(value);
    }
    /// Selects the IME purpose without discarding backend failures.
    pub fn try_set_ime_purpose(&self, value: ImePurpose) -> Result<(), PlatformError> {
        self.command(WindowCommand::SetImePurpose(value))
            .map(|_| ())
    }
    /// Sets the IME candidate-window cursor area.
    pub fn set_ime_cursor_area(&self, value: Rect<Logical, f64>) {
        let _ = self.try_set_ime_cursor_area(value);
    }
    /// Sets the IME cursor area without discarding backend failures.
    pub fn try_set_ime_cursor_area(&self, value: Rect<Logical, f64>) -> Result<(), PlatformError> {
        self.command(WindowCommand::SetImeCursorArea(value))
            .map(|_| ())
    }
    /// Starts native move dragging.
    pub fn drag_window(&self) -> Result<(), PlatformError> {
        self.command(WindowCommand::DragWindow).map(|_| ())
    }
    /// Starts native resize dragging.
    pub fn drag_resize_window(&self, direction: ResizeDirection) -> Result<(), PlatformError> {
        self.command(WindowCommand::DragResizeWindow(direction))
            .map(|_| ())
    }
    /// Returns the current theme.
    pub fn theme(&self) -> Option<Theme> {
        self.try_theme().unwrap_or(None)
    }
    /// Returns the current theme without discarding backend failures.
    pub fn try_theme(&self) -> Result<Option<Theme>, PlatformError> {
        match self.command(WindowCommand::Theme)? {
            Some(WindowValue::Theme(value)) => Ok(value),
            _ => Err(PlatformError::new("backend returned invalid theme state")),
        }
    }
    /// Returns the current monitor.
    pub fn current_monitor(&self) -> Option<Monitor> {
        self.try_current_monitor().unwrap_or(None)
    }
    /// Returns the current monitor without discarding backend failures.
    pub fn try_current_monitor(&self) -> Result<Option<Monitor>, PlatformError> {
        match self.command(WindowCommand::CurrentMonitor)? {
            Some(WindowValue::Monitor(value)) => Ok(value),
            _ => Err(PlatformError::new("backend returned invalid monitor state")),
        }
    }
}

impl fmt::Debug for Window {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Window")
            .field("id", &self.id())
            .finish()
    }
}

impl HasWindowHandle for Window {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.inner.window_handle()
    }
}

impl HasDisplayHandle for Window {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.inner.display_handle()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct FailingWindow;

    impl HasWindowHandle for FailingWindow {
        fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
            Err(HandleError::NotSupported)
        }
    }

    impl HasDisplayHandle for FailingWindow {
        fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
            Err(HandleError::NotSupported)
        }
    }

    impl backend::Window for FailingWindow {
        fn id(&self) -> WindowId {
            WindowId(7)
        }

        fn capabilities(&self) -> WindowCapabilities {
            WindowCapabilities::default()
        }

        fn command(&self, _command: WindowCommand) -> Result<Option<WindowValue>, PlatformError> {
            Err(PlatformError::new("scripted backend failure"))
        }
    }

    #[test]
    fn fallible_operations_preserve_backend_errors() {
        let window = Window::from_backend(Arc::new(FailingWindow));
        assert_eq!(
            window.try_set_title("title").unwrap_err().to_string(),
            "scripted backend failure"
        );
        assert_eq!(
            window.try_scale_factor().unwrap_err().to_string(),
            "scripted backend failure"
        );
    }

    #[test]
    fn compatibility_queries_keep_documented_fallbacks() {
        let window = Window::from_backend(Arc::new(FailingWindow));
        assert_eq!(window.scale_factor(), 1.0);
        assert!(!window.is_focused());
        assert_eq!(window.theme(), None);
    }
}
