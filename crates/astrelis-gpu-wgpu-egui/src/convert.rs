//! Conversion functions between astrelis-window types and egui types.

use astrelis_window::cursor::CursorIcon;
use astrelis_window::keyboard::{Key, KeyCode, NamedKey};
use astrelis_window::mouse::MouseButton;

/// Translate an astrelis [`KeyCode`] (physical key) to an [`egui::Key`].
pub fn translate_key_code(key: KeyCode) -> Option<egui::Key> {
    Some(match key {
        KeyCode::ArrowDown => egui::Key::ArrowDown,
        KeyCode::ArrowLeft => egui::Key::ArrowLeft,
        KeyCode::ArrowRight => egui::Key::ArrowRight,
        KeyCode::ArrowUp => egui::Key::ArrowUp,
        KeyCode::Escape => egui::Key::Escape,
        KeyCode::Tab => egui::Key::Tab,
        KeyCode::Backspace => egui::Key::Backspace,
        KeyCode::Enter | KeyCode::NumpadEnter => egui::Key::Enter,
        KeyCode::Insert => egui::Key::Insert,
        KeyCode::Delete => egui::Key::Delete,
        KeyCode::Home => egui::Key::Home,
        KeyCode::End => egui::Key::End,
        KeyCode::PageUp => egui::Key::PageUp,
        KeyCode::PageDown => egui::Key::PageDown,
        KeyCode::Space => egui::Key::Space,

        // Punctuation / symbols
        KeyCode::Comma => egui::Key::Comma,
        KeyCode::Period => egui::Key::Period,
        KeyCode::Semicolon => egui::Key::Semicolon,
        KeyCode::Backslash => egui::Key::Backslash,
        KeyCode::Slash | KeyCode::NumpadDivide => egui::Key::Slash,
        KeyCode::BracketLeft => egui::Key::OpenBracket,
        KeyCode::BracketRight => egui::Key::CloseBracket,
        KeyCode::Backquote => egui::Key::Backtick,
        KeyCode::Quote => egui::Key::Quote,
        KeyCode::Minus | KeyCode::NumpadSubtract => egui::Key::Minus,
        KeyCode::NumpadAdd => egui::Key::Plus,
        KeyCode::Equal => egui::Key::Equals,

        // Digits
        KeyCode::Digit0 | KeyCode::Numpad0 => egui::Key::Num0,
        KeyCode::Digit1 | KeyCode::Numpad1 => egui::Key::Num1,
        KeyCode::Digit2 | KeyCode::Numpad2 => egui::Key::Num2,
        KeyCode::Digit3 | KeyCode::Numpad3 => egui::Key::Num3,
        KeyCode::Digit4 | KeyCode::Numpad4 => egui::Key::Num4,
        KeyCode::Digit5 | KeyCode::Numpad5 => egui::Key::Num5,
        KeyCode::Digit6 | KeyCode::Numpad6 => egui::Key::Num6,
        KeyCode::Digit7 | KeyCode::Numpad7 => egui::Key::Num7,
        KeyCode::Digit8 | KeyCode::Numpad8 => egui::Key::Num8,
        KeyCode::Digit9 | KeyCode::Numpad9 => egui::Key::Num9,

        // Letters
        KeyCode::KeyA => egui::Key::A,
        KeyCode::KeyB => egui::Key::B,
        KeyCode::KeyC => egui::Key::C,
        KeyCode::KeyD => egui::Key::D,
        KeyCode::KeyE => egui::Key::E,
        KeyCode::KeyF => egui::Key::F,
        KeyCode::KeyG => egui::Key::G,
        KeyCode::KeyH => egui::Key::H,
        KeyCode::KeyI => egui::Key::I,
        KeyCode::KeyJ => egui::Key::J,
        KeyCode::KeyK => egui::Key::K,
        KeyCode::KeyL => egui::Key::L,
        KeyCode::KeyM => egui::Key::M,
        KeyCode::KeyN => egui::Key::N,
        KeyCode::KeyO => egui::Key::O,
        KeyCode::KeyP => egui::Key::P,
        KeyCode::KeyQ => egui::Key::Q,
        KeyCode::KeyR => egui::Key::R,
        KeyCode::KeyS => egui::Key::S,
        KeyCode::KeyT => egui::Key::T,
        KeyCode::KeyU => egui::Key::U,
        KeyCode::KeyV => egui::Key::V,
        KeyCode::KeyW => egui::Key::W,
        KeyCode::KeyX => egui::Key::X,
        KeyCode::KeyY => egui::Key::Y,
        KeyCode::KeyZ => egui::Key::Z,

        // Function keys
        KeyCode::F1 => egui::Key::F1,
        KeyCode::F2 => egui::Key::F2,
        KeyCode::F3 => egui::Key::F3,
        KeyCode::F4 => egui::Key::F4,
        KeyCode::F5 => egui::Key::F5,
        KeyCode::F6 => egui::Key::F6,
        KeyCode::F7 => egui::Key::F7,
        KeyCode::F8 => egui::Key::F8,
        KeyCode::F9 => egui::Key::F9,
        KeyCode::F10 => egui::Key::F10,
        KeyCode::F11 => egui::Key::F11,
        KeyCode::F12 => egui::Key::F12,
        KeyCode::F13 => egui::Key::F13,
        KeyCode::F14 => egui::Key::F14,
        KeyCode::F15 => egui::Key::F15,
        KeyCode::F16 => egui::Key::F16,
        KeyCode::F17 => egui::Key::F17,
        KeyCode::F18 => egui::Key::F18,
        KeyCode::F19 => egui::Key::F19,
        KeyCode::F20 => egui::Key::F20,

        _ => return None,
    })
}

/// Translate an astrelis [`NamedKey`] (logical non-character key) to an [`egui::Key`].
pub fn translate_named_key(named: NamedKey) -> Option<egui::Key> {
    Some(match named {
        NamedKey::Enter => egui::Key::Enter,
        NamedKey::Tab => egui::Key::Tab,
        NamedKey::Space => egui::Key::Space,
        NamedKey::Backspace => egui::Key::Backspace,
        NamedKey::Escape => egui::Key::Escape,
        NamedKey::Delete => egui::Key::Delete,
        NamedKey::ArrowUp => egui::Key::ArrowUp,
        NamedKey::ArrowDown => egui::Key::ArrowDown,
        NamedKey::ArrowLeft => egui::Key::ArrowLeft,
        NamedKey::ArrowRight => egui::Key::ArrowRight,
        NamedKey::Home => egui::Key::Home,
        NamedKey::End => egui::Key::End,
        NamedKey::PageUp => egui::Key::PageUp,
        NamedKey::PageDown => egui::Key::PageDown,
        NamedKey::Insert => egui::Key::Insert,
        NamedKey::F1 => egui::Key::F1,
        NamedKey::F2 => egui::Key::F2,
        NamedKey::F3 => egui::Key::F3,
        NamedKey::F4 => egui::Key::F4,
        NamedKey::F5 => egui::Key::F5,
        NamedKey::F6 => egui::Key::F6,
        NamedKey::F7 => egui::Key::F7,
        NamedKey::F8 => egui::Key::F8,
        NamedKey::F9 => egui::Key::F9,
        NamedKey::F10 => egui::Key::F10,
        NamedKey::F11 => egui::Key::F11,
        NamedKey::F12 => egui::Key::F12,
        NamedKey::F13 => egui::Key::F13,
        NamedKey::F14 => egui::Key::F14,
        NamedKey::F15 => egui::Key::F15,
        NamedKey::F16 => egui::Key::F16,
        NamedKey::F17 => egui::Key::F17,
        NamedKey::F18 => egui::Key::F18,
        NamedKey::F19 => egui::Key::F19,
        NamedKey::F20 => egui::Key::F20,
        NamedKey::F21 => egui::Key::F21,
        NamedKey::F22 => egui::Key::F22,
        NamedKey::F23 => egui::Key::F23,
        NamedKey::F24 => egui::Key::F24,
        _ => return None,
    })
}

/// Translate an astrelis [`Key`] (logical key value) to an [`egui::Key`].
pub fn translate_key(key: &Key) -> Option<egui::Key> {
    match key {
        Key::Named(named) => translate_named_key(*named),
        Key::Character(s) => egui::Key::from_name(s.as_str()),
        Key::Unidentified => None,
        _ => None,
    }
}

/// Translate an astrelis [`MouseButton`] to an [`egui::PointerButton`].
pub fn translate_mouse_button(button: MouseButton) -> Option<egui::PointerButton> {
    Some(match button {
        MouseButton::Left => egui::PointerButton::Primary,
        MouseButton::Right => egui::PointerButton::Secondary,
        MouseButton::Middle => egui::PointerButton::Middle,
        MouseButton::Back => egui::PointerButton::Extra1,
        MouseButton::Forward => egui::PointerButton::Extra2,
        MouseButton::Other(_) | _ => return None,
    })
}

/// Translate an [`egui::CursorIcon`] to an astrelis [`CursorIcon`].
pub fn translate_cursor_icon(icon: egui::CursorIcon) -> CursorIcon {
    match icon {
        egui::CursorIcon::Default => CursorIcon::Default,
        egui::CursorIcon::None => CursorIcon::Default,
        egui::CursorIcon::ContextMenu => CursorIcon::ContextMenu,
        egui::CursorIcon::Help => CursorIcon::Help,
        egui::CursorIcon::PointingHand => CursorIcon::Pointer,
        egui::CursorIcon::Progress => CursorIcon::Progress,
        egui::CursorIcon::Wait => CursorIcon::Wait,
        egui::CursorIcon::Cell => CursorIcon::Cell,
        egui::CursorIcon::Crosshair => CursorIcon::Crosshair,
        egui::CursorIcon::Text => CursorIcon::Text,
        egui::CursorIcon::VerticalText => CursorIcon::VerticalText,
        egui::CursorIcon::Alias => CursorIcon::Alias,
        egui::CursorIcon::Copy => CursorIcon::Copy,
        egui::CursorIcon::Move => CursorIcon::Move,
        egui::CursorIcon::NoDrop => CursorIcon::NoDrop,
        egui::CursorIcon::NotAllowed => CursorIcon::NotAllowed,
        egui::CursorIcon::Grab => CursorIcon::Grab,
        egui::CursorIcon::Grabbing => CursorIcon::Grabbing,
        egui::CursorIcon::ResizeEast => CursorIcon::EResize,
        egui::CursorIcon::ResizeNorth => CursorIcon::NResize,
        egui::CursorIcon::ResizeNorthEast => CursorIcon::NeResize,
        egui::CursorIcon::ResizeNorthWest => CursorIcon::NwResize,
        egui::CursorIcon::ResizeSouth => CursorIcon::SResize,
        egui::CursorIcon::ResizeSouthEast => CursorIcon::SeResize,
        egui::CursorIcon::ResizeSouthWest => CursorIcon::SwResize,
        egui::CursorIcon::ResizeWest => CursorIcon::WResize,
        egui::CursorIcon::ResizeHorizontal => CursorIcon::EwResize,
        egui::CursorIcon::ResizeVertical => CursorIcon::NsResize,
        egui::CursorIcon::ResizeNeSw => CursorIcon::NeswResize,
        egui::CursorIcon::ResizeNwSe => CursorIcon::NwseResize,
        egui::CursorIcon::ResizeColumn => CursorIcon::ColResize,
        egui::CursorIcon::ResizeRow => CursorIcon::RowResize,
        egui::CursorIcon::AllScroll => CursorIcon::AllScroll,
        egui::CursorIcon::ZoomIn => CursorIcon::ZoomIn,
        egui::CursorIcon::ZoomOut => CursorIcon::ZoomOut,
    }
}
