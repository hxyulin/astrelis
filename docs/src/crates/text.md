# astrelis-text

The `astrelis-text` crate provides high-quality text rendering using `cosmic-text`. It supports complex text layout, shaping, and system font loading.

## Features

- **Font Management**: Loads system fonts and custom fonts.
- **Shaping**: Handles complex scripts, ligatures, and bidirectional text.
- **Rendering**: GPU-accelerated rendering using a dynamic texture atlas.
- **Styling**: Rich text styling (size, weight, color, alignment).
- **Caching**: Efficient caching of shaped glyphs and layout.

## Usage

```rust
use astrelis_text::{FontSystem, FontRenderer, Text, Color};

let font_system = FontSystem::with_system_fonts();
let mut renderer = FontRenderer::new(context, font_system);

let text = Text::new("Hello World")
    .size(24.0)
    .color(Color::WHITE);

let mut buffer = renderer.prepare(&text);
renderer.draw_text(&mut buffer, position);
```

## Modules

### `font`

- `FontSystem`: Manages the font database and loading.
- `FontAttributes`: Properties like weight, style, and stretch.

### `renderer`

- `FontRenderer`: The main rendering interface. Manages the texture atlas and draw calls.
- `TextBuffer`: Cached layout result for a text string.

### `text`

- `Text`: Builder struct for defining text content and style.

### `shaping`

- Low-level access to the text shaping engine.
