# Multi-Window Applications

This guide explains how to create applications with multiple windows in Astrelis. Learn to manage window lifecycles, route events, and share rendering resources.

## Overview

**Multi-window applications** enable:

- Multiple editor windows (tool palettes, inspector panels)
- Multi-monitor support
- Detachable UI panels
- Secondary displays (preview windows, output monitors)
- Independent render targets per window

**Key Concepts:**
- WindowManager for window lifecycle
- WindowId for event routing
- Shared GraphicsContext across windows
- Per-window RenderableWindow instances
- Independent UI systems per window

**Comparison to Unity:** Similar to Unity's Display class but with explicit window management.

## WindowManager Basics

### Creating Multiple Windows

```rust
use astrelis_winit::{run_app, App, AppCtx, WindowDescriptor};
use std::collections::HashMap;

struct MultiWindowApp {
    windows: HashMap<WindowId, WindowState>,
}

struct WindowState {
    renderable: RenderableWindow,
    ui: UiSystem,
    title: String,
}

impl App for MultiWindowApp {
    fn on_start(&mut self, ctx: &mut AppCtx) {
        // Create main window
        let main_window = ctx.create_window(WindowDescriptor {
            title: "Main Window".to_string(),
            width: 1280,
            height: 720,
            ..Default::default()
        }).unwrap();

        // Create secondary window
        let tool_window = ctx.create_window(WindowDescriptor {
            title: "Tools".to_string(),
            width: 400,
            height: 600,
            ..Default::default()
        }).unwrap();

        // Initialize both windows
        self.init_window(main_window, "Main Window");
        self.init_window(tool_window, "Tools");
    }

    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        // Render correct window
        if let Some(state) = self.windows.get_mut(&window_id) {
            state.render();
        }
    }
}
```

### Shared GraphicsContext Pattern

**Best practice:** Share `Arc<GraphicsContext>` across all windows:

```rust
use std::sync::Arc;
use astrelis_render::GraphicsContext;

struct MultiWindowApp {
    graphics: Arc<GraphicsContext>,
    windows: HashMap<WindowId, WindowState>,
}

impl App for MultiWindowApp {
    fn on_start(&mut self, ctx: &mut AppCtx) {
        // Create shared graphics context once
        self.graphics = GraphicsContext::new_owned_sync();

        // All windows share this context
        let main_window = ctx.create_window(descriptor).unwrap();
        let main_renderable = RenderableWindow::new(
            main_window,
            self.graphics.clone(), // Cheap Arc clone
        );

        let tool_window = ctx.create_window(descriptor).unwrap();
        let tool_renderable = RenderableWindow::new(
            tool_window,
            self.graphics.clone(), // Same context
        );
    }
}
```

**Benefits:**
- Single GPU device and queue
- Shared texture/buffer resources
- Efficient resource management
- Proper cleanup when last Arc drops

### Window Lifecycle

**Creating windows:**
```rust
let window_id = ctx.create_window(WindowDescriptor {
    title: "New Window".to_string(),
    width: 800,
    height: 600,
    resizable: true,
    decorations: true,
    ..Default::default()
})?;
```

**Closing windows:**
```rust
fn on_window_close(&mut self, ctx: &mut AppCtx, window_id: WindowId) {
    // Clean up window state
    if let Some(state) = self.windows.remove(&window_id) {
        info!("Window closed: {}", state.title);
        // RenderableWindow and UiSystem automatically drop
    }

    // Close application if main window closed
    if window_id == self.main_window_id {
        ctx.request_exit();
    }
}
```

## Event Routing

### Per-Window Event Handling

Events are dispatched to the correct window:

```rust
impl App for MultiWindowApp {
    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        // Get window-specific state
        let Some(state) = self.windows.get_mut(&window_id) else {
            return;
        };

        // Handle events for this window only
        events.dispatch(|event| {
            match event {
                Event::KeyPressed { key, .. } => {
                    info!("Key pressed in window: {}", state.title);
                    HandleStatus::consumed()
                }
                Event::MouseButtonPressed { button, .. } => {
                    info!("Mouse click in window: {}", state.title);
                    HandleStatus::consumed()
                }
                _ => HandleStatus::ignored()
            }
        });

        // Render this window
        state.render();
    }
}
```

### Cross-Window Communication

**Shared state pattern:**
```rust
use std::sync::{Arc, RwLock};

pub struct SharedState {
    selected_tool: Tool,
    selected_color: Color,
    document: Document,
}

struct MultiWindowApp {
    shared_state: Arc<RwLock<SharedState>>,
    windows: HashMap<WindowId, WindowState>,
}

impl WindowState {
    fn on_tool_selected(&mut self, tool: Tool, shared: &Arc<RwLock<SharedState>>) {
        // Update shared state
        shared.write().unwrap().selected_tool = tool;

        // Other windows will see the change
    }

    fn render(&mut self, shared: &Arc<RwLock<SharedState>>) {
        let state = shared.read().unwrap();

        // Use shared state for rendering
        self.ui.update_text("tool_label", &format!("Tool: {:?}", state.selected_tool));
    }
}
```

**Message passing pattern:**
```rust
use crossbeam::channel::{unbounded, Sender, Receiver};

pub enum WindowMessage {
    ToolChanged(Tool),
    ColorChanged(Color),
    DocumentModified,
}

struct MultiWindowApp {
    tx: Sender<WindowMessage>,
    rx: Receiver<WindowMessage>,
}

impl MultiWindowApp {
    fn process_messages(&mut self) {
        for msg in self.rx.try_iter() {
            match msg {
                WindowMessage::ToolChanged(tool) => {
                    // Update all windows
                    for state in self.windows.values_mut() {
                        state.set_tool(tool);
                    }
                }
                // Handle other messages...
                _ => {}
            }
        }
    }
}

impl App for MultiWindowApp {
    fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
        // Process cross-window messages
        self.process_messages();
    }
}
```

## Multi-Monitor Support

### Positioning Windows on Specific Monitors

```rust
use winit::window::Window;

impl App for MultiWindowApp {
    fn on_start(&mut self, ctx: &mut AppCtx) {
        // Get available monitors
        let monitors = ctx.available_monitors();

        for (i, monitor) in monitors.enumerate() {
            let position = monitor.position();
            let size = monitor.size();

            info!("Monitor {}: {}x{} at ({}, {})",
                i, size.width, size.height, position.x, position.y);

            // Create window on specific monitor
            let window_id = ctx.create_window(WindowDescriptor {
                title: format!("Monitor {}", i),
                width: size.width / 2,
                height: size.height / 2,
                position: Some((position.x, position.y)),
                ..Default::default()
            })?;
        }
    }
}
```

### Fullscreen on Specific Monitor

```rust
let window_id = ctx.create_window(WindowDescriptor {
    title: "Fullscreen Window".to_string(),
    fullscreen: Some(Fullscreen::Borderless(Some(monitor.clone()))),
    ..Default::default()
})?;
```

### DPI Scaling Per Monitor

```rust
impl App for MultiWindowApp {
    fn on_window_scale_factor_changed(&mut self, window_id: WindowId, scale_factor: f64) {
        if let Some(state) = self.windows.get_mut(&window_id) {
            info!("Window {} scale factor: {}", state.title, scale_factor);

            // Update UI scale
            state.ui.set_scale_factor(scale_factor as f32);
        }
    }
}
```

## Independent UI Systems

### Per-Window UI State

Each window maintains its own UI tree:

```rust
struct WindowState {
    renderable: RenderableWindow,
    ui: UiSystem, // Independent UI tree
    state: WindowLocalState,
}

struct WindowLocalState {
    scroll_position: f32,
    selected_item: Option<usize>,
    input_text: String,
}

impl WindowState {
    fn build_ui(&mut self) {
        self.ui.build(|root| {
            root.column()
                .child(|c| {
                    c.text(&self.state.input_text)
                        .id("input_display")
                        .build()
                })
                .child(|c| {
                    c.button("Click Me")
                        .on_click(|| {
                            println!("Button clicked in this window");
                        })
                        .build()
                })
                .build();
        });
    }
}
```

### Shared UI Components

Reusable UI builders:

```rust
pub fn build_tool_palette(ui: &mut UiSystem, tools: &[Tool]) {
    ui.build(|root| {
        root.column()
            .child(|c| c.text("Tools").build())
            .children(tools.iter().map(|tool| {
                |c: &mut WidgetBuilder| {
                    c.button(&format!("{:?}", tool))
                        .id(&format!("tool_{:?}", tool))
                        .build()
                }
            }))
            .build();
    });
}

// Use in multiple windows
impl WindowState {
    fn build_main_window(&mut self) {
        build_tool_palette(&mut self.ui, &TOOLS);
    }

    fn build_tool_window(&mut self) {
        build_tool_palette(&mut self.ui, &TOOLS);
    }
}
```

## Complete Multi-Window Example

Full application with main window and tool palette:

```rust
use astrelis::*;
use astrelis_winit::*;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Clone, Copy, Debug)]
enum Tool {
    Brush,
    Eraser,
    Select,
}

struct SharedAppState {
    selected_tool: Tool,
    selected_color: Color,
}

struct WindowState {
    renderable: RenderableWindow,
    ui: UiSystem,
    window_type: WindowType,
}

enum WindowType {
    Main,
    Tools,
}

struct MultiWindowDemo {
    graphics: Arc<GraphicsContext>,
    shared_state: Arc<RwLock<SharedAppState>>,
    windows: HashMap<WindowId, WindowState>,
    main_window_id: Option<WindowId>,
}

impl App for MultiWindowDemo {
    fn on_start(&mut self, ctx: &mut AppCtx) {
        // Create shared graphics context
        self.graphics = GraphicsContext::new_owned_sync();

        // Initialize shared state
        self.shared_state = Arc::new(RwLock::new(SharedAppState {
            selected_tool: Tool::Brush,
            selected_color: Color::BLACK,
        }));

        // Create main window
        let main_window = ctx.create_window(WindowDescriptor {
            title: "Canvas".to_string(),
            width: 1280,
            height: 720,
            ..Default::default()
        }).unwrap();

        let main_renderable = RenderableWindow::new(
            main_window,
            self.graphics.clone(),
        );

        let mut main_ui = UiSystem::new(self.graphics.clone());
        self.build_main_ui(&mut main_ui);

        self.windows.insert(main_window, WindowState {
            renderable: main_renderable,
            ui: main_ui,
            window_type: WindowType::Main,
        });
        self.main_window_id = Some(main_window);

        // Create tool palette window
        let tool_window = ctx.create_window(WindowDescriptor {
            title: "Tools".to_string(),
            width: 300,
            height: 400,
            position: Some((1300, 100)),
            ..Default::default()
        }).unwrap();

        let tool_renderable = RenderableWindow::new(
            tool_window,
            self.graphics.clone(),
        );

        let mut tool_ui = UiSystem::new(self.graphics.clone());
        self.build_tool_ui(&mut tool_ui);

        self.windows.insert(tool_window, WindowState {
            renderable: tool_renderable,
            ui: tool_ui,
            window_type: WindowType::Tools,
        });
    }

    fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
        // Update shared state display in all windows
        let state = self.shared_state.read().unwrap();

        for window_state in self.windows.values_mut() {
            match window_state.window_type {
                WindowType::Main => {
                    window_state.ui.update_text(
                        "tool_status",
                        &format!("Tool: {:?}", state.selected_tool),
                    );
                }
                WindowType::Tools => {
                    // Tool palette updates...
                }
            }
        }
    }

    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        let Some(state) = self.windows.get_mut(&window_id) else {
            return;
        };

        // Handle window-specific events
        let shared_state = self.shared_state.clone();
        events.dispatch(|event| {
            match event {
                Event::KeyPressed { key, .. } => {
                    match window_state.window_type {
                        WindowType::Main => {
                            // Main window shortcuts
                            match key {
                                VirtualKeyCode::B => {
                                    shared_state.write().unwrap().selected_tool = Tool::Brush;
                                }
                                VirtualKeyCode::E => {
                                    shared_state.write().unwrap().selected_tool = Tool::Eraser;
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                    HandleStatus::consumed()
                }
                _ => HandleStatus::ignored()
            }
        });

        // Render window
        let mut frame = state.renderable.begin_drawing();

        let bg_color = match state.window_type {
            WindowType::Main => Color::from_rgb(50, 50, 50),
            WindowType::Tools => Color::from_rgb(40, 40, 40),
        };

        frame.clear_and_render(
            RenderTarget::Surface,
            bg_color,
            |pass| {
                state.ui.render(pass.wgpu_pass());
            },
        );

        frame.finish();
    }

    fn on_window_close(&mut self, ctx: &mut AppCtx, window_id: WindowId) {
        self.windows.remove(&window_id);

        // Exit if main window closed
        if Some(window_id) == self.main_window_id {
            ctx.request_exit();
        }
    }
}

impl MultiWindowDemo {
    fn build_main_ui(&self, ui: &mut UiSystem) {
        ui.build(|root| {
            root.column()
                .padding(Length::px(20))
                .child(|c| {
                    c.text("Canvas Window")
                        .font_size(24.0)
                        .build()
                })
                .child(|c| {
                    c.text("Tool: Brush")
                        .id("tool_status")
                        .build()
                })
                .build();
        });
    }

    fn build_tool_ui(&self, ui: &mut UiSystem) {
        let shared = self.shared_state.clone();

        ui.build(|root| {
            root.column()
                .padding(Length::px(10))
                .gap(Length::px(5))
                .child(|c| c.text("Tools").font_size(20.0).build())
                .child(|c| {
                    c.button("Brush")
                        .on_click(move || {
                            shared.write().unwrap().selected_tool = Tool::Brush;
                        })
                        .build()
                })
                .child(|c| {
                    c.button("Eraser")
                        .on_click(move || {
                            shared.write().unwrap().selected_tool = Tool::Eraser;
                        })
                        .build()
                })
                .child(|c| {
                    c.button("Select")
                        .on_click(move || {
                            shared.write().unwrap().selected_tool = Tool::Select;
                        })
                        .build()
                })
                .build();
        });
    }
}

fn main() {
    run_app(|ctx| {
        Box::new(MultiWindowDemo {
            graphics: Arc::new(GraphicsContext::new_owned_sync()),
            shared_state: Arc::new(RwLock::new(SharedAppState {
                selected_tool: Tool::Brush,
                selected_color: Color::BLACK,
            })),
            windows: HashMap::new(),
            main_window_id: None,
        })
    });
}
```

## Performance Considerations

### Resource Sharing

**Shared resources:**
- GraphicsContext (device, queue)
- Texture atlases
- Shader modules
- Bind group layouts

**Per-window resources:**
- Surface and swapchain
- RenderableWindow
- UI trees
- Frame buffers

### Frame Synchronization

**Each window renders independently:**
```rust
impl App for MultiWindowApp {
    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        // Windows render in sequence, not parallel
        // Each window waits for GPU completion before next
    }
}
```

**Optimization:** Minimize render time per window (<8ms target).

### Memory Usage

**Approximate per-window cost:**
- RenderableWindow: ~10MB (surface, command buffers)
- UiSystem: ~1-5MB (widget tree, draw commands)
- Total: ~15-20MB per window

**Budget guidance:**
- 1-2 windows: No concern
- 3-5 windows: Monitor memory usage
- 6+ windows: Consider lazy initialization

## Best Practices

### ✅ DO: Share GraphicsContext

```rust
// GOOD: Single shared context
let graphics = Arc::new(GraphicsContext::new_owned_sync());

for _ in 0..num_windows {
    RenderableWindow::new(window, graphics.clone());
}
```

### ✅ DO: Use Shared State with RwLock

```rust
// GOOD: Thread-safe shared state
let shared = Arc::new(RwLock::new(AppState::new()));

// Read from multiple windows
let state = shared.read().unwrap();

// Write from one window
let mut state = shared.write().unwrap();
state.tool = Tool::Brush;
```

### ✅ DO: Handle Window Close Gracefully

```rust
fn on_window_close(&mut self, ctx: &mut AppCtx, window_id: WindowId) {
    self.windows.remove(&window_id);

    if window_id == self.main_window_id {
        ctx.request_exit();
    }
}
```

### ❌ DON'T: Create Separate GraphicsContext Per Window

```rust
// BAD: Multiple contexts (resource waste)
for _ in 0..num_windows {
    let graphics = GraphicsContext::new_owned_sync(); // Don't do this!
    RenderableWindow::new(window, Arc::new(graphics));
}
```

### ❌ DON'T: Block Render Thread with Locks

```rust
// BAD: Holding write lock during render
let mut state = shared.write().unwrap();
expensive_render_operation(); // Other windows blocked!

// GOOD: Release lock quickly
{
    let mut state = shared.write().unwrap();
    state.value = new_value;
} // Lock released
expensive_render_operation(); // Other windows can proceed
```

## Comparison to Unity

| Unity | Astrelis | Notes |
|-------|----------|-------|
| `Display.displays` | `ctx.available_monitors()` | Monitor enumeration |
| `Screen.SetResolution()` | `WindowDescriptor` | Window configuration |
| Canvas per Display | UiSystem per window | Independent UI |
| N/A | Shared GraphicsContext | Astrelis-specific pattern |

## Troubleshooting

### Window Not Appearing

**Cause:** Window created off-screen or wrong monitor.

**Fix:**
```rust
let window = ctx.create_window(WindowDescriptor {
    position: Some((100, 100)), // Explicit position
    ..Default::default()
})?;
```

### Events Going to Wrong Window

**Cause:** Not checking `window_id` in event handler.

**Fix:**
```rust
fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
    let Some(state) = self.windows.get_mut(&window_id) else {
        return; // Ensure correct window
    };
    // Handle events for this window
}
```

### Surface Lost Errors

**Cause:** Window minimized or resized improperly.

**Fix:** See [Error Handling Guide](error-handling.md) for surface lost recovery.

## Next Steps

- **Practice:** Build a multi-window application
- **Advanced:** Implement window docking (user-defined layouts)
- **Integration:** Combine with [UI System](../ui/custom-widgets.md)
- **Examples:** `multi_window_demo`, `multi_monitor`

## See Also

- [Error Handling](error-handling.md) - Surface lost recovery
- [Input Handling](input-handling.md) - Per-window input
- API Reference: [`WindowDescriptor`](../../api/astrelis-winit/struct.WindowDescriptor.html)
- API Reference: [`WindowManager`](../../api/astrelis-winit/struct.WindowManager.html)
