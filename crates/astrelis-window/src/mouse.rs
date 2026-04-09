//! Mouse input types.

/// A mouse button identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MouseButton {
    /// Primary button (usually left).
    Left,
    /// Secondary button (usually right).
    Right,
    /// Middle button (scroll wheel click).
    Middle,
    /// Back button (side button 1).
    Back,
    /// Forward button (side button 2).
    Forward,
    /// Other numbered button.
    Other(u16),
}

/// Mouse scroll delta information.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MouseScrollDelta {
    /// Line-based scroll (discrete notches on a traditional mouse wheel).
    /// Positive y = scroll up, positive x = scroll right.
    LineDelta {
        /// Horizontal scroll amount in lines.
        x: f32,
        /// Vertical scroll amount in lines.
        y: f32,
    },
    /// Pixel-precise scroll (trackpad or high-resolution wheel).
    /// Values are in physical pixels.
    PixelDelta {
        /// Horizontal scroll amount in pixels.
        x: f32,
        /// Vertical scroll amount in pixels.
        y: f32,
    },
}
