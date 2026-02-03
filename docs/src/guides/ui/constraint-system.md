# Constraint System Deep Dive

Beyond basic width/height, Astrelis supports advanced constraint expressions for responsive and adaptive layouts. The constraint system integrates seamlessly with the Taffy layout engine (Flexbox/Grid) while providing additional flexibility.

## Overview

Constraints define how widgets compute their dimensions during layout. They support:

- **Absolute units** (pixels)
- **Relative units** (percentages, viewport units)
- **Constraint expressions** (min, max, clamp, calc)
- **Automatic sizing** (content-based)

## Basic Constraint Types

### Pixel Constraints

Fixed pixel values for precise control:

```rust
use astrelis_ui::Constraint;

root.rect()
    .width(Constraint::Px(200.0))   // 200 pixels wide
    .height(Constraint::Px(100.0))  // 100 pixels tall
    .build();
```

### Percentage Constraints

Relative to parent dimensions:

```rust
root.rect()
    .width(Constraint::Percent(50.0))   // 50% of parent width
    .height(Constraint::Percent(100.0)) // Full parent height
    .build();
```

### Auto Constraints

Size based on content or flex/grid rules:

```rust
root.rect()
    .width(Constraint::Auto)   // Size to content
    .height(Constraint::Auto)  // Size to content
    .build();
```

## Viewport Units

Viewport units enable responsive layouts that adapt to window size changes.

### vw() - Viewport Width

```rust
use astrelis_ui::constraint::vw;

root.rect()
    .width(vw(50.0))  // 50% of viewport width
    .build();
```

This is equivalent to `Constraint::Vw(50.0)`.

### vh() - Viewport Height

```rust
use astrelis_ui::constraint::vh;

root.rect()
    .height(vh(100.0))  // 100% of viewport height (fullscreen)
    .build();
```

This is equivalent to `Constraint::Vh(100.0)`.

### Combining Viewport Units

```rust
// Sidebar that's 20vw wide and fullscreen height
root.rect()
    .width(vw(20.0))
    .height(vh(100.0))
    .build();

// Content area taking remaining space (use Flexbox for this)
root.column(|col| {
    col.rect().width(vw(20.0)).height(vh(100.0)).build();  // Sidebar
    col.rect().flex(1.0).build();  // Fills remaining width
});
```

## Constraint Expressions

### min() - Minimum Constraint

Takes the larger of two constraints:

```rust
use astrelis_ui::constraint::min;

root.rect()
    .width(min(
        Constraint::Px(100.0),      // At least 100px
        Constraint::Percent(50.0),  // Or 50% of parent if larger
    ))
    .build();
```

### max() - Maximum Constraint

Takes the smaller of two constraints:

```rust
use astrelis_ui::constraint::max;

root.rect()
    .width(max(
        Constraint::Percent(80.0),  // Up to 80% of parent
        Constraint::Px(600.0),      // But never more than 600px
    ))
    .build();
```

### clamp() - Range Constraint

Clamps a value between min and max:

```rust
use astrelis_ui::constraint::clamp;

root.rect()
    .width(clamp(
        Constraint::Px(200.0),      // Minimum width
        Constraint::Percent(50.0),  // Preferred width
        Constraint::Px(800.0),      // Maximum width
    ))
    .build();
```

This is useful for responsive text columns:

```rust
// Text column that's 60% wide, but between 400px and 800px
root.text("Long text content...")
    .width(clamp(
        Constraint::Px(400.0),
        Constraint::Percent(60.0),
        Constraint::Px(800.0),
    ))
    .build();
```

### calc() - Arithmetic Expressions

Combine constraints with arithmetic:

```rust
use astrelis_ui::constraint::calc;

root.rect()
    .width(calc(
        Constraint::Percent(100.0),  // Full width
        Sub,                          // Minus
        Constraint::Px(40.0),         // 40px padding
    ))
    .build();
```

Supported operators:
- `Add` - Addition
- `Sub` - Subtraction
- `Mul` - Multiplication
- `Div` - Division

## Practical Examples

### Responsive Sidebar Layout

```rust
ui.build(|root| {
    root.row(|row| {
        // Sidebar: 250px on large screens, 20vw on smaller screens
        row.rect()
            .width(min(Constraint::Px(250.0), vw(20.0)))
            .height(vh(100.0))
            .color(Color::rgb(0.1, 0.1, 0.1))
            .build();

        // Content: Fills remaining space
        row.rect()
            .flex(1.0)
            .height(vh(100.0))
            .color(Color::rgb(0.15, 0.15, 0.15))
            .build();
    });
});
```

### Centered Content with Max Width

```rust
ui.build(|root| {
    root.column(|col| {
        col.rect()
            .width(clamp(
                Constraint::Percent(80.0),  // 80% on small screens
                Constraint::Percent(60.0),  // 60% preferred
                Constraint::Px(1200.0),     // Max 1200px
            ))
            .height(vh(100.0))
            .margin_horizontal(Constraint::Auto)  // Center horizontally
            .build();
    });
});
```

### Aspect Ratio Constraint

While there's no direct aspect ratio constraint, you can combine calc with vh/vw:

```rust
// 16:9 aspect ratio box
let width = vw(50.0);
let height = calc(
    width.clone(),
    Mul,
    Constraint::Ratio(9.0 / 16.0),  // Height = Width * (9/16)
);

root.rect()
    .width(width)
    .height(height)
    .build();
```

### Responsive Grid

```rust
use astrelis_ui::Layout;

ui.build(|root| {
    root.grid(|grid| {
        grid.columns(vec![
            // 3 columns on large screens, 1 on small
            min(Constraint::Px(300.0), vw(33.0)),
            min(Constraint::Px(300.0), vw(33.0)),
            min(Constraint::Px(300.0), vw(33.0)),
        ]);

        grid.gap(Constraint::Px(16.0));

        for i in 0..9 {
            grid.rect()
                .height(Constraint::Px(200.0))
                .color(Color::rgb(0.2, 0.3, 0.4))
                .id(format!("card-{}", i))
                .build();
        }
    });
});
```

## Performance Considerations

### Layout Propagation

Constraint changes trigger layout recalculation:

```rust
// Efficient: Updates only the affected widget
ui.update_constraint("my-widget", Constraint::Px(200.0));

// Inefficient: Rebuilds entire tree
ui.build(|root| {
    root.rect().width(Constraint::Px(200.0)).id("my-widget").build();
});
```

### Viewport Unit Caching

Viewport units are re-evaluated on window resize. For static layouts, prefer pixel or percentage constraints.

### Expression Complexity

Complex nested expressions (`calc(min(max(...)))`) have higher layout overhead. Prefer simpler constraints when possible.

## Constraint Resolution Order

During layout, constraints are resolved in this order:

1. **Explicit constraints** (width/height) are evaluated
2. **Viewport units** are converted to pixels based on window size
3. **Expressions** (min/max/clamp/calc) are computed
4. **Flexbox/Grid rules** are applied
5. **Auto constraints** are resolved based on content

## Common Patterns

### Full-Screen Overlay

```rust
root.rect()
    .width(vw(100.0))
    .height(vh(100.0))
    .color(Color::rgba(0.0, 0.0, 0.0, 0.5))
    .build();
```

### Header/Footer with Scrollable Content

```rust
root.column(|col| {
    // Fixed header
    col.rect().height(Constraint::Px(60.0)).build();

    // Scrollable content
    col.scroll_container(|scroll| {
        scroll.height(calc(vh(100.0), Sub, Constraint::Px(120.0)));  // Viewport - header - footer
    });

    // Fixed footer
    col.rect().height(Constraint::Px(60.0)).build();
});
```

### Responsive Font Sizes

While font sizes aren't constraints, you can compute them based on viewport:

```rust
let font_size = ui.viewport_width() * 0.02;  // 2vw equivalent
root.text("Responsive text")
    .size(font_size.clamp(12.0, 48.0))
    .build();
```

## Advanced: Custom Constraint Types

You can extend the constraint system with custom types:

```rust
// Example: Container query-like constraint
pub enum CustomConstraint {
    ParentMin(f32),  // Minimum of parent dimension
    ParentMax(f32),  // Maximum of parent dimension
}

// Implement resolution logic in your layout engine
```

## Next Steps

- See [Layout Deep Dive](./layout-deep-dive.md) for Flexbox/Grid integration
- Explore [Styling and Theming](./styling-and-theming.md) for combining constraints with styles
- Check [Performance Optimization](./performance-optimization.md) for layout tuning
