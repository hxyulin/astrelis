//! Backend-independent font discovery, shaping, and retained text layout.

#![warn(missing_docs)]

use std::{
    borrow::Cow,
    error::Error,
    fmt,
    ops::Range,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use astrelis_core::{
    color::Color,
    geometry::{LogicalPoint, LogicalRect, LogicalSize, Point, Rect, Size},
};
use parley::{
    Alignment as ParleyAlignment, AlignmentOptions, FontContext, FontFamily as ParleyFontFamily,
    FontFamilyName, FontStyle as ParleyFontStyle, FontWeight as ParleyFontWeight, FontWidth,
    GenericFamily, Layout as ParleyLayout, LayoutContext as ParleyLayoutContext,
    LineHeight as ParleyLineHeight, PositionedLayoutItem, StyleProperty, TextWrapMode,
    editing::{Cursor, Selection},
    fontique::{Blob, Collection, CollectionOptions, SourceCache},
    layout::Affinity as ParleyAffinity,
};

static NEXT_LAYOUT_ID: AtomicU64 = AtomicU64::new(1);

/// Failure produced while registering fonts or constructing text layout data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextError(String);

impl TextError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for TextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Error for TextError {}

/// Options controlling initial font discovery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FontDatabaseOptions {
    /// Discover fonts installed on the host operating system.
    pub system_fonts: bool,
}

impl Default for FontDatabaseOptions {
    fn default() -> Self {
        Self { system_fonts: true }
    }
}

/// Application-level font database and loaded-font cache.
pub struct FontDatabase {
    context: FontContext,
}

impl FontDatabase {
    /// Creates a database using the requested system-font policy.
    pub fn new(options: FontDatabaseOptions) -> Self {
        Self {
            context: FontContext {
                collection: Collection::new(CollectionOptions {
                    shared: false,
                    system_fonts: options.system_fonts,
                }),
                source_cache: SourceCache::default(),
            },
        }
    }

    /// Creates a deterministic database containing no system fonts.
    pub fn empty() -> Self {
        Self::new(FontDatabaseOptions {
            system_fonts: false,
        })
    }

    /// Registers every face contained in an OpenType font blob.
    ///
    /// Returns the number of faces that were accepted.
    pub fn register_font(&mut self, data: impl Into<Arc<[u8]>>) -> Result<usize, TextError> {
        let data = data.into();
        if data.is_empty() {
            return Err(TextError::new("font data must not be empty"));
        }
        let registered = self
            .context
            .collection
            .register_fonts(Blob::new(Arc::new(data)), None);
        let count = registered.iter().map(|(_, fonts)| fonts.len()).sum();
        if count == 0 {
            Err(TextError::new("font data contains no usable faces"))
        } else {
            Ok(count)
        }
    }

    /// Loads system fonts if they were not loaded when the database was created.
    pub fn load_system_fonts(&mut self) {
        self.context.collection.load_system_fonts();
    }

    /// Returns the currently known family names.
    pub fn family_names(&mut self) -> Vec<String> {
        self.context
            .collection
            .family_names()
            .map(str::to_owned)
            .collect()
    }
}

impl Default for FontDatabase {
    fn default() -> Self {
        Self::new(FontDatabaseOptions::default())
    }
}

/// Generic or explicitly named font family.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FontFamily {
    /// An explicitly named family.
    Named(String),
    /// Platform sans-serif family.
    SansSerif,
    /// Platform serif family.
    Serif,
    /// Platform monospace family.
    Monospace,
    /// Platform system UI family.
    SystemUi,
    /// Platform emoji family.
    Emoji,
}

impl FontFamily {
    fn to_parley(&self) -> FontFamilyName<'static> {
        match self {
            Self::Named(name) => FontFamilyName::Named(Cow::Owned(name.clone())),
            Self::SansSerif => GenericFamily::SansSerif.into(),
            Self::Serif => GenericFamily::Serif.into(),
            Self::Monospace => GenericFamily::Monospace.into(),
            Self::SystemUi => GenericFamily::SystemUi.into(),
            Self::Emoji => GenericFamily::Emoji.into(),
        }
    }
}

/// Requested font slope.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum FontSlant {
    /// Upright text.
    #[default]
    Normal,
    /// Italic face.
    Italic,
    /// Synthesized or native oblique face.
    Oblique,
}

/// Line-height policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineHeight {
    /// Use the font's preferred line height multiplied by this value.
    FontMetrics(f32),
    /// Use the font size multiplied by this value.
    FontSize(f32),
    /// Use an absolute logical height.
    Absolute(f32),
}

impl Default for LineHeight {
    fn default() -> Self {
        Self::FontMetrics(1.0)
    }
}

/// Complete style used as the default for a text layout.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    /// Ordered font fallback families.
    pub families: Vec<FontFamily>,
    /// Logical font size.
    pub size: f32,
    /// OpenType/CSS weight, normally in `1..=1000`.
    pub weight: f32,
    /// Width ratio where `1.0` is normal.
    pub stretch: f32,
    /// Requested slope.
    pub slant: FontSlant,
    /// Glyph color in linear space.
    pub color: Color,
    /// Draw an underline.
    pub underline: bool,
    /// Draw a strikethrough.
    pub strikethrough: bool,
    /// Line-height policy.
    pub line_height: LineHeight,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            families: vec![FontFamily::SansSerif],
            size: 16.0,
            weight: 400.0,
            stretch: 1.0,
            slant: FontSlant::Normal,
            color: Color::WHITE,
            underline: false,
            strikethrough: false,
            line_height: LineHeight::default(),
        }
    }
}

/// Optional ranged overrides applied to the default text style.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextStylePatch {
    /// Replacement ordered font families.
    pub families: Option<Vec<FontFamily>>,
    /// Replacement logical font size.
    pub size: Option<f32>,
    /// Replacement weight.
    pub weight: Option<f32>,
    /// Replacement width ratio.
    pub stretch: Option<f32>,
    /// Replacement slope.
    pub slant: Option<FontSlant>,
    /// Replacement glyph color.
    pub color: Option<Color>,
    /// Replacement underline state.
    pub underline: Option<bool>,
    /// Replacement strikethrough state.
    pub strikethrough: Option<bool>,
    /// Replacement line-height policy.
    pub line_height: Option<LineHeight>,
}

/// One non-overlapping UTF-8 byte range with style overrides.
#[derive(Clone, Debug, PartialEq)]
pub struct TextSpan {
    /// UTF-8 byte range.
    pub range: Range<usize>,
    /// Overrides active for the range.
    pub style: TextStylePatch,
}

/// Soft wrapping behavior.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextWrap {
    /// Wrap at Unicode line-breaking opportunities.
    #[default]
    Wrap,
    /// Only break at explicit line endings.
    NoWrap,
}

/// Horizontal paragraph alignment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlignment {
    /// Direction-aware leading edge.
    #[default]
    Start,
    /// Center each line.
    Center,
    /// Direction-aware trailing edge.
    End,
    /// Expand eligible whitespace.
    Justify,
}

/// Paragraph layout options.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParagraphStyle {
    /// Maximum logical line width, or no constraint.
    pub max_width: Option<f32>,
    /// Soft wrapping behavior.
    pub wrap: TextWrap,
    /// Horizontal alignment.
    pub alignment: TextAlignment,
}

impl Default for ParagraphStyle {
    fn default() -> Self {
        Self {
            max_width: None,
            wrap: TextWrap::Wrap,
            alignment: TextAlignment::Start,
        }
    }
}

/// Input used to construct one retained layout.
#[derive(Clone, Debug, PartialEq)]
pub struct TextLayoutRequest {
    /// UTF-8 text.
    pub text: String,
    /// Default style.
    pub style: TextStyle,
    /// Sorted, non-overlapping ranged overrides.
    pub spans: Vec<TextSpan>,
    /// Paragraph settings.
    pub paragraph: ParagraphStyle,
}

impl TextLayoutRequest {
    /// Creates a request with default styling and paragraph settings.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: TextStyle::default(),
            spans: Vec::new(),
            paragraph: ParagraphStyle::default(),
        }
    }
}

/// Reusable scratch context for shaping and constructing layouts.
pub struct TextLayoutContext {
    inner: ParleyLayoutContext<Color>,
}

impl TextLayoutContext {
    /// Creates an empty reusable context.
    pub fn new() -> Self {
        Self {
            inner: ParleyLayoutContext::new(),
        }
    }

    /// Shapes, breaks, aligns, and freezes one text layout.
    pub fn layout(
        &mut self,
        fonts: &mut FontDatabase,
        request: TextLayoutRequest,
    ) -> Result<TextLayout, TextError> {
        validate_request(&request)?;
        let mut builder = self
            .inner
            .ranged_builder(&mut fonts.context, &request.text, 1.0, false);
        push_default_style(&mut builder, &request.style);
        builder.push_default(StyleProperty::TextWrapMode(match request.paragraph.wrap {
            TextWrap::Wrap => TextWrapMode::Wrap,
            TextWrap::NoWrap => TextWrapMode::NoWrap,
        }));
        for span in &request.spans {
            push_patch(&mut builder, &span.style, span.range.clone());
        }
        let mut layout = builder.build(&request.text);
        layout.break_all_lines(match request.paragraph.wrap {
            TextWrap::Wrap => request.paragraph.max_width,
            TextWrap::NoWrap => None,
        });
        layout.align(
            match request.paragraph.alignment {
                TextAlignment::Start => ParleyAlignment::Start,
                TextAlignment::Center => ParleyAlignment::Center,
                TextAlignment::End => ParleyAlignment::End,
                TextAlignment::Justify => ParleyAlignment::Justify,
            },
            AlignmentOptions::default(),
        );
        Ok(TextLayout::from_parley(request.text.into(), layout))
    }
}

impl Default for TextLayoutContext {
    fn default() -> Self {
        Self::new()
    }
}

fn push_default_style(builder: &mut parley::RangedBuilder<'_, Color>, style: &TextStyle) {
    builder.push_default(StyleProperty::FontFamily(parley_families(&style.families)));
    builder.push_default(StyleProperty::FontSize(style.size));
    builder.push_default(StyleProperty::FontWeight(ParleyFontWeight::new(
        style.weight,
    )));
    builder.push_default(StyleProperty::FontWidth(FontWidth::from_ratio(
        style.stretch,
    )));
    builder.push_default(StyleProperty::FontStyle(parley_slant(style.slant)));
    builder.push_default(StyleProperty::Brush(style.color));
    builder.push_default(StyleProperty::Underline(style.underline));
    builder.push_default(StyleProperty::Strikethrough(style.strikethrough));
    builder.push_default(StyleProperty::LineHeight(parley_line_height(
        style.line_height,
    )));
}

fn push_patch(
    builder: &mut parley::RangedBuilder<'_, Color>,
    patch: &TextStylePatch,
    range: Range<usize>,
) {
    if let Some(value) = &patch.families {
        builder.push(
            StyleProperty::FontFamily(parley_families(value)),
            range.clone(),
        );
    }
    if let Some(value) = patch.size {
        builder.push(StyleProperty::FontSize(value), range.clone());
    }
    if let Some(value) = patch.weight {
        builder.push(
            StyleProperty::FontWeight(ParleyFontWeight::new(value)),
            range.clone(),
        );
    }
    if let Some(value) = patch.stretch {
        builder.push(
            StyleProperty::FontWidth(FontWidth::from_ratio(value)),
            range.clone(),
        );
    }
    if let Some(value) = patch.slant {
        builder.push(StyleProperty::FontStyle(parley_slant(value)), range.clone());
    }
    if let Some(value) = patch.color {
        builder.push(StyleProperty::Brush(value), range.clone());
    }
    if let Some(value) = patch.underline {
        builder.push(StyleProperty::Underline(value), range.clone());
    }
    if let Some(value) = patch.strikethrough {
        builder.push(StyleProperty::Strikethrough(value), range.clone());
    }
    if let Some(value) = patch.line_height {
        builder.push(StyleProperty::LineHeight(parley_line_height(value)), range);
    }
}

fn parley_families(families: &[FontFamily]) -> ParleyFontFamily<'static> {
    let families = if families.is_empty() {
        vec![FontFamily::SansSerif.to_parley()]
    } else {
        families.iter().map(FontFamily::to_parley).collect()
    };
    ParleyFontFamily::List(Cow::Owned(families))
}

fn parley_slant(value: FontSlant) -> ParleyFontStyle {
    match value {
        FontSlant::Normal => ParleyFontStyle::Normal,
        FontSlant::Italic => ParleyFontStyle::Italic,
        FontSlant::Oblique => ParleyFontStyle::Oblique(None),
    }
}

fn parley_line_height(value: LineHeight) -> ParleyLineHeight {
    match value {
        LineHeight::FontMetrics(value) => ParleyLineHeight::MetricsRelative(value),
        LineHeight::FontSize(value) => ParleyLineHeight::FontSizeRelative(value),
        LineHeight::Absolute(value) => ParleyLineHeight::Absolute(value),
    }
}

fn validate_request(request: &TextLayoutRequest) -> Result<(), TextError> {
    validate_style(&request.style)?;
    if request
        .paragraph
        .max_width
        .is_some_and(|width| !width.is_finite() || width < 0.0)
    {
        return Err(TextError::new(
            "maximum width must be finite and non-negative",
        ));
    }
    let mut previous_end = 0;
    for span in &request.spans {
        if span.range.start > span.range.end
            || span.range.end > request.text.len()
            || !request.text.is_char_boundary(span.range.start)
            || !request.text.is_char_boundary(span.range.end)
        {
            return Err(TextError::new(
                "text span ranges must be valid UTF-8 byte boundaries",
            ));
        }
        if span.range.start < previous_end {
            return Err(TextError::new(
                "text spans must be sorted and non-overlapping",
            ));
        }
        validate_patch(&span.style)?;
        previous_end = span.range.end;
    }
    Ok(())
}

fn validate_style(style: &TextStyle) -> Result<(), TextError> {
    validate_positive(style.size, "font size")?;
    validate_positive(style.stretch, "font stretch")?;
    validate_weight(style.weight)?;
    validate_color(style.color)?;
    validate_line_height(style.line_height)
}

fn validate_patch(patch: &TextStylePatch) -> Result<(), TextError> {
    if let Some(value) = patch.size {
        validate_positive(value, "font size")?;
    }
    if let Some(value) = patch.stretch {
        validate_positive(value, "font stretch")?;
    }
    if let Some(value) = patch.weight {
        validate_weight(value)?;
    }
    if let Some(value) = patch.color {
        validate_color(value)?;
    }
    if let Some(value) = patch.line_height {
        validate_line_height(value)?;
    }
    Ok(())
}

fn validate_positive(value: f32, name: &str) -> Result<(), TextError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(TextError::new(format!(
            "{name} must be finite and positive"
        )))
    }
}

fn validate_weight(value: f32) -> Result<(), TextError> {
    if value.is_finite() && (1.0..=1000.0).contains(&value) {
        Ok(())
    } else {
        Err(TextError::new(
            "font weight must be finite and within 1..=1000",
        ))
    }
}

fn validate_color(value: Color) -> Result<(), TextError> {
    if [value.r, value.g, value.b, value.a]
        .into_iter()
        .all(f32::is_finite)
    {
        Ok(())
    } else {
        Err(TextError::new("text color components must be finite"))
    }
}

fn validate_line_height(value: LineHeight) -> Result<(), TextError> {
    let value = match value {
        LineHeight::FontMetrics(value)
        | LineHeight::FontSize(value)
        | LineHeight::Absolute(value) => value,
    };
    validate_positive(value, "line height")
}

/// Affinity used when a byte boundary has two visual positions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Affinity {
    /// Associate with content before the byte boundary.
    Upstream,
    /// Associate with content after the byte boundary.
    #[default]
    Downstream,
}

/// Stable caret position within a retained layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextPosition {
    /// UTF-8 byte index.
    pub byte_index: usize,
    /// Visual affinity at directional or line boundaries.
    pub affinity: Affinity,
}

/// Direction in which to move a caret.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CaretMovement {
    /// Previous visual grapheme boundary.
    VisualLeft,
    /// Next visual grapheme boundary.
    VisualRight,
    /// Previous visual line.
    LineUp,
    /// Next visual line.
    LineDown,
    /// Start of the current visual line.
    LineStart,
    /// End of the current visual line.
    LineEnd,
}

/// Result of mapping a logical point into text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitTest {
    /// Nearest valid text position.
    pub position: TextPosition,
    /// Whether the point was within the layout's logical bounds.
    pub is_inside: bool,
}

/// Resolved shareable font face used by a glyph run.
#[derive(Clone)]
pub struct FontFace {
    data: parley::FontData,
}

impl FontFace {
    /// Stable identity for renderer caches.
    pub fn cache_id(&self) -> (u64, u32) {
        (self.data.data.id(), self.data.index)
    }

    /// Index within a font collection.
    pub fn index(&self) -> u32 {
        self.data.index
    }

    /// Raw OpenType bytes.
    #[doc(hidden)]
    pub fn data(&self) -> &[u8] {
        self.data.data.data()
    }
}

impl fmt::Debug for FontFace {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FontFace")
            .field("cache_id", &self.cache_id())
            .finish()
    }
}

/// One positioned glyph.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    /// Font-local glyph identifier.
    pub id: u32,
    /// Logical glyph origin relative to the layout origin.
    pub position: LogicalPoint,
    /// Logical advance.
    pub advance: f32,
}

/// One visual glyph run sharing a face and style.
#[derive(Clone, Debug)]
pub struct GlyphRun {
    /// Resolved font face.
    pub font: FontFace,
    /// Logical font size.
    pub font_size: f32,
    /// Normalized variation coordinates.
    pub normalized_coords: Arc<[i16]>,
    /// Original UTF-8 byte range.
    pub text_range: Range<usize>,
    /// Whether the run is right-to-left.
    pub is_rtl: bool,
    /// Glyph color.
    pub color: Color,
    /// Positioned glyphs.
    pub glyphs: Arc<[Glyph]>,
    /// Optional underline rectangle.
    pub underline: Option<LogicalRect>,
    /// Optional strikethrough rectangle.
    pub strikethrough: Option<LogicalRect>,
}

/// Metrics for one broken line.
#[derive(Clone, Debug, PartialEq)]
pub struct TextLine {
    /// UTF-8 byte range covered by the line.
    pub text_range: Range<usize>,
    /// Baseline offset from the layout origin.
    pub baseline: f32,
    /// Logical line rectangle.
    pub bounds: LogicalRect,
}

struct TextLayoutData {
    id: u64,
    text: Arc<str>,
    layout: ParleyLayout<Color>,
    size: LogicalSize,
    lines: Arc<[TextLine]>,
    runs: Arc<[GlyphRun]>,
}

/// Immutable retained text layout shared by measurement, hit testing, and painting.
#[derive(Clone)]
pub struct TextLayout(Arc<TextLayoutData>);

impl TextLayout {
    fn from_parley(text: Arc<str>, layout: ParleyLayout<Color>) -> Self {
        let lines = layout
            .lines()
            .map(|line| {
                let metrics = line.metrics();
                TextLine {
                    text_range: line.text_range(),
                    baseline: metrics.baseline,
                    bounds: Rect::from_xywh(
                        metrics.inline_min_coord + metrics.offset,
                        metrics.block_min_coord,
                        metrics.inline_max_coord - metrics.inline_min_coord,
                        metrics.block_max_coord - metrics.block_min_coord,
                    ),
                }
            })
            .collect::<Vec<_>>();
        let mut runs = Vec::new();
        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(run) = item else {
                    continue;
                };
                let source = run.run();
                let style = run.style();
                let metrics = source.metrics();
                let underline = style.underline.as_ref().map(|decoration| {
                    let offset = decoration.offset.unwrap_or(metrics.underline_offset);
                    let size = decoration.size.unwrap_or(metrics.underline_size);
                    Rect::from_xywh(run.offset(), run.baseline() + offset, run.advance(), size)
                });
                let strikethrough = style.strikethrough.as_ref().map(|decoration| {
                    let offset = decoration.offset.unwrap_or(metrics.strikethrough_offset);
                    let size = decoration.size.unwrap_or(metrics.strikethrough_size);
                    Rect::from_xywh(run.offset(), run.baseline() + offset, run.advance(), size)
                });
                runs.push(GlyphRun {
                    font: FontFace {
                        data: source.font().clone(),
                    },
                    font_size: source.font_size(),
                    normalized_coords: Arc::from(source.normalized_coords()),
                    text_range: source.text_range(),
                    is_rtl: source.is_rtl(),
                    color: style.brush,
                    glyphs: run
                        .positioned_glyphs()
                        .map(|glyph| Glyph {
                            id: glyph.id,
                            position: Point::new(glyph.x, glyph.y),
                            advance: glyph.advance,
                        })
                        .collect(),
                    underline,
                    strikethrough,
                });
            }
        }
        let size = Size::new(layout.full_width(), layout.height());
        Self(Arc::new(TextLayoutData {
            id: NEXT_LAYOUT_ID.fetch_add(1, Ordering::Relaxed),
            text,
            layout,
            size,
            lines: lines.into(),
            runs: runs.into(),
        }))
    }

    /// Original UTF-8 text.
    pub fn text(&self) -> &str {
        &self.0.text
    }

    /// Logical layout size.
    pub fn size(&self) -> LogicalSize {
        self.0.size
    }

    /// Broken lines.
    pub fn lines(&self) -> &[TextLine] {
        &self.0.lines
    }

    /// Positioned visual glyph runs.
    pub fn glyph_runs(&self) -> &[GlyphRun] {
        &self.0.runs
    }

    /// Returns whether the dominant paragraph direction is right-to-left.
    pub fn is_rtl(&self) -> bool {
        self.0.layout.is_rtl()
    }

    /// Maps a logical point to the nearest valid grapheme boundary.
    pub fn hit_test(&self, point: LogicalPoint) -> HitTest {
        let cursor = Cursor::from_point(&self.0.layout, point.x, point.y);
        HitTest {
            position: from_cursor(cursor),
            is_inside: Rect::new(Point::ZERO, self.size()).contains(point),
        }
    }

    /// Returns visual caret geometry with the requested logical width.
    pub fn caret_rect(&self, position: TextPosition, width: f32) -> LogicalRect {
        let cursor = to_cursor(&self.0.layout, position);
        let bounds = cursor.geometry(&self.0.layout, width.max(0.0));
        Rect::from_xywh(
            bounds.x0 as f32,
            bounds.y0 as f32,
            (bounds.x1 - bounds.x0) as f32,
            (bounds.y1 - bounds.y0) as f32,
        )
    }

    /// Moves a caret through visual grapheme or line boundaries.
    pub fn move_caret(&self, position: TextPosition, movement: CaretMovement) -> TextPosition {
        let selection = Selection::from(to_cursor(&self.0.layout, position));
        let moved = match movement {
            CaretMovement::VisualLeft => selection.previous_visual(&self.0.layout, false),
            CaretMovement::VisualRight => selection.next_visual(&self.0.layout, false),
            CaretMovement::LineUp => selection.previous_line(&self.0.layout, false),
            CaretMovement::LineDown => selection.next_line(&self.0.layout, false),
            CaretMovement::LineStart => selection.line_start(&self.0.layout, false),
            CaretMovement::LineEnd => selection.line_end(&self.0.layout, false),
        };
        from_cursor(moved.focus())
    }

    /// Returns visual rectangles for a logical selection.
    pub fn selection_rects(&self, anchor: TextPosition, focus: TextPosition) -> Vec<LogicalRect> {
        Selection::new(
            to_cursor(&self.0.layout, anchor),
            to_cursor(&self.0.layout, focus),
        )
        .geometry(&self.0.layout)
        .into_iter()
        .map(|(bounds, _)| {
            Rect::from_xywh(
                bounds.x0 as f32,
                bounds.y0 as f32,
                (bounds.x1 - bounds.x0) as f32,
                (bounds.y1 - bounds.y0) as f32,
            )
        })
        .collect()
    }

    /// Immutable identity used by display-list and renderer caches.
    #[doc(hidden)]
    pub fn cache_id(&self) -> u64 {
        self.0.id
    }
}

impl fmt::Debug for TextLayout {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TextLayout")
            .field("text", &self.text())
            .field("size", &self.size())
            .field("lines", &self.lines())
            .field("runs", &self.glyph_runs())
            .finish()
    }
}

fn to_cursor(layout: &ParleyLayout<Color>, position: TextPosition) -> Cursor {
    Cursor::from_byte_index(
        layout,
        position.byte_index,
        match position.affinity {
            Affinity::Upstream => ParleyAffinity::Upstream,
            Affinity::Downstream => ParleyAffinity::Downstream,
        },
    )
}

fn from_cursor(cursor: Cursor) -> TextPosition {
    TextPosition {
        byte_index: cursor.index(),
        affinity: match cursor.affinity() {
            ParleyAffinity::Upstream => Affinity::Upstream,
            ParleyAffinity::Downstream => Affinity::Downstream,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_spans_before_font_resolution() {
        let mut fonts = FontDatabase::empty();
        let mut context = TextLayoutContext::new();
        let mut request = TextLayoutRequest::new("é");
        request.spans.push(TextSpan {
            range: 1..2,
            style: TextStylePatch::default(),
        });
        assert!(context.layout(&mut fonts, request).is_err());
    }

    #[test]
    fn system_layout_supports_hit_testing_and_selection() {
        let mut fonts = FontDatabase::default();
        let mut context = TextLayoutContext::new();
        let layout = context
            .layout(&mut fonts, TextLayoutRequest::new("hello אבג"))
            .expect("layout");
        assert!(!layout.glyph_runs().is_empty());
        let start = TextPosition::default();
        let end = layout.move_caret(start, CaretMovement::VisualRight);
        assert_ne!(start, end);
        assert!(!layout.selection_rects(start, end).is_empty());
    }
}
