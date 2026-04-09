//! Runtime capability detection.

use std::collections::HashSet;

/// A windowing feature that may or may not be supported on the current platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Capability {
    /// Setting window opacity / transparency level.
    WindowOpacity,
    /// Changing the window level (always on top / always on bottom).
    WindowLevel,
    /// Toggling window decorations at runtime.
    Decorations,
    /// Programmatically minimizing windows.
    Minimize,
    /// Programmatically maximizing windows.
    Maximize,
    /// Borderless fullscreen.
    FullscreenBorderless,
    /// Exclusive fullscreen with video mode changes.
    FullscreenExclusive,
    /// Setting minimum/maximum window size constraints.
    SizeConstraints,
    /// Setting a window aspect ratio constraint.
    AspectRatio,
    /// Cursor confinement ([`CursorGrabMode::Confined`](crate::cursor::CursorGrabMode::Confined)).
    CursorConfine,
    /// Cursor lock ([`CursorGrabMode::Locked`](crate::cursor::CursorGrabMode::Locked)).
    CursorLock,
    /// Custom cursor images.
    CustomCursor,
    /// Window drag from client area (custom title bars).
    DragWindow,
    /// Window resize from client area.
    DragResizeWindow,
    /// Setting the window icon.
    WindowIcon,
    /// Detecting the system theme preference.
    ThemeDetection,
    /// Transparent window backgrounds.
    TransparentBackground,
    /// Touch input events.
    TouchInput,
    /// Content protection / DRM flag.
    ContentProtection,
    /// Input method editor (IME) support.
    Ime,
}

/// A set of capabilities supported by the current backend/platform.
#[derive(Clone, Debug, Default)]
pub struct Capabilities {
    supported: HashSet<Capability>,
}

impl Capabilities {
    /// Returns `true` if the given capability is supported.
    pub fn supports(&self, cap: Capability) -> bool {
        self.supported.contains(&cap)
    }

    /// Returns an iterator over all supported capabilities.
    pub fn iter(&self) -> impl Iterator<Item = &Capability> {
        self.supported.iter()
    }

    /// Adds a capability to the set.
    ///
    /// Intended for backend implementations.
    pub fn insert(&mut self, cap: Capability) {
        self.supported.insert(cap);
    }
}
