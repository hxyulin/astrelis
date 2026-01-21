# Event Handling

This guide covers input handling and event management in Astrelis UI, including mouse clicks, keyboard input, focus navigation, hover states, and custom events.

## Prerequisites

- Completed [First UI](../getting-started/05-first-ui.md)
- Understanding of Rust closures and ownership

## Event Processing Flow

### The Event Pipeline

```
Winit Events → EventBatch → UI System → Widget Handlers → Application
     ↓              ↓             ↓            ↓              ↓
Platform      Per-window     Hit testing   Callbacks    Game logic
 events        batching       + dispatch    executed     updates
```

### Processing Events in render()

**Critical**: Call `process_events()` every frame:

```rust
impl App for MyApp {
    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        if window_id != self.window_id {
            return;
        }

        // 1. Process UI events (REQUIRED!)
        self.ui.process_events(events);

        // 2. Render UI
        let mut frame = self.window.begin_drawing();
        frame.clear_and_render(RenderTarget::Surface, Color::BLACK, |pass| {
            self.ui.render(pass.descriptor());
        });
        frame.finish();
    }
}
```

**What `process_events()` does**:
- Hit tests mouse position against widget bounds
- Updates hover states
- Dispatches click events to clickable widgets
- Routes keyboard events to focused widgets

## Click Events

### Basic Click Handler

```rust
parent.button("Click Me")
    .on_click(|| {
        println!("Button clicked!");
    })
    .build()
```

### Click with Shared State

Use `Arc<RwLock<T>>` for mutable shared state:

```rust
use std::sync::{Arc, RwLock};

let counter = Arc::new(RwLock::new(0));
let counter_clone = counter.clone();

parent.button("+")
    .on_click(move || {
        *counter_clone.write().unwrap() += 1;
    })
    .build()

// Later, read the counter
let count = *counter.read().unwrap();
```

### Click with Callback Parameters

Pass data to click handlers:

```rust
let item_id = 42;

parent.button("Delete")
    .on_click(move || {
        println!("Deleting item {}", item_id);
        // item_id is captured by move
    })
    .build()
```

### Multiple Buttons with Same Handler

```rust
fn create_number_button(parent: &mut Builder, number: i32, selected: Arc<RwLock<i32>>) {
    parent.button(format!("{}", number))
        .on_click(move || {
            *selected.write().unwrap() = number;
        })
        .build()
}

// Create grid of number buttons
let selected = Arc::new(RwLock::new(0));
for i in 1..=9 {
    create_number_button(parent, i, selected.clone());
}
```

### Preventing Click Propagation

By default, clicks may propagate to parent widgets. Handle this with custom logic:

```rust
// In custom widget's click handler
fn handle_click(&mut self, position: Vec2) -> bool {
    if self.bounds().contains(position) {
        // Handle click
        true  // Consumed, stop propagation
    } else {
        false  // Not handled, continue propagation
    }
}
```

## Hover States

### Automatic Hover (Buttons)

Buttons automatically track hover state:

```rust
parent.button("Hover Me")
    .background_color(Color::from_rgb(0.3, 0.6, 0.9))
    .hover_color(Color::from_rgb(0.4, 0.7, 1.0))  // Lighter on hover
    .build()
```

### Custom Hover Handling

For custom widgets, implement `ClickableWidget`:

```rust
impl ClickableWidget for MyWidget {
    fn is_hovered(&self) -> bool {
        self.is_hovered
    }

    fn is_pressed(&self) -> bool {
        self.is_pressed
    }

    // Internal: UI system sets hover state
    fn set_hover(&mut self, hovered: bool) {
        self.is_hovered = hovered;
    }

    fn set_pressed(&mut self, pressed: bool) {
        self.is_pressed = pressed;
    }
}
```

### Hover Enter/Exit Callbacks

```rust
struct HoverableWidget {
    on_hover_enter: Option<Box<dyn FnMut() + Send + Sync>>,
    on_hover_exit: Option<Box<dyn FnMut() + Send + Sync>>,
}

impl HoverableWidget {
    pub fn on_hover_enter<F>(mut self, callback: F) -> Self
    where
        F: FnMut() + Send + Sync + 'static,
    {
        self.on_hover_enter = Some(Box::new(callback));
        self
    }

    fn handle_hover_change(&mut self, now_hovered: bool) {
        if now_hovered && self.on_hover_enter.is_some() {
            (self.on_hover_enter.as_mut().unwrap())();
        } else if !now_hovered && self.on_hover_exit.is_some() {
            (self.on_hover_exit.as_mut().unwrap())();
        }
    }
}
```

## Keyboard Input

### Handling Key Presses

Process keyboard events from `EventBatch`:

```rust
impl App for MyApp {
    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        // Handle keyboard before UI processes events
        events.dispatch(|event| {
            use astrelis_winit::event::{Event, HandleStatus, Key};

            match event {
                Event::KeyPressed(key) => {
                    match key {
                        Key::Escape => {
                            println!("Escape pressed");
                            HandleStatus::consumed()
                        }
                        Key::Enter => {
                            println!("Enter pressed");
                            HandleStatus::consumed()
                        }
                        Key::Space => {
                            println!("Space pressed");
                            HandleStatus::consumed()
                        }
                        _ => HandleStatus::ignored()
                    }
                }
                _ => HandleStatus::ignored()
            }
        });

        // Then process UI events
        self.ui.process_events(events);

        // ... render
    }
}
```

### Key Modifiers

Check for modifier keys:

```rust
Event::KeyPressed(key) => {
    use astrelis_winit::event::Modifiers;

    if key == Key::S && events.modifiers().ctrl() {
        // Ctrl+S pressed
        save_file();
        HandleStatus::consumed()
    } else if key == Key::C && events.modifiers().shift() {
        // Shift+C pressed
        HandleStatus::consumed()
    } else {
        HandleStatus::ignored()
    }
}
```

### Character Input (Text Editing)

For text input widgets:

```rust
Event::CharInput(c) => {
    if self.text_input_focused {
        self.text_input.push(c);
        self.ui.update_text(&self.text_input_id, &self.text_input);
        HandleStatus::consumed()
    } else {
        HandleStatus::ignored()
    }
}
```

### Backspace and Delete

```rust
Event::KeyPressed(key) => {
    if self.text_input_focused {
        match key {
            Key::Backspace => {
                self.text_input.pop();
                self.ui.update_text(&self.text_input_id, &self.text_input);
                HandleStatus::consumed()
            }
            Key::Delete => {
                // Handle delete
                HandleStatus::consumed()
            }
            _ => HandleStatus::ignored()
        }
    } else {
        HandleStatus::ignored()
    }
}
```

## Focus Management

### FocusManager

Astrelis UI includes focus management for keyboard navigation:

```rust
use astrelis_ui::FocusManager;

struct MyApp {
    ui: UiSystem,
    focus_manager: FocusManager,
}

impl App for MyApp {
    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        // Handle Tab key for focus navigation
        events.dispatch(|event| {
            use astrelis_winit::event::{Event, HandleStatus, Key};

            if let Event::KeyPressed(Key::Tab) = event {
                if events.modifiers().shift() {
                    self.focus_manager.focus_previous();
                } else {
                    self.focus_manager.focus_next();
                }
                HandleStatus::consumed()
            } else {
                HandleStatus::ignored()
            }
        });

        self.ui.process_events(events);
        // ... render
    }
}
```

### Focusable Widgets

Mark widgets as focusable:

```rust
parent.button("Button 1")
    .focusable(true)  // Can receive focus
    .tab_index(1)     // Tab order
    .build()

parent.button("Button 2")
    .focusable(true)
    .tab_index(2)
    .build()
```

### Focus Indicators

Visual feedback for focused widgets:

```rust
fn render_button(&self, button: &Button) {
    let border_color = if button.is_focused() {
        Color::from_rgb(0.5, 0.8, 1.0)  // Blue highlight
    } else {
        Color::from_rgb(0.4, 0.4, 0.6)  // Normal border
    };

    draw_button(button, border_color);
}
```

### Programmatic Focus

```rust
// Focus a specific widget
self.focus_manager.set_focus(widget_id);

// Clear focus
self.focus_manager.clear_focus();

// Check if widget has focus
if self.focus_manager.has_focus(widget_id) {
    // Widget is focused
}
```

## Custom Events

### Message-Based Architecture

For complex applications, use message passing:

```rust
use std::sync::mpsc::{channel, Sender, Receiver};

#[derive(Debug, Clone)]
pub enum UiMessage {
    ButtonClicked { id: String },
    ValueChanged { id: String, value: f32 },
    ItemSelected { index: usize },
    DialogClosed { result: bool },
}

struct MyApp {
    ui: UiSystem,
    ui_sender: Sender<UiMessage>,
    ui_receiver: Receiver<UiMessage>,
}

impl MyApp {
    fn new() -> Self {
        let (sender, receiver) = channel();
        // Pass sender to widgets
        Self {
            ui: UiSystem::new(graphics.clone()),
            ui_sender: sender,
            ui_receiver: receiver,
        }
    }
}

impl App for MyApp {
    fn update(&mut self, _ctx: &mut AppCtx, _time: &FrameTime) {
        // Process UI messages
        while let Ok(message) = self.ui_receiver.try_recv() {
            match message {
                UiMessage::ButtonClicked { id } => {
                    println!("Button clicked: {}", id);
                    // Handle click
                }
                UiMessage::ValueChanged { id, value } => {
                    println!("Value changed: {} = {}", id, value);
                    // Handle value change
                }
                UiMessage::ItemSelected { index } => {
                    println!("Item selected: {}", index);
                    // Handle selection
                }
                UiMessage::DialogClosed { result } => {
                    println!("Dialog closed: {}", result);
                    // Handle dialog result
                }
            }
        }
    }

    // ... render
}

// In widget creation
let sender = self.ui_sender.clone();
parent.button("Click")
    .on_click(move || {
        sender.send(UiMessage::ButtonClicked {
            id: "my_button".to_string()
        }).unwrap();
    })
    .build()
```

### Event Aggregation

Collect multiple events and process in batch:

```rust
struct EventAggregator {
    events: Vec<UiMessage>,
}

impl EventAggregator {
    fn push(&mut self, event: UiMessage) {
        self.events.push(event);
    }

    fn drain(&mut self) -> Vec<UiMessage> {
        std::mem::take(&mut self.events)
    }
}

// In update()
for event in self.event_aggregator.drain() {
    self.handle_event(event);
}
```

## Event Patterns

### Pattern 1: Direct Callback

Simple, immediate handling:

```rust
parent.button("Delete")
    .on_click(|| {
        delete_item();
    })
    .build()
```

**Pros**: Simple, direct
**Cons**: Hard to test, tight coupling

### Pattern 2: State Machine

Trigger state transitions:

```rust
#[derive(PartialEq)]
enum AppState {
    Menu,
    Playing,
    Paused,
    GameOver,
}

parent.button("Start")
    .on_click(move || {
        *state.write().unwrap() = AppState::Playing;
    })
    .build()
```

**Pros**: Clear state management
**Cons**: Can become complex

### Pattern 3: Command Pattern

Encapsulate actions:

```rust
trait Command {
    fn execute(&self);
    fn undo(&self);
}

struct DeleteCommand {
    item_id: usize,
    deleted_item: Option<Item>,
}

impl Command for DeleteCommand {
    fn execute(&self) {
        // Delete item, store for undo
    }

    fn undo(&self) {
        // Restore item
    }
}

parent.button("Delete")
    .on_click(move || {
        let cmd = DeleteCommand { item_id, deleted_item: None };
        cmd_queue.push(cmd);
    })
    .build()
```

**Pros**: Undo/redo, testable
**Cons**: More complex

### Pattern 4: Observer Pattern

Multiple subscribers:

```rust
struct Observable<T> {
    value: T,
    observers: Vec<Box<dyn Fn(&T) + Send + Sync>>,
}

impl<T> Observable<T> {
    fn subscribe<F>(&mut self, observer: F)
    where
        F: Fn(&T) + Send + Sync + 'static,
    {
        self.observers.push(Box::new(observer));
    }

    fn set(&mut self, value: T) {
        self.value = value;
        for observer in &self.observers {
            observer(&self.value);
        }
    }
}

// Usage
let mut health = Observable { value: 100, observers: vec![] };

health.subscribe(|h| {
    println!("Health changed: {}", h);
});

health.subscribe(move |h| {
    ui.update_text(&health_text_id, format!("HP: {}", h));
});
```

**Pros**: Decoupled, flexible
**Cons**: Overhead, potential leaks

## Drag and Drop (Advanced)

### Basic Drag Implementation

```rust
struct DraggableWidget {
    is_dragging: bool,
    drag_start: Vec2,
    position: Vec2,
}

impl DraggableWidget {
    fn on_mouse_down(&mut self, mouse_pos: Vec2) {
        if self.bounds().contains(mouse_pos) {
            self.is_dragging = true;
            self.drag_start = mouse_pos - self.position;
        }
    }

    fn on_mouse_move(&mut self, mouse_pos: Vec2) {
        if self.is_dragging {
            self.position = mouse_pos - self.drag_start;
        }
    }

    fn on_mouse_up(&mut self) {
        self.is_dragging = false;
    }
}
```

### Drop Zones

```rust
struct DropZone {
    bounds: Rect,
    accepts: fn(&Item) -> bool,
}

impl DropZone {
    fn can_drop(&self, item: &Item, position: Vec2) -> bool {
        self.bounds.contains(position) && (self.accepts)(item)
    }

    fn on_drop(&mut self, item: Item) {
        // Handle dropped item
        println!("Item dropped: {:?}", item);
    }
}
```

## Performance Considerations

### Debouncing Events

Limit event frequency:

```rust
struct Debouncer {
    last_trigger: Instant,
    delay: Duration,
}

impl Debouncer {
    fn should_trigger(&mut self) -> bool {
        let now = Instant::now();
        if now.duration_since(self.last_trigger) > self.delay {
            self.last_trigger = now;
            true
        } else {
            false
        }
    }
}

// Usage
if self.debouncer.should_trigger() {
    handle_search_input();
}
```

### Throttling Events

Limit event rate:

```rust
struct Throttler {
    last_call: Instant,
    min_interval: Duration,
}

impl Throttler {
    fn call<F: FnMut()>(&mut self, mut callback: F) {
        let now = Instant::now();
        if now.duration_since(self.last_call) >= self.min_interval {
            callback();
            self.last_call = now;
        }
    }
}
```

## Best Practices

1. **Always call `process_events()`**: Required in `render()` for UI to respond
2. **Handle events before UI**: Process global keys before `process_events()`
3. **Use message passing**: Decouples UI from game logic
4. **Consume handled events**: Return `HandleStatus::consumed()` to stop propagation
5. **Test event handling**: Unit test event callbacks separately
6. **Debounce/throttle**: Prevent excessive event processing
7. **Focus management**: Implement keyboard navigation with Tab
8. **Visual feedback**: Show hover, pressed, and focused states

## Common Issues

### Issue 1: Buttons Not Responding

**Cause**: Forgot to call `process_events()`

**Fix**:
```rust
self.ui.process_events(events);  // REQUIRED!
```

### Issue 2: Events Firing Multiple Times

**Cause**: Not consuming events

**Fix**:
```rust
return HandleStatus::consumed();  // Stop propagation
```

### Issue 3: Hover State Stuck

**Cause**: Widget bounds incorrect or mouse position not updated

**Fix**: Verify widget bounds with debug borders

## Next Steps

1. **[Performance Optimization](performance-optimization.md)** - Optimize event processing
2. **[Custom Widgets](custom-widgets.md)** - Build widgets with custom events
3. **Examples** - See `counter.rs`, `widget_gallery.rs` for working examples

## Summary

**Key takeaways**:
- **`process_events()`**: Call every frame for UI to work
- **Event dispatch**: Use `events.dispatch()` for keyboard/custom events
- **Click handlers**: `on_click()` for buttons, closures capture state
- **Focus management**: Tab navigation with `FocusManager`
- **Message passing**: Decouple UI from game logic with channels
- **Hover states**: Automatic for buttons, manual for custom widgets

You now know how to make your Astrelis UI fully interactive!
