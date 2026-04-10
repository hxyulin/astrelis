//! Keyboard type conversions from winit to astrelis.

use astrelis_window::keyboard::{Key, KeyCode, KeyLocation, ModifiersState, NamedKey};
use winit::keyboard::PhysicalKey;

/// Converts a winit physical key to an astrelis KeyCode.
pub(crate) fn convert_key_code(key: PhysicalKey) -> KeyCode {
    astrelis_profiling::profile_function!();
    match key {
        PhysicalKey::Code(code) => convert_winit_key_code(code),
        PhysicalKey::Unidentified(_) => KeyCode::Escape, // fallback
    }
}

/// Converts a winit KeyCode to astrelis KeyCode.
fn convert_winit_key_code(code: winit::keyboard::KeyCode) -> KeyCode {
    use winit::keyboard::KeyCode as W;
    match code {
        W::Escape => KeyCode::Escape,
        W::F1 => KeyCode::F1,
        W::F2 => KeyCode::F2,
        W::F3 => KeyCode::F3,
        W::F4 => KeyCode::F4,
        W::F5 => KeyCode::F5,
        W::F6 => KeyCode::F6,
        W::F7 => KeyCode::F7,
        W::F8 => KeyCode::F8,
        W::F9 => KeyCode::F9,
        W::F10 => KeyCode::F10,
        W::F11 => KeyCode::F11,
        W::F12 => KeyCode::F12,
        W::F13 => KeyCode::F13,
        W::F14 => KeyCode::F14,
        W::F15 => KeyCode::F15,
        W::F16 => KeyCode::F16,
        W::F17 => KeyCode::F17,
        W::F18 => KeyCode::F18,
        W::F19 => KeyCode::F19,
        W::F20 => KeyCode::F20,
        W::F21 => KeyCode::F21,
        W::F22 => KeyCode::F22,
        W::F23 => KeyCode::F23,
        W::F24 => KeyCode::F24,
        W::Backquote => KeyCode::Backquote,
        W::Digit1 => KeyCode::Digit1,
        W::Digit2 => KeyCode::Digit2,
        W::Digit3 => KeyCode::Digit3,
        W::Digit4 => KeyCode::Digit4,
        W::Digit5 => KeyCode::Digit5,
        W::Digit6 => KeyCode::Digit6,
        W::Digit7 => KeyCode::Digit7,
        W::Digit8 => KeyCode::Digit8,
        W::Digit9 => KeyCode::Digit9,
        W::Digit0 => KeyCode::Digit0,
        W::Minus => KeyCode::Minus,
        W::Equal => KeyCode::Equal,
        W::Backspace => KeyCode::Backspace,
        W::Tab => KeyCode::Tab,
        W::KeyA => KeyCode::KeyA,
        W::KeyB => KeyCode::KeyB,
        W::KeyC => KeyCode::KeyC,
        W::KeyD => KeyCode::KeyD,
        W::KeyE => KeyCode::KeyE,
        W::KeyF => KeyCode::KeyF,
        W::KeyG => KeyCode::KeyG,
        W::KeyH => KeyCode::KeyH,
        W::KeyI => KeyCode::KeyI,
        W::KeyJ => KeyCode::KeyJ,
        W::KeyK => KeyCode::KeyK,
        W::KeyL => KeyCode::KeyL,
        W::KeyM => KeyCode::KeyM,
        W::KeyN => KeyCode::KeyN,
        W::KeyO => KeyCode::KeyO,
        W::KeyP => KeyCode::KeyP,
        W::KeyQ => KeyCode::KeyQ,
        W::KeyR => KeyCode::KeyR,
        W::KeyS => KeyCode::KeyS,
        W::KeyT => KeyCode::KeyT,
        W::KeyU => KeyCode::KeyU,
        W::KeyV => KeyCode::KeyV,
        W::KeyW => KeyCode::KeyW,
        W::KeyX => KeyCode::KeyX,
        W::KeyY => KeyCode::KeyY,
        W::KeyZ => KeyCode::KeyZ,
        W::BracketLeft => KeyCode::BracketLeft,
        W::BracketRight => KeyCode::BracketRight,
        W::Backslash => KeyCode::Backslash,
        W::CapsLock => KeyCode::CapsLock,
        W::Semicolon => KeyCode::Semicolon,
        W::Quote => KeyCode::Quote,
        W::Enter => KeyCode::Enter,
        W::ShiftLeft => KeyCode::ShiftLeft,
        W::Comma => KeyCode::Comma,
        W::Period => KeyCode::Period,
        W::Slash => KeyCode::Slash,
        W::ShiftRight => KeyCode::ShiftRight,
        W::ControlLeft => KeyCode::ControlLeft,
        W::AltLeft => KeyCode::AltLeft,
        W::SuperLeft => KeyCode::MetaLeft,
        W::Space => KeyCode::Space,
        W::SuperRight => KeyCode::MetaRight,
        W::AltRight => KeyCode::AltRight,
        W::ControlRight => KeyCode::ControlRight,
        W::PrintScreen => KeyCode::PrintScreen,
        W::ScrollLock => KeyCode::ScrollLock,
        W::Pause => KeyCode::Pause,
        W::Insert => KeyCode::Insert,
        W::Home => KeyCode::Home,
        W::PageUp => KeyCode::PageUp,
        W::Delete => KeyCode::Delete,
        W::End => KeyCode::End,
        W::PageDown => KeyCode::PageDown,
        W::ArrowUp => KeyCode::ArrowUp,
        W::ArrowLeft => KeyCode::ArrowLeft,
        W::ArrowDown => KeyCode::ArrowDown,
        W::ArrowRight => KeyCode::ArrowRight,
        W::NumLock => KeyCode::NumLock,
        W::NumpadDivide => KeyCode::NumpadDivide,
        W::NumpadMultiply => KeyCode::NumpadMultiply,
        W::NumpadSubtract => KeyCode::NumpadSubtract,
        W::Numpad7 => KeyCode::Numpad7,
        W::Numpad8 => KeyCode::Numpad8,
        W::Numpad9 => KeyCode::Numpad9,
        W::NumpadAdd => KeyCode::NumpadAdd,
        W::Numpad4 => KeyCode::Numpad4,
        W::Numpad5 => KeyCode::Numpad5,
        W::Numpad6 => KeyCode::Numpad6,
        W::Numpad1 => KeyCode::Numpad1,
        W::Numpad2 => KeyCode::Numpad2,
        W::Numpad3 => KeyCode::Numpad3,
        W::Numpad0 => KeyCode::Numpad0,
        W::NumpadDecimal => KeyCode::NumpadDecimal,
        W::NumpadEnter => KeyCode::NumpadEnter,
        W::MediaPlayPause => KeyCode::MediaPlayPause,
        W::MediaStop => KeyCode::MediaStop,
        W::MediaTrackNext => KeyCode::MediaTrackNext,
        W::MediaTrackPrevious => KeyCode::MediaTrackPrevious,
        W::AudioVolumeUp => KeyCode::AudioVolumeUp,
        W::AudioVolumeDown => KeyCode::AudioVolumeDown,
        W::AudioVolumeMute => KeyCode::AudioVolumeMute,
        W::BrowserBack => KeyCode::BrowserBack,
        W::BrowserForward => KeyCode::BrowserForward,
        W::BrowserRefresh => KeyCode::BrowserRefresh,
        W::ContextMenu => KeyCode::ContextMenu,
        W::IntlBackslash => KeyCode::IntlBackslash,
        _ => KeyCode::Escape, // fallback for unmapped keys
    }
}

/// Converts a winit logical key to an astrelis Key.
pub(crate) fn convert_key(key: &winit::keyboard::Key) -> Key {
    match key {
        winit::keyboard::Key::Named(named) => {
            if let Some(n) = convert_named_key(*named) {
                Key::Named(n)
            } else {
                Key::Unidentified
            }
        }
        winit::keyboard::Key::Character(s) => Key::Character(s.to_string()),
        _ => Key::Unidentified,
    }
}

/// Converts a winit named key to an astrelis NamedKey.
fn convert_named_key(key: winit::keyboard::NamedKey) -> Option<NamedKey> {
    use winit::keyboard::NamedKey as W;
    Some(match key {
        W::Enter => NamedKey::Enter,
        W::Tab => NamedKey::Tab,
        W::Space => NamedKey::Space,
        W::Backspace => NamedKey::Backspace,
        W::Escape => NamedKey::Escape,
        W::Delete => NamedKey::Delete,
        W::ArrowUp => NamedKey::ArrowUp,
        W::ArrowDown => NamedKey::ArrowDown,
        W::ArrowLeft => NamedKey::ArrowLeft,
        W::ArrowRight => NamedKey::ArrowRight,
        W::Home => NamedKey::Home,
        W::End => NamedKey::End,
        W::PageUp => NamedKey::PageUp,
        W::PageDown => NamedKey::PageDown,
        W::Insert => NamedKey::Insert,
        W::F1 => NamedKey::F1,
        W::F2 => NamedKey::F2,
        W::F3 => NamedKey::F3,
        W::F4 => NamedKey::F4,
        W::F5 => NamedKey::F5,
        W::F6 => NamedKey::F6,
        W::F7 => NamedKey::F7,
        W::F8 => NamedKey::F8,
        W::F9 => NamedKey::F9,
        W::F10 => NamedKey::F10,
        W::F11 => NamedKey::F11,
        W::F12 => NamedKey::F12,
        W::F13 => NamedKey::F13,
        W::F14 => NamedKey::F14,
        W::F15 => NamedKey::F15,
        W::F16 => NamedKey::F16,
        W::F17 => NamedKey::F17,
        W::F18 => NamedKey::F18,
        W::F19 => NamedKey::F19,
        W::F20 => NamedKey::F20,
        W::F21 => NamedKey::F21,
        W::F22 => NamedKey::F22,
        W::F23 => NamedKey::F23,
        W::F24 => NamedKey::F24,
        W::PrintScreen => NamedKey::PrintScreen,
        W::ScrollLock => NamedKey::ScrollLock,
        W::Pause => NamedKey::Pause,
        W::CapsLock => NamedKey::CapsLock,
        W::NumLock => NamedKey::NumLock,
        W::Shift => NamedKey::Shift,
        W::Control => NamedKey::Control,
        W::Alt => NamedKey::Alt,
        W::Super => NamedKey::Meta,
        W::ContextMenu => NamedKey::ContextMenu,
        W::MediaPlayPause => NamedKey::MediaPlayPause,
        W::MediaStop => NamedKey::MediaStop,
        W::MediaTrackNext => NamedKey::MediaTrackNext,
        W::MediaTrackPrevious => NamedKey::MediaTrackPrevious,
        W::AudioVolumeUp => NamedKey::AudioVolumeUp,
        W::AudioVolumeDown => NamedKey::AudioVolumeDown,
        W::AudioVolumeMute => NamedKey::AudioVolumeMute,
        _ => return None,
    })
}

/// Converts winit key location to astrelis KeyLocation.
pub(crate) fn convert_key_location(
    location: winit::keyboard::KeyLocation,
) -> KeyLocation {
    match location {
        winit::keyboard::KeyLocation::Standard => KeyLocation::Standard,
        winit::keyboard::KeyLocation::Left => KeyLocation::Left,
        winit::keyboard::KeyLocation::Right => KeyLocation::Right,
        winit::keyboard::KeyLocation::Numpad => KeyLocation::Numpad,
    }
}

/// Converts winit modifiers to astrelis ModifiersState.
pub(crate) fn convert_modifiers(mods: &winit::event::Modifiers) -> ModifiersState {
    let state = mods.state();
    ModifiersState {
        shift: state.shift_key(),
        control: state.control_key(),
        alt: state.alt_key(),
        meta: state.super_key(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::{KeyCode as WinitKeyCode, NativeKeyCode, PhysicalKey};

    /// Helper to convert a winit KeyCode (via PhysicalKey::Code) and return the
    /// astrelis KeyCode.
    fn convert(code: WinitKeyCode) -> KeyCode {
        convert_key_code(PhysicalKey::Code(code))
    }

    #[test]
    fn common_letter_keys() {
        assert_eq!(convert(WinitKeyCode::KeyW), KeyCode::KeyW);
        assert_eq!(convert(WinitKeyCode::KeyA), KeyCode::KeyA);
        assert_eq!(convert(WinitKeyCode::KeyS), KeyCode::KeyS);
        assert_eq!(convert(WinitKeyCode::KeyD), KeyCode::KeyD);
        assert_eq!(convert(WinitKeyCode::KeyZ), KeyCode::KeyZ);
    }

    #[test]
    fn space_enter_escape() {
        assert_eq!(convert(WinitKeyCode::Space), KeyCode::Space);
        assert_eq!(convert(WinitKeyCode::Enter), KeyCode::Enter);
        assert_eq!(convert(WinitKeyCode::Escape), KeyCode::Escape);
    }

    #[test]
    fn modifier_keys() {
        assert_eq!(convert(WinitKeyCode::ShiftLeft), KeyCode::ShiftLeft);
        assert_eq!(convert(WinitKeyCode::ShiftRight), KeyCode::ShiftRight);
        assert_eq!(convert(WinitKeyCode::ControlLeft), KeyCode::ControlLeft);
        assert_eq!(convert(WinitKeyCode::ControlRight), KeyCode::ControlRight);
        assert_eq!(convert(WinitKeyCode::AltLeft), KeyCode::AltLeft);
        assert_eq!(convert(WinitKeyCode::AltRight), KeyCode::AltRight);
        // winit Super maps to astrelis Meta
        assert_eq!(convert(WinitKeyCode::SuperLeft), KeyCode::MetaLeft);
        assert_eq!(convert(WinitKeyCode::SuperRight), KeyCode::MetaRight);
    }

    #[test]
    fn function_keys() {
        assert_eq!(convert(WinitKeyCode::F1), KeyCode::F1);
        assert_eq!(convert(WinitKeyCode::F12), KeyCode::F12);
        assert_eq!(convert(WinitKeyCode::F24), KeyCode::F24);
    }

    #[test]
    fn arrow_keys() {
        assert_eq!(convert(WinitKeyCode::ArrowUp), KeyCode::ArrowUp);
        assert_eq!(convert(WinitKeyCode::ArrowDown), KeyCode::ArrowDown);
        assert_eq!(convert(WinitKeyCode::ArrowLeft), KeyCode::ArrowLeft);
        assert_eq!(convert(WinitKeyCode::ArrowRight), KeyCode::ArrowRight);
    }

    #[test]
    fn digit_keys() {
        assert_eq!(convert(WinitKeyCode::Digit0), KeyCode::Digit0);
        assert_eq!(convert(WinitKeyCode::Digit1), KeyCode::Digit1);
        assert_eq!(convert(WinitKeyCode::Digit9), KeyCode::Digit9);
    }

    #[test]
    fn numpad_keys() {
        assert_eq!(convert(WinitKeyCode::Numpad0), KeyCode::Numpad0);
        assert_eq!(convert(WinitKeyCode::Numpad5), KeyCode::Numpad5);
        assert_eq!(convert(WinitKeyCode::NumpadEnter), KeyCode::NumpadEnter);
        assert_eq!(convert(WinitKeyCode::NumpadAdd), KeyCode::NumpadAdd);
        assert_eq!(convert(WinitKeyCode::NumpadDecimal), KeyCode::NumpadDecimal);
    }

    #[test]
    fn punctuation_and_editing_keys() {
        assert_eq!(convert(WinitKeyCode::Tab), KeyCode::Tab);
        assert_eq!(convert(WinitKeyCode::Backspace), KeyCode::Backspace);
        assert_eq!(convert(WinitKeyCode::Delete), KeyCode::Delete);
        assert_eq!(convert(WinitKeyCode::Insert), KeyCode::Insert);
        assert_eq!(convert(WinitKeyCode::Home), KeyCode::Home);
        assert_eq!(convert(WinitKeyCode::End), KeyCode::End);
        assert_eq!(convert(WinitKeyCode::PageUp), KeyCode::PageUp);
        assert_eq!(convert(WinitKeyCode::PageDown), KeyCode::PageDown);
    }

    #[test]
    fn unmapped_winit_key_code_falls_back_to_escape() {
        // `Fn` is a valid winit KeyCode that is not handled by the match,
        // so it should hit the catch-all `_ => KeyCode::Escape` arm.
        assert_eq!(convert(WinitKeyCode::Fn), KeyCode::Escape);
    }

    #[test]
    fn unidentified_physical_key_falls_back_to_escape() {
        let unidentified = PhysicalKey::Unidentified(NativeKeyCode::Unidentified);
        assert_eq!(convert_key_code(unidentified), KeyCode::Escape);
    }
}
