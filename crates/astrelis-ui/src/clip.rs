//! Clip rectangle system for overflow clipping.
//!
//! Provides clip rectangles for GPU scissor-based content clipping.
//! Used in conjunction with the Overflow style property to clip content
//! that exceeds widget boundaries.

use crate::style::Overflow;
use crate::tree::LayoutRect;
use astrelis_core::math::Vec2;

/// A clip rectangle defining a region for scissor clipping.
///
/// Clip rectangles are used with GPU scissor tests to efficiently clip
/// content during rendering. They define axis-aligned rectangular regions
/// where pixels should be rendered.
///
/// # Examples
/// ```
/// use astrelis_ui::clip::ClipRect;
/// use astrelis_core::math::Vec2;
///
/// let clip = ClipRect::from_bounds(10.0, 20.0, 400.0, 300.0);
/// assert!(clip.contains(Vec2::new(100.0, 100.0)));
/// assert!(!clip.contains(Vec2::new(500.0, 100.0)));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ClipRect {
    /// Minimum point (top-left corner)
    pub min: Vec2,
    /// Maximum point (bottom-right corner)
    pub max: Vec2,
}

impl ClipRect {
    /// Create a clip rect that encompasses the entire viewport (no clipping).
    pub fn infinite() -> Self {
        Self {
            min: Vec2::new(f32::NEG_INFINITY, f32::NEG_INFINITY),
            max: Vec2::new(f32::INFINITY, f32::INFINITY),
        }
    }

    /// Create a clip rect from position and size.
    ///
    /// # Arguments
    /// * `x` - Left edge X coordinate
    /// * `y` - Top edge Y coordinate
    /// * `width` - Width of the clip region
    /// * `height` - Height of the clip region
    pub fn from_bounds(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            min: Vec2::new(x, y),
            max: Vec2::new(x + width, y + height),
        }
    }

    /// Create a clip rect from a layout rect.
    pub fn from_layout(layout: &LayoutRect) -> Self {
        Self::from_bounds(layout.x, layout.y, layout.width, layout.height)
    }

    /// Create a clip rect from min/max points.
    pub fn from_min_max(min: Vec2, max: Vec2) -> Self {
        Self { min, max }
    }

    /// Check if this clip rect is infinite (no clipping).
    pub fn is_infinite(&self) -> bool {
        self.min.x == f32::NEG_INFINITY
            || self.min.y == f32::NEG_INFINITY
            || self.max.x == f32::INFINITY
            || self.max.y == f32::INFINITY
    }

    /// Check if a point is inside this clip rect.
    pub fn contains(&self, point: Vec2) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }

    /// Check if this clip rect intersects with another.
    pub fn intersects(&self, other: &ClipRect) -> bool {
        self.min.x < other.max.x
            && self.max.x > other.min.x
            && self.min.y < other.max.y
            && self.max.y > other.min.y
    }

    /// Compute the intersection of two clip rects.
    ///
    /// Returns a clip rect that represents the overlapping region.
    /// If the rects don't overlap, returns a zero-area rect.
    pub fn intersect(&self, other: &ClipRect) -> ClipRect {
        ClipRect {
            min: Vec2::new(self.min.x.max(other.min.x), self.min.y.max(other.min.y)),
            max: Vec2::new(self.max.x.min(other.max.x), self.max.y.min(other.max.y)),
        }
    }

    /// Get the width of the clip rect.
    pub fn width(&self) -> f32 {
        (self.max.x - self.min.x).max(0.0)
    }

    /// Get the height of the clip rect.
    pub fn height(&self) -> f32 {
        (self.max.y - self.min.y).max(0.0)
    }

    /// Check if the clip rect has positive area.
    pub fn has_area(&self) -> bool {
        self.width() > 0.0 && self.height() > 0.0
    }

    /// Convert to physical pixel coordinates.
    ///
    /// # Arguments
    /// * `scale_factor` - Display scale factor (e.g., 2.0 for Retina)
    pub fn to_physical(&self, scale_factor: f64) -> PhysicalClipRect {
        let scale = scale_factor as f32;
        PhysicalClipRect {
            x: (self.min.x * scale).round() as u32,
            y: (self.min.y * scale).round() as u32,
            width: (self.width() * scale).round() as u32,
            height: (self.height() * scale).round() as u32,
        }
    }
}

impl Default for ClipRect {
    fn default() -> Self {
        Self::infinite()
    }
}

impl PartialEq for ClipRect {
    fn eq(&self, other: &Self) -> bool {
        // Compare using bit-level equality for floats to handle infinity correctly
        self.min.x.to_bits() == other.min.x.to_bits()
            && self.min.y.to_bits() == other.min.y.to_bits()
            && self.max.x.to_bits() == other.max.x.to_bits()
            && self.max.y.to_bits() == other.max.y.to_bits()
    }
}

impl Eq for ClipRect {}

impl std::hash::Hash for ClipRect {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Use bit representation for deterministic hashing of floats (including infinity)
        self.min.x.to_bits().hash(state);
        self.min.y.to_bits().hash(state);
        self.max.x.to_bits().hash(state);
        self.max.y.to_bits().hash(state);
    }
}

/// A clip rectangle in physical pixel coordinates.
///
/// Used directly with GPU scissor rect APIs which require integer pixel values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalClipRect {
    /// X coordinate of the left edge
    pub x: u32,
    /// Y coordinate of the top edge
    pub y: u32,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
}

impl PhysicalClipRect {
    /// Create a new physical clip rect.
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Clamp the clip rect to fit within viewport bounds.
    ///
    /// GPU scissor rects must not exceed the framebuffer dimensions.
    pub fn clamp_to_viewport(&self, viewport_width: u32, viewport_height: u32) -> Self {
        let x = self.x.min(viewport_width);
        let y = self.y.min(viewport_height);
        let width = self.width.min(viewport_width.saturating_sub(x));
        let height = self.height.min(viewport_height.saturating_sub(y));
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Determines if a node should clip its children based on overflow settings.
pub fn should_clip(overflow_x: Overflow, overflow_y: Overflow) -> bool {
    matches!(
        overflow_x,
        Overflow::Hidden | Overflow::Scroll | Overflow::Auto
    ) || matches!(
        overflow_y,
        Overflow::Hidden | Overflow::Scroll | Overflow::Auto
    )
}

/// Compute the clip rect for a node, considering its parent's clip.
///
/// This function handles the nested clipping case where a child's clip
/// must be intersected with its parent's clip to ensure proper rendering.
///
/// # Arguments
/// * `node_layout` - The layout rect of the node
/// * `overflow_x` - Horizontal overflow setting
/// * `overflow_y` - Vertical overflow setting
/// * `parent_clip` - The clip rect inherited from the parent (if any)
///
/// # Returns
/// The effective clip rect for this node's children.
pub fn compute_clip_rect(
    node_layout: &LayoutRect,
    overflow_x: Overflow,
    overflow_y: Overflow,
    parent_clip: Option<ClipRect>,
) -> ClipRect {
    // Start with the parent clip or infinite
    let mut clip = parent_clip.unwrap_or_else(ClipRect::infinite);

    // If this node has clipping, intersect with its bounds
    if should_clip(overflow_x, overflow_y) {
        let node_clip = ClipRect::from_layout(node_layout);
        clip = clip.intersect(&node_clip);
    }

    clip
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clip_rect_from_bounds() {
        let clip = ClipRect::from_bounds(10.0, 20.0, 100.0, 50.0);
        assert_eq!(clip.min, Vec2::new(10.0, 20.0));
        assert_eq!(clip.max, Vec2::new(110.0, 70.0));
        assert_eq!(clip.width(), 100.0);
        assert_eq!(clip.height(), 50.0);
    }

    #[test]
    fn test_clip_rect_contains() {
        let clip = ClipRect::from_bounds(0.0, 0.0, 100.0, 100.0);
        assert!(clip.contains(Vec2::new(50.0, 50.0)));
        assert!(clip.contains(Vec2::new(0.0, 0.0)));
        assert!(clip.contains(Vec2::new(100.0, 100.0)));
        assert!(!clip.contains(Vec2::new(101.0, 50.0)));
        assert!(!clip.contains(Vec2::new(-1.0, 50.0)));
    }

    #[test]
    fn test_clip_rect_intersect() {
        let a = ClipRect::from_bounds(0.0, 0.0, 100.0, 100.0);
        let b = ClipRect::from_bounds(50.0, 50.0, 100.0, 100.0);
        let intersection = a.intersect(&b);

        assert_eq!(intersection.min, Vec2::new(50.0, 50.0));
        assert_eq!(intersection.max, Vec2::new(100.0, 100.0));
        assert_eq!(intersection.width(), 50.0);
        assert_eq!(intersection.height(), 50.0);
    }

    #[test]
    fn test_clip_rect_no_intersection() {
        let a = ClipRect::from_bounds(0.0, 0.0, 50.0, 50.0);
        let b = ClipRect::from_bounds(100.0, 100.0, 50.0, 50.0);
        let intersection = a.intersect(&b);

        // No overlap results in negative/zero dimensions
        assert!(!intersection.has_area());
    }

    #[test]
    fn test_clip_rect_infinite() {
        let infinite = ClipRect::infinite();
        assert!(infinite.is_infinite());

        let finite = ClipRect::from_bounds(0.0, 0.0, 100.0, 100.0);
        assert!(!finite.is_infinite());

        // Intersecting with infinite returns the finite rect
        let intersection = infinite.intersect(&finite);
        assert_eq!(intersection.min, finite.min);
        assert_eq!(intersection.max, finite.max);
    }

    #[test]
    fn test_clip_rect_to_physical() {
        let clip = ClipRect::from_bounds(10.5, 20.5, 100.0, 50.0);
        let physical = clip.to_physical(2.0);

        assert_eq!(physical.x, 21); // 10.5 * 2 rounded
        assert_eq!(physical.y, 41); // 20.5 * 2 rounded
        assert_eq!(physical.width, 200); // 100 * 2
        assert_eq!(physical.height, 100); // 50 * 2
    }

    #[test]
    fn test_physical_clip_rect_clamp() {
        let physical = PhysicalClipRect::new(100, 100, 500, 500);
        let clamped = physical.clamp_to_viewport(400, 300);

        assert_eq!(clamped.x, 100);
        assert_eq!(clamped.y, 100);
        assert_eq!(clamped.width, 300); // Clamped to fit within 400-100
        assert_eq!(clamped.height, 200); // Clamped to fit within 300-100
    }

    #[test]
    fn test_should_clip() {
        assert!(!should_clip(Overflow::Visible, Overflow::Visible));
        assert!(should_clip(Overflow::Hidden, Overflow::Visible));
        assert!(should_clip(Overflow::Visible, Overflow::Hidden));
        assert!(should_clip(Overflow::Hidden, Overflow::Hidden));
        assert!(should_clip(Overflow::Scroll, Overflow::Visible));
        assert!(should_clip(Overflow::Auto, Overflow::Visible));
    }

    #[test]
    fn test_compute_clip_rect() {
        let layout = LayoutRect {
            x: 100.0,
            y: 100.0,
            width: 200.0,
            height: 150.0,
        };

        // No clipping
        let clip = compute_clip_rect(&layout, Overflow::Visible, Overflow::Visible, None);
        assert!(clip.is_infinite());

        // Hidden overflow
        let clip = compute_clip_rect(&layout, Overflow::Hidden, Overflow::Hidden, None);
        assert!(!clip.is_infinite());
        assert_eq!(clip.min, Vec2::new(100.0, 100.0));
        assert_eq!(clip.max, Vec2::new(300.0, 250.0));

        // With parent clip
        let parent_clip = ClipRect::from_bounds(50.0, 50.0, 300.0, 250.0);
        let clip = compute_clip_rect(
            &layout,
            Overflow::Hidden,
            Overflow::Hidden,
            Some(parent_clip),
        );
        // Should intersect with parent
        assert_eq!(clip.min, Vec2::new(100.0, 100.0));
        assert_eq!(clip.max, Vec2::new(300.0, 250.0));
    }
}
