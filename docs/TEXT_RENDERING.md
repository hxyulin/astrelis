# Text Rendering in Astrelis

Astrelis uses `cosmic-text` for text rendering, providing:
- Complex text shaping (ligatures, kerning)
- Bidirectional text (Arabic, Hebrew)
- Full CJK support (Chinese, Japanese, Korean)
- Dynamic glyph caching
- Line breaking and text layout

## Quick Start

### 1. Add Font Assets

```rust
use astrelis_core::{assets::Handle, text::Font};

// Load font from file
let font_data = std::fs::read("assets/fonts/NotoSans-Regular.ttf").unwrap();
let mut font = Font::from_bytes("Noto Sans".to_string(), font_data);

// Or use Font::from_file()
let mut font = Font::from_file("assets/fonts/NotoSans-Regular.ttf").unwrap();

// Add to asset manager
let font_handle: Handle<Font> = ctx.engine_mut().assets.add(font);
```

### 2. Register Fonts with FontSystem

```rust
use cosmic_text::FontSystem;

// Create FontSystem (should be stored in your app state)
let mut font_system = FontSystem::new();

// Register fonts
if let Some(font) = ctx.engine().assets.get_mut(font_handle) {
    font.register(&mut font_system);
}
```

### 3. Create Text Buffers

```rust
use astrelis_core::text::TextBufferBuilder;

let buffer = TextBufferBuilder::new("Hello, World!")
    .font_size(24.0)
    .line_height(1.4)
    .font_family("Noto Sans")
    .build(&mut font_system);

// For CJK text
let cjk_buffer = TextBufferBuilder::new("こんにちは世界")
    .font_size(32.0)
    .font_family("Noto Sans CJK JP")
    .build(&mut font_system);

// For mixed scripts
let mixed = TextBufferBuilder::new("Hello 你好 مرحبا")
    .font_size(20.0)
    .build(&mut font_system);
```

## CJK Font Recommendations

### Open Source Fonts

1. **Noto Sans CJK** (Google)
   - Covers all CJK characters
   - Multiple weights
   - https://github.com/notofonts/noto-cjk

2. **Source Han Sans** (Adobe)
   - Same as Noto Sans CJK (collaboration)
   - Professional quality

3. **M PLUS** (Japanese)
   - Good for UI
   - Free license

### Loading Multiple Fonts

```rust
// Load base font
let latin_font = Font::from_file("fonts/Roboto-Regular.ttf")?;
let latin_handle = assets.add(latin_font);

// Load CJK font for fallback
let cjk_font = Font::from_file("fonts/NotoSansCJK-Regular.ttf")?;
let cjk_handle = assets.add(cjk_font);

// Register both
if let Some(font) = assets.get_mut(latin_handle) {
    font.register(&mut font_system);
}
if let Some(font) = assets.get_mut(cjk_handle) {
    font.register(&mut font_system);
}

// cosmic-text will automatically use fallback fonts
```

## Advanced Usage

### Text Metrics and Layout

```rust
use cosmic_text::{Attrs, Buffer, Metrics, Shaping};

let metrics = Metrics::new(20.0, 24.0); // font_size, line_height
let mut buffer = Buffer::new(&mut font_system, metrics);

// Set width for wrapping
buffer.set_size(&mut font_system, Some(400.0), None);

// Add text with attributes
buffer.set_text(
    &mut font_system,
    "Your text here",
    Attrs::new().family(cosmic_text::Family::Name("Noto Sans")),
    Shaping::Advanced, // Full text shaping
);

// Get layout info
for run in buffer.layout_runs() {
    for glyph in run.glyphs.iter() {
        println!("Glyph: {:?} at ({}, {})", glyph.cache_key, glyph.x, glyph.y);
    }
}
```

### Rendering Glyphs

```rust
use cosmic_text::SwashCache;

// Create glyph rasterization cache
let mut swash_cache = SwashCache::new();

// Rasterize glyphs
for run in buffer.layout_runs() {
    for glyph in run.glyphs.iter() {
        swash_cache.with_pixels(
            &mut font_system,
            glyph.cache_key,
            glyph.color,
            |x, y, w, h, pixels| {
                // Upload pixels to texture atlas
                // x, y: position in atlas
                // w, h: glyph dimensions
                // pixels: RGBA8 bitmap data
            },
        );
    }
}
```

## Performance Tips

### 1. Cache Text Buffers
Don't recreate buffers every frame for static text:

```rust
struct TextCache {
    buffers: HashMap<String, Buffer>,
}

impl TextCache {
    fn get_or_create(&mut self, text: &str, font_system: &mut FontSystem) -> &Buffer {
        self.buffers.entry(text.to_string()).or_insert_with(|| {
            TextBufferBuilder::new(text)
                .font_size(16.0)
                .build(font_system)
        })
    }
}
```

### 2. Batch Text Rendering
Render all text in a single pass to minimize state changes.

### 3. Use SwashCache
`SwashCache` automatically caches rasterized glyphs - reuse it across frames.

### 4. Lazy Font Registration
Only register fonts when they're actually used:

```rust
fn ensure_font_registered(
    font: &mut Font,
    font_system: &mut FontSystem,
) {
    if !font.is_registered() {
        font.register(font_system);
    }
}
```

## Common Patterns

### UI Text Component

```rust
struct TextComponent {
    content: String,
    font_size: f32,
    color: Color,
    buffer: Option<Buffer>,
}

impl TextComponent {
    fn update_buffer(&mut self, font_system: &mut FontSystem) {
        self.buffer = Some(
            TextBufferBuilder::new(&self.content)
                .font_size(self.font_size)
                .build(font_system)
        );
    }
}
```

### Internationalization

```rust
// Different languages automatically work
let texts = vec![
    "English text",
    "日本語テキスト",
    "中文文本",
    "한국어 텍스트",
    "النص العربي",
    "Texto en español",
];

for text in texts {
    let buffer = TextBufferBuilder::new(text)
        .font_size(18.0)
        .build(&mut font_system);
    // All languages render correctly with proper fonts
}
```

## Next Steps

To implement a complete text renderer:

1. **Create texture atlas** - Pack glyph bitmaps into GPU textures
2. **Build glyph cache** - Track which glyphs are in which atlas
3. **Generate quads** - Create geometry for each glyph
4. **Render pass** - Draw textured quads with proper blending

Consider using `glyphon` crate for a complete wgpu-based text renderer built on cosmic-text.

## Resources

- [cosmic-text docs](https://docs.rs/cosmic-text/)
- [cosmic-text examples](https://github.com/pop-os/cosmic-text/tree/master/examples)
- [Text Rendering Hates You](https://faultlore.com/blah/text-hates-you/) - Why text is hard
- [Noto CJK Fonts](https://github.com/notofonts/noto-cjk)

## Troubleshooting

### CJK Characters Show as Boxes
- Ensure you've loaded a CJK font
- Verify font is registered: `font.is_registered()`
- Check font family name matches

### Poor Performance with Large Text
- Use `SwashCache` to avoid re-rasterizing
- Cache `Buffer` objects for static text
- Consider using SDF (Signed Distance Field) fonts for scaling

### Text Layout Issues
- Use `Shaping::Advanced` for complex scripts
- Set proper buffer width for text wrapping
- Check font contains required glyphs