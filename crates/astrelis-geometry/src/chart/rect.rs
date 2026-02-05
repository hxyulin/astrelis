//! Rectangular bounds utility for chart rendering.

use glam::Vec2;

/// Rectangular bounds for chart rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// X position (left)
    pub x: f32,
    /// Y position (top)
    pub y: f32,
    /// Width
    pub width: f32,
    /// Height
    pub height: f32,
}

impl Rect {
    /// Create a new rect.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Create from position and size.
    pub fn from_pos_size(pos: Vec2, size: Vec2) -> Self {
        Self {
            x: pos.x,
            y: pos.y,
            width: size.x,
            height: size.y,
        }
    }

    /// Get the position as a Vec2.
    pub fn position(&self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }

    /// Get the size as a Vec2.
    pub fn size(&self) -> Vec2 {
        Vec2::new(self.width, self.height)
    }

    /// Get the center point.
    pub fn center(&self) -> Vec2 {
        Vec2::new(self.x + self.width * 0.5, self.y + self.height * 0.5)
    }

    /// Inset the rect by a padding amount.
    pub fn inset(&self, padding: f32) -> Self {
        Self {
            x: self.x + padding,
            y: self.y + padding,
            width: (self.width - padding * 2.0).max(0.0),
            height: (self.height - padding * 2.0).max(0.0),
        }
    }

    /// Check if a point is inside the rect.
    pub fn contains(&self, point: Vec2) -> bool {
        point.x >= self.x
            && point.x <= self.x + self.width
            && point.y >= self.y
            && point.y <= self.y + self.height
    }

    /// Get the right edge.
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// Get the bottom edge.
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }
}
