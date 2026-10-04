//! Headless font loading, complex shaping, and retained layout. No window/GPU needed.
use astrelis::{TextAlign, TextBuffer, TextStyle, TextSystem, TextWrap};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut text = TextSystem::new();
    // Copy these licensed fonts with this example, or supply your own font files.
    text.load_font(include_bytes!("../tests/fonts/SourceSans3-Regular.otf"))?;
    text.load_font(include_bytes!("../tests/fonts/NotoSansArabic.ttf"))?;
    for face in text.fonts() {
        println!("Loaded {:?}, face {}", face.families, face.face_index);
    }

    let mut paragraph = TextBuffer::new();
    paragraph.set_text(
        "Hello, office! العربية 123\nCombining marks: e\u{301}. Fonts fall back per script.",
        TextStyle::new()
            .family("Source Sans 3")
            .font_size(20.)
            .line_height(28.),
    )?;
    paragraph.set_width(Some(280.))?;
    paragraph.set_wrap(TextWrap::WordOrGlyph);
    let layout = paragraph.layout(&mut text)?;
    println!(
        "Measured {:?} local units; {} lines, {} glyphs, {} selected faces",
        layout.size(),
        layout.lines().len(),
        layout.glyphs().len(),
        layout.fonts().len()
    );
    for line in layout.lines() {
        println!(
            "Paragraph {}, baseline {:.2}, width {:.2}, rtl={}, source {:?}",
            line.paragraph_index, line.baseline, line.width, line.rtl, line.source
        );
        for glyph in &layout.glyphs()[line.glyph_range.clone()] {
            println!(
                "  glyph {:4}, font {}, source {:?} {:?}, origin {:?}, bidi {}",
                glyph.glyph_id,
                glyph.font_index,
                glyph.cluster,
                &layout.text()[glyph.cluster.clone()],
                glyph.position,
                glyph.bidi_level
            );
        }
    }
    println!("Unresolved clusters: {:?}", layout.missing_glyphs());
    println!(
        "Unchanged evaluation reuses snapshot: {}",
        Arc::ptr_eq(&layout, &paragraph.layout(&mut text)?)
    );

    paragraph.set_width(Some(160.))?;
    paragraph.set_align(TextAlign::Center);
    let reflowed = paragraph.layout(&mut text)?;
    println!(
        "Narrow centered layout: {:?}, {} lines; previous snapshot still has {} lines",
        reflowed.size(),
        reflowed.lines().len(),
        layout.lines().len()
    );
    Ok(())
}
