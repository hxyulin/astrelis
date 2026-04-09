//! Platform-specific capability detection.

use astrelis_window::capability::{Capabilities, Capability};

/// Builds the capability set for the current platform.
pub(crate) fn build_capabilities() -> Capabilities {
    let mut caps = Capabilities::default();

    // Common to all winit platforms
    caps.insert(Capability::Maximize);
    caps.insert(Capability::Minimize);
    caps.insert(Capability::FullscreenBorderless);
    caps.insert(Capability::FullscreenExclusive);
    caps.insert(Capability::SizeConstraints);
    caps.insert(Capability::CursorLock);
    caps.insert(Capability::ThemeDetection);
    caps.insert(Capability::Ime);
    caps.insert(Capability::Decorations);

    #[cfg(target_os = "windows")]
    {
        caps.insert(Capability::WindowOpacity);
        caps.insert(Capability::WindowLevel);
        caps.insert(Capability::CursorConfine);
        caps.insert(Capability::WindowIcon);
        caps.insert(Capability::DragWindow);
        caps.insert(Capability::DragResizeWindow);
        caps.insert(Capability::TransparentBackground);
        caps.insert(Capability::ContentProtection);
    }

    #[cfg(target_os = "macos")]
    {
        caps.insert(Capability::WindowOpacity);
        caps.insert(Capability::WindowLevel);
        caps.insert(Capability::DragWindow);
        caps.insert(Capability::TransparentBackground);
        caps.insert(Capability::ContentProtection);
    }

    #[cfg(target_os = "linux")]
    {
        caps.insert(Capability::WindowOpacity);
        caps.insert(Capability::WindowLevel);
        caps.insert(Capability::CursorConfine);
        caps.insert(Capability::DragWindow);
        caps.insert(Capability::DragResizeWindow);
        caps.insert(Capability::WindowIcon);
        caps.insert(Capability::TransparentBackground);
    }

    caps
}
