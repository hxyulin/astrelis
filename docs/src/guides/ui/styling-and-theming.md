# Styling and Theming

This guide covers how to create consistent visual styles and themes for your Astrelis UI. You'll learn about the color system, borders, styling patterns, and how to implement runtime theme switching.

## Prerequisites

- Completed [First UI](../getting-started/05-first-ui.md)
- Understanding of [Custom Widgets](custom-widgets.md) (helpful)

## Color System

### Creating Colors

Astrelis uses the `Color` type from `astrelis-render`:

```rust
use astrelis_render::Color;

// RGB (values 0.0 to 1.0)
let red = Color::from_rgb(1.0, 0.0, 0.0);
let teal = Color::from_rgb(0.2, 0.5, 0.6);

// RGB with u8 (values 0 to 255)
let blue = Color::from_rgb_u8(50, 100, 200);

// RGBA (with alpha channel)
let semi_transparent = Color::rgba(1.0, 0.0, 0.0, 0.5);  // 50% transparent red

// Named colors
Color::WHITE
Color::BLACK
Color::RED
Color::GREEN
Color::BLUE
Color::TRANSPARENT
```

### Color Manipulation

```rust
// Lighten/darken
fn lighten(color: Color, amount: f32) -> Color {
    Color::from_rgb(
        (color.r + amount).min(1.0),
        (color.g + amount).min(1.0),
        (color.b + amount).min(1.0),
    )
}

fn darken(color: Color, amount: f32) -> Color {
    Color::from_rgb(
        (color.r - amount).max(0.0),
        (color.g - amount).max(0.0),
        (color.b - amount).max(0.0),
    )
}

// With alpha
fn with_alpha(color: Color, alpha: f32) -> Color {
    Color::rgba(color.r, color.g, color.b, alpha)
}

// Interpolate between colors
fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    Color::from_rgb(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
    )
}
```

### Color Palettes

Define a palette for your app:

```rust
pub struct ColorPalette {
    pub primary: Color,
    pub secondary: Color,
    pub accent: Color,
    pub background: Color,
    pub surface: Color,
    pub text: Color,
    pub text_secondary: Color,
    pub border: Color,
    pub error: Color,
    pub warning: Color,
    pub success: Color,
}

impl ColorPalette {
    pub fn dark() -> Self {
        Self {
            primary: Color::from_rgb_u8(100, 200, 255),
            secondary: Color::from_rgb_u8(150, 150, 200),
            accent: Color::from_rgb_u8(255, 180, 100),
            background: Color::from_rgb_u8(25, 25, 35),
            surface: Color::from_rgb_u8(40, 40, 55),
            text: Color::WHITE,
            text_secondary: Color::from_rgb_u8(150, 150, 150),
            border: Color::from_rgb_u8(80, 80, 120),
            error: Color::from_rgb_u8(220, 60, 60),
            warning: Color::from_rgb_u8(220, 180, 60),
            success: Color::from_rgb_u8(60, 180, 60),
        }
    }

    pub fn light() -> Self {
        Self {
            primary: Color::from_rgb_u8(60, 120, 200),
            secondary: Color::from_rgb_u8(100, 100, 150),
            accent: Color::from_rgb_u8(200, 120, 60),
            background: Color::from_rgb_u8(240, 240, 245),
            surface: Color::WHITE,
            text: Color::from_rgb_u8(20, 20, 30),
            text_secondary: Color::from_rgb_u8(100, 100, 110),
            border: Color::from_rgb_u8(200, 200, 210),
            error: Color::from_rgb_u8(200, 40, 40),
            warning: Color::from_rgb_u8(200, 150, 40),
            success: Color::from_rgb_u8(40, 150, 40),
        }
    }
}
```

## Widget Styling Properties

### Background Color

```rust
widget
    .background_color(Color::from_rgb(0.2, 0.2, 0.3))
```

### Text Color

```rust
text_widget
    .color(Color::WHITE)
```

### Borders

```rust
// Border color and width
widget
    .border_color(Color::from_rgb(0.4, 0.4, 0.6))
    .border_width(2.0)

// Individual sides (if supported)
widget
    .border_left_width(2.0)
    .border_right_width(2.0)
    .border_top_width(1.0)
    .border_bottom_width(1.0)
```

### Border Radius (Rounded Corners)

```rust
// All corners
widget.border_radius(8.0)

// Individual corners (if supported)
widget
    .border_top_left_radius(8.0)
    .border_top_right_radius(8.0)
    .border_bottom_left_radius(4.0)
    .border_bottom_right_radius(4.0)
```

**Visual**:
```
radius(0):      radius(4):      radius(8):      radius(16):
┌────────┐      ╭────────╮      ╭────────╮      ╭────────╮
│        │      │        │      │        │      │        │
│        │      │        │      │        │      │        │
└────────┘      ╰────────╯      ╰────────╯      ╰────────╯
```

### Padding and Margin

```rust
// Padding (inside border)
widget.padding(10.0)

// Margin (outside border)
widget.margin(10.0)

// Individual sides
widget
    .padding_left(10.0)
    .padding_right(10.0)
    .padding_top(5.0)
    .padding_bottom(5.0)
```

### Font Styling

```rust
text_widget
    .font_size(18.0)
    .bold()
    .italic()
```

## Theme System

### Defining a Theme

```rust
use astrelis_render::Color;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Theme {
    Dark,
    Light,
    Custom,
}

impl Theme {
    pub fn palette(&self) -> ColorPalette {
        match self {
            Theme::Dark => ColorPalette::dark(),
            Theme::Light => ColorPalette::light(),
            Theme::Custom => ColorPalette::custom(),
        }
    }

    // Convenience methods for direct access
    pub fn background_color(&self) -> Color {
        self.palette().background
    }

    pub fn surface_color(&self) -> Color {
        self.palette().surface
    }

    pub fn text_color(&self) -> Color {
        self.palette().text
    }

    pub fn primary_color(&self) -> Color {
        self.palette().primary
    }

    pub fn border_color(&self) -> Color {
        self.palette().border
    }

    pub fn accent_color(&self) -> Color {
        self.palette().accent
    }

    pub fn toggle(&self) -> Self {
        match self {
            Theme::Dark => Theme::Light,
            Theme::Light => Theme::Dark,
            Theme::Custom => Theme::Dark,
        }
    }
}
```

### Using Themes in UI

```rust
struct MyApp {
    // ... other fields
    theme: Theme,
}

fn build_ui(ui: &mut UiSystem, theme: Theme) {
    ui.build(|root| {
        root.container()
            .background_color(theme.background_color())
            .child(|parent| {
                parent.container()
                    .background_color(theme.surface_color())
                    .border_color(theme.border_color())
                    .border_width(2.0)
                    .padding(20.0)
                    .child(|parent| {
                        parent.text("Hello, World!")
                            .color(theme.text_color())
                            .size(18.0)
                            .build()
                    })
                    .build()
            })
            .build();
    });
}
```

### Runtime Theme Switching

```rust
impl App for MyApp {
    fn update(&mut self, _ctx: &mut AppCtx, time: &FrameTime) {
        // Handle theme toggle
        // (keyboard input would go here)
    }

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        if window_id != self.window_id {
            return;
        }

        // Check for theme toggle key
        events.dispatch(|event| {
            use astrelis_winit::event::{Event, HandleStatus, Key};
            if let Event::KeyPressed(Key::T) = event {
                // Toggle theme
                self.theme = self.theme.toggle();

                // Rebuild UI with new theme
                build_ui(&mut self.ui, self.theme);

                HandleStatus::consumed()
            } else {
                HandleStatus::ignored()
            }
        });

        // ... rendering
    }
}
```

### Incremental Theme Updates

For better performance, update colors without rebuilding:

```rust
fn apply_theme_colors(&mut self, theme: Theme) {
    // Update specific widget colors
    self.ui.update_color(&self.background_id, theme.background_color());
    self.ui.update_color(&self.panel_id, theme.surface_color());
    self.ui.update_color(&self.title_id, theme.text_color());
    self.ui.update_color(&self.button_id, theme.primary_color());
}

// In event handler
if theme_changed {
    self.theme = new_theme;
    self.apply_theme_colors(self.theme);  // Fast color updates (~0.1ms)
}
```

## Styling Patterns

### Pattern 1: Component Styles

Define reusable style functions:

```rust
pub fn button_style(builder: &mut ButtonBuilder, theme: Theme) -> &mut ButtonBuilder {
    builder
        .background_color(theme.primary_color())
        .hover_color(lighten(theme.primary_color(), 0.1))
        .border_width(0.0)
        .border_radius(6.0)
        .padding(12.0)
        .font_size(16.0)
}

// Usage
ui.build(|root| {
    root.child(|parent| {
        button_style(
            parent.button("Click Me"),
            theme,
        )
        .on_click(|| { /* handler */ })
        .build()
    })
});
```

### Pattern 2: Style Structs

Define styles as data:

```rust
pub struct ButtonStyle {
    pub background: Color,
    pub hover: Color,
    pub text: Color,
    pub border_radius: f32,
    pub padding: f32,
}

impl ButtonStyle {
    pub fn primary(theme: Theme) -> Self {
        Self {
            background: theme.primary_color(),
            hover: lighten(theme.primary_color(), 0.1),
            text: Color::WHITE,
            border_radius: 6.0,
            padding: 12.0,
        }
    }

    pub fn secondary(theme: Theme) -> Self {
        Self {
            background: theme.surface_color(),
            hover: lighten(theme.surface_color(), 0.1),
            text: theme.text_color(),
            border_radius: 6.0,
            padding: 12.0,
        }
    }

    pub fn apply(&self, button: &mut ButtonBuilder) -> &mut ButtonBuilder {
        button
            .background_color(self.background)
            .hover_color(self.hover)
            .color(self.text)
            .border_radius(self.border_radius)
            .padding(self.padding)
    }
}

// Usage
let style = ButtonStyle::primary(theme);
style.apply(parent.button("Submit")).build();
```

### Pattern 3: Style Inheritance

Pass theme through UI building:

```rust
fn build_card(parent: &mut Builder, theme: Theme, title: &str, content: &str) {
    parent.container()
        .background_color(theme.surface_color())
        .border_color(theme.border_color())
        .border_width(1.0)
        .border_radius(8.0)
        .padding(16.0)
        .child(|parent| {
            parent.column()
                .gap(10.0)
                .child(|parent| {
                    parent.text(title)
                        .color(theme.text_color())
                        .size(20.0)
                        .bold()
                        .build()
                })
                .child(|parent| {
                    parent.text(content)
                        .color(theme.text_secondary_color())
                        .size(14.0)
                        .build()
                })
                .build()
        })
        .build();
}
```

### Pattern 4: CSS-Like Classes

Define named styles:

```rust
pub struct StyleSheet {
    pub card: ContainerStyle,
    pub button_primary: ButtonStyle,
    pub button_secondary: ButtonStyle,
    pub heading: TextStyle,
    pub body: TextStyle,
}

impl StyleSheet {
    pub fn new(theme: Theme) -> Self {
        Self {
            card: ContainerStyle {
                background: theme.surface_color(),
                border: theme.border_color(),
                border_width: 1.0,
                border_radius: 8.0,
                padding: 16.0,
            },
            button_primary: ButtonStyle::primary(theme),
            button_secondary: ButtonStyle::secondary(theme),
            heading: TextStyle {
                color: theme.text_color(),
                size: 20.0,
                bold: true,
            },
            body: TextStyle {
                color: theme.text_secondary_color(),
                size: 14.0,
                bold: false,
            },
        }
    }
}

// Usage
struct MyApp {
    styles: StyleSheet,
}

fn build_ui(&self) {
    self.styles.card.apply(parent.container())
        .child(|parent| {
            self.styles.heading.apply(parent.text("Title")).build();
        })
        .build();
}
```

## Hover and Active States

### Button Hover States

```rust
parent.button("Click Me")
    .background_color(Color::from_rgb(0.3, 0.6, 0.9))
    .hover_color(Color::from_rgb(0.4, 0.7, 1.0))  // Lighter on hover
    .on_click(|| { /* handler */ })
    .build()
```

### Manual Hover Handling

For custom widgets:

```rust
impl MyWidget {
    pub fn on_hover_enter(&mut self) {
        self.is_hovered = true;
        // Trigger color update
    }

    pub fn on_hover_exit(&mut self) {
        self.is_hovered = false;
        // Trigger color update
    }

    pub fn color(&self) -> Color {
        if self.is_hovered {
            self.hover_color
        } else {
            self.normal_color
        }
    }
}
```

### Pressed State

```rust
impl ClickableWidget for MyButton {
    fn is_pressed(&self) -> bool {
        self.pressed
    }

    fn color(&self) -> Color {
        if self.is_pressed() {
            darken(self.normal_color, 0.2)
        } else if self.is_hovered() {
            lighten(self.normal_color, 0.1)
        } else {
            self.normal_color
        }
    }
}
```

## Advanced Styling

### Gradients (Custom Rendering)

For gradient backgrounds, implement custom rendering:

```rust
// Pseudo-code for gradient rendering
fn render_gradient_background(position: Vec2, size: Vec2, color_start: Color, color_end: Color) {
    // Render quad with vertex colors
    let vertices = [
        Vertex { pos: position, color: color_start },
        Vertex { pos: position + Vec2::new(size.x, 0.0), color: color_start },
        Vertex { pos: position + size, color: color_end },
        Vertex { pos: position + Vec2::new(0.0, size.y), color: color_end },
    ];
    // Submit to GPU
}
```

### Shadows (Custom Rendering)

Implement drop shadows with offset rendering:

```rust
pub struct ShadowStyle {
    pub color: Color,
    pub offset: Vec2,
    pub blur: f32,
}

// Render shadow behind widget
fn render_with_shadow(widget: &Widget, shadow: ShadowStyle) {
    // 1. Render shadow (offset, blurred, colored)
    render_shadow(widget.bounds(), shadow);

    // 2. Render widget on top
    render_widget(widget);
}
```

### Transparency and Layering

```rust
// Semi-transparent overlay
parent.container()
    .background_color(Color::rgba(0.0, 0.0, 0.0, 0.7))  // 70% black
    .child(|parent| {
        // Modal content
    })
```

## Theme Persistence

### Save/Load Theme Preference

```rust
use std::fs;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct AppConfig {
    theme: String,  // "dark" or "light"
}

impl AppConfig {
    pub fn load() -> Result<Self, std::io::Error> {
        let content = fs::read_to_string("config.json")?;
        Ok(serde_json::from_str(&content)?)
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write("config.json", content)?;
        Ok(())
    }
}

// Load theme on startup
fn main() {
    let config = AppConfig::load().unwrap_or_default();
    let theme = match config.theme.as_str() {
        "light" => Theme::Light,
        _ => Theme::Dark,
    };

    // ... use theme
}
```

## Color Accessibility

### Contrast Ratios

Ensure readable text:

```rust
fn contrast_ratio(foreground: Color, background: Color) -> f32 {
    // Simplified contrast calculation
    let l1 = relative_luminance(foreground);
    let l2 = relative_luminance(background);

    if l1 > l2 {
        (l1 + 0.05) / (l2 + 0.05)
    } else {
        (l2 + 0.05) / (l1 + 0.05)
    }
}

fn relative_luminance(color: Color) -> f32 {
    // sRGB to linear RGB
    let r = if color.r <= 0.03928 { color.r / 12.92 } else { ((color.r + 0.055) / 1.055).powf(2.4) };
    let g = if color.g <= 0.03928 { color.g / 12.92 } else { ((color.g + 0.055) / 1.055).powf(2.4) };
    let b = if color.b <= 0.03928 { color.b / 12.92 } else { ((color.b + 0.055) / 1.055).powf(2.4) };

    0.2126 * r + 0.7152 * g + 0.0722 * b
}

// WCAG AA requires 4.5:1 for normal text, 3:1 for large text
assert!(contrast_ratio(text_color, background_color) >= 4.5);
```

## Complete Theme Example

```rust
use astrelis_render::Color;

pub struct Theme {
    name: &'static str,
    colors: ColorPalette,
    typography: Typography,
    spacing: Spacing,
    borders: BorderStyle,
}

pub struct ColorPalette {
    pub primary: Color,
    pub secondary: Color,
    pub accent: Color,
    pub background: Color,
    pub surface: Color,
    pub text: Color,
    pub text_secondary: Color,
    pub border: Color,
    pub error: Color,
    pub warning: Color,
    pub success: Color,
}

pub struct Typography {
    pub heading_size: f32,
    pub body_size: f32,
    pub small_size: f32,
}

pub struct Spacing {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
}

pub struct BorderStyle {
    pub radius_sm: f32,
    pub radius_md: f32,
    pub radius_lg: f32,
    pub width: f32,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            name: "Dark",
            colors: ColorPalette {
                primary: Color::from_rgb_u8(100, 200, 255),
                secondary: Color::from_rgb_u8(150, 150, 200),
                accent: Color::from_rgb_u8(255, 180, 100),
                background: Color::from_rgb_u8(25, 25, 35),
                surface: Color::from_rgb_u8(40, 40, 55),
                text: Color::WHITE,
                text_secondary: Color::from_rgb_u8(150, 150, 150),
                border: Color::from_rgb_u8(80, 80, 120),
                error: Color::from_rgb_u8(220, 60, 60),
                warning: Color::from_rgb_u8(220, 180, 60),
                success: Color::from_rgb_u8(60, 180, 60),
            },
            typography: Typography {
                heading_size: 24.0,
                body_size: 14.0,
                small_size: 12.0,
            },
            spacing: Spacing {
                xs: 4.0,
                sm: 8.0,
                md: 16.0,
                lg: 24.0,
                xl: 32.0,
            },
            borders: BorderStyle {
                radius_sm: 4.0,
                radius_md: 8.0,
                radius_lg: 12.0,
                width: 1.0,
            },
        }
    }
}
```

## Best Practices

1. **Define themes early**: Establish your color palette before building UI
2. **Use named colors**: `theme.primary_color()` is better than hardcoded RGB values
3. **Consider accessibility**: Ensure sufficient contrast ratios
4. **Test both themes**: If supporting light/dark, test both regularly
5. **Use spacing constants**: Define spacing scale (4px, 8px, 16px, etc.)
6. **Consistent borders**: Use the same border radius values throughout
7. **Limit color palette**: 5-7 main colors plus semantic colors (error, success, etc.)
8. **Performance**: Use `update_color()` for theme switching, not full rebuilds

## Next Steps

Continue mastering UI with:

1. **[Event Handling](event-handling.md)** - Make your styled UI interactive
2. **[Performance Optimization](performance-optimization.md)** - Keep styled UI fast
3. **[Custom Widgets](custom-widgets.md)** - Build themeable custom widgets

## Summary

**Key takeaways**:
- **Color palette**: Define consistent colors for your app
- **Theme struct**: Encapsulate all theme settings
- **Runtime switching**: Rebuild UI or use `update_color()` for theme changes
- **Style patterns**: Component styles, style structs, CSS-like classes
- **Hover states**: `hover_color()` for interactive feedback
- **Accessibility**: Check contrast ratios for readability

You now know how to create cohesive, themeable UIs in Astrelis!
