//! Input state management for tracking keyboard, mouse, and gamepad state.
//!
//! This module provides a unified input system that tracks the current state
//! of input devices, making it easy to query whether keys or buttons are
//! pressed, just pressed, or just released.
//!
//! # Example
//!
//! ```ignore
//! use astrelis_input::InputState;
//!
//! let mut input = InputState::new();
//!
//! // In your event loop:
//! input.handle_events(&mut events);
//!
//! // Query input state:
//! if input.is_key_pressed(KeyCode::Space) {
//!     player.jump();
//! }
//!
//! if input.is_key_just_pressed(KeyCode::Escape) {
//!     game.pause();
//! }
//!
//! let mouse_delta = input.mouse_delta();
//!
//! // At the end of each frame:
//! input.end_frame();
//! ```

use astrelis_core::alloc::HashSet;
use astrelis_core::math::Vec2;
use astrelis_core::profiling::profile_function;
use astrelis_winit::event::{
    ElementState, Event, EventBatch, HandleStatus, KeyCode, MouseButton as WinitMouseButton,
    MouseScrollDelta, PhysicalKey,
};

// Re-export KeyCode for convenience
pub use astrelis_winit::event::KeyCode as Key;

/// Mouse button identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Other(u16),
}

impl From<WinitMouseButton> for MouseButton {
    fn from(button: WinitMouseButton) -> Self {
        match button {
            WinitMouseButton::Left => MouseButton::Left,
            WinitMouseButton::Right => MouseButton::Right,
            WinitMouseButton::Middle => MouseButton::Middle,
            WinitMouseButton::Back => MouseButton::Back,
            WinitMouseButton::Forward => MouseButton::Forward,
            WinitMouseButton::Other(id) => MouseButton::Other(id),
        }
    }
}

/// Modifier key state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool, // Command on macOS, Windows key on Windows
}

impl Modifiers {
    /// Create new modifiers with all keys released.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if any modifier is pressed.
    pub fn any(&self) -> bool {
        self.shift || self.ctrl || self.alt || self.meta
    }

    /// Check if no modifiers are pressed.
    pub fn none(&self) -> bool {
        !self.any()
    }
}

/// Input state tracker for keyboard, mouse, and other input devices.
///
/// This struct tracks the current state of all input devices and provides
/// convenient methods for querying input state. It distinguishes between
/// keys/buttons that are currently held, just pressed this frame, or just
/// released this frame.
#[derive(Debug)]
pub struct InputState {
    // Keyboard state
    keys_pressed: HashSet<KeyCode>,
    keys_just_pressed: HashSet<KeyCode>,
    keys_just_released: HashSet<KeyCode>,
    modifiers: Modifiers,

    // Mouse state
    mouse_buttons_pressed: HashSet<MouseButton>,
    mouse_buttons_just_pressed: HashSet<MouseButton>,
    mouse_buttons_just_released: HashSet<MouseButton>,
    mouse_position: Vec2,
    mouse_position_prev: Vec2,
    mouse_delta: Vec2,
    scroll_delta: Vec2,
    mouse_in_window: bool,

    // Text input
    text_input: String,
}

impl InputState {
    /// Create a new input state tracker.
    pub fn new() -> Self {
        Self {
            keys_pressed: HashSet::new(),
            keys_just_pressed: HashSet::new(),
            keys_just_released: HashSet::new(),
            modifiers: Modifiers::new(),

            mouse_buttons_pressed: HashSet::new(),
            mouse_buttons_just_pressed: HashSet::new(),
            mouse_buttons_just_released: HashSet::new(),
            mouse_position: Vec2::ZERO,
            mouse_position_prev: Vec2::ZERO,
            mouse_delta: Vec2::ZERO,
            scroll_delta: Vec2::ZERO,
            mouse_in_window: false,

            text_input: String::new(),
        }
    }

    /// Process events from the event batch.
    ///
    /// This should be called each frame before querying input state.
    pub fn handle_events(&mut self, events: &mut EventBatch) {
        profile_function!();
        events.dispatch(|event| {
            match event {
                Event::KeyInput(key_event) => {
                    if let PhysicalKey::Code(key_code) = key_event.physical_key {
                        match key_event.state {
                            ElementState::Pressed => {
                                if !key_event.repeat {
                                    self.keys_just_pressed.insert(key_code);
                                }
                                self.keys_pressed.insert(key_code);
                                self.update_modifiers(key_code, true);
                            }
                            ElementState::Released => {
                                self.keys_just_released.insert(key_code);
                                self.keys_pressed.remove(&key_code);
                                self.update_modifiers(key_code, false);
                            }
                        }
                    }

                    // Collect text input
                    if key_event.state == ElementState::Pressed
                        && let Some(ref text) = key_event.text
                    {
                        self.text_input.push_str(text);
                    }

                    HandleStatus::handled()
                }
                Event::MouseButtonDown(button) => {
                    let button = MouseButton::from(*button);
                    self.mouse_buttons_just_pressed.insert(button);
                    self.mouse_buttons_pressed.insert(button);
                    HandleStatus::handled()
                }
                Event::MouseButtonUp(button) => {
                    let button = MouseButton::from(*button);
                    self.mouse_buttons_just_released.insert(button);
                    self.mouse_buttons_pressed.remove(&button);
                    HandleStatus::handled()
                }
                Event::MouseMoved(pos) => {
                    self.mouse_position = Vec2::new(pos.x as f32, pos.y as f32);
                    HandleStatus::handled()
                }
                Event::MouseScrolled(delta) => {
                    let (dx, dy) = match delta {
                        MouseScrollDelta::LineDelta(x, y) => (*x, *y),
                        MouseScrollDelta::PixelDelta(pos) => (pos.x as f32, pos.y as f32),
                    };
                    self.scroll_delta = Vec2::new(dx, dy);
                    HandleStatus::handled()
                }
                Event::MouseEntered => {
                    self.mouse_in_window = true;
                    HandleStatus::handled()
                }
                Event::MouseLeft => {
                    self.mouse_in_window = false;
                    HandleStatus::handled()
                }
                _ => HandleStatus::ignored(),
            }
        });

        // Calculate mouse delta
        self.mouse_delta = self.mouse_position - self.mouse_position_prev;
    }

    /// Clear per-frame state. Call this at the end of each frame.
    pub fn end_frame(&mut self) {
        profile_function!();
        self.keys_just_pressed.clear();
        self.keys_just_released.clear();
        self.mouse_buttons_just_pressed.clear();
        self.mouse_buttons_just_released.clear();
        self.mouse_position_prev = self.mouse_position;
        self.mouse_delta = Vec2::ZERO;
        self.scroll_delta = Vec2::ZERO;
        self.text_input.clear();
    }

    // ==================== Keyboard Queries ====================

    /// Check if a key is currently pressed (held down).
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.keys_pressed.contains(&key)
    }

    /// Check if a key was just pressed this frame.
    pub fn is_key_just_pressed(&self, key: KeyCode) -> bool {
        self.keys_just_pressed.contains(&key)
    }

    /// Check if a key was just released this frame.
    pub fn is_key_just_released(&self, key: KeyCode) -> bool {
        self.keys_just_released.contains(&key)
    }

    /// Check if any of the given keys are pressed.
    pub fn is_any_key_pressed(&self, keys: &[KeyCode]) -> bool {
        keys.iter().any(|k| self.is_key_pressed(*k))
    }

    /// Check if all of the given keys are pressed.
    pub fn are_all_keys_pressed(&self, keys: &[KeyCode]) -> bool {
        keys.iter().all(|k| self.is_key_pressed(*k))
    }

    /// Get the current modifier key state.
    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    /// Check if Shift is held.
    pub fn is_shift_pressed(&self) -> bool {
        self.modifiers.shift
    }

    /// Check if Ctrl (or Cmd on macOS) is held.
    pub fn is_ctrl_pressed(&self) -> bool {
        self.modifiers.ctrl
    }

    /// Check if Alt (or Option on macOS) is held.
    pub fn is_alt_pressed(&self) -> bool {
        self.modifiers.alt
    }

    /// Check if Meta (Windows key or Cmd on macOS) is held.
    pub fn is_meta_pressed(&self) -> bool {
        self.modifiers.meta
    }

    /// Get text input received this frame.
    pub fn text_input(&self) -> &str {
        &self.text_input
    }

    /// Get all keys currently pressed.
    pub fn pressed_keys(&self) -> impl Iterator<Item = &KeyCode> {
        self.keys_pressed.iter()
    }

    // ==================== Mouse Queries ====================

    /// Check if a mouse button is currently pressed.
    pub fn is_mouse_button_pressed(&self, button: MouseButton) -> bool {
        self.mouse_buttons_pressed.contains(&button)
    }

    /// Check if a mouse button was just pressed this frame.
    pub fn is_mouse_button_just_pressed(&self, button: MouseButton) -> bool {
        self.mouse_buttons_just_pressed.contains(&button)
    }

    /// Check if a mouse button was just released this frame.
    pub fn is_mouse_button_just_released(&self, button: MouseButton) -> bool {
        self.mouse_buttons_just_released.contains(&button)
    }

    /// Check if left mouse button is pressed.
    pub fn is_left_mouse_pressed(&self) -> bool {
        self.is_mouse_button_pressed(MouseButton::Left)
    }

    /// Check if left mouse button was just pressed.
    pub fn is_left_mouse_just_pressed(&self) -> bool {
        self.is_mouse_button_just_pressed(MouseButton::Left)
    }

    /// Check if right mouse button is pressed.
    pub fn is_right_mouse_pressed(&self) -> bool {
        self.is_mouse_button_pressed(MouseButton::Right)
    }

    /// Check if right mouse button was just pressed.
    pub fn is_right_mouse_just_pressed(&self) -> bool {
        self.is_mouse_button_just_pressed(MouseButton::Right)
    }

    /// Check if middle mouse button is pressed.
    pub fn is_middle_mouse_pressed(&self) -> bool {
        self.is_mouse_button_pressed(MouseButton::Middle)
    }

    /// Get the current mouse position in window coordinates.
    pub fn mouse_position(&self) -> Vec2 {
        self.mouse_position
    }

    /// Get the mouse movement delta since last frame.
    pub fn mouse_delta(&self) -> Vec2 {
        self.mouse_delta
    }

    /// Get the scroll wheel delta since last frame.
    ///
    /// Positive Y = scroll up, Negative Y = scroll down.
    pub fn scroll_delta(&self) -> Vec2 {
        self.scroll_delta
    }

    /// Check if the mouse cursor is inside the window.
    pub fn is_mouse_in_window(&self) -> bool {
        self.mouse_in_window
    }

    // ==================== Helper Methods ====================

    /// Get horizontal input axis (-1, 0, or 1) from arrow keys or WASD.
    pub fn horizontal_axis(&self) -> f32 {
        let mut axis = 0.0;
        if self.is_key_pressed(KeyCode::ArrowLeft) || self.is_key_pressed(KeyCode::KeyA) {
            axis -= 1.0;
        }
        if self.is_key_pressed(KeyCode::ArrowRight) || self.is_key_pressed(KeyCode::KeyD) {
            axis += 1.0;
        }
        axis
    }

    /// Get vertical input axis (-1, 0, or 1) from arrow keys or WASD.
    pub fn vertical_axis(&self) -> f32 {
        let mut axis = 0.0;
        if self.is_key_pressed(KeyCode::ArrowUp) || self.is_key_pressed(KeyCode::KeyW) {
            axis -= 1.0;
        }
        if self.is_key_pressed(KeyCode::ArrowDown) || self.is_key_pressed(KeyCode::KeyS) {
            axis += 1.0;
        }
        axis
    }

    /// Get movement direction as a normalized vector.
    pub fn movement_direction(&self) -> Vec2 {
        let dir = Vec2::new(self.horizontal_axis(), self.vertical_axis());
        if dir.length_squared() > 0.0 {
            dir.normalize()
        } else {
            dir
        }
    }

    /// Reset all input state.
    pub fn reset(&mut self) {
        self.keys_pressed.clear();
        self.keys_just_pressed.clear();
        self.keys_just_released.clear();
        self.modifiers = Modifiers::new();
        self.mouse_buttons_pressed.clear();
        self.mouse_buttons_just_pressed.clear();
        self.mouse_buttons_just_released.clear();
        self.mouse_delta = Vec2::ZERO;
        self.scroll_delta = Vec2::ZERO;
        self.text_input.clear();
    }

    // ==================== Internal Methods ====================

    fn update_modifiers(&mut self, key: KeyCode, pressed: bool) {
        match key {
            KeyCode::ShiftLeft | KeyCode::ShiftRight => self.modifiers.shift = pressed,
            KeyCode::ControlLeft | KeyCode::ControlRight => self.modifiers.ctrl = pressed,
            KeyCode::AltLeft | KeyCode::AltRight => self.modifiers.alt = pressed,
            KeyCode::SuperLeft | KeyCode::SuperRight | KeyCode::Meta => {
                self.modifiers.meta = pressed
            }
            _ => {}
        }
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

/// An input system that wraps InputState and provides additional functionality.
pub struct InputSystem {
    state: InputState,
}

impl InputSystem {
    /// Create a new input system.
    pub fn new() -> Self {
        Self {
            state: InputState::new(),
        }
    }

    /// Get the input state.
    pub fn state(&self) -> &InputState {
        &self.state
    }

    /// Get mutable access to the input state.
    pub fn state_mut(&mut self) -> &mut InputState {
        &mut self.state
    }

    /// Process events from the event batch.
    pub fn handle_events(&mut self, events: &mut EventBatch) {
        profile_function!();
        self.state.handle_events(events);
    }

    /// Clear per-frame state.
    pub fn end_frame(&mut self) {
        self.state.end_frame();
    }
}

impl Default for InputSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Deref for InputSystem {
    type Target = InputState;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl std::ops::DerefMut for InputSystem {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modifiers_default() {
        let mods = Modifiers::new();
        assert!(!mods.any());
        assert!(mods.none());
    }

    #[test]
    fn test_modifiers_any() {
        let mut mods = Modifiers::new();
        mods.shift = true;
        assert!(mods.any());
        assert!(!mods.none());
    }

    #[test]
    fn test_input_state_new() {
        let state = InputState::new();
        assert!(!state.is_key_pressed(KeyCode::Space));
        assert!(!state.is_left_mouse_pressed());
        assert_eq!(state.mouse_position(), Vec2::ZERO);
    }

    #[test]
    fn test_movement_direction_normalized() {
        let mut state = InputState::new();
        state.keys_pressed.insert(KeyCode::KeyW);
        state.keys_pressed.insert(KeyCode::KeyD);

        let dir = state.movement_direction();
        let len = dir.length();
        assert!((len - 1.0).abs() < 0.001, "Direction should be normalized");
    }
}
