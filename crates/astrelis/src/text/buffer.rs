use super::{
    FontId, TextAlign, TextError, TextFont, TextGlyph, TextLayout, TextLine, TextStyle, TextSystem,
    TextWrap, cosmic,
};
use std::{collections::HashMap, sync::Arc};

/// Retained UTF-8 text and paragraph constraints, evaluated explicitly on the CPU.
///
/// Setters do no shaping. [`Self::layout`] applies validated changes and returns a
/// shared immutable snapshot. Unchanged evaluation clones its Arc only; width,
/// wrap, alignment and default metric changes preserve backend shaped runs. Font
/// selection/features changes reshape. Content edits preserve matching leading and
/// trailing paragraphs and reshape changed paragraphs. Loading fonts or evaluating with a
/// different system also reshapes, avoiding stale fallback or face identities.
/// Every snapshot remains usable after later edits. One style per buffer is supported.
#[derive(Debug)]
pub struct TextBuffer {
    text: Arc<str>,
    style: TextStyle,
    width: Option<f32>,
    wrap: TextWrap,
    align: TextAlign,
    backend: cosmic::Buffer,
    evaluated_system: Option<(u64, u64)>,
    text_dirty: bool,
    layout_dirty: bool,
    cached: Option<Arc<TextLayout>>,
}
impl Default for TextBuffer {
    fn default() -> Self {
        Self::new()
    }
}
impl TextBuffer {
    /// Creates empty text with default 16/22-unit style, unconstrained width,
    /// word-or-glyph wrapping, and paragraph-start alignment. Does not require fonts.
    pub fn new() -> Self {
        let style = TextStyle::new();
        Self {
            text: Arc::from(""),
            backend: cosmic::Buffer::new_empty(cosmic::Metrics::new(
                style.font_size,
                style.line_height,
            )),
            style,
            width: None,
            wrap: TextWrap::default(),
            align: TextAlign::default(),
            evaluated_system: None,
            text_dirty: true,
            layout_dirty: true,
            cached: None,
        }
    }
    /// Current original content; explicit line ending bytes are preserved.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Current font style and local-unit metrics.
    pub fn style(&self) -> &TextStyle {
        &self.style
    }
    /// Current wrapping/alignment width constraint, distinct from measured advance width.
    pub fn width(&self) -> Option<f32> {
        self.width
    }
    /// Current wrap policy.
    pub fn wrap(&self) -> TextWrap {
        self.wrap
    }
    /// Current paragraph alignment.
    pub fn align(&self) -> TextAlign {
        self.align
    }
    /// Replaces text and style after validation, without shaping. Identical input
    /// retains the snapshot and text allocation. Rejected input changes neither.
    pub fn set_text(&mut self, text: &str, style: TextStyle) -> Result<(), TextError> {
        style.validate()?;
        if self.text.as_ref() != text {
            self.text = Arc::from(text);
            self.text_dirty = true;
            self.layout_dirty = true;
        }
        self.apply_style(style);
        Ok(())
    }
    /// Replaces style after validation; text stays unchanged. Metric-only changes
    /// relayout existing shaped runs; font selection/features/tracking changes reshape.
    pub fn set_style(&mut self, style: TextStyle) -> Result<(), TextError> {
        style.validate()?;
        self.apply_style(style);
        Ok(())
    }
    fn apply_style(&mut self, style: TextStyle) {
        if style != self.style {
            self.text_dirty |= !self.style.same_shaping(&style);
            self.layout_dirty = true;
            self.style = style;
        }
    }
    /// Selects a finite nonnegative width or `None`. Zero width is supported;
    /// indivisible glyph clusters can exceed it. This does not clip or limit height.
    pub fn set_width(&mut self, width: Option<f32>) -> Result<(), TextError> {
        if width.is_some_and(|w| !w.is_finite() || w < 0.) {
            return Err(TextError::InvalidWidth);
        }
        if self.width != width {
            self.width = width;
            self.layout_dirty = true;
        }
        Ok(())
    }
    /// Selects wrapping; backend shaped runs are reused on next evaluation.
    pub fn set_wrap(&mut self, wrap: TextWrap) {
        if self.wrap != wrap {
            self.wrap = wrap;
            self.layout_dirty = true;
        }
    }
    /// Selects alignment; backend shaped runs are reused on next evaluation.
    pub fn set_align(&mut self, align: TextAlign) {
        if self.align != align {
            self.align = align;
            self.layout_dirty = true;
        }
    }
    /// Evaluates all paragraphs and wrapped lines, returning retained local-unit
    /// metrics/glyphs. No viewport-height culling, pixel snapping, DPI or GPU work occurs.
    /// Missing glyphs are reported within a usable snapshot; an empty font database
    /// with nonempty paragraph content returns `NoFonts` before backend shaping.
    /// A previous snapshot remains valid if evaluation fails.
    pub fn layout(&mut self, system: &mut TextSystem) -> Result<Arc<TextLayout>, TextError> {
        let source = (system.id, system.generation);
        if self.evaluated_system == Some(source) && !self.layout_dirty && !self.text_dirty {
            return Ok(self.cached.as_ref().unwrap().clone());
        }
        if system.backend.db().is_empty() && self.text.chars().any(|c| c != '\r' && c != '\n') {
            return Err(TextError::NoFonts);
        }
        self.backend.set_metrics(cosmic::Metrics::new(
            self.style.font_size,
            self.style.line_height,
        ));
        self.backend.set_size(self.width, None);
        self.backend.set_wrap(self.wrap.backend());
        if self.evaluated_system != Some(source) {
            self.backend.set_text(
                &self.text,
                &self.style.attrs(),
                cosmic::Shaping::Advanced,
                self.align.backend(),
            );
        } else if self.text_dirty {
            update_paragraphs(&mut self.backend, &self.text, &self.style, self.align);
        } else {
            for line in &mut self.backend.lines {
                line.set_align(self.align.backend());
            }
        }
        let layout = if system.backend.db().is_empty() {
            // cosmic-text requests a default face even for empty paragraphs.
            // Blank text needs only line boxes and must work without a font.
            blank_snapshot(
                &self.backend,
                system.id,
                self.text.clone(),
                self.style.clone(),
            )?
        } else {
            self.backend.shape_until_scroll(&mut system.backend, false);
            snapshot(
                &self.backend,
                system,
                self.text.clone(),
                self.style.clone(),
                self.cached
                    .as_ref()
                    .map_or(0, |layout| layout.glyphs().len().min(self.text.len())),
            )?
        };
        let layout = Arc::new(layout);
        self.cached = Some(layout.clone());
        self.evaluated_system = Some(source);
        self.text_dirty = false;
        self.layout_dirty = false;
        Ok(layout)
    }
}

fn update_paragraphs(buffer: &mut cosmic::Buffer, text: &str, style: &TextStyle, align: TextAlign) {
    let mut paragraphs: Vec<_> = cosmic::LineIter::new(text)
        .map(|(range, ending)| (&text[range], ending))
        .collect();
    if paragraphs
        .last()
        .is_none_or(|(_, ending)| *ending != cosmic::LineEnding::None)
    {
        paragraphs.push(("", cosmic::LineEnding::None));
    }
    let matches = |line: &cosmic::BufferLine, &(text, ending): &(&str, cosmic::LineEnding)| {
        line.text() == text && line.ending() == ending
    };
    let prefix = buffer
        .lines
        .iter()
        .zip(&paragraphs)
        .take_while(|(a, b)| matches(a, b))
        .count();
    let suffix = buffer.lines[prefix..]
        .iter()
        .rev()
        .zip(paragraphs[prefix..].iter().rev())
        .take_while(|(a, b)| matches(a, b))
        .count();
    let old_end = buffer.lines.len() - suffix;
    let new_end = paragraphs.len() - suffix;
    let attrs = style.attrs();
    if old_end - prefix == new_end - prefix {
        // Common label/paragraph edits keep String and backend shaping scratch.
        for (line, &(text, ending)) in buffer.lines[prefix..old_end]
            .iter_mut()
            .zip(&paragraphs[prefix..new_end])
        {
            line.set_text(text, ending, cosmic::AttrsList::new(&attrs));
        }
    } else {
        // Move matching suffix paragraphs rather than invalidating them when lines
        // are inserted or removed. Source offsets are rebuilt by the snapshot.
        buffer.lines.splice(
            prefix..old_end,
            paragraphs[prefix..new_end].iter().map(|&(text, ending)| {
                cosmic::BufferLine::new(
                    text,
                    ending,
                    cosmic::AttrsList::new(&attrs),
                    cosmic::Shaping::Advanced,
                )
            }),
        );
    }
    for line in &mut buffer.lines {
        if line.attrs_list().defaults() != attrs {
            line.set_attrs_list(cosmic::AttrsList::new(&attrs));
        }
        line.set_align(align.backend());
    }
    // Mark structural edits for evaluation without globally invalidating cached
    // shaping/layout. Newly inserted cosmic lines are Empty rather than Unused,
    // so the backend's individual-invalidation scan alone would miss them.
    // No evaluation occurs at the temporary scroll position.
    buffer.set_scroll(cosmic::Scroll {
        line: 1,
        ..Default::default()
    });
    buffer.set_scroll(cosmic::Scroll::default());
}

fn blank_snapshot(
    buffer: &cosmic::Buffer,
    system_id: u64,
    text: Arc<str>,
    style: TextStyle,
) -> Result<TextLayout, TextError> {
    let mut lines = Vec::with_capacity(buffer.lines.len());
    let mut offset = 0;
    let mut top = 0.;
    for (paragraph_index, line) in buffer.lines.iter().enumerate() {
        let height = style.line_height;
        let baseline = top + height / 2.;
        if !baseline.is_finite() || !(top + height).is_finite() {
            return Err(TextError::LayoutOverflow);
        }
        lines.push(TextLine {
            paragraph_index,
            source: offset..offset,
            glyph_range: 0..0,
            top,
            baseline,
            height,
            width: 0.,
            rtl: false,
        });
        offset += line.ending().as_str().len();
        top += height;
    }
    debug_assert_eq!(offset, text.len());
    Ok(TextLayout {
        system_id,
        text,
        style,
        lines,
        glyphs: Vec::new(),
        fonts: Vec::new(),
        missing: Vec::new(),
        size: [0., top],
    })
}

fn snapshot(
    buffer: &cosmic::Buffer,
    system: &mut TextSystem,
    text: Arc<str>,
    style: TextStyle,
    glyph_capacity: usize,
) -> Result<TextLayout, TextError> {
    // Match the backend's explicit CR/LF/CRLF/LFCR splitting exactly, including a
    // terminal empty paragraph. This avoids normalizing away source byte offsets.
    let mut offset = 0;
    let mut paragraphs = Vec::with_capacity(buffer.lines.len());
    for line in &buffer.lines {
        paragraphs.push(offset..offset + line.text().len());
        offset += line.text().len() + line.ending().as_str().len();
    }
    debug_assert_eq!(offset, text.len());
    let mut output = TextLayout {
        system_id: system.id,
        text,
        style,
        glyphs: Vec::with_capacity(glyph_capacity),
        lines: Vec::with_capacity(buffer.lines.len()),
        fonts: Vec::new(),
        missing: Vec::new(),
        size: [0., 0.],
    };
    let mut font_indices = HashMap::new();
    let mut last_font = None;
    for run in buffer.layout_runs() {
        if [
            run.line_y,
            run.line_top,
            run.line_height,
            run.line_w,
            run.line_top + run.line_height,
        ]
        .iter()
        .any(|v| !v.is_finite())
        {
            return Err(TextError::LayoutOverflow);
        }
        let line_index = output.lines.len();
        let glyph_start = output.glyphs.len();
        let paragraph = &paragraphs[run.line_i];
        for glyph in run.glyphs {
            let key = (glyph.font_id, glyph.font_weight.0);
            let font_index = if let Some((last, index)) = last_font
                && last == key
            {
                index
            } else if let Some(&index) = font_indices.get(&key) {
                index
            } else {
                let font = system
                    .backend
                    .get_font(glyph.font_id, glyph.font_weight)
                    .ok_or(TextError::FontUnavailable)?;
                let face_index = system
                    .backend
                    .db()
                    .face(glyph.font_id)
                    .ok_or(TextError::FontUnavailable)?
                    .index;
                let index = output.fonts.len();
                output.fonts.push(TextFont {
                    id: FontId {
                        system: system.id,
                        face: glyph.font_id,
                    },
                    face_index,
                    weight: glyph.font_weight.0,
                    backend: font,
                });
                // Single-face labels need neither per-glyph hashing nor a hash
                // allocation. Add the first face only when a second one appears.
                if index == 1 {
                    let first = &output.fonts[0];
                    font_indices.insert((first.id.face, first.weight), 0);
                }
                if index > 0 {
                    font_indices.insert(key, index);
                }
                index
            };
            last_font = Some((key, font_index));
            let offset = [
                glyph.font_size * glyph.x_offset,
                -glyph.font_size * glyph.y_offset,
            ];
            let advance_origin = [glyph.x, run.line_y + glyph.y];
            let position = [advance_origin[0] + offset[0], advance_origin[1] + offset[1]];
            if position
                .into_iter()
                .chain(advance_origin)
                .chain(offset)
                .chain([glyph.w, glyph.font_size])
                .any(|v| !v.is_finite())
            {
                return Err(TextError::LayoutOverflow);
            }
            let cluster = paragraph.start + glyph.start..paragraph.start + glyph.end;
            debug_assert!(output.text.get(cluster.clone()).is_some());
            if glyph.glyph_id == 0 {
                output.missing.push(cluster.clone());
            }
            output.glyphs.push(TextGlyph {
                font_index,
                glyph_id: glyph.glyph_id,
                cluster,
                line_index,
                position,
                advance_origin,
                advance: glyph.w,
                offset,
                font_size: glyph.font_size,
                bidi_level: glyph.level.number(),
                synthetic_italic: glyph
                    .cache_key_flags
                    .contains(cosmic::CacheKeyFlags::FAKE_ITALIC),
            });
        }
        output.size[0] = output.size[0].max(run.line_w);
        output.size[1] = output.size[1].max(run.line_top + run.line_height);
        output.lines.push(TextLine {
            paragraph_index: run.line_i,
            source: paragraph.clone(),
            glyph_range: glyph_start..output.glyphs.len(),
            top: run.line_top,
            baseline: run.line_y,
            height: run.line_height,
            width: run.line_w,
            rtl: run.rtl,
        });
    }
    output.missing.sort_by_key(|range| (range.start, range.end));
    output.missing.dedup();
    Ok(output)
}
