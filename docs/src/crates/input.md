# astrelis-input

The `astrelis-input` crate provides a unified input state management system for tracking keyboard, mouse, and other input devices. It integrates seamlessly with the windowing system to provide convenient per-frame input queries.

## Features

- **Keyboard Tracking**: Track key states, including pressed, just pressed, and just released.
- **Mouse Tracking**: Track mouse button states, position, movement delta, and scroll wheel.
- **Modifier Keys**: Easy access to Shift, Ctrl, Alt, and Meta key states.
- **Text Input**: Capture text input from keyboard events.
- **Movement Helpers**: Built-in support for common movement patterns (WASD, arrow keys).

## Usage

```rust
use astrelis_input::{InputState, Key, MouseButton};

// Create input state tracker
let mut input = InputState::new();

// In your event handling loop:
input.handle_events(&mut events);

// Query keyboard state
if input.is_key_pressed(Key::Space) {
    player.jump();
}

if input.is_key_just_pressed(Key::Escape) {
    game.pause();
}

// Query mouse state
if input.is_left_mouse_just_pressed() {
    let pos = input.mouse_position();
    handle_click(pos);
}

// Use movement helpers
let movement = input.movement_direction();
player.move_by(movement * speed * delta_time);

// At the end of each frame:
input.end_frame();
```

## Core Types

### `InputState`

The main input tracking struct. It maintains the current state of all input devices and provides convenient query methods.

```rust
let mut input = InputState::new();

// Process events
input.handle_events(&mut event_batch);

// Query state
let pressed = input.is_key_pressed(Key::KeyW);
let just_pressed = input.is_key_just_pressed(Key::Space);
let just_released = input.is_key_just_released(Key::Escape);

// End frame (clears per-frame state)
input.end_frame();
```

### `MouseButton`

Enum for mouse button identification:

```rust
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Other(u16),
}
```

### `Modifiers`

Struct for tracking modifier key state:

```rust
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool, // Command on macOS, Windows key on Windows
}
```

### `InputSystem`

A wrapper around `InputState` that provides additional functionality and convenience:

```rust
let mut system = InputSystem::new();

// InputSystem derefs to InputState
if system.is_key_pressed(Key::Space) {
    // ...
}
```

## Keyboard Queries

```rust
// Check if a key is currently held down
input.is_key_pressed(Key::Space)

// Check if a key was pressed this frame
input.is_key_just_pressed(Key::Enter)

// Check if a key was released this frame
input.is_key_just_released(Key::Escape)

// Check multiple keys
input.is_any_key_pressed(&[Key::KeyW, Key::ArrowUp])
input.are_all_keys_pressed(&[Key::ControlLeft, Key::KeyS])

// Modifier keys
input.is_shift_pressed()
input.is_ctrl_pressed()
input.is_alt_pressed()
input.is_meta_pressed()
let mods = input.modifiers();

// Get text input received this frame
let text = input.text_input();

// Iterate all pressed keys
for key in input.pressed_keys() {
    println!("{:?} is pressed", key);
}
```

## Mouse Queries

```rust
// Button state
input.is_mouse_button_pressed(MouseButton::Left)
input.is_mouse_button_just_pressed(MouseButton::Right)
input.is_mouse_button_just_released(MouseButton::Middle)

// Convenience methods
input.is_left_mouse_pressed()
input.is_left_mouse_just_pressed()
input.is_right_mouse_pressed()
input.is_right_mouse_just_pressed()
input.is_middle_mouse_pressed()

// Position and movement
let pos = input.mouse_position();    // Current position in window
let delta = input.mouse_delta();      // Movement since last frame
let scroll = input.scroll_delta();    // Scroll wheel delta

// Check if mouse is in window
input.is_mouse_in_window()
```

## Movement Helpers

Built-in support for common movement patterns using arrow keys and WASD:

```rust
// Get axis values (-1, 0, or 1)
let horizontal = input.horizontal_axis(); // Left/A = -1, Right/D = 1
let vertical = input.vertical_axis();     // Up/W = -1, Down/S = 1

// Get normalized movement direction
let direction = input.movement_direction();
player.position += direction * speed * delta_time;
```

## Integration Example

Complete example integrating with the windowing system:

```rust
use astrelis_input::{InputState, Key, MouseButton};
use astrelis_winit::app::{App, AppCtx, run_app};
use astrelis_winit::event::EventBatch;

struct MyGame {
    input: InputState,
    player_pos: Vec2,
}

impl App for MyGame {
    fn update(&mut self, ctx: &mut AppCtx) {
        // Movement
        let dir = self.input.movement_direction();
        self.player_pos += dir * 200.0 * ctx.delta_time;
        
        // Actions
        if self.input.is_key_just_pressed(Key::Space) {
            self.player.attack();
        }
        
        // Mouse look
        if self.input.is_right_mouse_pressed() {
            let delta = self.input.mouse_delta();
            self.camera.rotate(delta * 0.005);
        }
    }
    
    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        // Process input events
        self.input.handle_events(events);
        
        // ... render ...
        
        // Clear per-frame state
        self.input.end_frame();
    }
}
```

## Frame Lifecycle

For correct input handling, follow this lifecycle:

1. **Start of frame**: Call `input.handle_events(&mut events)` to process events.
2. **During frame**: Query input state using the various methods.
3. **End of frame**: Call `input.end_frame()` to clear per-frame state.

```rust
// Correct order:
input.handle_events(&mut events);  // 1. Process events

if input.is_key_just_pressed(Key::Space) {  // 2. Query state
    // This works!
}

input.end_frame();  // 3. Clear per-frame state

// On next frame, is_key_just_pressed will return false
// (unless Space was pressed again)
```

## Key Code Reference

The `Key` type (re-exported from `astrelis_winit::event::KeyCode`) includes all standard keyboard keys:

- **Letters**: `Key::KeyA` through `Key::KeyZ`
- **Numbers**: `Key::Digit0` through `Key::Digit9`
- **Function keys**: `Key::F1` through `Key::F24`
- **Arrows**: `Key::ArrowUp`, `Key::ArrowDown`, `Key::ArrowLeft`, `Key::ArrowRight`
- **Modifiers**: `Key::ShiftLeft`, `Key::ControlLeft`, `Key::AltLeft`, `Key::SuperLeft`, etc.
- **Special**: `Key::Space`, `Key::Enter`, `Key::Escape`, `Key::Tab`, `Key::Backspace`, etc.
