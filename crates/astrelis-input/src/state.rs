//! Button state machine for tracking press/release transitions across frames.

/// The state of a button (keyboard key or mouse button) within the frame lifecycle.
///
/// State transitions:
/// - **Press event** → [`JustPressed`](ButtonState::JustPressed)
/// - **[`begin_frame()`](crate::InputState::begin_frame)** → [`Held`](ButtonState::Held)
/// - **Release event** → [`JustReleased`](ButtonState::JustReleased)
/// - **[`begin_frame()`](crate::InputState::begin_frame)** → [`Released`](ButtonState::Released)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ButtonState {
    /// The button is not pressed and was not recently released.
    #[default]
    Released,
    /// The button was pressed this frame.
    JustPressed,
    /// The button has been held down since a previous frame.
    Held,
    /// The button was released this frame.
    JustReleased,
}

impl ButtonState {
    /// Advance the state for a new frame.
    ///
    /// - `JustPressed` → `Held`
    /// - `JustReleased` → `Released`
    /// - Others unchanged.
    pub(crate) fn advance(self) -> Self {
        match self {
            Self::JustPressed => Self::Held,
            Self::JustReleased => Self::Released,
            other => other,
        }
    }

    /// Transition to the pressed state.
    pub(crate) fn press(self) -> Self {
        Self::JustPressed
    }

    /// Transition to the released state.
    pub(crate) fn release(self) -> Self {
        Self::JustReleased
    }

    /// Returns `true` if the button is currently down (`JustPressed` or `Held`).
    pub(crate) fn is_pressed(self) -> bool {
        matches!(self, Self::JustPressed | Self::Held)
    }

    /// Returns `true` if the button was pressed this frame.
    pub(crate) fn is_just_pressed(self) -> bool {
        self == Self::JustPressed
    }

    /// Returns `true` if the button was released this frame.
    pub(crate) fn is_just_released(self) -> bool {
        self == Self::JustReleased
    }
}
