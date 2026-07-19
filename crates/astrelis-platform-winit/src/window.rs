use std::{fmt, sync::Arc};

use astrelis_core::geometry::{Point, Size};
use astrelis_platform::{
    CursorGrabMode, CursorIcon, PlatformError, ResizeDirection, Theme, WindowAttributes,
    WindowCapabilities, WindowCommand, WindowId, WindowLevel, WindowValue, backend,
};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition};

pub(crate) struct WinitWindow {
    pub(crate) id: WindowId,
    pub(crate) native: Arc<winit::window::Window>,
}

impl fmt::Debug for WinitWindow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("WinitWindow").field(&self.id).finish()
    }
}
impl HasWindowHandle for WinitWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.native.window_handle()
    }
}
impl HasDisplayHandle for WinitWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.native.display_handle()
    }
}

impl backend::Window for WinitWindow {
    fn id(&self) -> WindowId {
        self.id
    }
    fn capabilities(&self) -> WindowCapabilities {
        WindowCapabilities {
            ime: true,
            cursor_confined: cfg!(any(target_os = "linux", target_os = "windows")),
            cursor_locked: cfg!(any(target_os = "linux", target_os = "windows")),
            transparent: true,
            drag_window: true,
            drag_resize_window: true,
        }
    }
    fn command(&self, command: WindowCommand) -> Result<Option<WindowValue>, PlatformError> {
        let result = match command {
            WindowCommand::InnerSize => {
                let size = self.native.inner_size();
                Some(WindowValue::PhysicalSize(Size::new(
                    size.width,
                    size.height,
                )))
            }
            WindowCommand::OuterPosition => {
                let position = self.native.outer_position().map_err(error)?;
                Some(WindowValue::PhysicalPosition(Point::new(
                    position.x, position.y,
                )))
            }
            WindowCommand::ScaleFactor => Some(WindowValue::Float(self.native.scale_factor())),
            WindowCommand::SetTitle(value) => {
                self.native.set_title(&value);
                None
            }
            WindowCommand::SetVisible(value) => {
                self.native.set_visible(value);
                None
            }
            WindowCommand::Focus => {
                self.native.focus_window();
                None
            }
            WindowCommand::IsFocused => Some(WindowValue::Bool(self.native.has_focus())),
            WindowCommand::SetMinimized(value) => {
                self.native.set_minimized(value);
                None
            }
            WindowCommand::SetMaximized(value) => {
                self.native.set_maximized(value);
                None
            }
            WindowCommand::IsMaximized => Some(WindowValue::Bool(self.native.is_maximized())),
            WindowCommand::SetFullscreen(value) => {
                self.native.set_fullscreen(
                    value.then(|| {
                        winit::window::Fullscreen::Borderless(self.native.current_monitor())
                    }),
                );
                None
            }
            WindowCommand::SetResizable(value) => {
                self.native.set_resizable(value);
                None
            }
            WindowCommand::SetDecorations(value) => {
                self.native.set_decorations(value);
                None
            }
            WindowCommand::SetCursorIcon(value) => {
                #[cfg(not(target_arch = "wasm32"))]
                self.native.set_cursor(map_cursor(value));
                #[cfg(target_arch = "wasm32")]
                defer_window_command(self.native.clone(), move |window| {
                    window.set_cursor(map_cursor(value));
                });
                None
            }
            WindowCommand::SetCursorVisible(value) => {
                self.native.set_cursor_visible(value);
                None
            }
            WindowCommand::SetCursorGrab(value) => {
                self.native
                    .set_cursor_grab(map_grab(value))
                    .map_err(error)?;
                None
            }
            WindowCommand::SetCursorPosition(value) => {
                self.native
                    .set_cursor_position(PhysicalPosition::new(value.x, value.y))
                    .map_err(error)?;
                None
            }
            WindowCommand::RequestRedraw => {
                self.native.request_redraw();
                None
            }
            WindowCommand::SetImeAllowed(value) => {
                #[cfg(not(target_arch = "wasm32"))]
                self.native.set_ime_allowed(value);
                #[cfg(target_arch = "wasm32")]
                defer_window_command(self.native.clone(), move |window| {
                    window.set_ime_allowed(value);
                });
                None
            }
            WindowCommand::SetImePurpose(value) => {
                let purpose = match value {
                    astrelis_platform::ImePurpose::Normal => winit::window::ImePurpose::Normal,
                    astrelis_platform::ImePurpose::Password => winit::window::ImePurpose::Password,
                    astrelis_platform::ImePurpose::Terminal => winit::window::ImePurpose::Terminal,
                };
                #[cfg(not(target_arch = "wasm32"))]
                self.native.set_ime_purpose(purpose);
                #[cfg(target_arch = "wasm32")]
                defer_window_command(self.native.clone(), move |window| {
                    window.set_ime_purpose(purpose);
                });
                None
            }
            WindowCommand::SetImeCursorArea(value) => {
                let position = LogicalPosition::new(value.origin.x, value.origin.y);
                let size = LogicalSize::new(value.size.width, value.size.height);
                #[cfg(not(target_arch = "wasm32"))]
                self.native.set_ime_cursor_area(position, size);
                #[cfg(target_arch = "wasm32")]
                defer_window_command(self.native.clone(), move |window| {
                    window.set_ime_cursor_area(position, size);
                });
                None
            }
            WindowCommand::DragWindow => {
                self.native.drag_window().map_err(error)?;
                None
            }
            WindowCommand::DragResizeWindow(value) => {
                self.native
                    .drag_resize_window(map_resize(value))
                    .map_err(error)?;
                None
            }
            WindowCommand::Theme => Some(WindowValue::Theme(self.native.theme().map(map_theme))),
            WindowCommand::CurrentMonitor => Some(WindowValue::Monitor(
                self.native.current_monitor().map(crate::convert::monitor),
            )),
            _ => return Err(PlatformError::new("unsupported window command")),
        };
        Ok(result)
    }
}

#[cfg(target_arch = "wasm32")]
fn defer_window_command(
    window: Arc<winit::window::Window>,
    command: impl FnOnce(&winit::window::Window) + 'static,
) {
    wasm_bindgen_futures::spawn_local(async move {
        command(&window);
    });
}

pub(crate) fn attributes(value: WindowAttributes) -> winit::window::WindowAttributes {
    let mut result = winit::window::Window::default_attributes()
        .with_title(value.title)
        .with_visible(value.visible)
        .with_resizable(value.resizable)
        .with_decorations(value.decorations)
        .with_transparent(value.transparent)
        .with_active(value.active)
        .with_maximized(value.maximized)
        .with_window_level(match value.level {
            WindowLevel::AlwaysOnBottom => winit::window::WindowLevel::AlwaysOnBottom,
            WindowLevel::Normal => winit::window::WindowLevel::Normal,
            WindowLevel::AlwaysOnTop => winit::window::WindowLevel::AlwaysOnTop,
        })
        .with_theme(value.theme.map(|theme| match theme {
            Theme::Light => winit::window::Theme::Light,
            Theme::Dark => winit::window::Theme::Dark,
        }));
    if let Some(size) = value.inner_size {
        result = result.with_inner_size(LogicalSize::new(size.width, size.height));
    }
    if let Some(size) = value.min_inner_size {
        result = result.with_min_inner_size(LogicalSize::new(size.width, size.height));
    }
    if let Some(size) = value.max_inner_size {
        result = result.with_max_inner_size(LogicalSize::new(size.width, size.height));
    }
    if let Some(position) = value.position {
        result = result.with_position(LogicalPosition::new(position.x, position.y));
    }
    result
}

fn error(value: impl fmt::Display) -> PlatformError {
    PlatformError::new(value.to_string())
}
fn map_theme(value: winit::window::Theme) -> Theme {
    match value {
        winit::window::Theme::Light => Theme::Light,
        winit::window::Theme::Dark => Theme::Dark,
    }
}
fn map_grab(value: CursorGrabMode) -> winit::window::CursorGrabMode {
    match value {
        CursorGrabMode::None => winit::window::CursorGrabMode::None,
        CursorGrabMode::Confined => winit::window::CursorGrabMode::Confined,
        CursorGrabMode::Locked => winit::window::CursorGrabMode::Locked,
    }
}
fn map_resize(value: ResizeDirection) -> winit::window::ResizeDirection {
    match value {
        ResizeDirection::North => winit::window::ResizeDirection::North,
        ResizeDirection::NorthEast => winit::window::ResizeDirection::NorthEast,
        ResizeDirection::East => winit::window::ResizeDirection::East,
        ResizeDirection::SouthEast => winit::window::ResizeDirection::SouthEast,
        ResizeDirection::South => winit::window::ResizeDirection::South,
        ResizeDirection::SouthWest => winit::window::ResizeDirection::SouthWest,
        ResizeDirection::West => winit::window::ResizeDirection::West,
        ResizeDirection::NorthWest => winit::window::ResizeDirection::NorthWest,
    }
}
fn map_cursor(value: CursorIcon) -> winit::window::CursorIcon {
    match value {
        CursorIcon::Default => winit::window::CursorIcon::Default,
        CursorIcon::Pointer => winit::window::CursorIcon::Pointer,
        CursorIcon::Text => winit::window::CursorIcon::Text,
        CursorIcon::Crosshair => winit::window::CursorIcon::Crosshair,
        CursorIcon::Wait => winit::window::CursorIcon::Wait,
        CursorIcon::Move => winit::window::CursorIcon::Move,
        CursorIcon::EwResize => winit::window::CursorIcon::EwResize,
        CursorIcon::NsResize => winit::window::CursorIcon::NsResize,
        CursorIcon::NwseResize => winit::window::CursorIcon::NwseResize,
        CursorIcon::NeswResize => winit::window::CursorIcon::NeswResize,
        CursorIcon::NotAllowed => winit::window::CursorIcon::NotAllowed,
        _ => winit::window::CursorIcon::Default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagonal_resize_cursors_map_to_native_equivalents() {
        assert_eq!(
            map_cursor(CursorIcon::NwseResize),
            winit::window::CursorIcon::NwseResize
        );
        assert_eq!(
            map_cursor(CursorIcon::NeswResize),
            winit::window::CursorIcon::NeswResize
        );
    }
}
