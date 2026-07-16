//! Coordinate-space-aware geometric primitives.
//!
//! Types are parameterized by a coordinate space marker ([`Logical`] or
//! [`Physical`]) so the type system prevents accidentally mixing pixel
//! coordinates with logical (DPI-scaled) coordinates.
//!
//! # Example
//!
//! ```
//! use astrelis_core::geometry::{Logical, Physical, Point, Size};
//!
//! let logical = Point::<Logical>::new(100.0, 200.0);
//! let physical: Point<Physical> = logical.to_physical(2.0);
//! assert_eq!(physical.x, 200.0);
//! assert_eq!(physical.y, 400.0);
//! ```

use std::marker::PhantomData;

/// Marker type for logical (DPI-independent) coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Logical;

/// Marker type for physical (pixel) coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Physical;

/// A 2D point in coordinate space `S`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Point<S, Scalar = f32> {
    /// X coordinate.
    pub x: Scalar,
    /// Y coordinate.
    pub y: Scalar,
    _space: PhantomData<S>,
}

/// A 2D size in coordinate space `S`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size<S, Scalar = f32> {
    /// Width.
    pub width: Scalar,
    /// Height.
    pub height: Scalar,
    _space: PhantomData<S>,
}

/// An axis-aligned rectangle in coordinate space `S`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect<S, Scalar = f32> {
    /// Top-left origin.
    pub origin: Point<S, Scalar>,
    /// Size of the rectangle.
    pub size: Size<S, Scalar>,
}

// --- Point ---

impl<S, Scalar> Point<S, Scalar> {
    /// Creates a new point.
    #[inline]
    pub const fn new(x: Scalar, y: Scalar) -> Self {
        Self {
            x,
            y,
            _space: PhantomData,
        }
    }
}

impl<S, Scalar: Default> Point<S, Scalar> {
    /// The origin point, using the scalar's default value.
    pub fn zero() -> Self {
        Self::new(Scalar::default(), Scalar::default())
    }
}

impl<S> Point<S> {
    /// The origin point (0, 0).
    pub const ZERO: Self = Self::new(0.0, 0.0);
}

impl Point<Logical, f32> {
    /// Converts to physical coordinates by multiplying by the scale factor.
    #[inline]
    pub fn to_physical(self, scale_factor: f32) -> Point<Physical> {
        Point::new(self.x * scale_factor, self.y * scale_factor)
    }
}

impl Point<Physical, f32> {
    /// Converts to logical coordinates by dividing by the scale factor.
    #[inline]
    pub fn to_logical(self, scale_factor: f32) -> Point<Logical> {
        Point::new(self.x / scale_factor, self.y / scale_factor)
    }
}

// --- Size ---

impl<S, Scalar> Size<S, Scalar> {
    /// Creates a new size.
    #[inline]
    pub const fn new(width: Scalar, height: Scalar) -> Self {
        Self {
            width,
            height,
            _space: PhantomData,
        }
    }
}

impl<S, Scalar: Default> Size<S, Scalar> {
    /// A zero size, using the scalar's default value.
    pub fn zero() -> Self {
        Self::new(Scalar::default(), Scalar::default())
    }
}

impl<S> Size<S> {
    /// A zero size.
    pub const ZERO: Self = Self::new(0.0, 0.0);
}

impl Size<Logical, f32> {
    /// Converts to physical coordinates.
    #[inline]
    pub fn to_physical(self, scale_factor: f32) -> Size<Physical> {
        Size::new(self.width * scale_factor, self.height * scale_factor)
    }
}

impl Size<Physical, f32> {
    /// Converts to logical coordinates.
    #[inline]
    pub fn to_logical(self, scale_factor: f32) -> Size<Logical> {
        Size::new(self.width / scale_factor, self.height / scale_factor)
    }
}

// --- Rect ---

impl<S, Scalar> Rect<S, Scalar> {
    /// Creates a new rectangle from origin and size.
    #[inline]
    pub const fn new(origin: Point<S, Scalar>, size: Size<S, Scalar>) -> Self {
        Self { origin, size }
    }

    /// Creates a rectangle from position and dimensions.
    #[inline]
    pub const fn from_xywh(x: Scalar, y: Scalar, width: Scalar, height: Scalar) -> Self {
        Self {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        }
    }
}

impl<S, Scalar> Rect<S, Scalar>
where
    Scalar: Copy + std::ops::Add<Output = Scalar> + PartialOrd,
{
    /// Returns the minimum x coordinate (left edge).
    #[inline]
    pub fn min_x(&self) -> Scalar {
        self.origin.x
    }

    /// Returns the minimum y coordinate (top edge).
    #[inline]
    pub fn min_y(&self) -> Scalar {
        self.origin.y
    }

    /// Returns the maximum x coordinate (right edge).
    #[inline]
    pub fn max_x(&self) -> Scalar {
        self.origin.x + self.size.width
    }

    /// Returns the maximum y coordinate (bottom edge).
    #[inline]
    pub fn max_y(&self) -> Scalar {
        self.origin.y + self.size.height
    }

    /// Returns `true` if the point is inside this rectangle.
    #[inline]
    pub fn contains(&self, point: Point<S, Scalar>) -> bool {
        point.x >= self.min_x()
            && point.x <= self.max_x()
            && point.y >= self.min_y()
            && point.y <= self.max_y()
    }
}

impl Rect<Logical, f32> {
    /// Converts to physical coordinates.
    #[inline]
    pub fn to_physical(self, scale_factor: f32) -> Rect<Physical> {
        Rect::new(
            self.origin.to_physical(scale_factor),
            self.size.to_physical(scale_factor),
        )
    }
}

impl Rect<Physical, f32> {
    /// Converts to logical coordinates.
    #[inline]
    pub fn to_logical(self, scale_factor: f32) -> Rect<Logical> {
        Rect::new(
            self.origin.to_logical(scale_factor),
            self.size.to_logical(scale_factor),
        )
    }
}

/// Logical point type alias.
pub type LogicalPoint = Point<Logical>;

/// Physical point type alias.
pub type PhysicalPoint = Point<Physical>;

/// Logical size type alias.
pub type LogicalSize = Size<Logical>;

/// Physical size type alias.
pub type PhysicalSize = Size<Physical>;

/// Logical rectangle type alias.
pub type LogicalRect = Rect<Logical>;

/// Physical rectangle type alias.
pub type PhysicalRect = Rect<Physical>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_to_physical_roundtrip() {
        let logical = Point::<Logical>::new(100.0, 200.0);
        let physical = logical.to_physical(2.0);
        let back = physical.to_logical(2.0);
        assert!((back.x - logical.x).abs() < f32::EPSILON);
        assert!((back.y - logical.y).abs() < f32::EPSILON);
    }

    #[test]
    fn rect_contains() {
        let rect = Rect::<Logical>::from_xywh(10.0, 20.0, 100.0, 50.0);
        assert!(rect.contains(Point::new(50.0, 40.0)));
        assert!(!rect.contains(Point::new(5.0, 40.0)));
        assert!(!rect.contains(Point::new(50.0, 80.0)));
    }

    #[test]
    fn rect_edges() {
        let rect = Rect::<Physical>::from_xywh(10.0, 20.0, 30.0, 40.0);
        assert_eq!(rect.min_x(), 10.0);
        assert_eq!(rect.min_y(), 20.0);
        assert_eq!(rect.max_x(), 40.0);
        assert_eq!(rect.max_y(), 60.0);
    }

    #[test]
    fn generic_integer_size_is_exact() {
        let size = Size::<Physical, u32>::new(3840, 2160);
        assert_eq!(size.width, 3840);
        assert_eq!(size.height, 2160);
    }

    #[test]
    fn default_scalar_remains_f32() {
        let point: Point<Logical> = Point::new(1.5, 2.5);
        assert_eq!(point.x, 1.5_f32);
    }
}
