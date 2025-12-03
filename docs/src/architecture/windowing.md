# Windowing & Events

The `astrelis-winit` crate provides window management and event handling built on the `winit` library. It abstracts platform-specific windowing and provides a clean event system for applications.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   Application                           │
│               (Your game code)                          │
└─────────────────────────────────────────────────────────┘
                          │
┌─────────────────────────────────────────────────────────┐
│                  astrelis-winit                         │
│  ┌──────────────┐  ┌────────────────┐  ┌────────────┐  │
│  │     App      │  │    Window      │  │   Event    │  │
│  │   (trait)    │  │  (descriptor)  │  │  (queue)   │  │
│  └──────────────┘  └────────────────┘  └────────────┘  │
│         │                   │                  │         │
│  ┌──────────────┐  ┌────────────────┐                   │
│  │   AppCtx     │  │  EventBatch    │                   │
│  │  (context)   │  │  (consumer)    │                   │
│  └──────────────┘  └────────────────┘                   │
└─────────────────────────────────────────────────────────┘
                          │
┌─────────────────────────────────────────────────────────┐
│                      winit                              │
│              (platform windowing)                       │
└─────────────────────────────────────────────────────────┘
```

## Core Components

### App Trait

Main application interface:

```rust
use astrelis_winit::app::{App, AppCtx};
use winit::window::WindowId;

pub struct MyApp {
    // Your application state
}

impl App for MyApp {
    fn update(&mut self, ctx: &mut AppCtx) {
        // Called once per frame (global logic)
        // No window-specific input here
    }
    
    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        // Called per window that needs rendering
        // Window-specific input via events
        
        events.dispatch(|event| {
            match event {
                Event::KeyDown { key, .. } => {
                    println!("Key pressed: {:?}", key);
                    HandleStatus::consumed()
                }
                _ => HandleStatus::ignored()
            }
        });
    }
}
```

### AppCtx

Context for application operations:

```rust
pub struct AppCtx<'event_loop> {
    event_loop: &'event_loop ActiveEventLoop,
}

impl AppCtx<'_> {
    pub fn create_window(&mut self, descriptor: WindowDescriptor) -> Result<Window, OsError>;
    pub fn exit(&self);
}
```

### Window

Window creation and management:

```rust
use astrelis_winit::window::{Window, WindowDescriptor};

let descriptor = WindowDescriptor {
    title: "My Game".to_string(),
    width: 1280,
    height: 720,
    resizable: true,
    maximized: false,
    ..Default::default()
};

let window = ctx.create_window(descriptor)?;
```

### Window Descriptor

Configuration for window creation:

```rust
pub struct WindowDescriptor {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
    pub maximized: bool,
    pub decorations: bool,
    pub transparent: bool,
    pub position: Option<(i32, i32)>,
}
```

## Event System

### Event Types

```rust
pub enum Event {
    // Window events
    Resized { width: u32, height: u32 },
    CloseRequested,
    Focused(bool),
    
    // Mouse events
    MouseMoved { x: f64, y: f64 },
    MouseButton { button: MouseButton, pressed: bool },
    MouseWheel { delta: f32 },
    MouseEntered,
    MouseLeft,
    
    // Keyboard events
    KeyDown { key: KeyCode, modifiers: Modifiers },
    KeyUp { key: KeyCode, modifiers: Modifiers },
    ReceivedCharacter(char),
    
    // Touch events (mobile)
    Touch { phase: TouchPhase, id: u64, x: f64, y: f64 },
}
```

### Event Queue

Per-window event collection:

```rust
pub struct EventQueue {
    events: Vec<Event>,
}

impl EventQueue {
    pub fn push(&mut self, event: Event);
    pub fn drain(&mut self) -> EventBatch;
    pub fn clear(&mut self);
}
```

### Event Batch

Consumed by application per render call:

```rust
pub struct EventBatch {
    events: Vec<Event>,
}

impl EventBatch {
    pub fn dispatch<F>(&mut self, mut handler: F)
    where
        F: FnMut(&Event) -> HandleStatus,
    {
        self.events.retain(|event| {
            match handler(event) {
                HandleStatus::Consumed => false,  // Remove
                HandleStatus::Ignored => true,    // Keep
            }
        });
    }
    
    pub fn iter(&self) -> impl Iterator<Item = &Event>;
    pub fn is_empty(&self) -> bool;
}
```

### Handle Status

Event consumption tracking:

```rust
pub enum HandleStatus {
    Consumed,  // Event handled, stop propagation
    Ignored,   // Event not handled, continue
}

impl HandleStatus {
    pub fn consumed() -> Self { Self::Consumed }
    pub fn ignored() -> Self { Self::Ignored }
}
```

## Application Lifecycle

### Initialization

```rust
use astrelis_winit::app::run_app;

fn main() {
    run_app(|ctx| {
        // Create window
        let window = ctx.create_window(WindowDescriptor::default())?;
        
        // Create app
        Box::new(MyApp::new(window))
    });
}
```

### Frame Loop

```
1. OS events arrive
   ↓
2. Events queued per window
   ↓
3. AboutToWait event
   ↓
4. App::update() called once
   ↓
5. Windows request redraw
   ↓
6. App::render() called per window
   ↓
7. EventBatch consumed
   ↓
8. Default event handling
   ↓
9. Repeat from step 1
```

### Shutdown

```rust
impl App for MyApp {
    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        events.dispatch(|event| {
            match event {
                Event::CloseRequested => {
                    ctx.exit();
                    HandleStatus::consumed()
                }
                _ => HandleStatus::ignored()
            }
        });
    }
}
```

## Multi-Window Support

### Creating Multiple Windows

```rust
impl App for MyApp {
    fn update(&mut self, ctx: &mut AppCtx) {
        if self.should_open_second_window {
            let window = ctx.create_window(WindowDescriptor {
                title: "Second Window".to_string(),
                ..Default::default()
            }).unwrap();
            self.windows.insert(window.id(), window);
        }
    }
    
    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        // Handle events per window
        match window_id {
            id if id == self.main_window_id => {
                // Main window rendering
            }
            id if self.windows.contains_key(&id) => {
                // Other window rendering
            }
            _ => {}
        }
    }
}
```

### Per-Window State

Each window has:
- Own event queue
- Own render context
- Own input state
- Separate `render()` calls

## Input Handling

### Keyboard

```rust
events.dispatch(|event| {
    match event {
        Event::KeyDown { key, modifiers } => {
            if modifiers.ctrl && key == KeyCode::KeyS {
                save_file();
                return HandleStatus::consumed();
            }
            HandleStatus::ignored()
        }
        Event::ReceivedCharacter(ch) => {
            text_input.push(*ch);
            HandleStatus::consumed()
        }
        _ => HandleStatus::ignored()
    }
});
```

### Mouse

```rust
events.dispatch(|event| {
    match event {
        Event::MouseMoved { x, y } => {
            self.mouse_pos = Vec2::new(*x as f32, *y as f32);
            HandleStatus::ignored()
        }
        Event::MouseButton { button, pressed } => {
            if *button == MouseButton::Left && *pressed {
                self.handle_click(self.mouse_pos);
                return HandleStatus::consumed();
            }
            HandleStatus::ignored()
        }
        Event::MouseWheel { delta } => {
            self.zoom *= 1.0 + delta * 0.1;
            HandleStatus::consumed()
        }
        _ => HandleStatus::ignored()
    }
});
```

### Touch (Mobile)

```rust
events.dispatch(|event| {
    match event {
        Event::Touch { phase, id, x, y } => {
            match phase {
                TouchPhase::Started => {
                    self.touches.insert(*id, Vec2::new(*x as f32, *y as f32));
                }
                TouchPhase::Moved => {
                    if let Some(pos) = self.touches.get_mut(id) {
                        *pos = Vec2::new(*x as f32, *y as f32);
                    }
                }
                TouchPhase::Ended | TouchPhase::Cancelled => {
                    self.touches.remove(id);
                }
            }
            HandleStatus::consumed()
        }
        _ => HandleStatus::ignored()
    }
});
```

## Window Operations

### Resizing

```rust
events.dispatch(|event| {
    match event {
        Event::Resized { width, height } => {
            // Update viewport
            self.viewport_size = Vec2::new(*width as f32, *height as f32);
            
            // Resize rendering resources
            self.window_context.resize(*width, *height);
            self.ui.set_viewport_size(self.viewport_size);
            
            HandleStatus::consumed()
        }
        _ => HandleStatus::ignored()
    }
});
```

### Focus

```rust
events.dispatch(|event| {
    match event {
        Event::Focused(focused) => {
            self.is_focused = *focused;
            
            if !focused {
                // Pause game, release input
                self.pause();
            } else {
                // Resume game
                self.resume();
            }
            
            HandleStatus::consumed()
        }
        _ => HandleStatus::ignored()
    }
});
```

## Platform Differences

### Windows
- Native Win32 windowing
- Hardware acceleration via DX12/Vulkan
- Full keyboard/mouse support
- Multi-monitor support

### macOS
- Native Cocoa windowing
- Hardware acceleration via Metal
- Retina display support
- Trackpad gestures

### Linux
- X11 or Wayland
- Hardware acceleration via Vulkan
- Window manager integration
- Variable DPI support

### Web (WASM)
- Canvas-based windowing
- WebGPU for rendering
- Browser event handling
- Fullscreen API support

### Mobile (iOS/Android)
- Touch-first input
- Lifecycle management (suspend/resume)
- Orientation changes
- Native backing (UIKit/Android SDK)

## Best Practices

### 1. Separate Update and Render

```rust
impl App for MyApp {
    fn update(&mut self, ctx: &mut AppCtx) {
        // Global game logic
        self.physics.step(FIXED_DT);
        self.ai.update();
    }
    
    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        // Window-specific rendering and input
        self.handle_input(events);
        self.render_frame(window_id);
    }
}
```

### 2. Handle All CloseRequested Events

```rust
events.dispatch(|event| {
    match event {
        Event::CloseRequested => {
            // Save state, cleanup
            self.save_game();
            ctx.exit();
            HandleStatus::consumed()
        }
        _ => HandleStatus::ignored()
    }
});
```

### 3. Track Event Consumption

```rust
// Let UI handle events first
let handled = ui.handle_events(events);

// Then handle game input
if !handled {
    events.dispatch(|event| {
        // Game input handling
    });
}
```

### 4. Debounce Resize Events

```rust
match event {
    Event::Resized { width, height } => {
        self.pending_resize = Some((*width, *height));
        // Apply resize after stabilization
        HandleStatus::consumed()
    }
    _ => HandleStatus::ignored()
}
```

## Integration with Other Systems

### Rendering

```rust
fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
    let frame = self.window_context.current_frame().unwrap();
    let view = frame.texture.create_view(&Default::default());
    
    let mut encoder = self.graphics_context.device.create_command_encoder(&Default::default());
    
    {
        let mut pass = encoder.begin_render_pass(/* ... */);
        self.ui.render(&mut pass, self.viewport_size);
    }
    
    self.graphics_context.queue.submit(Some(encoder.finish()));
    frame.present();
}
```

### UI System

```rust
ui.handle_events(&mut events);
```

The UI system consumes events internally and marks remaining as ignored.

## Future Enhancements

1. **Input abstraction** - Unified input system across mouse/touch/gamepad
2. **Window decorations** - Custom title bars, borders
3. **Drag and drop** - File drop support
4. **Clipboard** - Copy/paste integration
5. **IME** - Input method editor for CJK languages
6. **Window icons** - Custom window/taskbar icons
7. **Cursor management** - Custom cursors, hiding, locking
8. **Fullscreen** - Exclusive and borderless modes
9. **Window positioning** - Multi-monitor positioning
10. **High DPI** - Automatic DPI scaling