//! Window trait definition.

use astrelis_core::geometry::{Logical, Point};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::cursor::{CursorGrabMode, CursorIcon};
use crate::error::WindowError;
use crate::fullscreen::FullscreenMode;
use crate::monitor::MonitorInfo;
use crate::theme::Theme;
use crate::types::{
    InnerPosition, InnerSize, LogicalInnerSize, LogicalOuterPosition, OuterPosition, OuterSize,
};
use crate::window_id::WindowId;
use crate::window_level::WindowLevel;

/// A handle to an open window.
///
/// All getters return current state. All setters are requests to the OS and
/// may not take effect immediately (or at all — check capabilities).
///
/// Implementations must also provide [`HasWindowHandle`] and
/// [`HasDisplayHandle`] for GPU surface creation.
pub trait Window: HasWindowHandle + HasDisplayHandle {
    // --- Identity ---

    /// Returns the unique identifier for this window.
    fn id(&self) -> WindowId;

    // --- Geometry (getters) ---

    /// Returns the inner size (drawable area) in physical pixels.
    fn inner_size(&self) -> InnerSize;

    /// Returns the outer size (including decorations) in physical pixels.
    fn outer_size(&self) -> OuterSize;

    /// Returns the inner position (top-left of drawable area) in physical pixels.
    fn inner_position(&self) -> Result<InnerPosition, WindowError>;

    /// Returns the outer position (top-left of window frame) in physical pixels.
    fn outer_position(&self) -> Result<OuterPosition, WindowError>;

    /// Returns the current scale factor for this window's monitor.
    fn scale_factor(&self) -> f32;

    // --- Geometry (setters) ---

    /// Requests a new inner size in logical coordinates.
    fn request_inner_size(&self, size: LogicalInnerSize);

    /// Sets the minimum inner size constraint.
    fn set_min_inner_size(&self, size: Option<LogicalInnerSize>);

    /// Sets the maximum inner size constraint.
    fn set_max_inner_size(&self, size: Option<LogicalInnerSize>);

    /// Requests a new outer position in logical coordinates.
    fn set_outer_position(&self, position: LogicalOuterPosition);

    // --- Title ---

    /// Sets the window title text.
    fn set_title(&self, title: &str);

    /// Returns the current title.
    fn title(&self) -> String;

    // --- Visibility & state ---

    /// Shows or hides the window.
    fn set_visible(&self, visible: bool);

    /// Returns whether the window is currently visible.
    fn is_visible(&self) -> bool;

    /// Requests the window to be minimized.
    fn set_minimized(&self, minimized: bool);

    /// Returns whether the window is minimized.
    fn is_minimized(&self) -> bool;

    /// Requests the window to be maximized.
    fn set_maximized(&self, maximized: bool);

    /// Returns whether the window is maximized.
    fn is_maximized(&self) -> bool;

    /// Enters or exits fullscreen mode.
    fn set_fullscreen(&self, mode: Option<FullscreenMode>);

    /// Returns the current fullscreen mode, or `None` if windowed.
    fn fullscreen(&self) -> Option<FullscreenMode>;

    /// Sets whether the window has OS decorations.
    fn set_decorations(&self, decorations: bool);

    /// Returns whether decorations are enabled.
    fn has_decorations(&self) -> bool;

    /// Sets the window opacity (0.0 to 1.0).
    fn set_opacity(&self, opacity: f32);

    /// Sets the window stacking level.
    fn set_window_level(&self, level: WindowLevel);

    /// Sets whether the user can resize the window.
    fn set_resizable(&self, resizable: bool);

    /// Returns whether the window is resizable.
    fn is_resizable(&self) -> bool;

    // --- Focus ---

    /// Brings the window to the front and gives it keyboard focus.
    fn focus(&self);

    /// Returns whether this window currently has keyboard focus.
    fn has_focus(&self) -> bool;

    // --- Cursor ---

    /// Sets the cursor icon.
    fn set_cursor_icon(&self, icon: CursorIcon);

    /// Sets cursor visibility when over this window.
    fn set_cursor_visible(&self, visible: bool);

    /// Sets the cursor grab mode.
    fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), WindowError>;

    /// Warps the cursor to a position in logical coordinates relative to
    /// the window's top-left corner.
    fn set_cursor_position(&self, position: Point<Logical>) -> Result<(), WindowError>;

    // --- Redraw ---

    /// Requests that a [`WindowEvent::RedrawRequested`](crate::event::WindowEvent::RedrawRequested)
    /// event be emitted for this window.
    fn request_redraw(&self);

    // --- Monitor ---

    /// Returns the monitor that currently contains the largest portion of
    /// this window, or `None` if undetermined.
    fn current_monitor(&self) -> Option<MonitorInfo>;

    // --- Content protection ---

    /// Sets whether the window's content should be protected from capture.
    fn set_content_protected(&self, protected: bool);

    // --- Theme ---

    /// Returns the current effective theme for this window.
    fn theme(&self) -> Option<Theme>;

    /// Sets the preferred theme override. `None` follows system preference.
    fn set_theme(&self, theme: Option<Theme>);

    // --- OS drag ---

    /// Initiates a window drag from the client area (for custom title bars).
    fn drag_window(&self) -> Result<(), WindowError>;

    /// Initiates a window resize-drag from the client area.
    fn drag_resize_window(&self, direction: ResizeDirection) -> Result<(), WindowError>;
}

/// Direction for programmatic window resize drag.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResizeDirection {
    /// North (top edge).
    North,
    /// South (bottom edge).
    South,
    /// East (right edge).
    East,
    /// West (left edge).
    West,
    /// Northwest (top-left corner).
    NorthWest,
    /// Northeast (top-right corner).
    NorthEast,
    /// Southwest (bottom-left corner).
    SouthWest,
    /// Southeast (bottom-right corner).
    SouthEast,
}
