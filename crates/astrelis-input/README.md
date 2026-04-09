# astrelis-input

Polling-style input state tracking for the Astrelis engine.

This crate sits at Layer 2 and provides a stateful input tracker that
accumulates `WindowEvent` and `DeviceEvent` data each frame. Game code
queries the current state ("is W pressed?", "was left mouse clicked this
frame?") rather than subscribing to events.

## Key Types

| Type | Description |
|------|-------------|
| `InputState` | Central state tracker — feed events in, query state out |
| `ButtonState` | Per-button state machine: Released → JustPressed → Held → JustReleased |

## Usage

```rust
use astrelis_input::InputState;
use astrelis_window::keyboard::KeyCode;
use astrelis_window::mouse::MouseButton;

let mut input = InputState::new();

// Each frame:
input.begin_frame();
// ... feed window events via input.handle_event(&event) ...

if input.is_key_just_pressed(KeyCode::Space) {
    // player jumped this frame
}

if input.is_mouse_button_pressed(MouseButton::Left) {
    // fire weapon while held
}

let (dx, dy) = input.mouse_delta();
// rotate camera by (dx, dy)
```

## Frame Lifecycle

1. Call `begin_frame()` — advances JustPressed → Held, clears per-frame accumulators
2. Feed events via `handle_event()` and `handle_device_event()`
3. Query state with `is_key_pressed()`, `mouse_position()`, etc.

## License

MIT
