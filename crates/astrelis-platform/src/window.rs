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
        match self.command(WindowCommand::ScaleFactor) {
            Ok(Some(WindowValue::Float(v))) => v,
            _ => 1.0,
        }
    }
    /// Changes the title.
    pub fn set_title(&self, title: impl Into<String>) {
        let _ = self.command(WindowCommand::SetTitle(title.into()));
    }
    /// Changes visibility.
    pub fn set_visible(&self, visible: bool) {
        let _ = self.command(WindowCommand::SetVisible(visible));
    }
    /// Requests keyboard focus.
    pub fn focus(&self) {
        let _ = self.command(WindowCommand::Focus);
    }
    /// Reports focus.
    pub fn is_focused(&self) -> bool {
        matches!(
            self.command(WindowCommand::IsFocused),
            Ok(Some(WindowValue::Bool(true)))
        )
    }
    /// Changes minimized state.
    pub fn set_minimized(&self, value: bool) {
        let _ = self.command(WindowCommand::SetMinimized(value));
    }
    /// Changes maximized state.
    pub fn set_maximized(&self, value: bool) {
        let _ = self.command(WindowCommand::SetMaximized(value));
    }
    /// Enables borderless fullscreen on the current monitor.
    pub fn set_borderless_fullscreen(&self, value: bool) {
        let _ = self.command(WindowCommand::SetFullscreen(value));
    }
    /// Changes resizability.
    pub fn set_resizable(&self, value: bool) {
        let _ = self.command(WindowCommand::SetResizable(value));
    }
    /// Changes native decorations.
    pub fn set_decorations(&self, value: bool) {
        let _ = self.command(WindowCommand::SetDecorations(value));
    }
    /// Changes the standard cursor.
    pub fn set_cursor_icon(&self, value: CursorIcon) {
        let _ = self.command(WindowCommand::SetCursorIcon(value));
    }
    /// Changes cursor visibility.
    pub fn set_cursor_visible(&self, value: bool) {
        let _ = self.command(WindowCommand::SetCursorVisible(value));
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
        let _ = self.command(WindowCommand::RequestRedraw);
    }
    /// Enables or disables IME.
    pub fn set_ime_allowed(&self, value: bool) {
        let _ = self.command(WindowCommand::SetImeAllowed(value));
    }
    /// Selects the IME purpose.
    pub fn set_ime_purpose(&self, value: ImePurpose) {
        let _ = self.command(WindowCommand::SetImePurpose(value));
    }
    /// Sets the IME candidate-window cursor area.
    pub fn set_ime_cursor_area(&self, value: Rect<Logical, f64>) {
        let _ = self.command(WindowCommand::SetImeCursorArea(value));
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
        match self.command(WindowCommand::Theme) {
            Ok(Some(WindowValue::Theme(v))) => v,
            _ => None,
        }
    }
    /// Returns the current monitor.
    pub fn current_monitor(&self) -> Option<Monitor> {
        match self.command(WindowCommand::CurrentMonitor) {
            Ok(Some(WindowValue::Monitor(v))) => v,
            _ => None,
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
