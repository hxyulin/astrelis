# Layout Deep Dive

Astrelis UI uses [Taffy](https://github.com/DioxusLabs/taffy) for layout computation, which implements CSS Flexbox and Grid specifications. This guide provides a comprehensive reference for creating complex layouts.

## Prerequisites

- Completed [First UI](../getting-started/05-first-ui.md)
- Basic understanding of CSS Flexbox (helpful but not required)

## Layout Fundamentals

### The Taffy Layout Engine

Taffy is a pure Rust implementation of CSS layout algorithms:
- **Flexbox**: 1D layouts (rows or columns)
- **Grid**: 2D layouts (rows and columns simultaneously)
- **Block**: Traditional top-to-bottom flow

Astrelis primarily uses **Flexbox** for UI layouts.

### Layout Process

1. **Build UI tree**: Widgets are nested with parent-child relationships
2. **Set styles**: Each widget has layout properties (size, padding, flex, etc.)
3. **Compute layout**: Taffy calculates positions and sizes
4. **Render**: Widgets draw at computed positions

**Performance**: Layout is only recomputed for dirty subtrees (~10ms for complex layouts).

## Flexbox Basics

### Main Axis vs Cross Axis

Flexbox layouts have two axes:

**Column layout** (flex_direction: column):
```
Main Axis ↓  Cross Axis →
┌─────────────────────┐
│  ┌───────────────┐  │
│  │   Child 1     │  │
│  └───────────────┘  │
│  ┌───────────────┐  │
│  │   Child 2     │  │
│  └───────────────┘  │
│  ┌───────────────┐  │
│  │   Child 3     │  │
│  └───────────────┘  │
└─────────────────────┘
```

**Row layout** (flex_direction: row):
```
Main Axis →
┌───────────────────────────────────────┐
│ ┌──────┐ ┌──────┐ ┌──────┐           │
│ │Child1│ │Child2│ │Child3│           │ ↓ Cross Axis
│ └──────┘ └──────┘ └──────┘           │
└───────────────────────────────────────┘
```

## Essential Layout Properties

### Container Properties

#### flex_direction

Controls main axis direction:

```rust
// Vertical stack (default)
parent.column()  // flex_direction: column

// Horizontal row
parent.row()  // flex_direction: row
```

**Visual**:
```
Column:          Row:
┌─────┐         ┌─┬─┬─┐
│  A  │         │A│B│C│
├─────┤         └─┴─┴─┘
│  B  │
├─────┤
│  C  │
└─────┘
```

#### justify_content

Aligns children along **main axis**:

```rust
use taffy::JustifyContent;

parent.column()
    .justify_content(JustifyContent::Start)    // Top (column) or left (row)
    .justify_content(JustifyContent::End)      // Bottom (column) or right (row)
    .justify_content(JustifyContent::Center)   // Middle
    .justify_content(JustifyContent::SpaceBetween)  // Edges, equal spacing
    .justify_content(JustifyContent::SpaceAround)   // Equal space around items
    .justify_content(JustifyContent::SpaceEvenly)   // Equal space everywhere
```

**Visual (column with various justify_content)**:
```
Start:    End:      Center:   SpaceBetween: SpaceAround: SpaceEvenly:
┌───┐     ┌───┐     ┌───┐     ┌───┐         ┌───┐        ┌───┐
│ A │     │   │     │   │     │ A │         │   │        │   │
├───┤     │   │     │ A │     │   │         │ A │        │ A │
│ B │     │   │     ├───┤     │   │         │   │        │   │
├───┤     │   │     │ B │     │ B │         │   │        │   │
│ C │     │   │     ├───┤     │   │         │ B │        │ B │
└───┘     │   │     │ C │     │   │         │   │        │   │
          │ A │     │   │     │ C │         │   │        │   │
          ├───┤     └───┘     └───┘         │ C │        │ C │
          │ B │                              │   │        │   │
          ├───┤                              └───┘        └───┘
          │ C │
          └───┘
```

#### align_items

Aligns children along **cross axis**:

```rust
use taffy::AlignItems;

parent.row()
    .align_items(AlignItems::Start)     // Top (row) or left (column)
    .align_items(AlignItems::End)       // Bottom (row) or right (column)
    .align_items(AlignItems::Center)    // Middle
    .align_items(AlignItems::Stretch)   // Fill container height/width
```

**Visual (row with various align_items)**:
```
Start:           Center:          End:             Stretch:
┌──┬──┬──┐       ┌──┬──┬──┐       ┌──┬──┬──┐       ┌──┬──┬──┐
│A │B │C │       │  │  │  │       │  │  │  │       │  │  │  │
└──┴──┴──┘       │A │B │C │       │  │  │  │       │A │B │C │
                 └──┴──┴──┘       │A │B │C │       │  │  │  │
                                  └──┴──┴──┘       └──┴──┴──┘
```

#### gap

Spacing between children:

```rust
parent.column()
    .gap(10.0)  // 10 pixels between children
```

**Visual**:
```
gap(0):      gap(10):     gap(20):
┌────┐       ┌────┐       ┌────┐
│ A  │       │ A  │       │ A  │
├────┤       └────┘       └────┘
│ B  │          ↓
├────┤       ┌────┐          ↓
│ C  │       │ B  │       ┌────┐
└────┘       └────┘       │ B  │
                ↓         └────┘
             ┌────┐
             │ C  │          ↓
             └────┘       ┌────┐
                          │ C  │
                          └────┘
```

### Child Properties

#### flex_grow

How much a child grows to fill available space:

```rust
child.flex_grow(1.0)  // Take up remaining space proportionally
```

**Visual (3 children with different flex_grow in a row)**:
```
All flex_grow(0):        flex_grow(1,0,0):      flex_grow(1,2,1):
┌──┬──┬──┬──────────┐   ┌────────┬──┬──┐      ┌───┬──────┬───┐
│A │B │C │          │   │   A    │B │C │      │ A │  B   │ C │
└──┴──┴──┴──────────┘   └────────┴──┴──┘      └───┴──────┴───┘
                        A gets all space      B gets twice A&C
```

#### flex_shrink

How much a child shrinks when container is too small:

```rust
child.flex_shrink(1.0)  // Default: shrink proportionally
child.flex_shrink(0.0)  // Don't shrink (maintain min size)
```

#### flex_basis

Initial size before flex_grow/shrink:

```rust
use taffy::Dimension;

child.flex_basis(Dimension::Length(100.0))  // 100px base size
child.flex_basis(Dimension::Auto)           // Use content size
child.flex_basis(Dimension::Percent(0.5))   // 50% of container
```

#### align_self

Override parent's align_items for this child:

```rust
use taffy::AlignSelf;

child.align_self(AlignSelf::Center)  // Center this child only
```

### Sizing Properties

#### width / height

Explicit dimensions:

```rust
use taffy::Dimension;

widget
    .width(Dimension::Length(200.0))      // Fixed 200px
    .width(Dimension::Percent(0.5))       // 50% of parent
    .width(Dimension::Auto)               // Size to content
```

#### min_width / min_height

Minimum dimensions:

```rust
widget
    .min_width(Dimension::Length(100.0))  // At least 100px wide
    .min_height(Dimension::Length(50.0))  // At least 50px tall
```

#### max_width / max_height

Maximum dimensions:

```rust
widget
    .max_width(Dimension::Length(500.0))  // No more than 500px wide
    .max_height(Dimension::Length(300.0)) // No more than 300px tall
```

### Spacing Properties

#### padding

Inner spacing (inside border):

```rust
use taffy::Rect;

// All sides
widget.padding(10.0)

// Individual sides
widget.padding_left(10.0)
      .padding_right(10.0)
      .padding_top(5.0)
      .padding_bottom(5.0)

// Rect for all sides at once
widget.padding(Rect {
    left: taffy::LengthPercentage::Length(10.0),
    right: taffy::LengthPercentage::Length(10.0),
    top: taffy::LengthPercentage::Length(5.0),
    bottom: taffy::LengthPercentage::Length(5.0),
})
```

**Visual**:
```
Without padding:        With padding(10):
┌──────────────┐       ┌──────────────┐
│┌────────────┐│       │              │
││  Content   ││       │  ┌────────┐  │
│└────────────┘│       │  │Content │  │
└──────────────┘       │  └────────┘  │
                       └──────────────┘
```

#### margin

Outer spacing (outside border):

```rust
// All sides
widget.margin(10.0)

// Individual sides
widget.margin_left(10.0)
      .margin_right(10.0)
      .margin_top(5.0)
      .margin_bottom(5.0)
```

**Visual**:
```
margin_top(10):          margin_left(20):
     ↓                        ↓
┌────────────┐          ────┬──────────┐
│            │              │          │
│            │              │          │
└────────────┘              └──────────┘
```

## Common Layout Patterns

### Pattern 1: Header, Content, Footer

```rust
ui.build(|root| {
    root.column()
        .width(Dimension::Percent(1.0))   // Full width
        .height(Dimension::Percent(1.0))  // Full height
        .child(|parent| {
            // Header (fixed height)
            parent.container()
                .height(Dimension::Length(60.0))
                .background_color(Color::from_rgb(0.2, 0.2, 0.3))
                .child(|parent| {
                    parent.text("Header").build()
                })
                .build()
        })
        .child(|parent| {
            // Content (flex grows to fill space)
            parent.container()
                .flex_grow(1.0)
                .background_color(Color::from_rgb(0.1, 0.1, 0.15))
                .child(|parent| {
                    parent.text("Main Content").build()
                })
                .build()
        })
        .child(|parent| {
            // Footer (fixed height)
            parent.container()
                .height(Dimension::Length(40.0))
                .background_color(Color::from_rgb(0.2, 0.2, 0.3))
                .child(|parent| {
                    parent.text("Footer").build()
                })
                .build()
        })
        .build();
});
```

**Result**:
```
┌─────────────────────────────┐
│         Header              │ ← Fixed 60px
├─────────────────────────────┤
│                             │
│                             │
│       Main Content          │ ← Grows to fill
│                             │
│                             │
├─────────────────────────────┤
│         Footer              │ ← Fixed 40px
└─────────────────────────────┘
```

### Pattern 2: Sidebar Layout

```rust
ui.build(|root| {
    root.row()
        .width(Dimension::Percent(1.0))
        .height(Dimension::Percent(1.0))
        .child(|parent| {
            // Sidebar (fixed width)
            parent.container()
                .width(Dimension::Length(200.0))
                .background_color(Color::from_rgb(0.15, 0.15, 0.2))
                .child(|parent| {
                    parent.text("Sidebar").build()
                })
                .build()
        })
        .child(|parent| {
            // Main content (flex grows)
            parent.container()
                .flex_grow(1.0)
                .background_color(Color::from_rgb(0.1, 0.1, 0.15))
                .child(|parent| {
                    parent.text("Main Content").build()
                })
                .build()
        })
        .build();
});
```

**Result**:
```
┌─────────┬───────────────────────┐
│         │                       │
│ Sidebar │    Main Content       │
│         │                       │
│         │                       │
└─────────┴───────────────────────┘
   200px       Flexible
```

### Pattern 3: Centered Content

```rust
ui.build(|root| {
    root.column()
        .width(Dimension::Percent(1.0))
        .height(Dimension::Percent(1.0))
        .justify_content(JustifyContent::Center)  // Vertical center
        .align_items(AlignItems::Center)          // Horizontal center
        .child(|parent| {
            parent.container()
                .width(Dimension::Length(400.0))
                .height(Dimension::Length(300.0))
                .background_color(Color::from_rgb(0.2, 0.2, 0.3))
                .child(|parent| {
                    parent.text("Centered Box").build()
                })
                .build()
        })
        .build();
});
```

**Result**:
```
┌─────────────────────────────┐
│                             │
│     ┌───────────────┐       │
│     │               │       │
│     │ Centered Box  │       │
│     │               │       │
│     └───────────────┘       │
│                             │
└─────────────────────────────┘
```

### Pattern 4: Card Grid

```rust
ui.build(|root| {
    root.row()
        .width(Dimension::Percent(1.0))
        .gap(20.0)
        .flex_wrap(FlexWrap::Wrap)  // Wrap to next row
        .padding(20.0)
        .child(|parent| {
            // Card 1
            create_card(parent, "Card 1");
        })
        .child(|parent| {
            // Card 2
            create_card(parent, "Card 2");
        })
        .child(|parent| {
            // Card 3
            create_card(parent, "Card 3");
        })
        // ... more cards
        .build();
});

fn create_card(parent: &mut Builder, title: &str) {
    parent.container()
        .width(Dimension::Length(200.0))
        .height(Dimension::Length(150.0))
        .background_color(Color::from_rgb(0.2, 0.2, 0.3))
        .padding(10.0)
        .child(|parent| {
            parent.text(title).build()
        })
        .build()
}
```

**Result**:
```
┌──────┐  ┌──────┐  ┌──────┐
│Card 1│  │Card 2│  │Card 3│
└──────┘  └──────┘  └──────┘

┌──────┐  ┌──────┐  ┌──────┐
│Card 4│  │Card 5│  │Card 6│
└──────┘  └──────┘  └──────┘
```

### Pattern 5: Dashboard Panels

```rust
ui.build(|root| {
    root.column()
        .width(Dimension::Percent(1.0))
        .height(Dimension::Percent(1.0))
        .gap(10.0)
        .padding(10.0)
        .child(|parent| {
            // Top row: 2 panels side by side
            parent.row()
                .gap(10.0)
                .flex_basis(Dimension::Percent(0.5))  // Take 50% height
                .child(|parent| {
                    create_panel(parent, "Panel 1");
                })
                .child(|parent| {
                    create_panel(parent, "Panel 2");
                })
                .build()
        })
        .child(|parent| {
            // Bottom row: 1 wide panel
            parent.container()
                .flex_basis(Dimension::Percent(0.5))  // Take 50% height
                .flex_grow(1.0)
                .background_color(Color::from_rgb(0.2, 0.2, 0.3))
                .child(|parent| {
                    parent.text("Panel 3 (Wide)").build()
                })
                .build()
        })
        .build();
});

fn create_panel(parent: &mut Builder, title: &str) {
    parent.container()
        .flex_grow(1.0)  // Grow to fill horizontal space
        .background_color(Color::from_rgb(0.2, 0.2, 0.3))
        .padding(10.0)
        .child(|parent| {
            parent.text(title).build()
        })
        .build()
}
```

**Result**:
```
┌─────────────┬─────────────┐
│   Panel 1   │   Panel 2   │
│             │             │
│             │             │
├─────────────┴─────────────┤
│      Panel 3 (Wide)       │
│                           │
│                           │
└───────────────────────────┘
```

## Responsive Layouts

### Using Percentage Widths

```rust
// Responsive column widths
parent.row()
    .child(|parent| {
        parent.container()
            .width(Dimension::Percent(0.25))  // 25% width
            .build()
    })
    .child(|parent| {
        parent.container()
            .width(Dimension::Percent(0.75))  // 75% width
            .build()
    })
```

### Flex-Based Responsive

```rust
// Panels that adapt to available space
parent.row()
    .child(|parent| {
        parent.container()
            .flex_grow(1.0)  // Takes 1 unit of space
            .min_width(Dimension::Length(200.0))  // But never smaller than 200px
            .build()
    })
    .child(|parent| {
        parent.container()
            .flex_grow(2.0)  // Takes 2 units of space (twice as much)
            .build()
    })
```

### Wrapping for Small Screens

```rust
use taffy::FlexWrap;

parent.row()
    .flex_wrap(FlexWrap::Wrap)  // Wrap to next row if needed
    .child(|parent| {
        parent.container()
            .width(Dimension::Length(300.0))
            .min_width(Dimension::Length(200.0))
            .flex_shrink(1.0)  // Can shrink
            .build()
    })
    // More children...
```

## Advanced Techniques

### Absolute Positioning (When Supported)

Some UI systems support absolute positioning:

```rust
widget
    .position(Position::Absolute)
    .left(Dimension::Length(10.0))
    .top(Dimension::Length(10.0))
```

### Z-Index (Layering)

For overlapping widgets:

```rust
modal.z_index(1000)  // Render on top
```

### Scrolling Containers

```rust
parent.scroll_view()
    .height(Dimension::Length(400.0))  // Fixed height
    .child(|parent| {
        // Content taller than 400px will scroll
        parent.column()
            .child(|p| { /* many children */ })
            .build()
    })
```

## Debugging Layouts

### Visual Debugging

Enable border debugging:

```rust
widget
    .border_color(Color::RED)
    .border_width(1.0)
```

### Inspector Middleware

Enable the UI inspector (if available):

```rust
ui.enable_inspector();  // Shows widget bounds and IDs
```

### Logging Layout

```rust
tracing::debug!("Widget size: {:?}", widget.size());
tracing::debug!("Widget position: {:?}", widget.position());
```

## Common Layout Issues

### Issue 1: Widget Not Visible

**Problem**: Widget exists but doesn't appear.

**Causes**:
1. Zero size (no width/height set and no content)
2. Outside visible area (negative position)
3. Hidden behind other widgets

**Fix**:
```rust
widget
    .width(Dimension::Length(100.0))
    .height(Dimension::Length(50.0))
    .background_color(Color::RED)  // Make it visible
```

### Issue 2: Content Overflowing

**Problem**: Content is cut off or extends beyond container.

**Fix**: Enable scrolling or increase container size:

```rust
container
    .height(Dimension::Auto)  // Size to content
    // OR
    .overflow(Overflow::Scroll)  // Enable scrolling
```

### Issue 3: Uneven Spacing

**Problem**: Items have inconsistent spacing.

**Fix**: Use gap instead of margin:

```rust
// Bad: Margin on children causes uneven spacing
child.margin(10.0)

// Good: Gap on parent
parent.column().gap(10.0)
```

### Issue 4: Items Not Growing

**Problem**: flex_grow doesn't seem to work.

**Cause**: Parent doesn't have defined size.

**Fix**:
```rust
parent.column()
    .height(Dimension::Percent(1.0))  // Parent must have size
    .child(|child| {
        child.container()
            .flex_grow(1.0)  // Now this works
            .build()
    })
```

## Performance Considerations

### Layout Caching

Taffy caches layout results. Avoid:
- Changing layout properties every frame
- Unnecessary rebuilds of the UI tree

### Dirty Flag Optimization

Only dirty subtrees are re-laid out:

```rust
// Good: Only updates text, doesn't trigger layout
ui.update_text(&text_id, "New text");

// Bad: Rebuilds entire tree, triggers full layout
ui.build(|root| { /* rebuild everything */ });
```

### Complex Nesting

Deep nesting can slow layout computation:

```rust
// Try to avoid:
root → container → container → container → container → text

// Prefer:
root → container → text
```

## Next Steps

Master layout techniques with:

1. **[Styling and Theming](styling-and-theming.md)** - Visual polish for layouts
2. **[Performance Optimization](performance-optimization.md)** - Speed up complex layouts
3. **[Event Handling](event-handling.md)** - Make layouts interactive

## Summary

**Key takeaways**:
- **Flexbox**: One-dimensional layouts (row or column)
- **Main axis**: Direction of flex_direction (column = vertical, row = horizontal)
- **Cross axis**: Perpendicular to main axis
- **justify_content**: Aligns along main axis
- **align_items**: Aligns along cross axis
- **flex_grow**: Children grow to fill space
- **gap**: Space between children (cleaner than margin)
- **Common patterns**: Header/footer, sidebar, centered, grid, dashboard

You now have comprehensive knowledge of Astrelis layout system!
