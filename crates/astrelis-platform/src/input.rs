//! Backend-neutral input value types.

use astrelis_core::geometry::{Logical, Physical, Point};

/// Opaque input-device identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DeviceId(pub u64);

/// Whether a button or key is pressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ElementState {
    /// The input is pressed.
    Pressed,
    /// The input is released.
    Released,
}

/// Physical position of a keyboard key.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PhysicalKey {
    /// A standardized USB-HID-style key code.
    Code(KeyCode),
    /// A platform-native key code not otherwise identified.
    Native(NativeKeyCode),
    /// The key could not be identified.
    Unidentified,
}

/// Common physical key codes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum KeyCode {
    /// `A` key.
    KeyA,
    /// `B` key.
    KeyB,
    /// `C` key.
    KeyC,
    /// `D` key.
    KeyD,
    /// `E` key.
    KeyE,
    /// `F` key.
    KeyF,
    /// `G` key.
    KeyG,
    /// `H` key.
    KeyH,
    /// `I` key.
    KeyI,
    /// `J` key.
    KeyJ,
    /// `K` key.
    KeyK,
    /// `L` key.
    KeyL,
    /// `M` key.
    KeyM,
    /// `N` key.
    KeyN,
    /// `O` key.
    KeyO,
    /// `P` key.
    KeyP,
    /// `Q` key.
    KeyQ,
    /// `R` key.
    KeyR,
    /// `S` key.
    KeyS,
    /// `T` key.
    KeyT,
    /// `U` key.
    KeyU,
    /// `V` key.
    KeyV,
    /// `W` key.
    KeyW,
    /// `X` key.
    KeyX,
    /// `Y` key.
    KeyY,
    /// `Z` key.
    KeyZ,
    /// Escape.
    Escape,
    /// Enter.
    Enter,
    /// Space.
    Space,
    /// Tab.
    Tab,
    /// Backspace.
    Backspace,
    /// An otherwise known key represented by its stable debug name.
    Other(String),
}

/// Platform-native physical key identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NativeKeyCode {
    /// Android scan code.
    Android(u32),
    /// macOS virtual key code.
    MacOS(u16),
    /// Windows scan code.
    Windows(u16),
    /// XKB key code.
    Xkb(u32),
}

/// Meaning of a keyboard key.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Key {
    /// A Unicode text value.
    Character(String),
    /// A standardized named key.
    Named(NamedKey),
    /// A platform-native logical key.
    Native(NativeKey),
    /// The key could not be identified.
    Unidentified,
}

/// Common named logical keys.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NamedKey {
    /// Alt modifier.
    Alt,
    /// Control modifier.
    Control,
    /// Shift modifier.
    Shift,
    /// Super/Command/Windows modifier.
    Super,
    /// Enter.
    Enter,
    /// Escape.
    Escape,
    /// Space.
    Space,
    /// Tab.
    Tab,
    /// Backspace.
    Backspace,
    /// An otherwise known named key represented by its stable debug name.
    Other(String),
}

/// Platform-native logical key identity.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum NativeKey {
    /// Android key code.
    Android(u32),
    /// macOS scan code.
    MacOS(u16),
    /// Windows virtual key.
    Windows(u16),
    /// XKB keysym.
    Xkb(u32),
    /// Web key string.
    Web(String),
}

/// Physical location of a key on the keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyLocation {
    /// Standard section.
    Standard,
    /// Left-hand duplicate.
    Left,
    /// Right-hand duplicate.
    Right,
    /// Numeric keypad.
    Numpad,
}

/// Modifier state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    /// Shift is active.
    pub shift: bool,
    /// Control is active.
    pub control: bool,
    /// Alt/Option is active.
    pub alt: bool,
    /// Super/Command/Windows is active.
    pub super_key: bool,
}

/// A keyboard event.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyboardInput {
    /// Source device.
    pub device_id: DeviceId,
    /// Physical key identity.
    pub physical_key: PhysicalKey,
    /// Logical key identity.
    pub logical_key: Key,
    /// Text produced by this key event.
    pub text: Option<String>,
    /// Physical key location.
    pub location: KeyLocation,
    /// Pressed or released.
    pub state: ElementState,
    /// Whether this is an auto-repeat.
    pub repeat: bool,
    /// Whether the platform synthesized the event.
    pub synthetic: bool,
}

/// Pointer or mouse button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerButton {
    /// Primary button.
    Primary,
    /// Secondary button.
    Secondary,
    /// Middle button.
    Middle,
    /// Browser back button.
    Back,
    /// Browser forward button.
    Forward,
    /// Another numbered button.
    Other(u16),
}

/// Phase of a scrolling or touch gesture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TouchPhase {
    /// Gesture began.
    Started,
    /// Gesture moved.
    Moved,
    /// Gesture ended.
    Ended,
    /// Gesture was cancelled.
    Cancelled,
}

/// Mouse wheel displacement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScrollDelta {
    /// Logical line units.
    Lines {
        /// Horizontal lines.
        x: f32,
        /// Vertical lines.
        y: f32,
    },
    /// Physical pixel units.
    Pixels(Point<Physical, f64>),
}

/// Touch pressure.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TouchForce {
    /// Calibrated force where `1.0` is normal pressure.
    Calibrated {
        /// Normalized force.
        force: f64,
        /// Maximum possible force.
        max_possible_force: f64,
        /// Stylus altitude angle.
        altitude_angle: Option<f64>,
    },
    /// Platform-normalized pressure.
    Normalized(f64),
}

/// A touch contact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Touch {
    /// Source device.
    pub device_id: DeviceId,
    /// Contact phase.
    pub phase: TouchPhase,
    /// Physical position.
    pub position: Point<Physical, f64>,
    /// Contact identifier.
    pub id: u64,
    /// Optional force information.
    pub force: Option<TouchForce>,
}

/// IME composition event.
#[derive(Clone, Debug, PartialEq)]
pub enum ImeEvent {
    /// IME became active.
    Enabled,
    /// Composition text and optional selected byte range.
    Preedit(String, Option<(usize, usize)>),
    /// Final committed text.
    Commit(String),
    /// IME became inactive.
    Disabled,
}

/// Intended kind of IME input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ImePurpose {
    /// General text.
    #[default]
    Normal,
    /// Password text.
    Password,
    /// Terminal input.
    Terminal,
}

/// A two-dimensional gesture displacement.
pub type GestureDelta = Point<Logical, f64>;
