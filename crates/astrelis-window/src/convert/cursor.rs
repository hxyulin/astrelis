//! Cursor type conversions from astrelis to winit.

use crate::cursor::CursorIcon;

/// Converts an astrelis CursorIcon to a winit CursorIcon.
pub(crate) fn to_winit_cursor(icon: CursorIcon) -> winit::window::CursorIcon {
    use winit::window::CursorIcon as W;
    match icon {
        CursorIcon::Default => W::Default,
        CursorIcon::ContextMenu => W::ContextMenu,
        CursorIcon::Help => W::Help,
        CursorIcon::Pointer => W::Pointer,
        CursorIcon::Progress => W::Progress,
        CursorIcon::Wait => W::Wait,
        CursorIcon::Cell => W::Cell,
        CursorIcon::Text => W::Text,
        CursorIcon::VerticalText => W::VerticalText,
        CursorIcon::Crosshair => W::Crosshair,
        CursorIcon::Alias => W::Alias,
        CursorIcon::Copy => W::Copy,
        CursorIcon::Move => W::Move,
        CursorIcon::NoDrop => W::NoDrop,
        CursorIcon::NotAllowed => W::NotAllowed,
        CursorIcon::Grab => W::Grab,
        CursorIcon::Grabbing => W::Grabbing,
        CursorIcon::EResize => W::EResize,
        CursorIcon::NResize => W::NResize,
        CursorIcon::NeResize => W::NeResize,
        CursorIcon::NwResize => W::NwResize,
        CursorIcon::SResize => W::SResize,
        CursorIcon::SeResize => W::SeResize,
        CursorIcon::SwResize => W::SwResize,
        CursorIcon::WResize => W::WResize,
        CursorIcon::EwResize => W::EwResize,
        CursorIcon::NsResize => W::NsResize,
        CursorIcon::NeswResize => W::NeswResize,
        CursorIcon::NwseResize => W::NwseResize,
        CursorIcon::ColResize => W::ColResize,
        CursorIcon::RowResize => W::RowResize,
        CursorIcon::AllScroll => W::AllScroll,
        CursorIcon::ZoomIn => W::ZoomIn,
        CursorIcon::ZoomOut => W::ZoomOut,
        _ => W::Default, // fallback for future variants
    }
}
