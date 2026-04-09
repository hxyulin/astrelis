//! Window event types.
//!
//! [`WindowEvent`] is a comprehensive superset covering winit, SDL3, and GLFW events.

use std::path::PathBuf;

use astrelis_core::geometry::{Physical, Point};

use crate::keyboard::{Key, KeyCode, KeyLocation, ModifiersState};
use crate::mouse::{MouseButton, MouseScrollDelta};
use crate::theme::Theme;
use crate::types::{InnerSize, OuterPosition};

/// The state of a button or key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ElementState {
    /// The element was pressed.
    Pressed,
    /// The element was released.
    Released,
}

impl ElementState {
    /// Returns `true` if this is [`ElementState::Pressed`].
    pub fn is_pressed(self) -> bool {
        self == Self::Pressed
    }
}

/// A keyboard input event.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyEvent {
    /// The physical key code (scan code / position on keyboard).
    pub key_code: KeyCode,
    /// The logical key value (after layout + modifier processing).
    pub key: Key,
    /// Whether this is a press or release.
    pub state: ElementState,
    /// The location of the key (left, right, numpad, standard).
    pub location: KeyLocation,
    /// Whether this is an auto-repeat event.
    pub repeat: bool,
}

/// A unique identifier for a touch point / finger.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TouchId(pub u64);

/// The phase of a touch event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TouchPhase {
    /// A new finger touched the screen.
    Started,
    /// A finger moved on the screen.
    Moved,
    /// A finger was lifted from the screen.
    Ended,
    /// The system cancelled tracking of this finger.
    Cancelled,
}

/// A touch input event.
#[derive(Clone, Debug, PartialEq)]
pub struct TouchEvent {
    /// Unique identifier for this touch point (finger).
    pub id: TouchId,
    /// The phase of the touch.
    pub phase: TouchPhase,
    /// Position of the touch in physical pixels.
    pub position: Point<Physical>,
}

/// Input method editor (IME) events.
#[derive(Clone, Debug, PartialEq)]
pub enum ImeEvent {
    /// IME composition has been enabled.
    Enabled,
    /// Pre-edit text is being composed. The optional range indicates the
    /// cursor position within the composition.
    Preedit(String, Option<(usize, usize)>),
    /// The composition has been committed as final text.
    Commit(String),
    /// IME composition has been disabled.
    Disabled,
}

/// Events specific to a single window.
///
/// This is a comprehensive superset covering winit, SDL3, and GLFW window events.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum WindowEvent {
    // --- Lifecycle ---
    /// The close button was pressed or the OS requested the window to close.
    /// The window is NOT closed yet; the application decides what to do.
    CloseRequested,

    /// The window has been destroyed and is no longer valid.
    Destroyed,

    // --- Geometry ---
    /// The window was resized. Contains the new inner size in physical pixels.
    Resized(InnerSize),

    /// The window was moved. Contains the new outer position in physical pixels.
    Moved(OuterPosition),

    // --- Display ---
    /// The scale factor (DPI) of the window's monitor changed.
    ScaleFactorChanged {
        /// The new scale factor.
        scale_factor: f32,
        /// The new inner size in physical pixels after the change.
        new_inner_size: InnerSize,
    },

    /// The system theme changed for this window.
    ThemeChanged(Theme),

    /// The window's content needs to be redrawn.
    RedrawRequested,

    // --- Focus ---
    /// The window gained or lost keyboard focus.
    Focused(bool),

    /// The window became occluded (fully hidden) or visible again.
    Occluded(bool),

    // --- Keyboard ---
    /// A keyboard key was pressed or released.
    KeyboardInput(KeyEvent),

    /// Modifier key state changed.
    ModifiersChanged(ModifiersState),

    /// Input method editor event.
    Ime(ImeEvent),

    // --- Mouse ---
    /// The mouse cursor entered the window's client area.
    CursorEntered,

    /// The mouse cursor left the window's client area.
    CursorLeft,

    /// The mouse cursor moved within the window.
    /// Position is in physical pixels relative to the top-left corner.
    CursorMoved(Point<Physical>),

    /// A mouse button was pressed or released.
    MouseButtonInput {
        /// Which button.
        button: MouseButton,
        /// Press or release.
        state: ElementState,
    },

    /// The mouse wheel was scrolled.
    MouseWheel(MouseScrollDelta),

    // --- Touch ---
    /// A touch event occurred.
    Touch(TouchEvent),

    // --- Drag & drop ---
    /// File(s) are being dragged over the window (hovering).
    DroppedFileHovered(Vec<PathBuf>),

    /// File(s) were dropped onto the window.
    DroppedFile(Vec<PathBuf>),

    /// A file drag operation was cancelled (left the window without dropping).
    DroppedFileCancelled,

    // --- Window state ---
    /// The window was minimized (iconified).
    Minimized,

    /// The window was restored from minimized state.
    Restored,

    /// The window was maximized.
    Maximized,

    /// The window exited maximized state.
    Unmaximized,
}

/// Device-level events not tied to a specific window.
///
/// These are raw hardware events. The most important use case is
/// [`DeviceEvent::MouseMotion`], which provides raw mouse deltas even when
/// the cursor is locked — essential for first-person camera controls.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum DeviceEvent {
    /// Raw mouse motion delta (not tied to a window or cursor position).
    ///
    /// When the cursor is locked via [`CursorGrabMode::Locked`](crate::cursor::CursorGrabMode::Locked),
    /// `CursorMoved` window events stop reporting meaningful positions.
    /// Use this event instead to get frame-to-frame deltas for camera rotation.
    MouseMotion {
        /// Horizontal delta in unscaled device units.
        delta_x: f64,
        /// Vertical delta in unscaled device units.
        delta_y: f64,
    },

    /// Raw mouse wheel delta from the device.
    MouseWheel {
        /// Horizontal delta.
        delta_x: f32,
        /// Vertical delta.
        delta_y: f32,
    },

    /// A device button was pressed or released.
    Button {
        /// The button identifier.
        button: u32,
        /// Press or release.
        state: ElementState,
    },

    /// A key was pressed or released at the device level.
    Key(KeyEvent),
}
