# Input Handling

This guide explains how to handle keyboard, mouse, and gamepad input in Astrelis. Learn to create responsive controls and input mapping systems for your game.

## Overview

Astrelis provides input handling through:

- **Event-based input**: Direct event handling from winit
- **State-based input**: Query input state (is key pressed?)
- **Input mapping**: Map inputs to actions (e.g., Space → Jump)
- **Input contexts**: Different control schemes per game state

**Comparison to Unity:** Similar to Unity's Input System, but with explicit event handling and state querying.

## Input Architecture

### Event Flow

```text
OS/Hardware → winit → EventBatch → App::render() → Game Logic
                                  → UI System
```

**Key Points:**
- Events are batched per frame
- UI consumes events first (can block game input)
- Unconsumed events go to game logic

## Event-Based Input

### Handling Keyboard Events

```rust
use astrelis_winit::event::{Event, HandleStatus, VirtualKeyCode};

impl App for MyGame {
    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        events.dispatch(|event| {
            match event {
                Event::KeyPressed { key, .. } => {
                    match key {
                        VirtualKeyCode::Space => {
                            self.player.jump();
                            HandleStatus::consumed()
                        }
                        VirtualKeyCode::Escape => {
                            self.toggle_pause();
                            HandleStatus::consumed()
                        }
                        _ => HandleStatus::ignored()
                    }
                }
                Event::KeyReleased { key, .. } => {
                    // Handle key release
                    HandleStatus::ignored()
                }
                _ => HandleStatus::ignored()
            }
        });
    }
}
```

### Mouse Events

```rust
events.dispatch(|event| {
    match event {
        Event::MouseMoved { position } => {
            self.cursor_position = *position;
            HandleStatus::ignored() // Let UI also process
        }
        Event::MouseButtonPressed { button } => {
            match button {
                MouseButton::Left => {
                    self.on_click(self.cursor_position);
                    HandleStatus::consumed()
                }
                MouseButton::Right => {
                    self.on_right_click(self.cursor_position);
                    HandleStatus::consumed()
                }
                _ => HandleStatus::ignored()
            }
        }
        Event::MouseWheel { delta } => {
            self.zoom_camera(delta.y);
            HandleStatus::consumed()
        }
        _ => HandleStatus::ignored()
    }
});
```

### Key Modifiers

```rust
Event::KeyPressed { key, modifiers } => {
    // Check modifiers
    if modifiers.ctrl && key == &VirtualKeyCode::S {
        self.save_game();
        return HandleStatus::consumed();
    }

    if modifiers.shift && key == &VirtualKeyCode::Tab {
        self.cycle_targets_backward();
        return HandleStatus::consumed();
    }

    HandleStatus::ignored()
}
```

## State-Based Input

### InputState Resource

```rust
use astrelis_input::InputState;

// Access input state
if let Some(input) = engine.get::<Arc<InputState>>() {
    if input.is_key_pressed(VirtualKeyCode::W) {
        player.move_forward(speed * delta_time);
    }

    if input.is_key_pressed(VirtualKeyCode::S) {
        player.move_backward(speed * delta_time);
    }

    if input.is_mouse_button_down(MouseButton::Left) {
        player.fire_weapon();
    }
}
```

### Querying Input State

```rust
impl InputState {
    // Keyboard
    pub fn is_key_pressed(&self, key: VirtualKeyCode) -> bool;
    pub fn is_key_just_pressed(&self, key: VirtualKeyCode) -> bool;
    pub fn is_key_just_released(&self, key: VirtualKeyCode) -> bool;

    // Mouse
    pub fn is_mouse_button_down(&self, button: MouseButton) -> bool;
    pub fn mouse_position(&self) -> Vec2;
    pub fn mouse_delta(&self) -> Vec2;
    pub fn mouse_wheel_delta(&self) -> f32;
}
```

### Input in Update Loop

```rust
impl App for MyGame {
    fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
        let Some(input) = ctx.engine.get::<Arc<InputState>>() else {
            return;
        };

        // Movement
        let mut movement = Vec2::ZERO;

        if input.is_key_pressed(VirtualKeyCode::W) {
            movement.y += 1.0;
        }
        if input.is_key_pressed(VirtualKeyCode::S) {
            movement.y -= 1.0;
        }
        if input.is_key_pressed(VirtualKeyCode::A) {
            movement.x -= 1.0;
        }
        if input.is_key_pressed(VirtualKeyCode::D) {
            movement.x += 1.0;
        }

        // Normalize diagonal movement
        if movement.length() > 0.0 {
            movement = movement.normalize();
            self.player.velocity = movement * self.player.speed;
        }

        // Actions
        if input.is_key_just_pressed(VirtualKeyCode::Space) {
            self.player.jump();
        }
    }
}
```

## Input Mapping

### Action-Based Input

Define actions instead of raw keys:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameAction {
    MoveForward,
    MoveBackward,
    MoveLeft,
    MoveRight,
    Jump,
    Fire,
    Reload,
    Interact,
}

pub struct InputMapper {
    bindings: HashMap<VirtualKeyCode, GameAction>,
}

impl InputMapper {
    pub fn new() -> Self {
        let mut bindings = HashMap::new();

        // Default key bindings
        bindings.insert(VirtualKeyCode::W, GameAction::MoveForward);
        bindings.insert(VirtualKeyCode::S, GameAction::MoveBackward);
        bindings.insert(VirtualKeyCode::A, GameAction::MoveLeft);
        bindings.insert(VirtualKeyCode::D, GameAction::MoveRight);
        bindings.insert(VirtualKeyCode::Space, GameAction::Jump);
        bindings.insert(VirtualKeyCode::LShift, GameAction::Fire);
        bindings.insert(VirtualKeyCode::R, GameAction::Reload);
        bindings.insert(VirtualKeyCode::E, GameAction::Interact);

        Self { bindings }
    }

    pub fn is_action_active(&self, action: GameAction, input: &InputState) -> bool {
        self.bindings.iter()
            .any(|(key, &mapped_action)| {
                mapped_action == action && input.is_key_pressed(*key)
            })
    }

    pub fn rebind(&mut self, action: GameAction, new_key: VirtualKeyCode) {
        // Remove old binding
        self.bindings.retain(|_, &mut a| a != action);

        // Add new binding
        self.bindings.insert(new_key, action);
    }
}
```

### Using Input Mapper

```rust
impl App for MyGame {
    fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
        let Some(input) = ctx.engine.get::<Arc<InputState>>() else {
            return;
        };

        let mapper = &self.input_mapper;

        // Check actions instead of keys
        if mapper.is_action_active(GameAction::MoveForward, &input) {
            self.player.move_forward(time.delta.as_secs_f32());
        }

        if mapper.is_action_active(GameAction::Jump, &input) {
            self.player.jump();
        }

        if mapper.is_action_active(GameAction::Fire, &input) {
            self.player.fire_weapon();
        }
    }
}
```

## Input Contexts

Different control schemes for different game states:

```rust
pub enum InputContext {
    Gameplay,
    Menu,
    Inventory,
    Dialogue,
}

pub struct InputSystem {
    current_context: InputContext,
    gameplay_mapper: InputMapper,
    menu_mapper: InputMapper,
}

impl InputSystem {
    pub fn process_input(&self, input: &InputState) -> Vec<GameAction> {
        let mapper = match self.current_context {
            InputContext::Gameplay => &self.gameplay_mapper,
            InputContext::Menu => &self.menu_mapper,
            _ => return Vec::new(),
        };

        // Return active actions for current context
        [
            GameAction::MoveForward,
            GameAction::MoveBackward,
            GameAction::Jump,
            GameAction::Fire,
        ]
        .iter()
        .filter(|&&action| mapper.is_action_active(action, input))
        .copied()
        .collect()
    }

    pub fn set_context(&mut self, context: InputContext) {
        self.current_context = context;
    }
}
```

## UI vs Game Input Separation

UI consumes input first:

```rust
impl App for MyGame {
    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        // UI processes events first
        ui.handle_events(events);

        // Remaining events go to game
        events.dispatch(|event| {
            match event {
                Event::KeyPressed { key, .. } => {
                    // This only fires if UI didn't consume the event
                    self.handle_game_input(key);
                    HandleStatus::consumed()
                }
                _ => HandleStatus::ignored()
            }
        });
    }
}
```

## Mouse Input Patterns

### Click Detection

```rust
struct ClickTracker {
    last_click_time: Instant,
    double_click_threshold: Duration,
}

impl ClickTracker {
    pub fn on_click(&mut self) -> ClickType {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_click_time);

        let result = if elapsed < self.double_click_threshold {
            ClickType::Double
        } else {
            ClickType::Single
        };

        self.last_click_time = now;
        result
    }
}

pub enum ClickType {
    Single,
    Double,
}
```

### Drag Detection

```rust
struct DragTracker {
    drag_start: Option<Vec2>,
    drag_threshold: f32,
}

impl DragTracker {
    pub fn on_mouse_down(&mut self, position: Vec2) {
        self.drag_start = Some(position);
    }

    pub fn on_mouse_move(&mut self, position: Vec2) -> Option<DragEvent> {
        if let Some(start) = self.drag_start {
            let distance = (position - start).length();

            if distance > self.drag_threshold {
                return Some(DragEvent {
                    start,
                    current: position,
                    delta: position - start,
                });
            }
        }

        None
    }

    pub fn on_mouse_up(&mut self) {
        self.drag_start = None;
    }
}
```

## Gamepad Support

### Gamepad State

```rust
#[cfg(feature = "gamepad")]
pub struct GamepadState {
    pub left_stick: Vec2,
    pub right_stick: Vec2,
    pub triggers: (f32, f32), // Left, Right
    pub buttons: HashSet<GamepadButton>,
}

#[cfg(feature = "gamepad")]
impl GamepadState {
    pub fn is_button_pressed(&self, button: GamepadButton) -> bool {
        self.buttons.contains(&button)
    }
}

#[cfg(feature = "gamepad")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadButton {
    South,      // A/Cross
    East,       // B/Circle
    West,       // X/Square
    North,      // Y/Triangle
    L1,
    R1,
    L2,
    R2,
    Select,
    Start,
}
```

## Input Buffer

For precise timing (fighting games, combos):

```rust
pub struct InputBuffer {
    events: VecDeque<InputEvent>,
    max_age: Duration,
}

#[derive(Clone)]
pub struct InputEvent {
    pub action: GameAction,
    pub timestamp: Instant,
}

impl InputBuffer {
    pub fn push(&mut self, action: GameAction) {
        self.events.push_back(InputEvent {
            action,
            timestamp: Instant::now(),
        });

        // Remove old events
        let cutoff = Instant::now() - self.max_age;
        self.events.retain(|e| e.timestamp > cutoff);
    }

    pub fn check_sequence(&self, sequence: &[GameAction]) -> bool {
        if sequence.len() > self.events.len() {
            return false;
        }

        self.events.iter()
            .rev()
            .take(sequence.len())
            .map(|e| e.action)
            .eq(sequence.iter().rev().copied())
    }
}

// Usage: check for combo (Down, Down+Forward, Forward + Punch)
if input_buffer.check_sequence(&[
    GameAction::Down,
    GameAction::DownForward,
    GameAction::Forward,
    GameAction::Punch,
]) {
    player.execute_special_move();
}
```

## Performance Considerations

### Input Polling Frequency

Input is processed per frame:

```rust
// 60 FPS = 16.6ms between input checks
// 144 FPS = 6.9ms between input checks
```

**Best for:** Most games

**Alternative:** Poll at higher rate if needed (e.g., rhythm games)

### Debouncing

Prevent input spam:

```rust
pub struct DebouncedInput {
    last_action_time: HashMap<GameAction, Instant>,
    debounce_duration: Duration,
}

impl DebouncedInput {
    pub fn can_perform(&mut self, action: GameAction) -> bool {
        let now = Instant::now();

        if let Some(&last_time) = self.last_action_time.get(&action) {
            if now.duration_since(last_time) < self.debounce_duration {
                return false; // Too soon
            }
        }

        self.last_action_time.insert(action, now);
        true
    }
}
```

## Best Practices

### ✅ DO: Use Action Mapping

```rust
// Good: Actions, not keys
if mapper.is_action_active(GameAction::Jump, input) {
    player.jump();
}

// Bad: Hard-coded keys
if input.is_key_pressed(VirtualKeyCode::Space) {
    player.jump();
}
```

### ✅ DO: Let UI Consume Events First

```rust
// Good: UI handles events before game
ui.handle_events(events);
events.dispatch(|event| {
    // Game logic here
});
```

### ✅ DO: Normalize Movement Vectors

```rust
// Good: Diagonal movement same speed
let movement = Vec2::new(x, y).normalize();

// Bad: Diagonal is faster
let movement = Vec2::new(x, y);
```

### ❌ DON'T: Check Input in Render

```rust
// BAD: Input in render loop
fn render(&mut self, ...) {
    if input.is_key_pressed(VirtualKeyCode::W) {
        self.player.position.y += 1.0; // Frame-rate dependent!
    }
}

// GOOD: Input in update loop
fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
    if input.is_key_pressed(VirtualKeyCode::W) {
        self.player.position.y += speed * time.delta.as_secs_f32();
    }
}
```

## Comparison to Unity

| Unity | Astrelis | Notes |
|-------|----------|-------|
| `Input.GetKey()` | `input.is_key_pressed()` | Similar |
| `Input.GetKeyDown()` | `input.is_key_just_pressed()` | Frame-perfect |
| `Input.GetAxis()` | Manual mapping | Custom implementation |
| `Input.mousePosition` | `input.mouse_position()` | Direct access |
| Input System Package | InputMapper | Action-based |

## Troubleshooting

### Key Not Detected

**Cause:** Event consumed by UI or wrong event type.

**Fix:** Check UI isn't consuming input:
```rust
// Ensure UI is not blocking
ui.handle_events(events);

// Debug remaining events
events.dispatch(|event| {
    println!("Unconsumed event: {:?}", event);
    HandleStatus::ignored()
});
```

### Input Lag

**Cause:** Processing input in wrong place or buffering.

**Fix:** Process in update loop with delta time:
```rust
fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
    // Process input here, not in render
}
```

## Next Steps

- **Practice:** Implement a control settings menu
- **Advanced:** Create input profiles for different players
- **Integration:** Combine with [UI Event Handling](../ui/event-handling.md)

## See Also

- [UI Event Handling](../ui/event-handling.md) - UI-specific input
- API Reference: [`InputState`](../../api/astrelis-input/struct.InputState.html)
- API Reference: [`Event`](../../api/astrelis-winit/event/enum.Event.html)
