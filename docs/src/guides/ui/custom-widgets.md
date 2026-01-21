# Custom Widgets

Astrelis's UI system allows you to create custom, reusable widgets by implementing the `Widget` trait and capability traits. This guide walks you through creating custom widgets from scratch, including a complete toggle switch example.

## Prerequisites

- Completed [First UI](../getting-started/05-first-ui.md)
- Understanding of Rust traits and dynamic dispatch
- Familiarity with the builder pattern

## Widget Architecture

### The Capability System

Astrelis uses a **capability-based trait system** instead of downcasting. Widgets expose their features through capability traits:

```rust
pub trait Widget: Send + Sync {
    fn id(&self) -> WidgetId;
    fn layout_node(&self) -> taffy::NodeId;

    // Capability queries
    fn as_text_widget(&self) -> Option<&dyn TextWidget> { None }
    fn as_color_widget(&self) -> Option<&dyn ColorWidget> { None }
    fn as_sized_widget(&self) -> Option<&dyn SizedWidget> { None }
    fn as_clickable_widget(&self) -> Option<&dyn ClickableWidget> { None }
    fn as_container(&self) -> Option<&dyn ParentWidget> { None }

    // Mutable versions
    fn as_text_widget_mut(&mut self) -> Option<&mut dyn TextWidget> { None }
    // ... other mutable queries
}
```

**Benefits**:
- **Compile-time safety**: No runtime downcasting failures
- **Clear capabilities**: Traits document what a widget can do
- **Composable**: Mix and match capabilities as needed

### Capability Traits

#### TextWidget

Widgets that display text:

```rust
pub trait TextWidget: Widget {
    fn text(&self) -> &str;
    fn set_text(&mut self, text: &str);
    fn build_text_style(&self) -> TextStyle;
    fn text_align(&self) -> TextAlign;
    fn set_text_align(&mut self, align: TextAlign);
}
```

#### ColorWidget

Widgets with background color:

```rust
pub trait ColorWidget: Widget {
    fn color(&self) -> Color;
    fn set_color(&mut self, color: Color);
}
```

#### SizedWidget

Widgets with explicit size:

```rust
pub trait SizedWidget: Widget {
    fn size(&self) -> Vec2;
    fn set_size(&mut self, size: Vec2);
    fn min_size(&self) -> Option<Vec2>;
    fn max_size(&self) -> Option<Vec2>;
}
```

#### ClickableWidget

Widgets that respond to clicks:

```rust
pub trait ClickableWidget: Widget {
    fn is_pressed(&self) -> bool;
    fn is_hovered(&self) -> bool;
    fn on_click(&mut self, callback: Box<dyn FnMut() + Send + Sync>);
}
```

#### ParentWidget

Widgets that contain children:

```rust
pub trait ParentWidget: Widget {
    fn children(&self) -> &[Box<dyn Widget>];
    fn children_mut(&mut self) -> &mut [Box<dyn Widget>];
    fn add_child(&mut self, child: Box<dyn Widget>);
    fn remove_child(&mut self, id: WidgetId) -> Option<Box<dyn Widget>>;
}
```

## Creating a Simple Widget

Let's build a **Label** widget step by step.

### Step 1: Define the Struct

```rust
use astrelis_core::math::Vec2;
use astrelis_render::Color;
use astrelis_text::{TextStyle, TextAlign, VerticalAlign};
use astrelis_ui::widget::{Widget, TextWidget, ColorWidget, WidgetId};

pub struct Label {
    id: WidgetId,
    layout_node: taffy::NodeId,
    text: String,
    color: Color,
    font_size: f32,
    text_align: TextAlign,
    vertical_align: VerticalAlign,
}
```

**Required fields**:
- `id`: Unique identifier for this widget instance
- `layout_node`: Taffy layout engine node ID

**Widget-specific fields**:
- Custom state for your widget's behavior

### Step 2: Implement Widget Trait

```rust
impl Widget for Label {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout_node(&self) -> taffy::NodeId {
        self.layout_node
    }

    fn debug_name(&self) -> &str {
        "Label"
    }

    // Expose text capability
    fn as_text_widget(&self) -> Option<&dyn TextWidget> {
        Some(self)
    }

    fn as_text_widget_mut(&mut self) -> Option<&mut dyn TextWidget> {
        Some(self)
    }

    // Expose color capability
    fn as_color_widget(&self) -> Option<&dyn ColorWidget> {
        Some(self)
    }

    fn as_color_widget_mut(&mut self) -> Option<&mut dyn ColorWidget> {
        Some(self)
    }
}
```

**Key pattern**: Return `Some(self)` for capabilities you support.

### Step 3: Implement Capability Traits

```rust
impl TextWidget for Label {
    fn text(&self) -> &str {
        &self.text
    }

    fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
    }

    fn build_text_style(&self) -> TextStyle {
        TextStyle {
            size: self.font_size,
            color: self.color,
            align: self.text_align,
            vertical_align: self.vertical_align,
            bold: false,
            italic: false,
        }
    }

    fn text_align(&self) -> TextAlign {
        self.text_align
    }

    fn set_text_align(&mut self, align: TextAlign) {
        self.text_align = align;
    }

    fn vertical_align(&self) -> VerticalAlign {
        self.vertical_align
    }

    fn set_vertical_align(&mut self, align: VerticalAlign) {
        self.vertical_align = align;
    }
}

impl ColorWidget for Label {
    fn color(&self) -> Color {
        self.color
    }

    fn set_color(&mut self, color: Color) {
        self.color = color;
    }
}
```

### Step 4: Add Constructor

```rust
impl Label {
    pub fn new(
        id: WidgetId,
        layout_node: taffy::NodeId,
        text: impl Into<String>,
    ) -> Self {
        Self {
            id,
            layout_node,
            text: text.into(),
            color: Color::WHITE,
            font_size: 14.0,
            text_align: TextAlign::Left,
            vertical_align: VerticalAlign::Top,
        }
    }

    // Builder methods for customization
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }
}
```

### Step 5: Usage

```rust
// In your UI building code
let label = Label::new(
    WidgetId::new("my_label"),
    layout_node,
    "Hello, World!",
)
.color(Color::from_rgb(0.2, 0.8, 1.0))
.font_size(18.0);

// Store in widget storage
storage.add(Box::new(label));
```

## Complete Example: Toggle Switch

Now let's build a more complex widget: a toggle switch with state and click handling.

### Toggle Switch Structure

```rust
use astrelis_render::Color;
use astrelis_ui::widget::{Widget, ClickableWidget, ColorWidget, SizedWidget, WidgetId};
use astrelis_core::math::Vec2;
use std::sync::{Arc, RwLock};

pub struct ToggleSwitch {
    id: WidgetId,
    layout_node: taffy::NodeId,

    // State
    enabled: Arc<RwLock<bool>>,

    // Visual properties
    size: Vec2,
    on_color: Color,
    off_color: Color,
    handle_color: Color,

    // Interaction state
    is_pressed: bool,
    is_hovered: bool,

    // Callback
    on_toggle: Option<Box<dyn FnMut(bool) + Send + Sync>>,
}
```

**Shared state**: Use `Arc<RwLock<bool>>` so external code can read/write the toggle state.

### Implementation

```rust
impl ToggleSwitch {
    pub fn new(
        id: WidgetId,
        layout_node: taffy::NodeId,
        enabled: Arc<RwLock<bool>>,
    ) -> Self {
        Self {
            id,
            layout_node,
            enabled,
            size: Vec2::new(50.0, 26.0),
            on_color: Color::from_rgb(0.2, 0.8, 0.2),
            off_color: Color::from_rgb(0.4, 0.4, 0.4),
            handle_color: Color::WHITE,
            is_pressed: false,
            is_hovered: false,
            on_toggle: None,
        }
    }

    pub fn on_color(mut self, color: Color) -> Self {
        self.on_color = color;
        self
    }

    pub fn off_color(mut self, color: Color) -> Self {
        self.off_color = color;
        self
    }

    pub fn on_toggle<F>(mut self, callback: F) -> Self
    where
        F: FnMut(bool) + Send + Sync + 'static,
    {
        self.on_toggle = Some(Box::new(callback));
        self
    }

    fn toggle(&mut self) {
        // Toggle state
        let mut enabled = self.enabled.write().unwrap();
        *enabled = !*enabled;
        let new_state = *enabled;
        drop(enabled);

        // Call callback
        if let Some(callback) = &mut self.on_toggle {
            callback(new_state);
        }
    }

    pub fn is_enabled(&self) -> bool {
        *self.enabled.read().unwrap()
    }
}

impl Widget for ToggleSwitch {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout_node(&self) -> taffy::NodeId {
        self.layout_node
    }

    fn debug_name(&self) -> &str {
        "ToggleSwitch"
    }

    // Expose capabilities
    fn as_clickable_widget(&self) -> Option<&dyn ClickableWidget> {
        Some(self)
    }

    fn as_clickable_widget_mut(&mut self) -> Option<&mut dyn ClickableWidget> {
        Some(self)
    }

    fn as_sized_widget(&self) -> Option<&dyn SizedWidget> {
        Some(self)
    }

    fn as_sized_widget_mut(&mut self) -> Option<&mut dyn SizedWidget> {
        Some(self)
    }

    fn as_color_widget(&self) -> Option<&dyn ColorWidget> {
        Some(self)
    }

    fn as_color_widget_mut(&mut self) -> Option<&mut dyn ColorWidget> {
        Some(self)
    }
}

impl ClickableWidget for ToggleSwitch {
    fn is_pressed(&self) -> bool {
        self.is_pressed
    }

    fn is_hovered(&self) -> bool {
        self.is_hovered
    }

    fn on_click(&mut self, mut callback: Box<dyn FnMut() + Send + Sync>) {
        // Wrap the toggle logic
        self.on_toggle = Some(Box::new(move |_enabled| {
            callback();
        }));
    }
}

impl SizedWidget for ToggleSwitch {
    fn size(&self) -> Vec2 {
        self.size
    }

    fn set_size(&mut self, size: Vec2) {
        self.size = size;
    }

    fn min_size(&self) -> Option<Vec2> {
        Some(Vec2::new(40.0, 20.0))
    }

    fn set_min_size(&mut self, _size: Option<Vec2>) {
        // Ignore - toggle has fixed minimum
    }

    fn max_size(&self) -> Option<Vec2> {
        None
    }

    fn set_max_size(&mut self, _size: Option<Vec2>) {}
}

impl ColorWidget for ToggleSwitch {
    fn color(&self) -> Color {
        if self.is_enabled() {
            self.on_color
        } else {
            self.off_color
        }
    }

    fn set_color(&mut self, _color: Color) {
        // Could set on_color here
    }
}
```

### Custom Rendering (Advanced)

For custom rendering, you'd implement rendering logic in the UI system. The widget provides:
- Position (from layout)
- Size (from `SizedWidget`)
- Color (from `ColorWidget`)
- State (enabled, hovered, pressed)

The renderer draws:
1. Background track (rounded rectangle in on/off color)
2. Handle circle (positioned based on state, animates on toggle)

```rust
// Pseudo-code for rendering (in UI system)
fn render_toggle_switch(&self, widget: &ToggleSwitch, position: Vec2) {
    let size = widget.size();
    let enabled = widget.is_enabled();

    // Background track
    let track_color = if enabled { widget.on_color } else { widget.off_color };
    draw_rounded_rect(position, size, size.y / 2.0, track_color);

    // Handle position (lerp from left to right)
    let handle_x = if enabled {
        position.x + size.x - size.y / 2.0
    } else {
        position.x + size.y / 2.0
    };

    // Handle circle
    draw_circle(Vec2::new(handle_x, position.y + size.y / 2.0), size.y * 0.4, widget.handle_color);
}
```

### Using the Toggle Switch

```rust
use std::sync::{Arc, RwLock};

// Shared state
let dark_mode = Arc::new(RwLock::new(false));

// Create toggle
let toggle = ToggleSwitch::new(
    WidgetId::new("dark_mode_toggle"),
    layout_node,
    dark_mode.clone(),
)
.on_color(Color::from_rgb(0.2, 0.7, 1.0))
.off_color(Color::from_rgb(0.3, 0.3, 0.3))
.on_toggle(|enabled| {
    println!("Dark mode: {}", enabled);
});

// Add to UI
storage.add(Box::new(toggle));

// Later, read state
if *dark_mode.read().unwrap() {
    apply_dark_theme();
}
```

## State Management Patterns

### Pattern 1: Internal State

Simple widgets can manage state internally:

```rust
pub struct Counter {
    id: WidgetId,
    layout_node: taffy::NodeId,
    count: i32,
}

impl Counter {
    pub fn increment(&mut self) {
        self.count += 1;
    }
}
```

**Use when**: State is only used by the widget itself.

### Pattern 2: Shared State (Arc<RwLock<T>>)

For state accessed externally:

```rust
pub struct Slider {
    id: WidgetId,
    layout_node: taffy::NodeId,
    value: Arc<RwLock<f32>>,
}
```

**Use when**: Multiple parts of your app need the value.

### Pattern 3: Callback-Based

For event-driven updates:

```rust
pub struct Button {
    id: WidgetId,
    layout_node: taffy::NodeId,
    on_click: Option<Box<dyn FnMut() + Send + Sync>>,
}
```

**Use when**: Widget triggers actions but doesn't own state.

### Pattern 4: Message Passing

For complex apps, use channels:

```rust
use std::sync::mpsc::Sender;

pub enum UiMessage {
    ButtonClicked(String),
    ValueChanged(f32),
}

pub struct MessageButton {
    id: WidgetId,
    layout_node: taffy::NodeId,
    sender: Sender<UiMessage>,
    message: UiMessage,
}

impl MessageButton {
    fn click(&self) {
        let _ = self.sender.send(self.message.clone());
    }
}
```

**Use when**: Decoupling UI from game logic.

## Advanced Topics

### Widget Composition

Build complex widgets from simpler ones:

```rust
pub struct Card {
    id: WidgetId,
    layout_node: taffy::NodeId,
    title: Box<dyn Widget>,
    content: Box<dyn Widget>,
    children: Vec<Box<dyn Widget>>,
}

impl ParentWidget for Card {
    fn children(&self) -> &[Box<dyn Widget>] {
        &self.children
    }

    fn children_mut(&mut self) -> &mut [Box<dyn Widget>] {
        &mut self.children
    }

    fn add_child(&mut self, child: Box<dyn Widget>) {
        self.children.push(child);
    }

    // ... other methods
}
```

### Dynamic Widget Lists

For variable-length content:

```rust
pub struct ListView {
    id: WidgetId,
    layout_node: taffy::NodeId,
    items: Vec<Box<dyn Widget>>,
}

impl ListView {
    pub fn add_item(&mut self, item: Box<dyn Widget>) {
        self.items.push(item);
    }

    pub fn remove_item(&mut self, index: usize) -> Option<Box<dyn Widget>> {
        if index < self.items.len() {
            Some(self.items.remove(index))
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}
```

### Animated Widgets

Store animation state:

```rust
pub struct AnimatedButton {
    id: WidgetId,
    layout_node: taffy::NodeId,

    // Animation state
    animation_time: f32,
    target_scale: f32,
    current_scale: f32,
}

impl AnimatedButton {
    pub fn update(&mut self, delta: f32) {
        // Lerp towards target
        self.current_scale += (self.target_scale - self.current_scale) * delta * 5.0;
    }
}
```

Call `update()` from your app's `update()` method.

## Common Patterns and Best Practices

### 1. Builder Pattern for Construction

```rust
impl MyWidget {
    pub fn new(id: WidgetId, layout_node: taffy::NodeId) -> Self {
        Self {
            id,
            layout_node,
            ..Default::default()
        }
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }
}

// Usage
let widget = MyWidget::new(id, node)
    .color(Color::RED)
    .size(Vec2::new(100.0, 50.0));
```

### 2. Default Trait for Sensible Defaults

```rust
impl Default for MyWidget {
    fn default() -> Self {
        Self {
            id: WidgetId(0),
            layout_node: taffy::NodeId::from(0),
            color: Color::WHITE,
            size: Vec2::new(100.0, 30.0),
        }
    }
}
```

### 3. Type-Safe IDs

```rust
#[derive(Copy, Clone)]
pub struct ToggleSwitchId(WidgetId);

impl ToggleSwitchId {
    pub fn new(name: &str) -> Self {
        Self(WidgetId::from_name(name))
    }
}
```

### 4. Capability Forwarding

If your widget wraps another widget:

```rust
impl TextWidget for WrapperWidget {
    fn text(&self) -> &str {
        self.inner.text()  // Forward to inner widget
    }

    fn set_text(&mut self, text: &str) {
        self.inner.set_text(text);
    }

    // ... other methods
}
```

## Testing Custom Widgets

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_switch_starts_off() {
        let enabled = Arc::new(RwLock::new(false));
        let toggle = ToggleSwitch::new(
            WidgetId::new("test"),
            taffy::NodeId::from(0),
            enabled.clone(),
        );

        assert!(!toggle.is_enabled());
    }

    #[test]
    fn toggle_switch_toggles_state() {
        let enabled = Arc::new(RwLock::new(false));
        let mut toggle = ToggleSwitch::new(
            WidgetId::new("test"),
            taffy::NodeId::from(0),
            enabled.clone(),
        );

        toggle.toggle();
        assert!(toggle.is_enabled());

        toggle.toggle();
        assert!(!toggle.is_enabled());
    }

    #[test]
    fn toggle_switch_calls_callback() {
        let enabled = Arc::new(RwLock::new(false));
        let called = Arc::new(RwLock::new(false));
        let called_clone = called.clone();

        let mut toggle = ToggleSwitch::new(
            WidgetId::new("test"),
            taffy::NodeId::from(0),
            enabled,
        )
        .on_toggle(move |_| {
            *called_clone.write().unwrap() = true;
        });

        toggle.toggle();
        assert!(*called.read().unwrap());
    }
}
```

## Next Steps

You've learned how to create custom widgets! Continue with:

1. **[Layout Deep Dive](layout-deep-dive.md)** - Master Flexbox and Grid layouts
2. **[Styling and Theming](styling-and-theming.md)** - Create cohesive visual styles
3. **[Event Handling](event-handling.md)** - Advanced interaction patterns

## Summary

**Key takeaways**:
- **Widget trait**: Base trait with `id()` and `layout_node()`
- **Capability traits**: TextWidget, ColorWidget, SizedWidget, ClickableWidget, ParentWidget
- **State management**: Internal, shared (`Arc<RwLock<T>>`), callbacks, or message passing
- **Builder pattern**: Fluent API for widget construction
- **Composition**: Build complex widgets from simple ones

You now have the tools to create any custom widget you need in Astrelis!
