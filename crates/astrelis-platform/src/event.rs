//! Lifecycle, window, and raw-device events.

use std::time::Instant;

use astrelis_core::geometry::{Logical, Physical, Point, Size};

use crate::{
    DeviceId, ElementState, ImeEvent, KeyboardInput, Modifiers, PhysicalKey, PointerButton,
    ScrollDelta, Theme, Touch, TouchPhase,
};

/// Reason a new event batch began.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum StartCause {
    /// The loop is starting.
    Init,
    /// Poll mode started another iteration.
    Poll,
    /// A wait deadline elapsed.
    ResumeTimeReached {
        /// Requested deadline.
        requested_resume: Instant,
        /// Actual wake time.
        start: Instant,
    },
    /// An event interrupted a timed wait.
    WaitCancelled {
        /// Requested deadline, if any.
        requested_resume: Option<Instant>,
        /// Actual wake time.
        start: Instant,
    },
}

/// An event associated with a window.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum WindowEvent {
    /// The close button was requested.
    CloseRequested,
    /// The final native window owner was dropped.
    Destroyed,
    /// The framebuffer changed size.
    Resized(Size<Physical, u32>),
    /// The outer window moved.
    Moved(Point<Physical, i32>),
    /// DPI changed, including the backend's current inner size.
    ScaleFactorChanged {
        /// New DPI scale.
        scale_factor: f64,
        /// Current framebuffer size.
        inner_size: Size<Physical, u32>,
    },
    /// Focus changed.
    Focused(bool),
    /// Occlusion changed.
    Occluded(bool),
    /// System theme changed.
    ThemeChanged(Theme),
    /// The window should render now.
    RedrawRequested,
    /// Keyboard input.
    KeyboardInput(KeyboardInput),
    /// Modifier keys changed.
    ModifiersChanged(Modifiers),
    /// Pointer entered the window.
    PointerEntered {
        /// Source device.
        device_id: DeviceId,
    },
    /// Pointer left the window.
    PointerLeft {
        /// Source device.
        device_id: DeviceId,
    },
    /// Pointer moved in physical coordinates.
    PointerMoved {
        /// Source device.
        device_id: DeviceId,
        /// Position.
        position: Point<Physical, f64>,
    },
    /// Pointer button changed.
    PointerButton {
        /// Source device.
        device_id: DeviceId,
        /// Button.
        button: PointerButton,
        /// State.
        state: ElementState,
    },
    /// Pointer wheel moved.
    PointerWheel {
        /// Source device.
        device_id: DeviceId,
        /// Displacement.
        delta: ScrollDelta,
        /// Gesture phase.
        phase: TouchPhase,
    },
    /// IME composition changed.
    Ime(ImeEvent),
    /// Touch contact changed.
    Touch(Touch),
    /// Trackpad pinch amount.
    PinchGesture {
        /// Source device.
        device_id: DeviceId,
        /// Magnification delta.
        delta: f64,
        /// Gesture phase.
        phase: TouchPhase,
    },
    /// Trackpad pan displacement.
    PanGesture {
        /// Source device.
        device_id: DeviceId,
        /// Displacement.
        delta: Point<Logical, f64>,
        /// Gesture phase.
        phase: TouchPhase,
    },
    /// Trackpad rotation in degrees.
    RotationGesture {
        /// Source device.
        device_id: DeviceId,
        /// Rotation delta.
        delta_degrees: f32,
        /// Gesture phase.
        phase: TouchPhase,
    },
    /// Trackpad double tap.
    DoubleTapGesture {
        /// Source device.
        device_id: DeviceId,
    },
    /// A file is hovering over the window.
    HoveredFile(std::path::PathBuf),
    /// File hovering was cancelled.
    HoveredFileCancelled,
    /// A file was dropped.
    DroppedFile(std::path::PathBuf),
}

/// Raw input not associated with a window.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum DeviceEvent {
    /// Device was added.
    Added,
    /// Device was removed.
    Removed,
    /// Raw mouse motion.
    MouseMotion {
        /// Delta in device units.
        delta: (f64, f64),
    },
    /// Raw wheel motion.
    MouseWheel(ScrollDelta),
    /// Raw axis motion.
    Motion {
        /// Axis number.
        axis: u32,
        /// Axis value.
        value: f64,
    },
    /// Raw button state.
    Button {
        /// Button number.
        button: u32,
        /// State.
        state: ElementState,
    },
    /// Raw physical keyboard event.
    Key {
        /// Physical key.
        physical_key: PhysicalKey,
        /// State.
        state: ElementState,
        /// Auto-repeat status.
        repeat: bool,
    },
}
