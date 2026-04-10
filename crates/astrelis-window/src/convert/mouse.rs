//! Mouse type conversions from winit to astrelis.

use crate::mouse::{MouseButton, MouseScrollDelta};

/// Converts a winit mouse button to astrelis.
pub(crate) fn convert_mouse_button(button: winit::event::MouseButton) -> MouseButton {
    match button {
        winit::event::MouseButton::Left => MouseButton::Left,
        winit::event::MouseButton::Right => MouseButton::Right,
        winit::event::MouseButton::Middle => MouseButton::Middle,
        winit::event::MouseButton::Back => MouseButton::Back,
        winit::event::MouseButton::Forward => MouseButton::Forward,
        winit::event::MouseButton::Other(id) => MouseButton::Other(id),
    }
}

/// Converts a winit mouse scroll delta to astrelis.
pub(crate) fn convert_scroll_delta(
    delta: winit::event::MouseScrollDelta,
) -> MouseScrollDelta {
    match delta {
        winit::event::MouseScrollDelta::LineDelta(x, y) => {
            MouseScrollDelta::LineDelta { x, y }
        }
        winit::event::MouseScrollDelta::PixelDelta(pos) => MouseScrollDelta::PixelDelta {
            x: pos.x as f32,
            y: pos.y as f32,
        },
    }
}
