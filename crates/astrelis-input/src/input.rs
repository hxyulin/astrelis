//! Core input state tracker.

use std::collections::HashMap;

use astrelis_core::geometry::{Physical, Point};
use astrelis_window::event::{DeviceEvent, ElementState, ImeEvent, KeyEvent, WindowEvent};
use astrelis_window::keyboard::{Key, KeyCode, ModifiersState};
use astrelis_window::mouse::{MouseButton, MouseScrollDelta};

use crate::state::ButtonState;

/// Tracks the current state of keyboard and mouse input across frames.
///
/// Feed [`WindowEvent`]s and [`DeviceEvent`]s into this struct each frame,
/// then query the accumulated state. This is a polling-style tracker — think
/// "is W pressed right now?" rather than event callbacks.
///
/// # Frame lifecycle
///
/// 1. Call [`begin_frame()`](Self::begin_frame) **before** processing events.
/// 2. Feed events via [`handle_event()`](Self::handle_event) and
///    [`handle_device_event()`](Self::handle_device_event).
/// 3. Query state with the various `is_*` methods.
///
/// # Example
///
/// ```
/// use astrelis_input::InputState;
/// use astrelis_window::keyboard::KeyCode;
///
/// let mut input = InputState::new();
///
/// // Each frame:
/// input.begin_frame();
/// // ... feed events ...
/// if input.is_key_pressed(KeyCode::KeyW) {
///     // move forward
/// }
/// ```
pub struct InputState {
    /// Physical key states.
    keyboard: HashMap<KeyCode, ButtonState>,
    /// Logical key states (layout-aware).
    logical_keys: HashMap<Key, ButtonState>,
    /// Current modifier key state.
    modifiers: ModifiersState,
    /// Mouse button states.
    mouse_buttons: HashMap<MouseButton, ButtonState>,
    /// Cursor position in physical pixels, or `None` if the cursor is outside the window.
    mouse_position: Option<Point<Physical>>,
    /// Raw mouse motion delta accumulated this frame (from [`DeviceEvent::MouseMotion`]).
    mouse_delta: (f64, f64),
    /// Scroll delta accumulated this frame.
    scroll_delta: (f32, f32),
    /// Text committed via IME this frame.
    text_input: String,
}

impl InputState {
    /// Creates a new input state with everything in the default (released/zeroed) state.
    pub fn new() -> Self {
        Self {
            keyboard: HashMap::new(),
            logical_keys: HashMap::new(),
            modifiers: ModifiersState::default(),
            mouse_buttons: HashMap::new(),
            mouse_position: None,
            mouse_delta: (0.0, 0.0),
            scroll_delta: (0.0, 0.0),
            text_input: String::new(),
        }
    }

    /// Advances state for a new frame.
    ///
    /// Call this **once per frame before processing events**. It transitions
    /// `JustPressed` → `Held` and `JustReleased` → `Released`, and clears
    /// per-frame accumulators (scroll delta, mouse delta, text input).
    pub fn begin_frame(&mut self) {
        advance_and_clean(&mut self.keyboard);
        advance_and_clean(&mut self.logical_keys);
        advance_and_clean(&mut self.mouse_buttons);
        self.mouse_delta = (0.0, 0.0);
        self.scroll_delta = (0.0, 0.0);
        self.text_input.clear();
    }

    /// Processes a [`WindowEvent`], updating internal state accordingly.
    ///
    /// Call this for each window event received during the frame.
    pub fn handle_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput(key_event) => {
                self.handle_key_event(key_event);
            }
            WindowEvent::ModifiersChanged(state) => {
                self.modifiers = *state;
            }
            WindowEvent::CursorMoved(pos) => {
                self.mouse_position = Some(*pos);
            }
            WindowEvent::CursorLeft => {
                self.mouse_position = None;
            }
            WindowEvent::MouseButtonInput { button, state } => {
                let entry = self.mouse_buttons.entry(*button).or_default();
                *entry = match state {
                    ElementState::Pressed => entry.press(),
                    ElementState::Released => entry.release(),
                };
            }
            WindowEvent::MouseWheel(delta) => {
                let (dx, dy) = match *delta {
                    MouseScrollDelta::LineDelta { x, y } => (x, y),
                    MouseScrollDelta::PixelDelta { x, y } => (x, y),
                };
                self.scroll_delta.0 += dx;
                self.scroll_delta.1 += dy;
            }
            WindowEvent::Ime(ImeEvent::Commit(text)) => {
                self.text_input.push_str(text);
            }
            _ => {}
        }
    }

    /// Processes a [`DeviceEvent`], updating internal state accordingly.
    ///
    /// The primary use case is [`DeviceEvent::MouseMotion`] for raw mouse
    /// deltas (essential for first-person camera controls when the cursor is
    /// locked).
    pub fn handle_device_event(&mut self, event: &DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta_x, delta_y } = event {
            self.mouse_delta.0 += delta_x;
            self.mouse_delta.1 += delta_y;
        }
    }

    // --- Keyboard queries ---

    /// Returns `true` if the key is currently pressed (held or just pressed this frame).
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.keyboard
            .get(&key)
            .is_some_and(|s| s.is_pressed())
    }

    /// Returns `true` if the key was pressed this frame (not held from a previous frame).
    pub fn is_key_just_pressed(&self, key: KeyCode) -> bool {
        self.keyboard
            .get(&key)
            .is_some_and(|s| s.is_just_pressed())
    }

    /// Returns `true` if the key was released this frame.
    pub fn is_key_just_released(&self, key: KeyCode) -> bool {
        self.keyboard
            .get(&key)
            .is_some_and(|s| s.is_just_released())
    }

    /// Returns the current state of modifier keys.
    pub fn modifiers(&self) -> ModifiersState {
        self.modifiers
    }

    // --- Logical key queries ---

    /// Returns `true` if the logical key is currently pressed.
    ///
    /// Logical keys are layout-aware: on an AZERTY keyboard, pressing the
    /// physical `KeyW` position produces the logical key `Character("z")`.
    pub fn is_logical_key_pressed(&self, key: &Key) -> bool {
        self.logical_keys
            .get(key)
            .is_some_and(|s| s.is_pressed())
    }

    /// Returns `true` if the logical key was pressed this frame.
    pub fn is_logical_key_just_pressed(&self, key: &Key) -> bool {
        self.logical_keys
            .get(key)
            .is_some_and(|s| s.is_just_pressed())
    }

    // --- Mouse queries ---

    /// Returns `true` if the mouse button is currently pressed.
    pub fn is_mouse_button_pressed(&self, button: MouseButton) -> bool {
        self.mouse_buttons
            .get(&button)
            .is_some_and(|s| s.is_pressed())
    }

    /// Returns `true` if the mouse button was pressed this frame.
    pub fn is_mouse_button_just_pressed(&self, button: MouseButton) -> bool {
        self.mouse_buttons
            .get(&button)
            .is_some_and(|s| s.is_just_pressed())
    }

    /// Returns `true` if the mouse button was released this frame.
    pub fn is_mouse_button_just_released(&self, button: MouseButton) -> bool {
        self.mouse_buttons
            .get(&button)
            .is_some_and(|s| s.is_just_released())
    }

    /// Returns the cursor position in physical pixels, or `None` if the cursor
    /// is outside the window.
    pub fn mouse_position(&self) -> Option<Point<Physical>> {
        self.mouse_position
    }

    /// Returns the raw mouse motion delta accumulated this frame.
    ///
    /// This comes from [`DeviceEvent::MouseMotion`] and works even when the
    /// cursor is locked — essential for first-person camera controls.
    pub fn mouse_delta(&self) -> (f64, f64) {
        self.mouse_delta
    }

    /// Returns the scroll delta accumulated this frame as `(horizontal, vertical)`.
    ///
    /// Both [`LineDelta`](MouseScrollDelta::LineDelta) and
    /// [`PixelDelta`](MouseScrollDelta::PixelDelta) are accumulated into the
    /// same value. Positive y = scroll up, positive x = scroll right.
    pub fn scroll_delta(&self) -> (f32, f32) {
        self.scroll_delta
    }

    // --- Text input ---

    /// Returns characters committed via IME this frame.
    ///
    /// Useful for text fields where you need the actual characters typed,
    /// not just which keys are held.
    pub fn text_input(&self) -> &str {
        &self.text_input
    }

    // --- Internal helpers ---

    fn handle_key_event(&mut self, event: &KeyEvent) {
        // OS key-repeat events fire as Pressed with `repeat: true`. Ignore
        // these for state tracking — otherwise a held key would flip back to
        // JustPressed on every repeat, causing spurious "just pressed" queries.
        // Text input from repeats is handled separately via IME Commit events.
        if event.repeat {
            return;
        }

        let kb_entry = self.keyboard.entry(event.key_code).or_default();
        *kb_entry = match event.state {
            ElementState::Pressed => kb_entry.press(),
            ElementState::Released => kb_entry.release(),
        };

        let logical_entry = self.logical_keys.entry(event.key.clone()).or_default();
        *logical_entry = match event.state {
            ElementState::Pressed => logical_entry.press(),
            ElementState::Released => logical_entry.release(),
        };
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

/// Advance all entries in a button state map and remove entries that have
/// returned to `Released` to prevent unbounded growth.
fn advance_and_clean<K: Eq + std::hash::Hash>(map: &mut HashMap<K, ButtonState>) {
    map.retain(|_, state| {
        *state = state.advance();
        *state != ButtonState::Released
    });
}

#[cfg(test)]
mod tests {
    use astrelis_core::geometry::{Physical, Point};
    use astrelis_window::event::{DeviceEvent, ElementState, ImeEvent, KeyEvent, WindowEvent};
    use astrelis_window::keyboard::{Key, KeyCode, KeyLocation, ModifiersState, NamedKey};
    use astrelis_window::mouse::{MouseButton, MouseScrollDelta};

    use super::InputState;

    fn press_key(key_code: KeyCode) -> WindowEvent {
        WindowEvent::KeyboardInput(KeyEvent {
            key_code,
            key: Key::Named(NamedKey::Space),
            state: ElementState::Pressed,
            location: KeyLocation::Standard,
            repeat: false,
        })
    }

    fn release_key(key_code: KeyCode) -> WindowEvent {
        WindowEvent::KeyboardInput(KeyEvent {
            key_code,
            key: Key::Named(NamedKey::Space),
            state: ElementState::Released,
            location: KeyLocation::Standard,
            repeat: false,
        })
    }

    fn press_key_logical(key_code: KeyCode, key: Key) -> WindowEvent {
        WindowEvent::KeyboardInput(KeyEvent {
            key_code,
            key,
            state: ElementState::Pressed,
            location: KeyLocation::Standard,
            repeat: false,
        })
    }

    fn release_key_logical(key_code: KeyCode, key: Key) -> WindowEvent {
        WindowEvent::KeyboardInput(KeyEvent {
            key_code,
            key,
            state: ElementState::Released,
            location: KeyLocation::Standard,
            repeat: false,
        })
    }

    #[test]
    fn keyboard_state_transitions() {
        let mut input = InputState::new();

        // Press key
        input.handle_event(&press_key(KeyCode::KeyW));
        assert!(input.is_key_just_pressed(KeyCode::KeyW));
        assert!(input.is_key_pressed(KeyCode::KeyW));
        assert!(!input.is_key_just_released(KeyCode::KeyW));

        // begin_frame → JustPressed becomes Held
        input.begin_frame();
        assert!(!input.is_key_just_pressed(KeyCode::KeyW));
        assert!(input.is_key_pressed(KeyCode::KeyW));

        // Release key
        input.handle_event(&release_key(KeyCode::KeyW));
        assert!(input.is_key_just_released(KeyCode::KeyW));
        assert!(!input.is_key_pressed(KeyCode::KeyW));

        // begin_frame → JustReleased clears
        input.begin_frame();
        assert!(!input.is_key_just_released(KeyCode::KeyW));
        assert!(!input.is_key_pressed(KeyCode::KeyW));
    }

    #[test]
    fn logical_key_tracking() {
        let mut input = InputState::new();
        let key = Key::Character("w".to_string());

        input.handle_event(&press_key_logical(KeyCode::KeyW, key.clone()));
        assert!(input.is_logical_key_just_pressed(&key));
        assert!(input.is_logical_key_pressed(&key));

        input.begin_frame();
        assert!(!input.is_logical_key_just_pressed(&key));
        assert!(input.is_logical_key_pressed(&key));

        input.handle_event(&release_key_logical(KeyCode::KeyW, key.clone()));
        assert!(!input.is_logical_key_pressed(&key));
    }

    #[test]
    fn mouse_button_transitions() {
        let mut input = InputState::new();

        input.handle_event(&WindowEvent::MouseButtonInput {
            button: MouseButton::Left,
            state: ElementState::Pressed,
        });
        assert!(input.is_mouse_button_just_pressed(MouseButton::Left));
        assert!(input.is_mouse_button_pressed(MouseButton::Left));

        input.begin_frame();
        assert!(!input.is_mouse_button_just_pressed(MouseButton::Left));
        assert!(input.is_mouse_button_pressed(MouseButton::Left));

        input.handle_event(&WindowEvent::MouseButtonInput {
            button: MouseButton::Left,
            state: ElementState::Released,
        });
        assert!(input.is_mouse_button_just_released(MouseButton::Left));
        assert!(!input.is_mouse_button_pressed(MouseButton::Left));

        input.begin_frame();
        assert!(!input.is_mouse_button_just_released(MouseButton::Left));
    }

    #[test]
    fn mouse_position_and_cursor_left() {
        let mut input = InputState::new();
        assert!(input.mouse_position().is_none());

        input.handle_event(&WindowEvent::CursorMoved(Point::<Physical>::new(100.0, 200.0)));
        let pos = input.mouse_position().unwrap();
        assert_eq!(pos.x, 100.0);
        assert_eq!(pos.y, 200.0);

        input.handle_event(&WindowEvent::CursorLeft);
        assert!(input.mouse_position().is_none());
    }

    #[test]
    fn mouse_delta_accumulates_and_clears() {
        let mut input = InputState::new();

        input.handle_device_event(&DeviceEvent::MouseMotion {
            delta_x: 1.5,
            delta_y: -2.0,
        });
        input.handle_device_event(&DeviceEvent::MouseMotion {
            delta_x: 0.5,
            delta_y: 1.0,
        });
        assert_eq!(input.mouse_delta(), (2.0, -1.0));

        input.begin_frame();
        assert_eq!(input.mouse_delta(), (0.0, 0.0));
    }

    #[test]
    fn scroll_delta_accumulates_and_clears() {
        let mut input = InputState::new();

        input.handle_event(&WindowEvent::MouseWheel(MouseScrollDelta::LineDelta {
            x: 0.0,
            y: 3.0,
        }));
        input.handle_event(&WindowEvent::MouseWheel(MouseScrollDelta::PixelDelta {
            x: 10.0,
            y: -5.0,
        }));
        assert_eq!(input.scroll_delta(), (10.0, -2.0));

        input.begin_frame();
        assert_eq!(input.scroll_delta(), (0.0, 0.0));
    }

    #[test]
    fn text_input_appends_and_clears() {
        let mut input = InputState::new();

        input.handle_event(&WindowEvent::Ime(ImeEvent::Commit("hello".to_string())));
        input.handle_event(&WindowEvent::Ime(ImeEvent::Commit(" world".to_string())));
        assert_eq!(input.text_input(), "hello world");

        input.begin_frame();
        assert_eq!(input.text_input(), "");
    }

    #[test]
    fn repeat_events_do_not_re_trigger_just_pressed() {
        let mut input = InputState::new();

        // Initial press
        input.handle_event(&press_key(KeyCode::Space));
        assert!(input.is_key_just_pressed(KeyCode::Space));

        // Advance so it becomes Held
        input.begin_frame();
        assert!(!input.is_key_just_pressed(KeyCode::Space));
        assert!(input.is_key_pressed(KeyCode::Space));

        // OS repeat event — should NOT flip back to JustPressed
        input.handle_event(&WindowEvent::KeyboardInput(KeyEvent {
            key_code: KeyCode::Space,
            key: Key::Named(NamedKey::Space),
            state: ElementState::Pressed,
            location: KeyLocation::Standard,
            repeat: true,
        }));
        assert!(!input.is_key_just_pressed(KeyCode::Space));
        assert!(input.is_key_pressed(KeyCode::Space));
    }

    #[test]
    fn modifiers_changed() {
        let mut input = InputState::new();
        assert!(!input.modifiers().shift);

        input.handle_event(&WindowEvent::ModifiersChanged(ModifiersState {
            shift: true,
            control: false,
            alt: false,
            meta: false,
        }));
        assert!(input.modifiers().shift);
        assert!(!input.modifiers().control);
    }

    #[test]
    fn ime_non_commit_events_are_ignored() {
        let mut input = InputState::new();
        input.handle_event(&WindowEvent::Ime(ImeEvent::Enabled));
        input.handle_event(&WindowEvent::Ime(ImeEvent::Preedit(
            "composing".to_string(),
            None,
        )));
        input.handle_event(&WindowEvent::Ime(ImeEvent::Disabled));
        assert_eq!(input.text_input(), "");
    }
}
