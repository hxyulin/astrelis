//! Type-safe coordinate system with explicit Logical/Physical coordinate spaces.
//!
//! This module provides compile-time safety for coordinate handling, preventing
//! accidental mixing of logical (DPI-independent) and physical (pixel) coordinates.
//!
//! # Overview
//!
//! - [`Logical`] coordinates are DPI-independent (e.g., 100x100 logical pixels)
//! - [`Physical`] coordinates are actual screen pixels (e.g., 200x200 on a 2x DPI display)
//! - [`ScaleFactor`] represents the DPI scale factor for conversions
//!
//! # Example
//!
//! ```rust
//! use astrelis_core::geometry::{LogicalSize, PhysicalSize, ScaleFactor};
//!
//! let logical = LogicalSize::new(800.0, 600.0);
//! let scale = ScaleFactor(2.0);
//! let physical: PhysicalSize<u32> = logical.to_physical(scale);
//! assert_eq!(physical.width, 1600);
//! assert_eq!(physical.height, 1200);
//! ```

use std::marker::PhantomData;
use std::ops::{Add, Mul, Sub};

// =============================================================================
// Coordinate Space Markers
// =============================================================================

/// Marker trait for coordinate spaces.
///
/// This trait is sealed and cannot be implemented outside this module.
pub trait CoordinateSpace: Copy + Clone + private::Sealed {}

mod private {
    pub trait Sealed {}
    impl Sealed for super::Logical {}
    impl Sealed for super::Physical {}
}

/// Marker type for logical (DPI-independent) coordinates.
///
/// Logical coordinates remain constant regardless of the display's DPI.
/// For example, a 100x100 logical size window will appear the same physical
/// size on any display, but the actual pixel count will vary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Logical;
impl CoordinateSpace for Logical {}

/// Marker type for physical (pixel) coordinates.
///
/// Physical coordinates represent actual screen pixels.
/// A 100x100 physical size means exactly 100x100 pixels on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Physical;
impl CoordinateSpace for Physical {}

// =============================================================================
// Scale Factor
// =============================================================================

/// Scale factor for converting between logical and physical coordinates.
///
/// Common values:
/// - 1.0: Standard DPI (96 DPI on Windows, 72 DPI on macOS historically)
/// - 2.0: Retina/HiDPI displays
/// - 1.5, 1.25: Common Windows scaling factors
///
/// # Safety
///
/// The scale factor must be positive (greater than 0). Methods like `inverse()`
/// and coordinate conversions will produce infinity or NaN if the scale is 0.
/// Use `try_new()` or `is_valid()` to validate scale factors from untrusted input.
///
/// # Example
///
/// ```rust
/// use astrelis_core::geometry::ScaleFactor;
///
/// let scale = ScaleFactor(2.0);
/// assert_eq!(scale.0, 2.0);
/// assert_eq!(scale.inverse().0, 0.5);
///
/// // Validate scale factors from user input
/// let scale = ScaleFactor::try_new(2.0).expect("Invalid scale");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ScaleFactor(pub f64);

impl ScaleFactor {
    /// Create a new scale factor.
    ///
    /// # Panics
    ///
    /// In debug builds, panics if the scale is not positive (> 0) or is NaN.
    /// In release builds, invalid values may cause division by zero or NaN propagation.
    #[inline]
    pub const fn new(scale: f64) -> Self {
        // Note: Can't do runtime checks in const fn, but we document the requirement
        Self(scale)
    }

    /// Try to create a new scale factor, returning None if invalid.
    ///
    /// Returns `None` if:
    /// - The scale is zero or negative
    /// - The scale is NaN or infinity
    ///
    /// # Example
    ///
    /// ```rust
    /// use astrelis_core::geometry::ScaleFactor;
    ///
    /// assert!(ScaleFactor::try_new(2.0).is_some());
    /// assert!(ScaleFactor::try_new(0.0).is_none());
    /// assert!(ScaleFactor::try_new(-1.0).is_none());
    /// assert!(ScaleFactor::try_new(f64::NAN).is_none());
    /// ```
    #[inline]
    pub fn try_new(scale: f64) -> Option<Self> {
        if scale.is_finite() && scale > 0.0 {
            Some(Self(scale))
        } else {
            None
        }
    }

    /// Check if this scale factor is valid (positive and finite).
    #[inline]
    pub fn is_valid(self) -> bool {
        self.0.is_finite() && self.0 > 0.0
    }

    /// Get the inverse scale factor (1.0 / scale).
    ///
    /// # Note
    ///
    /// Returns infinity if the scale is 0. Use `is_valid()` to check first
    /// if the scale factor comes from untrusted input.
    #[inline]
    pub fn inverse(self) -> Self {
        debug_assert!(
            self.is_valid(),
            "ScaleFactor::inverse() called on invalid scale factor: {}",
            self.0
        );
        Self(1.0 / self.0)
    }

    /// Get the scale as f32.
    #[inline]
    pub fn as_f32(self) -> f32 {
        self.0 as f32
    }

    /// Get the scale as f64.
    #[inline]
    pub fn as_f64(self) -> f64 {
        self.0
    }
}

impl Default for ScaleFactor {
    fn default() -> Self {
        Self(1.0)
    }
}

impl From<f64> for ScaleFactor {
    fn from(scale: f64) -> Self {
        Self(scale)
    }
}

impl From<f32> for ScaleFactor {
    fn from(scale: f32) -> Self {
        Self(scale as f64)
    }
}

impl From<ScaleFactor> for f64 {
    fn from(scale: ScaleFactor) -> Self {
        scale.0
    }
}

impl From<ScaleFactor> for f32 {
    fn from(scale: ScaleFactor) -> Self {
        scale.0 as f32
    }
}

// =============================================================================
// Size2D
// =============================================================================

/// A 2D size with a coordinate space marker.
///
/// Use the type aliases [`LogicalSize`] and [`PhysicalSize`] for convenience.
///
/// # Example
///
/// ```rust
/// use astrelis_core::geometry::{LogicalSize, PhysicalSize, ScaleFactor};
///
/// let logical = LogicalSize::new(800.0_f32, 600.0);
/// let physical = logical.to_physical(ScaleFactor(2.0));
/// assert_eq!(physical.width, 1600);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Size2D<T, S: CoordinateSpace> {
    pub width: T,
    pub height: T,
    _marker: PhantomData<S>,
}

impl<T: PartialEq, S: CoordinateSpace> PartialEq for Size2D<T, S> {
    fn eq(&self, other: &Self) -> bool {
        self.width == other.width && self.height == other.height
    }
}

impl<T: Eq, S: CoordinateSpace> Eq for Size2D<T, S> {}

impl<T: Default, S: CoordinateSpace> Default for Size2D<T, S> {
    fn default() -> Self {
        Self {
            width: T::default(),
            height: T::default(),
            _marker: PhantomData,
        }
    }
}

impl<T: std::hash::Hash, S: CoordinateSpace> std::hash::Hash for Size2D<T, S> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.width.hash(state);
        self.height.hash(state);
    }
}

impl<T, S: CoordinateSpace> Size2D<T, S> {
    /// Create a new size.
    #[inline]
    pub const fn new(width: T, height: T) -> Self {
        Self {
            width,
            height,
            _marker: PhantomData,
        }
    }

    /// Convert to a tuple (width, height).
    #[inline]
    pub fn into_tuple(self) -> (T, T) {
        (self.width, self.height)
    }

    /// Create from a tuple (width, height).
    #[inline]
    pub fn from_tuple(tuple: (T, T)) -> Self {
        Self::new(tuple.0, tuple.1)
    }
}

impl<T: Copy, S: CoordinateSpace> Size2D<T, S> {
    /// Get the width.
    #[inline]
    pub fn width(&self) -> T {
        self.width
    }

    /// Get the height.
    #[inline]
    pub fn height(&self) -> T {
        self.height
    }
}

impl<T: Copy, S: CoordinateSpace> Size2D<T, S> {
    /// Cast to a different numeric type within the same coordinate space.
    #[inline]
    pub fn cast<U>(self) -> Size2D<U, S>
    where
        T: Into<U>,
    {
        Size2D::new(self.width.into(), self.height.into())
    }
}

impl<T, S: CoordinateSpace> Size2D<T, S>
where
    T: Copy + Into<f64>,
{
    /// Cast to f32 type within the same coordinate space.
    #[inline]
    pub fn to_f32(self) -> Size2D<f32, S> {
        Size2D::new(self.width.into() as f32, self.height.into() as f32)
    }

    /// Cast to f64 type within the same coordinate space.
    #[inline]
    pub fn to_f64(self) -> Size2D<f64, S> {
        Size2D::new(self.width.into(), self.height.into())
    }
}

impl<T: Mul<Output = T> + Copy, S: CoordinateSpace> Mul<T> for Size2D<T, S> {
    type Output = Self;

    fn mul(self, rhs: T) -> Self::Output {
        Self::new(self.width * rhs, self.height * rhs)
    }
}

// Logical size conversions
impl<T> Size2D<T, Logical>
where
    T: Copy + Into<f64>,
{
    /// Convert to physical coordinates.
    #[inline]
    pub fn to_physical(self, scale: ScaleFactor) -> PhysicalSize<u32> {
        PhysicalSize::new(
            (self.width.into() * scale.0).round() as u32,
            (self.height.into() * scale.0).round() as u32,
        )
    }

    /// Convert to physical coordinates as f32.
    #[inline]
    pub fn to_physical_f32(self, scale: ScaleFactor) -> PhysicalSize<f32> {
        PhysicalSize::new(
            (self.width.into() * scale.0) as f32,
            (self.height.into() * scale.0) as f32,
        )
    }

    /// Convert to physical coordinates as f64.
    #[inline]
    pub fn to_physical_f64(self, scale: ScaleFactor) -> PhysicalSize<f64> {
        PhysicalSize::new(
            self.width.into() * scale.0,
            self.height.into() * scale.0,
        )
    }
}

// Physical size conversions
impl<T> Size2D<T, Physical>
where
    T: Copy + Into<f64>,
{
    /// Convert to logical coordinates.
    #[inline]
    pub fn to_logical(self, scale: ScaleFactor) -> LogicalSize<f32> {
        LogicalSize::new(
            (self.width.into() / scale.0) as f32,
            (self.height.into() / scale.0) as f32,
        )
    }

    /// Convert to logical coordinates as f64.
    #[inline]
    pub fn to_logical_f64(self, scale: ScaleFactor) -> LogicalSize<f64> {
        LogicalSize::new(
            self.width.into() / scale.0,
            self.height.into() / scale.0,
        )
    }
}

/// Logical (DPI-independent) size.
pub type LogicalSize<T> = Size2D<T, Logical>;

/// Physical (pixel) size.
pub type PhysicalSize<T> = Size2D<T, Physical>;

// =============================================================================
// Position2D
// =============================================================================

/// A 2D position with a coordinate space marker.
///
/// Use the type aliases [`LogicalPosition`] and [`PhysicalPosition`] for convenience.
#[derive(Debug, Clone, Copy)]
pub struct Position2D<T, S: CoordinateSpace> {
    pub x: T,
    pub y: T,
    _marker: PhantomData<S>,
}

impl<T: PartialEq, S: CoordinateSpace> PartialEq for Position2D<T, S> {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y
    }
}

impl<T: Eq, S: CoordinateSpace> Eq for Position2D<T, S> {}

impl<T: Default, S: CoordinateSpace> Default for Position2D<T, S> {
    fn default() -> Self {
        Self {
            x: T::default(),
            y: T::default(),
            _marker: PhantomData,
        }
    }
}

impl<T: std::hash::Hash, S: CoordinateSpace> std::hash::Hash for Position2D<T, S> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.x.hash(state);
        self.y.hash(state);
    }
}

impl<T, S: CoordinateSpace> Position2D<T, S> {
    /// Create a new position.
    #[inline]
    pub const fn new(x: T, y: T) -> Self {
        Self {
            x,
            y,
            _marker: PhantomData,
        }
    }

    /// Origin position (0, 0).
    #[inline]
    pub fn origin() -> Self
    where
        T: Default,
    {
        Self::default()
    }

    /// Convert to a tuple (x, y).
    #[inline]
    pub fn into_tuple(self) -> (T, T) {
        (self.x, self.y)
    }

    /// Create from a tuple (x, y).
    #[inline]
    pub fn from_tuple(tuple: (T, T)) -> Self {
        Self::new(tuple.0, tuple.1)
    }
}

impl<T: Copy, S: CoordinateSpace> Position2D<T, S> {
    /// Get the x coordinate.
    #[inline]
    pub fn x(&self) -> T {
        self.x
    }

    /// Get the y coordinate.
    #[inline]
    pub fn y(&self) -> T {
        self.y
    }
}

impl<T, S: CoordinateSpace> Position2D<T, S>
where
    T: Copy + Into<f64>,
{
    /// Cast to f32 type within the same coordinate space.
    #[inline]
    pub fn to_f32(self) -> Position2D<f32, S> {
        Position2D::new(self.x.into() as f32, self.y.into() as f32)
    }

    /// Cast to f64 type within the same coordinate space.
    #[inline]
    pub fn to_f64(self) -> Position2D<f64, S> {
        Position2D::new(self.x.into(), self.y.into())
    }
}

impl<T: Mul<Output = T> + Copy, S: CoordinateSpace> Mul<T> for Position2D<T, S> {
    type Output = Self;

    fn mul(self, rhs: T) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

impl<T: Add<Output = T>, S: CoordinateSpace> Add for Position2D<T, S> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl<T: Sub<Output = T>, S: CoordinateSpace> Sub for Position2D<T, S> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

// Logical position conversions
impl<T> Position2D<T, Logical>
where
    T: Copy + Into<f64>,
{
    /// Convert to physical coordinates.
    #[inline]
    pub fn to_physical(self, scale: ScaleFactor) -> PhysicalPosition<i32> {
        PhysicalPosition::new(
            (self.x.into() * scale.0).round() as i32,
            (self.y.into() * scale.0).round() as i32,
        )
    }

    /// Convert to physical coordinates as f32.
    #[inline]
    pub fn to_physical_f32(self, scale: ScaleFactor) -> PhysicalPosition<f32> {
        PhysicalPosition::new(
            (self.x.into() * scale.0) as f32,
            (self.y.into() * scale.0) as f32,
        )
    }

    /// Convert to physical coordinates as f64.
    #[inline]
    pub fn to_physical_f64(self, scale: ScaleFactor) -> PhysicalPosition<f64> {
        PhysicalPosition::new(
            self.x.into() * scale.0,
            self.y.into() * scale.0,
        )
    }
}

// Physical position conversions
impl<T> Position2D<T, Physical>
where
    T: Copy + Into<f64>,
{
    /// Convert to logical coordinates.
    #[inline]
    pub fn to_logical(self, scale: ScaleFactor) -> LogicalPosition<f32> {
        LogicalPosition::new(
            (self.x.into() / scale.0) as f32,
            (self.y.into() / scale.0) as f32,
        )
    }

    /// Convert to logical coordinates as f64.
    #[inline]
    pub fn to_logical_f64(self, scale: ScaleFactor) -> LogicalPosition<f64> {
        LogicalPosition::new(
            self.x.into() / scale.0,
            self.y.into() / scale.0,
        )
    }
}

/// Logical (DPI-independent) position.
pub type LogicalPosition<T> = Position2D<T, Logical>;

/// Physical (pixel) position.
pub type PhysicalPosition<T> = Position2D<T, Physical>;

// =============================================================================
// Rect2D
// =============================================================================

/// A 2D rectangle with a coordinate space marker.
///
/// Use the type aliases [`LogicalRect`] and [`PhysicalRect`] for convenience.
#[derive(Debug, Clone, Copy)]
pub struct Rect2D<T, S: CoordinateSpace> {
    pub x: T,
    pub y: T,
    pub width: T,
    pub height: T,
    _marker: PhantomData<S>,
}

impl<T: PartialEq, S: CoordinateSpace> PartialEq for Rect2D<T, S> {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x
            && self.y == other.y
            && self.width == other.width
            && self.height == other.height
    }
}

impl<T: Eq, S: CoordinateSpace> Eq for Rect2D<T, S> {}

impl<T: Default, S: CoordinateSpace> Default for Rect2D<T, S> {
    fn default() -> Self {
        Self {
            x: T::default(),
            y: T::default(),
            width: T::default(),
            height: T::default(),
            _marker: PhantomData,
        }
    }
}

impl<T: std::hash::Hash, S: CoordinateSpace> std::hash::Hash for Rect2D<T, S> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.x.hash(state);
        self.y.hash(state);
        self.width.hash(state);
        self.height.hash(state);
    }
}

impl<T, S: CoordinateSpace> Rect2D<T, S> {
    /// Create a new rectangle.
    #[inline]
    pub const fn new(x: T, y: T, width: T, height: T) -> Self {
        Self {
            x,
            y,
            width,
            height,
            _marker: PhantomData,
        }
    }

    /// Create a rectangle from position and size.
    #[inline]
    pub fn from_position_size(position: Position2D<T, S>, size: Size2D<T, S>) -> Self {
        Self::new(position.x, position.y, size.width, size.height)
    }
}

impl<T: Copy, S: CoordinateSpace> Rect2D<T, S> {
    /// Get the position (x, y) as a Position2D.
    #[inline]
    pub fn position(&self) -> Position2D<T, S> {
        Position2D::new(self.x, self.y)
    }

    /// Get the size (width, height) as a Size2D.
    #[inline]
    pub fn size(&self) -> Size2D<T, S> {
        Size2D::new(self.width, self.height)
    }

    /// Get the x coordinate.
    #[inline]
    pub fn x(&self) -> T {
        self.x
    }

    /// Get the y coordinate.
    #[inline]
    pub fn y(&self) -> T {
        self.y
    }

    /// Get the width.
    #[inline]
    pub fn width(&self) -> T {
        self.width
    }

    /// Get the height.
    #[inline]
    pub fn height(&self) -> T {
        self.height
    }
}

impl<T, S: CoordinateSpace> Rect2D<T, S>
where
    T: Copy + Add<Output = T>,
{
    /// Get the right edge (x + width).
    #[inline]
    pub fn right(&self) -> T {
        self.x + self.width
    }

    /// Get the bottom edge (y + height).
    #[inline]
    pub fn bottom(&self) -> T {
        self.y + self.height
    }
}

impl<T, S: CoordinateSpace> Rect2D<T, S>
where
    T: Copy + Into<f64>,
{
    /// Cast to f32 type within the same coordinate space.
    #[inline]
    pub fn to_f32(self) -> Rect2D<f32, S> {
        Rect2D::new(
            self.x.into() as f32,
            self.y.into() as f32,
            self.width.into() as f32,
            self.height.into() as f32,
        )
    }

    /// Cast to f64 type within the same coordinate space.
    #[inline]
    pub fn to_f64(self) -> Rect2D<f64, S> {
        Rect2D::new(
            self.x.into(),
            self.y.into(),
            self.width.into(),
            self.height.into(),
        )
    }
}

impl<T, S: CoordinateSpace> Rect2D<T, S>
where
    T: Copy + PartialOrd + Add<Output = T>,
{
    /// Check if a point is contained within this rectangle.
    #[inline]
    pub fn contains(&self, point: Position2D<T, S>) -> bool {
        point.x >= self.x
            && point.x < self.x + self.width
            && point.y >= self.y
            && point.y < self.y + self.height
    }
}

// Logical rect conversions
impl<T> Rect2D<T, Logical>
where
    T: Copy + Into<f64>,
{
    /// Convert to physical coordinates.
    #[inline]
    pub fn to_physical(self, scale: ScaleFactor) -> PhysicalRect<u32> {
        PhysicalRect::new(
            (self.x.into() * scale.0).round() as u32,
            (self.y.into() * scale.0).round() as u32,
            (self.width.into() * scale.0).round() as u32,
            (self.height.into() * scale.0).round() as u32,
        )
    }

    /// Convert to physical coordinates as f32.
    #[inline]
    pub fn to_physical_f32(self, scale: ScaleFactor) -> PhysicalRect<f32> {
        PhysicalRect::new(
            (self.x.into() * scale.0) as f32,
            (self.y.into() * scale.0) as f32,
            (self.width.into() * scale.0) as f32,
            (self.height.into() * scale.0) as f32,
        )
    }
}

// Physical rect conversions
impl<T> Rect2D<T, Physical>
where
    T: Copy + Into<f64>,
{
    /// Convert to logical coordinates.
    #[inline]
    pub fn to_logical(self, scale: ScaleFactor) -> LogicalRect<f32> {
        LogicalRect::new(
            (self.x.into() / scale.0) as f32,
            (self.y.into() / scale.0) as f32,
            (self.width.into() / scale.0) as f32,
            (self.height.into() / scale.0) as f32,
        )
    }

    /// Convert to logical coordinates as f64.
    #[inline]
    pub fn to_logical_f64(self, scale: ScaleFactor) -> LogicalRect<f64> {
        LogicalRect::new(
            self.x.into() / scale.0,
            self.y.into() / scale.0,
            self.width.into() / scale.0,
            self.height.into() / scale.0,
        )
    }
}

/// Logical (DPI-independent) rectangle.
pub type LogicalRect<T> = Rect2D<T, Logical>;

/// Physical (pixel) rectangle.
pub type PhysicalRect<T> = Rect2D<T, Physical>;

// =============================================================================
// winit Interop
// =============================================================================

#[cfg(feature = "winit")]
mod winit_interop {
    use super::*;

    impl From<winit::dpi::PhysicalSize<u32>> for PhysicalSize<u32> {
        fn from(size: winit::dpi::PhysicalSize<u32>) -> Self {
            Self::new(size.width, size.height)
        }
    }

    impl From<PhysicalSize<u32>> for winit::dpi::PhysicalSize<u32> {
        fn from(size: PhysicalSize<u32>) -> Self {
            Self::new(size.width, size.height)
        }
    }

    impl From<winit::dpi::PhysicalSize<f32>> for PhysicalSize<f32> {
        fn from(size: winit::dpi::PhysicalSize<f32>) -> Self {
            Self::new(size.width, size.height)
        }
    }

    impl From<PhysicalSize<f32>> for winit::dpi::PhysicalSize<f32> {
        fn from(size: PhysicalSize<f32>) -> Self {
            Self::new(size.width, size.height)
        }
    }

    impl From<winit::dpi::LogicalSize<u32>> for LogicalSize<u32> {
        fn from(size: winit::dpi::LogicalSize<u32>) -> Self {
            Self::new(size.width, size.height)
        }
    }

    impl From<LogicalSize<u32>> for winit::dpi::LogicalSize<u32> {
        fn from(size: LogicalSize<u32>) -> Self {
            Self::new(size.width, size.height)
        }
    }

    impl From<winit::dpi::LogicalSize<f32>> for LogicalSize<f32> {
        fn from(size: winit::dpi::LogicalSize<f32>) -> Self {
            Self::new(size.width, size.height)
        }
    }

    impl From<LogicalSize<f32>> for winit::dpi::LogicalSize<f32> {
        fn from(size: LogicalSize<f32>) -> Self {
            Self::new(size.width, size.height)
        }
    }

    impl From<winit::dpi::PhysicalPosition<i32>> for PhysicalPosition<i32> {
        fn from(pos: winit::dpi::PhysicalPosition<i32>) -> Self {
            Self::new(pos.x, pos.y)
        }
    }

    impl From<PhysicalPosition<i32>> for winit::dpi::PhysicalPosition<i32> {
        fn from(pos: PhysicalPosition<i32>) -> Self {
            Self::new(pos.x, pos.y)
        }
    }

    impl From<winit::dpi::PhysicalPosition<f64>> for PhysicalPosition<f64> {
        fn from(pos: winit::dpi::PhysicalPosition<f64>) -> Self {
            Self::new(pos.x, pos.y)
        }
    }

    impl From<PhysicalPosition<f64>> for winit::dpi::PhysicalPosition<f64> {
        fn from(pos: PhysicalPosition<f64>) -> Self {
            Self::new(pos.x, pos.y)
        }
    }

    impl From<winit::dpi::LogicalPosition<f64>> for LogicalPosition<f64> {
        fn from(pos: winit::dpi::LogicalPosition<f64>) -> Self {
            Self::new(pos.x, pos.y)
        }
    }

    impl From<LogicalPosition<f64>> for winit::dpi::LogicalPosition<f64> {
        fn from(pos: LogicalPosition<f64>) -> Self {
            Self::new(pos.x, pos.y)
        }
    }
}

// =============================================================================
// Generic Types (for layout calculations where coordinate space is implicit)
// =============================================================================

/// Generic 2D size without explicit coordinate space.
///
/// Use this for internal calculations where the coordinate space is implicit
/// or doesn't matter (e.g., layout calculations that always work in logical space).
///
/// For explicit coordinate safety, prefer [`LogicalSize`] or [`PhysicalSize`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Size<T> {
    pub width: T,
    pub height: T,
}

impl<T> Size<T> {
    /// Create a new size.
    #[inline]
    pub const fn new(width: T, height: T) -> Self {
        Self { width, height }
    }
}

impl<T: Copy> Size<T> {
    /// Convert to a tuple (width, height).
    #[inline]
    pub fn into_tuple(self) -> (T, T) {
        (self.width, self.height)
    }
}

impl<T: Mul<Output = T> + Copy> Mul<T> for Size<T> {
    type Output = Self;

    fn mul(self, rhs: T) -> Self::Output {
        Self::new(self.width * rhs, self.height * rhs)
    }
}

impl<T: Add<Output = T>> Add for Size<T> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.width + rhs.width, self.height + rhs.height)
    }
}

impl<T: Sub<Output = T>> Sub for Size<T> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.width - rhs.width, self.height - rhs.height)
    }
}

// Conversions from typed coordinate types to generic types
impl<T, S: CoordinateSpace> From<Size2D<T, S>> for Size<T> {
    fn from(size: Size2D<T, S>) -> Self {
        Self::new(size.width, size.height)
    }
}

impl<T, S: CoordinateSpace> From<Position2D<T, S>> for Pos<T> {
    fn from(pos: Position2D<T, S>) -> Self {
        Self::new(pos.x, pos.y)
    }
}

impl<T, S: CoordinateSpace> From<Rect2D<T, S>> for Rect<T> {
    fn from(rect: Rect2D<T, S>) -> Self {
        Self::new(rect.x, rect.y, rect.width, rect.height)
    }
}

/// Generic 2D position without explicit coordinate space.
///
/// Use this for internal calculations where the coordinate space is implicit.
/// For explicit coordinate safety, prefer [`LogicalPosition`] or [`PhysicalPosition`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Pos<T> {
    pub x: T,
    pub y: T,
}

impl<T> Pos<T> {
    /// Create a new position.
    #[inline]
    pub const fn new(x: T, y: T) -> Self {
        Self { x, y }
    }
}

impl<T: Copy> Pos<T> {
    /// Convert to a tuple (x, y).
    #[inline]
    pub fn into_tuple(self) -> (T, T) {
        (self.x, self.y)
    }
}

impl<T: Mul<Output = T> + Copy> Mul<T> for Pos<T> {
    type Output = Self;

    fn mul(self, rhs: T) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

impl<T: Add<Output = T>> Add for Pos<T> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl<T: Sub<Output = T>> Sub for Pos<T> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

/// Generic 2D rectangle without explicit coordinate space.
///
/// Use this for internal calculations where the coordinate space is implicit.
/// For explicit coordinate safety, prefer [`LogicalRect`] or [`PhysicalRect`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect<T> {
    pub x: T,
    pub y: T,
    pub width: T,
    pub height: T,
}

impl<T> Rect<T> {
    /// Create a new rectangle.
    #[inline]
    pub const fn new(x: T, y: T, width: T, height: T) -> Self {
        Self { x, y, width, height }
    }

    /// Create from position and size.
    #[inline]
    pub fn from_position_size(pos: Pos<T>, size: Size<T>) -> Self {
        Self::new(pos.x, pos.y, size.width, size.height)
    }

    /// Get the position.
    #[inline]
    pub fn position(&self) -> Pos<T>
    where
        T: Copy,
    {
        Pos::new(self.x, self.y)
    }

    /// Get the size.
    #[inline]
    pub fn size(&self) -> Size<T>
    where
        T: Copy,
    {
        Size::new(self.width, self.height)
    }
}

impl<T> Rect<T>
where
    T: Copy + PartialOrd + Add<Output = T>,
{
    /// Check if a point is inside the rectangle.
    #[inline]
    pub fn contains(&self, point: Pos<T>) -> bool {
        point.x >= self.x
            && point.x < self.x + self.width
            && point.y >= self.y
            && point.y < self.y + self.height
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logical_to_physical_size() {
        let logical = LogicalSize::new(100.0_f64, 50.0);
        let scale = ScaleFactor(2.0);
        let physical = logical.to_physical(scale);
        assert_eq!(physical.width, 200);
        assert_eq!(physical.height, 100);
    }

    #[test]
    fn test_physical_to_logical_size() {
        let physical = PhysicalSize::new(200_u32, 100);
        let scale = ScaleFactor(2.0);
        let logical = physical.to_logical(scale);
        assert_eq!(logical.width, 100.0);
        assert_eq!(logical.height, 50.0);
    }

    #[test]
    fn test_logical_to_physical_position() {
        let logical = LogicalPosition::new(50.0_f64, 25.0);
        let scale = ScaleFactor(2.0);
        let physical = logical.to_physical(scale);
        assert_eq!(physical.x, 100);
        assert_eq!(physical.y, 50);
    }

    #[test]
    fn test_rect_contains() {
        let rect = LogicalRect::new(10.0_f64, 10.0, 100.0, 50.0);
        assert!(rect.contains(LogicalPosition::new(50.0, 30.0)));
        assert!(!rect.contains(LogicalPosition::new(5.0, 30.0)));
        assert!(!rect.contains(LogicalPosition::new(50.0, 5.0)));
    }

    #[test]
    fn test_scale_factor_inverse() {
        let scale = ScaleFactor(2.0);
        let inv = scale.inverse();
        assert_eq!(inv.0, 0.5);
    }

    #[test]
    fn test_size_multiplication() {
        let size = LogicalSize::new(10.0_f32, 20.0);
        let scaled = size * 2.0;
        assert_eq!(scaled.width, 20.0);
        assert_eq!(scaled.height, 40.0);
    }

    #[test]
    fn test_position_arithmetic() {
        let a = LogicalPosition::new(10.0_f32, 20.0);
        let b = LogicalPosition::new(5.0, 10.0);
        let sum = a + b;
        let diff = a - b;
        assert_eq!(sum.x, 15.0);
        assert_eq!(sum.y, 30.0);
        assert_eq!(diff.x, 5.0);
        assert_eq!(diff.y, 10.0);
    }

    #[test]
    fn test_rect_from_position_size() {
        let pos = LogicalPosition::new(10.0_f32, 20.0);
        let size = LogicalSize::new(100.0_f32, 50.0);
        let rect = LogicalRect::from_position_size(pos, size);
        assert_eq!(rect.x, 10.0);
        assert_eq!(rect.y, 20.0);
        assert_eq!(rect.width, 100.0);
        assert_eq!(rect.height, 50.0);
    }
}
