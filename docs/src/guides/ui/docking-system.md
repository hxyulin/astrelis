# Docking System

The Astrelis UI docking system provides flexible panel layouts with drag-and-drop functionality, splitter-based resizing, and tab-based window management. It's modular and extensible through the plugin system.

## Overview

The docking system allows you to create complex, resizable panel layouts similar to modern IDEs like VS Code, Blender, or Unity Editor. Panels can be:

- **Split** horizontally or vertically with draggable splitters
- **Tabbed** to stack multiple panels in the same area
- **Dragged** to rearrange the layout at runtime
- **Resized** via splitter handles
- **Nested** to create arbitrary hierarchies

## Basic Usage

### Enabling the Docking Feature

The docking system requires the `docking` feature flag:

```toml
[dependencies]
astrelis-ui = { version = "0.1", features = ["docking"] }
```

### Creating a Simple Docked Layout

```rust
use astrelis_ui::{UiSystem, Color};
use astrelis_ui::widgets::docking::{DockNode, DockSplit, DockNodeId};

// Initialize UI with docking support
let mut ui = UiSystem::new(graphics.clone());

// Build a docked layout
ui.build(|root| {
    root.dock_root(|dock_root| {
        // Split horizontally: left panel (30%) and right panel (70%)
        dock_root.split_horizontal(0.3, |left, right| {
            // Left panel with a tab group
            left.tab_group(|tabs| {
                tabs.tab("Files", |panel| {
                    panel.text("File browser goes here").build();
                });
                tabs.tab("Search", |panel| {
                    panel.text("Search results").build();
                });
            });

            // Right panel split vertically
            right.split_vertical(0.6, |top, bottom| {
                top.tab_group(|tabs| {
                    tabs.tab("Editor", |panel| {
                        panel.text("Code editor").build();
                    });
                });

                bottom.tab_group(|tabs| {
                    tabs.tab("Console", |panel| {
                        panel.text("Console output").build();
                    });
                    tabs.tab("Terminal", |panel| {
                        panel.text("Terminal").build();
                    });
                });
            });
        });
    });
});
```

## Architecture

### DockNode Tree Structure

The docking system is built on a tree of `DockNode` instances:

```rust
pub enum DockNode {
    /// Leaf node containing a tab group
    Leaf {
        tabs: Vec<DockTab>,
        active_tab: usize,
    },
    /// Split node dividing space horizontally or vertically
    Split {
        split_type: DockSplit,
        ratio: f32,  // 0.0 to 1.0
        left: Box<DockNode>,
        right: Box<DockNode>,
    },
}
```

- **Leaf nodes** contain tab groups with content panels
- **Split nodes** divide space between two child nodes
- The tree is dynamically modified during drag-and-drop operations

### DockContext

The `DockContext` manages the docking state:

```rust
pub struct DockContext {
    root: DockNode,
    drag_state: Option<DragState>,
    splitter_state: SplitterState,
}
```

- **root** - The root of the dock tree
- **drag_state** - Tracks ongoing drag operations
- **splitter_state** - Manages splitter interactions

### Drag Operations

Dragging tabs triggers drop zone visualization:

1. User begins dragging a tab
2. Drop zones appear over valid targets
3. Preview overlay shows where the tab will be inserted
4. On drop, the dock tree is reorganized

```rust
pub enum DropZone {
    TabBar(DockNodeId),      // Drop into existing tab group
    SplitLeft(DockNodeId),   // Create left split
    SplitRight(DockNodeId),  // Create right split
    SplitTop(DockNodeId),    // Create top split
    SplitBottom(DockNodeId), // Create bottom split
}
```

## Customization

### Splitter Appearance

Customize splitter size, color, and hover effects:

```rust
use astrelis_ui::widgets::docking::SplitterStyle;

let splitter_style = SplitterStyle {
    width: 4.0,
    color: Color::rgb(0.2, 0.2, 0.2),
    hover_color: Color::rgb(0.4, 0.4, 0.4),
    active_color: Color::rgb(0.6, 0.6, 0.6),
};

dock_context.set_splitter_style(splitter_style);
```

### Tab Styles

Control tab appearance and behavior:

```rust
use astrelis_ui::widgets::docking::TabStyle;

let tab_style = TabStyle {
    height: 30.0,
    padding: 8.0,
    background_color: Color::rgb(0.15, 0.15, 0.15),
    active_color: Color::rgb(0.25, 0.25, 0.25),
    hover_color: Color::rgb(0.2, 0.2, 0.2),
    text_color: Color::WHITE,
};

dock_context.set_tab_style(tab_style);
```

### Drop Zone Visuals

Customize the drop zone preview overlay:

```rust
use astrelis_ui::widgets::docking::DropZoneStyle;

let drop_zone_style = DropZoneStyle {
    overlay_color: Color::rgba(0.3, 0.5, 1.0, 0.3),
    border_color: Color::rgba(0.3, 0.5, 1.0, 0.8),
    border_width: 2.0,
};

dock_context.set_drop_zone_style(drop_zone_style);
```

## Event Handling

The docking system emits events during layout changes:

```rust
use astrelis_ui::widgets::docking::DockEvent;

// Poll for docking events
while let Some(event) = dock_context.poll_event() {
    match event {
        DockEvent::TabClosed { node_id, tab_index } => {
            println!("Tab {} closed in node {:?}", tab_index, node_id);
        }
        DockEvent::TabMoved { from_node, to_node, tab_index } => {
            println!("Tab moved from {:?} to {:?}", from_node, to_node);
        }
        DockEvent::SplitCreated { parent, new_child } => {
            println!("Split created: {:?} -> {:?}", parent, new_child);
        }
        DockEvent::SplitResized { node_id, new_ratio } => {
            println!("Split {:?} resized to {:.2}", node_id, new_ratio);
        }
    }
}
```

## Persistence

Save and restore dock layouts:

```rust
use astrelis_ui::widgets::docking::DockLayout;

// Serialize layout
let layout = dock_context.save_layout();
let json = serde_json::to_string(&layout)?;
std::fs::write("layout.json", json)?;

// Deserialize and restore layout
let json = std::fs::read_to_string("layout.json")?;
let layout: DockLayout = serde_json::from_str(&json)?;
dock_context.load_layout(layout);
```

## Performance Considerations

- **Dirty Flags**: Only modified dock nodes are re-rendered
- **Layout Caching**: Split ratios and tab positions are cached
- **GPU Rendering**: Tabs and splitters use instanced rendering
- **Event Batching**: Drag operations are debounced to avoid frame drops

## Example: Full IDE Layout

See the `docking_demo.rs` example for a complete implementation:

```bash
cargo run -p astrelis-ui --example docking_demo
```

This demonstrates:
- Multi-level splits (horizontal and vertical)
- Tab groups with multiple tabs
- Drag-and-drop tab reorganization
- Splitter resizing
- Tab close buttons
- Persistence to JSON

## Plugin Integration

The docking system is implemented as a UI plugin:

```rust
use astrelis_ui::plugin::PluginRegistry;
use astrelis_ui::widgets::docking::DockingPlugin;

let mut registry = PluginRegistry::new();
registry.register(DockingPlugin::new());

ui.set_plugin_registry(registry);
```

This allows custom docking behaviors to be added without modifying core UI code.

## Next Steps

- Explore [Event Handling](./event-handling.md) for custom drag interactions
- See [Performance Optimization](./performance-optimization.md) for layout tuning
- Check [Custom Widgets](./custom-widgets.md) for extending the docking system
