//! Type-safe wrappers for inner/outer positions and sizes.
//!
//! These newtypes prevent accidentally mixing inner (drawable area) and outer
//! (including window decorations) measurements at the type level.

use astrelis_core::geometry::{Logical, Physical, Point, Size};

/// An inner size (the drawable area, excluding decorations) in physical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InnerSize(pub Size<Physical>);

/// An outer size (including decorations) in physical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OuterSize(pub Size<Physical>);

/// An inner position (top-left of the drawable area) in physical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InnerPosition(pub Point<Physical>);

/// An outer position (top-left of the window frame) in physical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OuterPosition(pub Point<Physical>);

/// An inner size in logical (DPI-independent) coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LogicalInnerSize(pub Size<Logical>);

/// An outer position in logical coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LogicalOuterPosition(pub Point<Logical>);

impl InnerSize {
    /// Creates a new inner size from width and height in physical pixels.
    pub fn new(width: f32, height: f32) -> Self {
        Self(Size::new(width, height))
    }

    /// Converts to logical coordinates.
    pub fn to_logical(self, scale_factor: f32) -> LogicalInnerSize {
        LogicalInnerSize(self.0.to_logical(scale_factor))
    }

    /// Returns the underlying physical size.
    pub fn physical(self) -> Size<Physical> {
        self.0
    }
}

impl OuterSize {
    /// Creates a new outer size from width and height in physical pixels.
    pub fn new(width: f32, height: f32) -> Self {
        Self(Size::new(width, height))
    }

    /// Returns the underlying physical size.
    pub fn physical(self) -> Size<Physical> {
        self.0
    }
}

impl InnerPosition {
    /// Creates a new inner position in physical pixels.
    pub fn new(x: f32, y: f32) -> Self {
        Self(Point::new(x, y))
    }

    /// Returns the underlying physical point.
    pub fn physical(self) -> Point<Physical> {
        self.0
    }
}

impl OuterPosition {
    /// Creates a new outer position in physical pixels.
    pub fn new(x: f32, y: f32) -> Self {
        Self(Point::new(x, y))
    }

    /// Returns the underlying physical point.
    pub fn physical(self) -> Point<Physical> {
        self.0
    }
}

impl LogicalInnerSize {
    /// Creates a new logical inner size.
    pub fn new(width: f32, height: f32) -> Self {
        Self(Size::new(width, height))
    }

    /// Converts to physical coordinates.
    pub fn to_physical(self, scale_factor: f32) -> InnerSize {
        InnerSize(self.0.to_physical(scale_factor))
    }

    /// Returns the underlying logical size.
    pub fn logical(self) -> Size<Logical> {
        self.0
    }
}

impl LogicalOuterPosition {
    /// Creates a new logical outer position.
    pub fn new(x: f32, y: f32) -> Self {
        Self(Point::new(x, y))
    }

    /// Returns the underlying logical point.
    pub fn logical(self) -> Point<Logical> {
        self.0
    }
}
