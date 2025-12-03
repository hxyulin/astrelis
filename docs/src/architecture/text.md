# Text Rendering

The `astrelis-text` crate provides GPU-accelerated text rendering built on the `cosmic-text` library. It handles complex text layout, shaping, and rasterization with support for system fonts and custom typography.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   Application                           │
│              (Your text rendering code)                 │
└─────────────────────────────────────────────────────────┘
                          │
┌─────────────────────────────────────────────────────────┐
│                  astrelis-text                          │
│  ┌──────────────┐  ┌────────────────┐  ┌────────────┐  │
│  │ FontSystem   │  │ FontRenderer   │  │    Text    │  │
│  │ (font mgmt)  │  │ (GPU render)   │  │  (builder) │  │
│  └──────────────┘  └────────────────┘  └────────────┘  │
│         │                   │                  │         │
│  ┌──────────────┐  ┌────────────────┐                   │
│  │FontDatabase  │  │  TextBuffer    │                   │
│  │(system fonts)│  │ (layout cache) │                   │
│  └──────────────┘  └────────────────┘                   │
└─────────────────────────────────────────────────────────┘
                          │
┌─────────────────────────────────────────────────────────┐
│              External Dependencies                      │
│  ┌──────────────┐  ┌────────────────┐                   │
│  │ cosmic-text  │  │ astrelis-render│                   │
│  │  (layout,    │  │   (GPU ops)    │                   │
│  │   shaping)   │  │                │                   │
│  └──────────────┘  └────────────────┘                   │
└─────────────────────────────────────────────────────────┘
```

## Core Components

### FontSystem

Global font management and database:

```rust
use astrelis_text::{FontSystem, FontDatabase};

// Load system fonts automatically
let font_system = FontSystem::with_system_fonts();

// Create empty and add custom fonts
let mut font_system = FontSystem::new();
font_system.db_mut().load_font_file("path/to/font.ttf")?;
font_system.db_mut().load_font_data(font_bytes.to_vec());

// Query available fonts
let families = font_system.db().families();
for family in families {
    println!("Font family: {}", family);
}
```

**Lifetime**: Created once, typically as `&'static` via leak or lazy_static.

### FontRenderer

GPU-accelerated renderer with texture atlas:

```rust
use astrelis_text::{FontRenderer, Text};
use astrelis_render::GraphicsContext;

let context = GraphicsContext::new_sync();
let font_system = FontSystem::with_system_fonts();
let mut renderer = FontRenderer::new(context, font_system);

// Prepare text for rendering
let text = Text::new("Hello, World!")
    .size(24.0)
    .color(Color::WHITE);

let mut buffer = renderer.prepare(&text);

// Draw at position
renderer.draw_text(&mut buffer, Vec2::new(100.0, 100.0));

// Render all prepared text
renderer.render(&mut render_pass, viewport_size);
```

### Text Builder

Declarative text styling API:

```rust
use astrelis_text::{Text, TextAlign, TextWrap, FontWeight, FontStyle};

let text = Text::new("Styled Text")
    .size(32.0)
    .color(Color::rgba(1.0, 0.8, 0.2, 1.0))
    .bold()
    .italic()
    .align(TextAlign::Center)
    .wrap(TextWrap::Word)
    .max_width(400.0)
    .line_height(1.5);
```

### TextBuffer

Cached layout data:

```rust
pub struct TextBuffer {
    pub(crate) buffer: cosmic_text::Buffer,
    pub(crate) metrics: TextMetrics,
}

pub struct TextMetrics {
    pub width: f32,
    pub height: f32,
    pub ascent: f32,
    pub descent: f32,
    pub line_height: f32,
}
```

Buffers are expensive to create (layout required) but cheap to reuse.

## Text Styling

### Font Properties

```rust
// Family (defaults to system default)
.family("Arial")
.family("Helvetica Neue")

// Size in pixels
.size(16.0)
.size(24.0)

// Weight (100-900 or named)
.weight(FontWeight::Normal)      // 400
.weight(FontWeight::Bold)        // 700
.weight(FontWeight::from_u16(600)) // Semibold

// Style
.style(FontStyle::Normal)
.style(FontStyle::Italic)
.style(FontStyle::Oblique)

// Convenience methods
.bold()      // weight(Bold)
.italic()    // style(Italic)
```

### Color

```rust
use astrelis_render::Color;

// Solid colors
.color(Color::WHITE)
.color(Color::rgba(1.0, 0.5, 0.0, 1.0))
.color(Color::from_hex(0xFF8000FF))

// Transparency
.color(Color::rgba(1.0, 1.0, 1.0, 0.5)) // 50% transparent
```

### Layout

```rust
// Alignment
.align(TextAlign::Left)
.align(TextAlign::Center)
.align(TextAlign::Right)
.align(TextAlign::Justify)

// Wrapping
.wrap(TextWrap::None)       // Single line, overflow
.wrap(TextWrap::Word)       // Wrap at word boundaries
.wrap(TextWrap::Character)  // Wrap at any character

// Width constraint
.max_width(400.0)  // Wrap text to fit width

// Line spacing
.line_height(1.0)  // Single-spaced
.line_height(1.5)  // 1.5x line height
.line_height(2.0)  // Double-spaced
```

## Text Rendering Pipeline

### 1. Text Preparation

```rust
let text = Text::new("Hello").size(24.0);
let buffer = renderer.prepare(&text);
```

Process:
1. Create cosmic-text buffer
2. Set font, size, color from Text
3. Layout text (expensive: ~0.5-2ms)
4. Store buffer with metrics

### 2. Glyph Rasterization

```rust
renderer.draw_text(&mut buffer, position);
```

Process:
1. Query required glyphs from buffer
2. Check atlas cache for each glyph
3. Rasterize missing glyphs
4. Upload to atlas texture
5. Record draw commands (position, UV coords)

### 3. Batch Rendering

```rust
renderer.render(&mut render_pass, viewport_size);
```

Process:
1. Upload vertex buffer (all text instances)
2. Bind atlas texture
3. Single draw call for all text
4. Clear draw commands for next frame

## Texture Atlas

### Structure

The font renderer maintains a dynamic texture atlas:

```rust
struct Atlas {
    texture: wgpu::Texture,
    width: u32,
    height: u32,
    allocator: AtlasAllocator,
    cache: HashMap<GlyphKey, AtlasRegion>,
}

struct GlyphKey {
    glyph_id: u16,
    font_id: FontId,
    size: u32,
    subpixel_x: u8,
    subpixel_y: u8,
}
```

### Allocation Strategy

1. **Initial size**: 1024x1024 RGBA (~4MB)
2. **Allocation**: Shelf-packing algorithm
3. **Growth**: Double size when full (2048x2048, etc.)
4. **Eviction**: LRU cache for rarely-used glyphs (future)

### Cache Management

```rust
// Check cache
if let Some(region) = atlas.cache.get(&glyph_key) {
    // Use cached glyph
    let uvs = region.uv_rect();
} else {
    // Rasterize and cache
    let bitmap = rasterize_glyph(glyph_key);
    let region = atlas.allocate(bitmap.width, bitmap.height)?;
    atlas.upload(region, &bitmap.data);
    atlas.cache.insert(glyph_key, region);
}
```

## Text Measurement

### Measuring Text

```rust
let text = Text::new("Measure me").size(24.0);
let (width, height) = renderer.measure_text(&text);

// With constraints
let text = Text::new("Long text...").size(16.0).max_width(200.0);
let (width, height) = renderer.measure_text(&text); // Width <= 200
```

**Cost**: Requires layout (~0.5ms), cache when possible.

### Metrics

```rust
let buffer = renderer.prepare(&text);
let metrics = buffer.metrics();

println!("Width: {}", metrics.width);
println!("Height: {}", metrics.height);
println!("Ascent: {}", metrics.ascent);   // Above baseline
println!("Descent: {}", metrics.descent); // Below baseline
println!("Line height: {}", metrics.line_height);
```

## Performance Characteristics

### Text Preparation

- **First time**: Layout + cache = ~1-2ms
- **From cache**: Buffer clone = ~0.1ms
- **Factor**: Text length, font complexity, wrapping

### Glyph Rasterization

- **Cache hit**: UV lookup = ~0.01ms
- **Cache miss**: Rasterize + upload = ~0.5ms per glyph
- **Amortized**: Most glyphs cached after warmup

### Rendering

- **Per text**: Record vertices = ~0.05ms
- **Draw call**: Single call for all text = ~0.5ms
- **GPU time**: ~0.2-1ms depending on text count

### Memory

- **FontSystem**: ~50MB (system fonts loaded)
- **Atlas**: 4MB initial (grows to 16MB+)
- **Per buffer**: ~1-5KB (layout data)

## Caching Strategies

### UI Text Caching

In `astrelis-ui`, text measurements are cached per-node:

```rust
pub struct UiNode {
    pub text_measurement: Option<(f32, f32)>,
    // ...
}
```

Only remeasure when:
- Text content changes
- Font size changes
- Max width constraint changes
- Font family/weight/style changes

### Buffer Reuse

Reuse buffers to avoid layout cost:

```rust
// Bad: Create new buffer every frame
loop {
    let buffer = renderer.prepare(&text);
    renderer.draw_text(&mut buffer, pos);
}

// Good: Reuse buffer
let mut buffer = renderer.prepare(&text);
loop {
    renderer.draw_text(&mut buffer, pos);
}

// Update content
buffer.set_text(&renderer, "New text");
```

## Complex Text Support

Cosmic-text handles complex scripts:

### Bidirectional Text

Automatic support for RTL languages (Arabic, Hebrew):

```rust
let text = Text::new("Hello مرحبا שלום");
// Automatically handles mixed LTR/RTL
```

### Shaping

Handles ligatures, kerning, contextual forms:

```rust
let text = Text::new("fi fl ffi ffl"); // Ligatures
let text = Text::new("WAVE");          // Kerning applied
```

### Emoji

Color emoji support (platform-dependent):

```rust
let text = Text::new("Hello 👋 World 🌍");
// Rendered with color emoji when available
```

## Font Loading

### System Fonts

Automatically loads all installed fonts:

```rust
let font_system = FontSystem::with_system_fonts();
// Fonts available immediately
```

**Platforms**:
- **Windows**: C:\Windows\Fonts
- **macOS**: /System/Library/Fonts, /Library/Fonts, ~/Library/Fonts
- **Linux**: /usr/share/fonts, /usr/local/share/fonts, ~/.fonts

### Custom Fonts

Load from file or memory:

```rust
// From file
font_system.db_mut().load_font_file("fonts/MyFont.ttf")?;

// From memory (embedded)
let font_data = include_bytes!("../assets/MyFont.ttf");
font_system.db_mut().load_font_data(font_data.to_vec());

// Use custom font
let text = Text::new("Custom Font")
    .family("MyFont")
    .size(24.0);
```

### Font Fallback

Automatic fallback when glyphs missing:

```rust
let text = Text::new("Hello 世界"); // Latin + Chinese
// System automatically selects fonts for missing glyphs
```

## Best Practices

### 1. Cache Text Buffers

Don't recreate buffers unnecessarily:

```rust
// Bad
loop {
    let buffer = renderer.prepare(&text);
    renderer.draw_text(&mut buffer, pos);
}

// Good
let mut buffer = renderer.prepare(&text);
loop {
    if text_changed {
        buffer = renderer.prepare(&text);
    }
    renderer.draw_text(&mut buffer, pos);
}
```

### 2. Batch Text Rendering

Draw multiple text instances before render:

```rust
// Prepare all text
let mut buffers = vec![];
for text in texts {
    buffers.push(renderer.prepare(text));
}

// Draw all text
for (buffer, pos) in buffers.iter_mut().zip(positions) {
    renderer.draw_text(buffer, pos);
}

// Single render call
renderer.render(&mut pass, viewport);
```

### 3. Prefer Static Text

Static text can be prepared once:

```rust
// Static UI labels
lazy_static! {
    static ref LABEL_BUFFER: TextBuffer = {
        renderer.prepare(&Text::new("Label").size(16.0))
    };
}
```

### 4. Profile Text Operations

Identify expensive operations:

```rust
{
    profile_scope!("text_prepare");
    let buffer = renderer.prepare(&text);
}
{
    profile_scope!("text_draw");
    renderer.draw_text(&mut buffer, pos);
}
```

### 5. Use Appropriate Font Sizes

Larger sizes use more atlas space:
- Small UI: 12-16px
- Body text: 16-20px
- Headers: 24-48px
- Avoid: 100px+ (use vector graphics instead)

## Integration with UI System

The UI system caches text measurements:

```rust
// In UiTree
pub fn measure_text(&mut self, node_id: NodeId, font_renderer: &FontRenderer) -> (f32, f32) {
    let node = &self.nodes[&node_id];
    
    // Return cached if not dirty
    if !node.dirty {
        if let Some(cached) = node.text_measurement {
            return cached;
        }
    }
    
    // Measure and cache
    let widget = &node.widget;
    let size = font_renderer.measure_text(/* ... */);
    self.nodes.get_mut(&node_id).unwrap().text_measurement = Some(size);
    size
}
```

## Troubleshooting

### Text Not Rendering

Check:
1. Font system initialized?
2. Font family exists?
3. Text color opaque? (alpha = 1.0)
4. Viewport size set correctly?
5. Render pass cleared?

### Text Blurry

Causes:
1. Non-integer positions (use `.round()`)
2. Size too small then scaled up
3. Wrong atlas format (use RGBA8Unorm)

### Performance Issues

Profile these areas:
1. Too many unique font sizes (atlas fragmentation)
2. Recreating buffers every frame
3. Large blocks of text (split into paragraphs)
4. Too many draw calls (batch text)

## Future Enhancements

1. **SDF rendering** - Resolution-independent text
2. **Text effects** - Outline, shadow, glow
3. **Rich text** - Inline formatting, colors, styles
4. **Text selection** - Interactive text editing
5. **Hyphenation** - Better wrapping for long words
6. **Variable fonts** - Interpolated font weights
7. **Multi-atlas** - Separate atlases per size range
8. **Streaming** - Load glyphs on demand
