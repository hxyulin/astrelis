//! Window builder and attributes.

use crate::cursor::CursorIcon;
use crate::fullscreen::FullscreenMode;
use crate::theme::Theme;
use crate::types::{LogicalInnerSize, LogicalOuterPosition};
use crate::window_level::WindowLevel;

/// Collected configuration for creating a new window.
///
/// All sizes and positions are in logical coordinates.
#[derive(Clone, Debug)]
pub struct WindowAttributes {
    /// Window title (shown in title bar and taskbar).
    pub title: String,
    /// Initial inner size (the drawable area) in logical coordinates.
    pub inner_size: LogicalInnerSize,
    /// Minimum allowed inner size.
    pub min_inner_size: Option<LogicalInnerSize>,
    /// Maximum allowed inner size.
    pub max_inner_size: Option<LogicalInnerSize>,
    /// Initial position. `None` lets the OS choose.
    pub position: Option<LogicalOuterPosition>,
    /// Whether the window is resizable by the user.
    pub resizable: bool,
    /// Whether the window has OS decorations (title bar, borders).
    pub decorations: bool,
    /// Whether the window is visible immediately after creation.
    pub visible: bool,
    /// Whether the window background is transparent.
    pub transparent: bool,
    /// Whether the window is focused on creation.
    pub focused: bool,
    /// Whether the window is maximized on creation.
    pub maximized: bool,
    /// Fullscreen mode on creation, or `None` for windowed.
    pub fullscreen: Option<FullscreenMode>,
    /// Window opacity (0.0 = fully transparent, 1.0 = fully opaque).
    pub opacity: f32,
    /// The stacking level.
    pub window_level: WindowLevel,
    /// Initial cursor icon.
    pub cursor_icon: CursorIcon,
    /// Whether the cursor is visible over this window.
    pub cursor_visible: bool,
    /// Preferred theme. `None` follows system default.
    pub preferred_theme: Option<Theme>,
    /// Aspect ratio constraint (width / height).
    pub aspect_ratio: Option<f32>,
    /// Whether content protection (DRM) is enabled.
    pub content_protected: bool,
}

impl Default for WindowAttributes {
    fn default() -> Self {
        Self {
            title: "Astrelis".to_string(),
            inner_size: LogicalInnerSize::new(1280.0, 720.0),
            min_inner_size: None,
            max_inner_size: None,
            position: None,
            resizable: true,
            decorations: true,
            visible: true,
            transparent: false,
            focused: true,
            maximized: false,
            fullscreen: None,
            opacity: 1.0,
            window_level: WindowLevel::Normal,
            cursor_icon: CursorIcon::Default,
            cursor_visible: true,
            preferred_theme: None,
            aspect_ratio: None,
            content_protected: false,
        }
    }
}

/// Fluent builder for [`WindowAttributes`].
pub struct WindowBuilder {
    attrs: WindowAttributes,
}

impl WindowBuilder {
    /// Creates a new builder with default attributes.
    pub fn new() -> Self {
        Self {
            attrs: WindowAttributes::default(),
        }
    }

    /// Sets the window title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.attrs.title = title.into();
        self
    }

    /// Sets the initial inner size in logical coordinates.
    pub fn with_inner_size(mut self, size: LogicalInnerSize) -> Self {
        self.attrs.inner_size = size;
        self
    }

    /// Sets the minimum inner size constraint.
    pub fn with_min_inner_size(mut self, size: LogicalInnerSize) -> Self {
        self.attrs.min_inner_size = Some(size);
        self
    }

    /// Sets the maximum inner size constraint.
    pub fn with_max_inner_size(mut self, size: LogicalInnerSize) -> Self {
        self.attrs.max_inner_size = Some(size);
        self
    }

    /// Sets the initial position in logical coordinates.
    pub fn with_position(mut self, pos: LogicalOuterPosition) -> Self {
        self.attrs.position = Some(pos);
        self
    }

    /// Sets whether the window is resizable.
    pub fn with_resizable(mut self, resizable: bool) -> Self {
        self.attrs.resizable = resizable;
        self
    }

    /// Sets whether the window has OS decorations.
    pub fn with_decorations(mut self, decorations: bool) -> Self {
        self.attrs.decorations = decorations;
        self
    }

    /// Sets initial visibility.
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.attrs.visible = visible;
        self
    }

    /// Enables a transparent window background.
    pub fn with_transparent(mut self, transparent: bool) -> Self {
        self.attrs.transparent = transparent;
        self
    }

    /// Sets whether the window starts maximized.
    pub fn with_maximized(mut self, maximized: bool) -> Self {
        self.attrs.maximized = maximized;
        self
    }

    /// Sets fullscreen mode for initial creation.
    pub fn with_fullscreen(mut self, mode: FullscreenMode) -> Self {
        self.attrs.fullscreen = Some(mode);
        self
    }

    /// Sets the window opacity.
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.attrs.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// Sets the window stacking level.
    pub fn with_window_level(mut self, level: WindowLevel) -> Self {
        self.attrs.window_level = level;
        self
    }

    /// Sets the preferred theme.
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.attrs.preferred_theme = Some(theme);
        self
    }

    /// Sets an aspect ratio constraint (width / height).
    pub fn with_aspect_ratio(mut self, ratio: f32) -> Self {
        self.attrs.aspect_ratio = Some(ratio);
        self
    }

    /// Enables content protection.
    pub fn with_content_protected(mut self, protected: bool) -> Self {
        self.attrs.content_protected = protected;
        self
    }

    /// Consumes the builder and returns the final attributes.
    pub fn build(self) -> WindowAttributes {
        self.attrs
    }
}

impl Default for WindowBuilder {
    fn default() -> Self {
        Self::new()
    }
}
