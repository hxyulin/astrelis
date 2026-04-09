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
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point<S> {
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
    _space: PhantomData<S>,
}

/// A 2D size in coordinate space `S`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size<S> {
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
    _space: PhantomData<S>,
}

/// An axis-aligned rectangle in coordinate space `S`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect<S> {
    /// Top-left origin.
    pub origin: Point<S>,
    /// Size of the rectangle.
    pub size: Size<S>,
}

// --- Point ---

impl<S> Point<S> {
    /// Creates a new point.
    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            _space: PhantomData,
        }
    }

    /// The origin point (0, 0).
    pub const ZERO: Self = Self::new(0.0, 0.0);
}

impl Point<Logical> {
    /// Converts to physical coordinates by multiplying by the scale factor.
    #[inline]
    pub fn to_physical(self, scale_factor: f32) -> Point<Physical> {
        Point::new(self.x * scale_factor, self.y * scale_factor)
    }
}

impl Point<Physical> {
    /// Converts to logical coordinates by dividing by the scale factor.
    #[inline]
    pub fn to_logical(self, scale_factor: f32) -> Point<Logical> {
        Point::new(self.x / scale_factor, self.y / scale_factor)
    }
}

// --- Size ---

impl<S> Size<S> {
    /// Creates a new size.
    #[inline]
    pub const fn new(width: f32, height: f32) -> Self {
        Self {
            width,
            height,
            _space: PhantomData,
        }
    }

    /// A zero size.
    pub const ZERO: Self = Self::new(0.0, 0.0);
}

impl Size<Logical> {
    /// Converts to physical coordinates.
    #[inline]
    pub fn to_physical(self, scale_factor: f32) -> Size<Physical> {
        Size::new(self.width * scale_factor, self.height * scale_factor)
    }
}

impl Size<Physical> {
    /// Converts to logical coordinates.
    #[inline]
    pub fn to_logical(self, scale_factor: f32) -> Size<Logical> {
        Size::new(self.width / scale_factor, self.height / scale_factor)
    }
}

// --- Rect ---

impl<S> Rect<S> {
    /// Creates a new rectangle from origin and size.
    #[inline]
    pub const fn new(origin: Point<S>, size: Size<S>) -> Self {
        Self { origin, size }
    }

    /// Creates a rectangle from position and dimensions.
    #[inline]
    pub const fn from_xywh(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        }
    }

    /// Returns the minimum x coordinate (left edge).
    #[inline]
    pub fn min_x(&self) -> f32 {
        self.origin.x
    }

    /// Returns the minimum y coordinate (top edge).
    #[inline]
    pub fn min_y(&self) -> f32 {
        self.origin.y
    }

    /// Returns the maximum x coordinate (right edge).
    #[inline]
    pub fn max_x(&self) -> f32 {
        self.origin.x + self.size.width
    }

    /// Returns the maximum y coordinate (bottom edge).
    #[inline]
    pub fn max_y(&self) -> f32 {
        self.origin.y + self.size.height
    }

    /// Returns `true` if the point is inside this rectangle.
    #[inline]
    pub fn contains(&self, point: Point<S>) -> bool {
        point.x >= self.min_x()
            && point.x <= self.max_x()
            && point.y >= self.min_y()
            && point.y <= self.max_y()
    }
}

impl Rect<Logical> {
    /// Converts to physical coordinates.
    #[inline]
    pub fn to_physical(self, scale_factor: f32) -> Rect<Physical> {
        Rect::new(
            self.origin.to_physical(scale_factor),
            self.size.to_physical(scale_factor),
        )
    }
}

impl Rect<Physical> {
    /// Converts to logical coordinates.
    #[inline]
    pub fn to_logical(self, scale_factor: f32) -> Rect<Logical> {
        Rect::new(
            self.origin.to_logical(scale_factor),
            self.size.to_logical(scale_factor),
        )
    }
}

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
}
