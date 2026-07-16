//! Backend-independent paint paths, images, and display lists.

#![warn(missing_docs)]

use std::{
    collections::HashMap,
    error::Error,
    fmt,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use astrelis_core::{
    color::Color,
    geometry::{LogicalPoint, LogicalRect, Physical, Point, Rect, Size},
    math::Affine2,
};

static NEXT_PATH_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_IMAGE_ID: AtomicU64 = AtomicU64::new(1);

/// Error produced while constructing paint data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaintError {
    message: String,
}

impl PaintError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for PaintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(formatter)
    }
}

impl Error for PaintError {}

/// One immutable path verb.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathVerb {
    /// Starts a new contour.
    MoveTo(LogicalPoint),
    /// Adds a line segment.
    LineTo(LogicalPoint),
    /// Adds a quadratic Bézier segment.
    QuadTo(LogicalPoint, LogicalPoint),
    /// Adds a cubic Bézier segment.
    CubicTo(LogicalPoint, LogicalPoint, LogicalPoint),
    /// Closes the current contour.
    Close,
}

#[derive(Debug)]
struct PathData {
    id: u64,
    verbs: Box<[PathVerb]>,
    bounds: Option<LogicalRect>,
}

/// Immutable backend-independent vector path.
#[derive(Clone)]
pub struct Path(Arc<PathData>);

impl Path {
    /// Starts building a path.
    pub fn builder() -> PathBuilder {
        PathBuilder::new()
    }

    /// Path verbs in recording order.
    pub fn verbs(&self) -> &[PathVerb] {
        &self.0.verbs
    }

    /// Conservative bounds including Bézier control points.
    pub fn bounds(&self) -> Option<LogicalRect> {
        self.0.bounds
    }

    /// Returns whether the path contains no drawable segments.
    pub fn is_empty(&self) -> bool {
        !self
            .verbs()
            .iter()
            .any(|verb| !matches!(verb, PathVerb::MoveTo(_) | PathVerb::Close))
    }

    /// Internal immutable identity used for renderer caches.
    #[doc(hidden)]
    pub fn cache_id(&self) -> u64 {
        self.0.id
    }
}

impl fmt::Debug for Path {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Path")
            .field("verbs", &self.0.verbs)
            .field("bounds", &self.0.bounds)
            .finish()
    }
}

/// Mutable path construction helper.
#[derive(Clone, Debug, Default)]
pub struct PathBuilder {
    verbs: Vec<PathVerb>,
    current: Option<LogicalPoint>,
    contour_start: Option<LogicalPoint>,
    bounds: Option<(f32, f32, f32, f32)>,
}

impl PathBuilder {
    /// Creates an empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts a new contour.
    pub fn move_to(&mut self, point: LogicalPoint) -> Result<&mut Self, PaintError> {
        validate_point(point)?;
        self.include(point);
        self.current = Some(point);
        self.contour_start = Some(point);
        self.verbs.push(PathVerb::MoveTo(point));
        Ok(self)
    }

    /// Adds a line segment.
    pub fn line_to(&mut self, point: LogicalPoint) -> Result<&mut Self, PaintError> {
        self.require_contour()?;
        validate_point(point)?;
        self.include(point);
        self.current = Some(point);
        self.verbs.push(PathVerb::LineTo(point));
        Ok(self)
    }

    /// Adds a quadratic Bézier segment.
    pub fn quad_to(
        &mut self,
        control: LogicalPoint,
        point: LogicalPoint,
    ) -> Result<&mut Self, PaintError> {
        self.require_contour()?;
        validate_point(control)?;
        validate_point(point)?;
        self.include(control);
        self.include(point);
        self.current = Some(point);
        self.verbs.push(PathVerb::QuadTo(control, point));
        Ok(self)
    }

    /// Adds a cubic Bézier segment.
    pub fn cubic_to(
        &mut self,
        control1: LogicalPoint,
        control2: LogicalPoint,
        point: LogicalPoint,
    ) -> Result<&mut Self, PaintError> {
        self.require_contour()?;
        validate_point(control1)?;
        validate_point(control2)?;
        validate_point(point)?;
        self.include(control1);
        self.include(control2);
        self.include(point);
        self.current = Some(point);
        self.verbs
            .push(PathVerb::CubicTo(control1, control2, point));
        Ok(self)
    }

    /// Closes the current contour.
    pub fn close(&mut self) -> Result<&mut Self, PaintError> {
        let start = self
            .contour_start
            .ok_or_else(|| PaintError::new("close requires an active contour"))?;
        self.current = Some(start);
        self.verbs.push(PathVerb::Close);
        Ok(self)
    }

    /// Freezes the path.
    pub fn finish(self) -> Path {
        let bounds = self.bounds.map(|(min_x, min_y, max_x, max_y)| {
            Rect::from_xywh(min_x, min_y, max_x - min_x, max_y - min_y)
        });
        Path(Arc::new(PathData {
            id: NEXT_PATH_ID.fetch_add(1, Ordering::Relaxed),
            verbs: self.verbs.into_boxed_slice(),
            bounds,
        }))
    }

    fn require_contour(&self) -> Result<(), PaintError> {
        self.current
            .map(|_| ())
            .ok_or_else(|| PaintError::new("path segment requires move_to first"))
    }

    fn include(&mut self, point: LogicalPoint) {
        match &mut self.bounds {
            Some((min_x, min_y, max_x, max_y)) => {
                *min_x = min_x.min(point.x);
                *min_y = min_y.min(point.y);
                *max_x = max_x.max(point.x);
                *max_y = max_y.max(point.y);
            }
            slot @ None => *slot = Some((point.x, point.y, point.x, point.y)),
        }
    }
}

/// Fill winding rule.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FillRule {
    /// Non-zero winding.
    #[default]
    NonZero,
    /// Odd-even winding.
    EvenOdd,
}

/// Stroke endpoint shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LineCap {
    /// End exactly at the endpoint.
    #[default]
    Butt,
    /// Extend by half the stroke width.
    Square,
    /// Add a semicircular cap.
    Round,
}

/// Stroke corner shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LineJoin {
    /// Extend edges until the miter limit.
    #[default]
    Miter,
    /// Bevel the corner.
    Bevel,
    /// Round the corner.
    Round,
}

/// Basic path stroke settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrokeStyle {
    /// Stroke width in local logical units.
    pub width: f32,
    /// Endpoint shape.
    pub cap: LineCap,
    /// Corner shape.
    pub join: LineJoin,
    /// Maximum miter length as a multiple of stroke width.
    pub miter_limit: f32,
}

impl Default for StrokeStyle {
    fn default() -> Self {
        Self {
            width: 1.0,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            miter_limit: 4.0,
        }
    }
}

/// Paint source for a draw operation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Brush {
    /// A solid linear-space color.
    Solid(Color),
}

/// Four rounded-rectangle corner radii.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CornerRadii {
    /// Top-left radius.
    pub top_left: f32,
    /// Top-right radius.
    pub top_right: f32,
    /// Bottom_right radius.
    pub bottom_right: f32,
    /// Bottom-left radius.
    pub bottom_left: f32,
}

impl CornerRadii {
    /// Uses the same radius for all corners.
    pub const fn uniform(radius: f32) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }
}

/// Validated rounded rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoundedRect {
    rect: LogicalRect,
    radii: CornerRadii,
}

impl RoundedRect {
    /// Creates a rounded rectangle and proportionally normalizes oversized radii.
    pub fn new(rect: LogicalRect, mut radii: CornerRadii) -> Result<Self, PaintError> {
        validate_rect(rect)?;
        for radius in [
            radii.top_left,
            radii.top_right,
            radii.bottom_right,
            radii.bottom_left,
        ] {
            if !radius.is_finite() || radius < 0.0 {
                return Err(PaintError::new(
                    "rounded rectangle radii must be finite and non-negative",
                ));
            }
        }
        let width = rect.size.width;
        let height = rect.size.height;
        let scale = [
            ratio(width, radii.top_left + radii.top_right),
            ratio(width, radii.bottom_left + radii.bottom_right),
            ratio(height, radii.top_left + radii.bottom_left),
            ratio(height, radii.top_right + radii.bottom_right),
            1.0,
        ]
        .into_iter()
        .fold(1.0_f32, f32::min);
        radii.top_left *= scale;
        radii.top_right *= scale;
        radii.bottom_right *= scale;
        radii.bottom_left *= scale;
        Ok(Self { rect, radii })
    }

    /// Underlying rectangle.
    pub const fn rect(self) -> LogicalRect {
        self.rect
    }

    /// Normalized radii.
    pub const fn radii(self) -> CornerRadii {
        self.radii
    }
}

fn ratio(limit: f32, sum: f32) -> f32 {
    if sum > 0.0 { limit / sum } else { 1.0 }
}

#[derive(Clone)]
struct ImageData {
    id: u64,
    size: Size<Physical, u32>,
    rgba: Arc<[u8]>,
    checksum: u64,
}

/// Immutable straight-alpha RGBA8 sRGB image.
#[derive(Clone)]
pub struct Image(Arc<ImageData>);

impl Image {
    /// Creates an image after validating byte length and dimensions.
    pub fn from_rgba8(
        size: Size<Physical, u32>,
        rgba: impl Into<Arc<[u8]>>,
    ) -> Result<Self, PaintError> {
        if size.width == 0 || size.height == 0 {
            return Err(PaintError::new("image dimensions must be non-zero"));
        }
        let expected = u64::from(size.width)
            .checked_mul(u64::from(size.height))
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(|| PaintError::new("image byte length overflow"))?;
        let rgba = rgba.into();
        if rgba.len() as u64 != expected {
            return Err(PaintError::new(format!(
                "image byte length is {}, expected {expected}",
                rgba.len()
            )));
        }
        let checksum = fnv1a(&rgba);
        Ok(Self(Arc::new(ImageData {
            id: NEXT_IMAGE_ID.fetch_add(1, Ordering::Relaxed),
            size,
            rgba,
            checksum,
        })))
    }

    /// Physical pixel dimensions.
    pub fn size(&self) -> Size<Physical, u32> {
        self.0.size
    }

    /// Pixel bytes.
    pub fn rgba8(&self) -> &[u8] {
        &self.0.rgba
    }

    /// Internal immutable identity used for renderer caches.
    #[doc(hidden)]
    pub fn cache_id(&self) -> u64 {
        self.0.id
    }
}

impl fmt::Debug for Image {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Image")
            .field("size", &self.0.size)
            .field("checksum", &format_args!("{:016x}", self.0.checksum))
            .finish()
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

/// Image filter selection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ImageSampling {
    /// Nearest-neighbor filtering.
    Nearest,
    /// Bilinear filtering.
    #[default]
    Linear,
}

/// Image draw settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageOptions {
    /// Optional source rectangle in physical texel coordinates.
    pub source: Option<Rect<Physical>>,
    /// Texture filtering.
    pub sampling: ImageSampling,
    /// Per-draw opacity.
    pub opacity: f32,
}

impl Default for ImageOptions {
    fn default() -> Self {
        Self {
            source: None,
            sampling: ImageSampling::Linear,
            opacity: 1.0,
        }
    }
}

/// Display-list-local path reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PathRef(pub u32);

/// Display-list-local image reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageRef(pub u32);

/// One semantic display-list command.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Saves transform and clip state.
    Save,
    /// Restores transform and clip state.
    Restore,
    /// Post-concatenates an affine transform.
    Transform(Affine2),
    /// Intersects with a rectangular clip.
    ClipRect(LogicalRect),
    /// Intersects with a rounded-rectangle clip.
    ClipRoundedRect(RoundedRect),
    /// Intersects with a path clip.
    ClipPath { path: PathRef, rule: FillRule },
    /// Fills a rectangle.
    FillRect { rect: LogicalRect, brush: Brush },
    /// Fills a rounded rectangle.
    FillRoundedRect { rect: RoundedRect, brush: Brush },
    /// Fills a path.
    FillPath {
        path: PathRef,
        rule: FillRule,
        brush: Brush,
    },
    /// Strokes a path.
    StrokePath {
        path: PathRef,
        style: StrokeStyle,
        brush: Brush,
    },
    /// Draws an image.
    DrawImage {
        image: ImageRef,
        destination: LogicalRect,
        options: ImageOptions,
    },
}

/// Immutable, validated semantic display list.
#[derive(Clone, Debug)]
pub struct DisplayList {
    commands: Arc<[Command]>,
    paths: Arc<[Path]>,
    images: Arc<[Image]>,
}

impl DisplayList {
    /// Commands in painter order.
    pub fn commands(&self) -> &[Command] {
        &self.commands
    }

    /// Resolves a path reference.
    pub fn path(&self, reference: PathRef) -> &Path {
        &self.paths[reference.0 as usize]
    }

    /// Resolves an image reference.
    pub fn image(&self, reference: ImageRef) -> &Image {
        &self.images[reference.0 as usize]
    }

    /// Resource paths in local-index order.
    pub fn paths(&self) -> &[Path] {
        &self.paths
    }

    /// Resource images in local-index order.
    pub fn images(&self) -> &[Image] {
        &self.images
    }
}

/// Semantic display-list recorder.
#[derive(Debug, Default)]
pub struct Painter {
    commands: Vec<Command>,
    paths: Vec<Path>,
    images: Vec<Image>,
    path_refs: HashMap<u64, PathRef>,
    image_refs: HashMap<u64, ImageRef>,
    save_depth: usize,
}

impl Painter {
    /// Creates an empty painter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Saves transform and clip state.
    pub fn save(&mut self) {
        self.save_depth += 1;
        self.commands.push(Command::Save);
    }

    /// Restores transform and clip state.
    pub fn restore(&mut self) -> Result<(), PaintError> {
        if self.save_depth == 0 {
            return Err(PaintError::new("restore called at the root state"));
        }
        self.save_depth -= 1;
        self.commands.push(Command::Restore);
        Ok(())
    }

    /// Executes a closure within a balanced saved state.
    pub fn with_save<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, PaintError>,
    ) -> Result<T, PaintError> {
        self.save();
        let result = operation(self);
        let restore = self.restore();
        match result {
            Err(error) => Err(error),
            Ok(value) => {
                restore?;
                Ok(value)
            }
        }
    }

    /// Post-concatenates a local affine transform.
    pub fn transform(&mut self, transform: Affine2) -> Result<(), PaintError> {
        if !transform
            .to_cols_array()
            .iter()
            .all(|value| value.is_finite())
        {
            return Err(PaintError::new("transform components must be finite"));
        }
        self.commands.push(Command::Transform(transform));
        Ok(())
    }

    /// Adds a rectangular clip.
    pub fn clip_rect(&mut self, rect: LogicalRect) -> Result<(), PaintError> {
        validate_rect(rect)?;
        self.commands.push(Command::ClipRect(rect));
        Ok(())
    }

    /// Adds a rounded-rectangle clip.
    pub fn clip_rounded_rect(&mut self, rect: RoundedRect) -> Result<(), PaintError> {
        self.commands.push(Command::ClipRoundedRect(rect));
        Ok(())
    }

    /// Adds a path clip.
    pub fn clip_path(&mut self, path: &Path, rule: FillRule) -> Result<(), PaintError> {
        let path = self.intern_path(path);
        self.commands.push(Command::ClipPath { path, rule });
        Ok(())
    }

    /// Fills a rectangle.
    pub fn fill_rect(&mut self, rect: LogicalRect, brush: Brush) -> Result<(), PaintError> {
        validate_rect(rect)?;
        validate_brush(brush)?;
        self.commands.push(Command::FillRect { rect, brush });
        Ok(())
    }

    /// Fills a rounded rectangle.
    pub fn fill_rounded_rect(&mut self, rect: RoundedRect, brush: Brush) -> Result<(), PaintError> {
        validate_brush(brush)?;
        self.commands.push(Command::FillRoundedRect { rect, brush });
        Ok(())
    }

    /// Fills a path.
    pub fn fill_path(
        &mut self,
        path: &Path,
        rule: FillRule,
        brush: Brush,
    ) -> Result<(), PaintError> {
        validate_brush(brush)?;
        let path = self.intern_path(path);
        self.commands.push(Command::FillPath { path, rule, brush });
        Ok(())
    }

    /// Strokes a path.
    pub fn stroke_path(
        &mut self,
        path: &Path,
        style: StrokeStyle,
        brush: Brush,
    ) -> Result<(), PaintError> {
        if !style.width.is_finite() || style.width < 0.0 {
            return Err(PaintError::new(
                "stroke width must be finite and non-negative",
            ));
        }
        if !style.miter_limit.is_finite() || style.miter_limit < 1.0 {
            return Err(PaintError::new(
                "stroke miter limit must be finite and at least 1",
            ));
        }
        validate_brush(brush)?;
        let path = self.intern_path(path);
        self.commands
            .push(Command::StrokePath { path, style, brush });
        Ok(())
    }

    /// Draws an image.
    pub fn draw_image(
        &mut self,
        image: &Image,
        destination: LogicalRect,
        options: ImageOptions,
    ) -> Result<(), PaintError> {
        validate_rect(destination)?;
        if !options.opacity.is_finite() || !(0.0..=1.0).contains(&options.opacity) {
            return Err(PaintError::new("image opacity must be within 0..=1"));
        }
        if let Some(source) = options.source {
            validate_rect(source)?;
            if source.max_x() > image.size().width as f32
                || source.max_y() > image.size().height as f32
            {
                return Err(PaintError::new(
                    "image source rectangle exceeds image dimensions",
                ));
            }
        }
        let image = self.intern_image(image);
        self.commands.push(Command::DrawImage {
            image,
            destination,
            options,
        });
        Ok(())
    }

    /// Freezes the display list.
    pub fn finish(self) -> Result<DisplayList, PaintError> {
        if self.save_depth != 0 {
            return Err(PaintError::new(format!(
                "{} saved paint states were not restored",
                self.save_depth
            )));
        }
        Ok(DisplayList {
            commands: self.commands.into(),
            paths: self.paths.into(),
            images: self.images.into(),
        })
    }

    fn intern_path(&mut self, path: &Path) -> PathRef {
        *self.path_refs.entry(path.cache_id()).or_insert_with(|| {
            let reference = PathRef(self.paths.len() as u32);
            self.paths.push(path.clone());
            reference
        })
    }

    fn intern_image(&mut self, image: &Image) -> ImageRef {
        *self.image_refs.entry(image.cache_id()).or_insert_with(|| {
            let reference = ImageRef(self.images.len() as u32);
            self.images.push(image.clone());
            reference
        })
    }
}

fn validate_point<S>(point: Point<S>) -> Result<(), PaintError> {
    if point.x.is_finite() && point.y.is_finite() {
        Ok(())
    } else {
        Err(PaintError::new("point coordinates must be finite"))
    }
}

fn validate_rect<S>(rect: Rect<S>) -> Result<(), PaintError> {
    validate_point(rect.origin)?;
    if rect.size.width.is_finite()
        && rect.size.height.is_finite()
        && rect.size.width >= 0.0
        && rect.size.height >= 0.0
    {
        Ok(())
    } else {
        Err(PaintError::new(
            "rectangle dimensions must be finite and non-negative",
        ))
    }
}

fn validate_brush(brush: Brush) -> Result<(), PaintError> {
    let Brush::Solid(color) = brush;
    if [color.r, color.g, color.b, color.a]
        .into_iter()
        .all(f32::is_finite)
    {
        Ok(())
    } else {
        Err(PaintError::new("brush components must be finite"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_balanced_state() {
        let mut painter = Painter::new();
        painter.save();
        assert!(painter.finish().is_err());
        assert!(Painter::new().restore().is_err());
    }

    #[test]
    fn deduplicates_resources() {
        let mut builder = Path::builder();
        builder.move_to(Point::new(0.0, 0.0)).unwrap();
        builder.line_to(Point::new(1.0, 1.0)).unwrap();
        let path = builder.finish();
        let mut painter = Painter::new();
        painter
            .fill_path(&path, FillRule::NonZero, Brush::Solid(Color::WHITE))
            .unwrap();
        painter
            .fill_path(&path, FillRule::EvenOdd, Brush::Solid(Color::BLACK))
            .unwrap();
        let list = painter.finish().unwrap();
        assert_eq!(list.paths().len(), 1);
    }

    #[test]
    fn normalizes_radii() {
        let rounded = RoundedRect::new(
            Rect::from_xywh(0.0, 0.0, 10.0, 20.0),
            CornerRadii::uniform(10.0),
        )
        .unwrap();
        assert_eq!(rounded.radii(), CornerRadii::uniform(5.0));
    }

    #[test]
    fn validates_images() {
        assert!(Image::from_rgba8(Size::new(2, 2), vec![0; 15]).is_err());
        assert!(Image::from_rgba8(Size::new(2, 2), vec![0; 16]).is_ok());
    }

    #[test]
    fn display_list_snapshot() {
        let mut path = Path::builder();
        path.move_to(Point::new(1.0, 2.0)).unwrap();
        path.line_to(Point::new(8.0, 2.0)).unwrap();
        path.quad_to(Point::new(9.0, 5.0), Point::new(8.0, 8.0))
            .unwrap();
        path.close().unwrap();
        let path = path.finish();
        let image = Image::from_rgba8(Size::new(1, 1), vec![255_u8, 128_u8, 0_u8, 255_u8]).unwrap();
        let mut painter = Painter::new();
        painter.save();
        painter
            .transform(Affine2::from_translation(astrelis_core::math::Vec2::new(
                3.0, 4.0,
            )))
            .unwrap();
        painter
            .clip_rect(Rect::from_xywh(0.0, 0.0, 10.0, 10.0))
            .unwrap();
        painter
            .fill_path(&path, FillRule::EvenOdd, Brush::Solid(Color::RED))
            .unwrap();
        painter
            .draw_image(
                &image,
                Rect::from_xywh(2.0, 3.0, 4.0, 5.0),
                ImageOptions::default(),
            )
            .unwrap();
        painter.restore().unwrap();
        let list = painter.finish().unwrap();
        let summary = format!(
            "commands={:?}\npath={:?}\nimage={:?}",
            list.commands(),
            list.path(PathRef(0)),
            list.image(ImageRef(0))
        );
        insta::assert_snapshot!(summary, @r###"
        commands=[Save, Transform(Affine2 { matrix2: Mat2 { x_axis: Vec2(1.0, 0.0), y_axis: Vec2(0.0, 1.0) }, translation: Vec2(3.0, 4.0) }), ClipRect(Rect { origin: Point { x: 0.0, y: 0.0, _space: PhantomData<astrelis_core::geometry::Logical> }, size: Size { width: 10.0, height: 10.0, _space: PhantomData<astrelis_core::geometry::Logical> } }), FillPath { path: PathRef(0), rule: EvenOdd, brush: Solid(Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }) }, DrawImage { image: ImageRef(0), destination: Rect { origin: Point { x: 2.0, y: 3.0, _space: PhantomData<astrelis_core::geometry::Logical> }, size: Size { width: 4.0, height: 5.0, _space: PhantomData<astrelis_core::geometry::Logical> } }, options: ImageOptions { source: None, sampling: Linear, opacity: 1.0 } }, Restore]
        path=Path { verbs: [MoveTo(Point { x: 1.0, y: 2.0, _space: PhantomData<astrelis_core::geometry::Logical> }), LineTo(Point { x: 8.0, y: 2.0, _space: PhantomData<astrelis_core::geometry::Logical> }), QuadTo(Point { x: 9.0, y: 5.0, _space: PhantomData<astrelis_core::geometry::Logical> }, Point { x: 8.0, y: 8.0, _space: PhantomData<astrelis_core::geometry::Logical> }), Close], bounds: Some(Rect { origin: Point { x: 1.0, y: 2.0, _space: PhantomData<astrelis_core::geometry::Logical> }, size: Size { width: 8.0, height: 6.0, _space: PhantomData<astrelis_core::geometry::Logical> } }) }
        image=Image { size: Size { width: 1, height: 1, _space: PhantomData<astrelis_core::geometry::Physical> }, checksum: be1f5b6705cd0753 }
        "###);
    }
}
