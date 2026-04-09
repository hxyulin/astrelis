//! Keyboard input types.
//!
//! Separates physical key codes (scan codes) from logical key values
//! (layout-dependent), following the W3C UI Events model.

/// A physical key on the keyboard, independent of layout.
///
/// Named after the US QWERTY layout position (e.g., `KeyW` is always the key
/// in that physical position, even on AZERTY where it produces 'Z').
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum KeyCode {
    // Function keys
    /// Escape key.
    Escape,
    /// F1 key.
    F1,
    /// F2 key.
    F2,
    /// F3 key.
    F3,
    /// F4 key.
    F4,
    /// F5 key.
    F5,
    /// F6 key.
    F6,
    /// F7 key.
    F7,
    /// F8 key.
    F8,
    /// F9 key.
    F9,
    /// F10 key.
    F10,
    /// F11 key.
    F11,
    /// F12 key.
    F12,
    /// F13 key.
    F13,
    /// F14 key.
    F14,
    /// F15 key.
    F15,
    /// F16 key.
    F16,
    /// F17 key.
    F17,
    /// F18 key.
    F18,
    /// F19 key.
    F19,
    /// F20 key.
    F20,
    /// F21 key.
    F21,
    /// F22 key.
    F22,
    /// F23 key.
    F23,
    /// F24 key.
    F24,

    // Number row
    /// Backquote / tilde key.
    Backquote,
    /// Digit 1 key.
    Digit1,
    /// Digit 2 key.
    Digit2,
    /// Digit 3 key.
    Digit3,
    /// Digit 4 key.
    Digit4,
    /// Digit 5 key.
    Digit5,
    /// Digit 6 key.
    Digit6,
    /// Digit 7 key.
    Digit7,
    /// Digit 8 key.
    Digit8,
    /// Digit 9 key.
    Digit9,
    /// Digit 0 key.
    Digit0,
    /// Minus / underscore key.
    Minus,
    /// Equal / plus key.
    Equal,
    /// Backspace key.
    Backspace,

    // Letter row
    /// Tab key.
    Tab,
    /// A key.
    KeyA,
    /// B key.
    KeyB,
    /// C key.
    KeyC,
    /// D key.
    KeyD,
    /// E key.
    KeyE,
    /// F key.
    KeyF,
    /// G key.
    KeyG,
    /// H key.
    KeyH,
    /// I key.
    KeyI,
    /// J key.
    KeyJ,
    /// K key.
    KeyK,
    /// L key.
    KeyL,
    /// M key.
    KeyM,
    /// N key.
    KeyN,
    /// O key.
    KeyO,
    /// P key.
    KeyP,
    /// Q key.
    KeyQ,
    /// R key.
    KeyR,
    /// S key.
    KeyS,
    /// T key.
    KeyT,
    /// U key.
    KeyU,
    /// V key.
    KeyV,
    /// W key.
    KeyW,
    /// X key.
    KeyX,
    /// Y key.
    KeyY,
    /// Z key.
    KeyZ,
    /// Left bracket key.
    BracketLeft,
    /// Right bracket key.
    BracketRight,
    /// Backslash key.
    Backslash,

    // Middle row
    /// Caps lock key.
    CapsLock,
    /// Semicolon key.
    Semicolon,
    /// Quote key.
    Quote,
    /// Enter key.
    Enter,

    // Bottom row
    /// Left shift key.
    ShiftLeft,
    /// Comma key.
    Comma,
    /// Period key.
    Period,
    /// Slash key.
    Slash,
    /// Right shift key.
    ShiftRight,

    // Modifier row
    /// Left control key.
    ControlLeft,
    /// Left alt key.
    AltLeft,
    /// Left meta (Command/Windows) key.
    MetaLeft,
    /// Space bar.
    Space,
    /// Right meta key.
    MetaRight,
    /// Right alt key.
    AltRight,
    /// Right control key.
    ControlRight,

    // Navigation cluster
    /// Print screen key.
    PrintScreen,
    /// Scroll lock key.
    ScrollLock,
    /// Pause key.
    Pause,
    /// Insert key.
    Insert,
    /// Home key.
    Home,
    /// Page up key.
    PageUp,
    /// Delete key.
    Delete,
    /// End key.
    End,
    /// Page down key.
    PageDown,
    /// Up arrow key.
    ArrowUp,
    /// Left arrow key.
    ArrowLeft,
    /// Down arrow key.
    ArrowDown,
    /// Right arrow key.
    ArrowRight,

    // Numpad
    /// Num lock key.
    NumLock,
    /// Numpad divide key.
    NumpadDivide,
    /// Numpad multiply key.
    NumpadMultiply,
    /// Numpad subtract key.
    NumpadSubtract,
    /// Numpad 7 key.
    Numpad7,
    /// Numpad 8 key.
    Numpad8,
    /// Numpad 9 key.
    Numpad9,
    /// Numpad add key.
    NumpadAdd,
    /// Numpad 4 key.
    Numpad4,
    /// Numpad 5 key.
    Numpad5,
    /// Numpad 6 key.
    Numpad6,
    /// Numpad 1 key.
    Numpad1,
    /// Numpad 2 key.
    Numpad2,
    /// Numpad 3 key.
    Numpad3,
    /// Numpad 0 key.
    Numpad0,
    /// Numpad decimal key.
    NumpadDecimal,
    /// Numpad enter key.
    NumpadEnter,

    // Media
    /// Media play/pause key.
    MediaPlayPause,
    /// Media stop key.
    MediaStop,
    /// Media track next key.
    MediaTrackNext,
    /// Media track previous key.
    MediaTrackPrevious,
    /// Audio volume up key.
    AudioVolumeUp,
    /// Audio volume down key.
    AudioVolumeDown,
    /// Audio volume mute key.
    AudioVolumeMute,

    // Browser
    /// Browser back key.
    BrowserBack,
    /// Browser forward key.
    BrowserForward,
    /// Browser refresh key.
    BrowserRefresh,

    // Misc
    /// Context menu key.
    ContextMenu,
    /// International backslash key.
    IntlBackslash,
}

/// A logical key value, representing the meaning of a key press after
/// layout and modifier processing.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Key {
    /// A named (non-character) key.
    Named(NamedKey),
    /// A character or string produced by the key press.
    Character(String),
    /// Key could not be identified.
    Unidentified,
}

/// Named (non-character) keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NamedKey {
    /// Enter key.
    Enter,
    /// Tab key.
    Tab,
    /// Space bar.
    Space,
    /// Backspace key.
    Backspace,
    /// Escape key.
    Escape,
    /// Delete key.
    Delete,
    /// Up arrow.
    ArrowUp,
    /// Down arrow.
    ArrowDown,
    /// Left arrow.
    ArrowLeft,
    /// Right arrow.
    ArrowRight,
    /// Home key.
    Home,
    /// End key.
    End,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,
    /// Insert key.
    Insert,
    /// F1 key.
    F1,
    /// F2 key.
    F2,
    /// F3 key.
    F3,
    /// F4 key.
    F4,
    /// F5 key.
    F5,
    /// F6 key.
    F6,
    /// F7 key.
    F7,
    /// F8 key.
    F8,
    /// F9 key.
    F9,
    /// F10 key.
    F10,
    /// F11 key.
    F11,
    /// F12 key.
    F12,
    /// F13 key.
    F13,
    /// F14 key.
    F14,
    /// F15 key.
    F15,
    /// F16 key.
    F16,
    /// F17 key.
    F17,
    /// F18 key.
    F18,
    /// F19 key.
    F19,
    /// F20 key.
    F20,
    /// F21 key.
    F21,
    /// F22 key.
    F22,
    /// F23 key.
    F23,
    /// F24 key.
    F24,
    /// Print screen key.
    PrintScreen,
    /// Scroll lock key.
    ScrollLock,
    /// Pause key.
    Pause,
    /// Caps lock key.
    CapsLock,
    /// Num lock key.
    NumLock,
    /// Shift key.
    Shift,
    /// Control key.
    Control,
    /// Alt key.
    Alt,
    /// Meta (Command/Windows) key.
    Meta,
    /// Context menu key.
    ContextMenu,
    /// Media play/pause.
    MediaPlayPause,
    /// Media stop.
    MediaStop,
    /// Media track next.
    MediaTrackNext,
    /// Media track previous.
    MediaTrackPrevious,
    /// Audio volume up.
    AudioVolumeUp,
    /// Audio volume down.
    AudioVolumeDown,
    /// Audio volume mute.
    AudioVolumeMute,
}

/// The location of a key on the keyboard.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum KeyLocation {
    /// Standard position (non-modifier or unique key).
    #[default]
    Standard,
    /// Left-side key (e.g., left Shift).
    Left,
    /// Right-side key (e.g., right Shift).
    Right,
    /// Numpad.
    Numpad,
}

/// State of keyboard modifier keys at the time of an event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ModifiersState {
    /// Shift key is held.
    pub shift: bool,
    /// Control key is held.
    pub control: bool,
    /// Alt (Option on macOS) key is held.
    pub alt: bool,
    /// Meta (Command on macOS, Windows key on Windows) key is held.
    pub meta: bool,
}
