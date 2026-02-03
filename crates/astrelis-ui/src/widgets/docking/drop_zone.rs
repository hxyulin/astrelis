//! Drop zone detection for cross-container tab dragging.

use super::types::DockZone;
use crate::tree::{LayoutRect, NodeId};
use astrelis_core::math::Vec2;

/// Threshold for edge zones (25% of width/height by default).
pub const DEFAULT_EDGE_THRESHOLD: f32 = 0.25;

/// Drop zone detector for finding where tabs can be dropped.
///
/// Divides a rectangular area into 5 zones:
/// - Left: x < 25%
/// - Right: x > 75%
/// - Top: y < 25%
/// - Bottom: y > 75%
/// - Center: everything else
#[derive(Debug, Clone)]
pub struct DropZoneDetector {
    /// Edge threshold as a fraction (0.0-0.5).
    /// Default: 0.25 (25% of width/height)
    pub edge_threshold: f32,
}

impl Default for DropZoneDetector {
    fn default() -> Self {
        Self {
            edge_threshold: DEFAULT_EDGE_THRESHOLD,
        }
    }
}

impl DropZoneDetector {
    /// Create a new drop zone detector with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a detector with a custom edge threshold.
    pub fn with_edge_threshold(mut self, threshold: f32) -> Self {
        self.edge_threshold = threshold.clamp(0.1, 0.5);
        self
    }

    /// Detect which drop zone a cursor is in relative to a target bounds.
    ///
    /// Returns None if the cursor is not within the target bounds or the bounds have no area.
    pub fn detect_zone(&self, cursor: Vec2, bounds: LayoutRect) -> Option<DockZone> {
        // Guard against zero-area bounds (avoid division by zero)
        if bounds.width <= 0.0 || bounds.height <= 0.0 {
            return None;
        }

        // Check if cursor is within bounds
        if !bounds.contains(cursor) {
            return None;
        }

        // Calculate relative position within bounds (0.0-1.0)
        let rel_x = (cursor.x - bounds.x) / bounds.width;
        let rel_y = (cursor.y - bounds.y) / bounds.height;

        // Check edge zones first (priority: edges > center)
        if rel_x < self.edge_threshold {
            Some(DockZone::Left)
        } else if rel_x > (1.0 - self.edge_threshold) {
            Some(DockZone::Right)
        } else if rel_y < self.edge_threshold {
            Some(DockZone::Top)
        } else if rel_y > (1.0 - self.edge_threshold) {
            Some(DockZone::Bottom)
        } else {
            Some(DockZone::Center)
        }
    }

    /// Calculate the preview bounds for a given zone.
    ///
    /// Returns a rectangle showing where the dropped tab will appear.
    pub fn preview_bounds(&self, zone: DockZone, target: LayoutRect) -> LayoutRect {
        match zone {
            DockZone::Left => LayoutRect {
                x: target.x,
                y: target.y,
                width: target.width * self.edge_threshold * 2.0, // Double for visibility
                height: target.height,
            },
            DockZone::Right => {
                let width = target.width * self.edge_threshold * 2.0;
                LayoutRect {
                    x: target.x + target.width - width,
                    y: target.y,
                    width,
                    height: target.height,
                }
            }
            DockZone::Top => LayoutRect {
                x: target.x,
                y: target.y,
                width: target.width,
                height: target.height * self.edge_threshold * 2.0,
            },
            DockZone::Bottom => {
                let height = target.height * self.edge_threshold * 2.0;
                LayoutRect {
                    x: target.x,
                    y: target.y + target.height - height,
                    width: target.width,
                    height,
                }
            }
            DockZone::Center => {
                // Center zone takes full area (for tabbed drop)
                target
            }
        }
    }
}

/// A potential drop target for a dragged tab.
#[derive(Debug, Clone, Copy)]
pub struct DropTarget {
    /// The container node where the tab can be dropped.
    pub container_id: NodeId,
    /// The zone within the container.
    pub zone: DockZone,
    /// For center zone: insertion index for the tab.
    /// For edge zones: None (will create new container).
    pub insert_index: Option<usize>,
}

impl DropTarget {
    /// Create a new drop target.
    pub fn new(container_id: NodeId, zone: DockZone) -> Self {
        Self {
            container_id,
            zone,
            insert_index: None,
        }
    }

    /// Set the insertion index for center zone drops.
    pub fn with_insert_index(mut self, index: usize) -> Self {
        self.insert_index = Some(index);
        self
    }

    /// Check if this is an edge zone drop (requires split).
    pub fn is_edge_drop(&self) -> bool {
        !matches!(self.zone, DockZone::Center)
    }

    /// Check if this is a center zone drop (tabbed).
    pub fn is_center_drop(&self) -> bool {
        matches!(self.zone, DockZone::Center)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_left_zone() {
        let detector = DropZoneDetector::new();
        let bounds = LayoutRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        };

        // Left edge
        assert_eq!(
            detector.detect_zone(Vec2::new(10.0, 50.0), bounds),
            Some(DockZone::Left)
        );
    }

    #[test]
    fn test_detect_right_zone() {
        let detector = DropZoneDetector::new();
        let bounds = LayoutRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        };

        // Right edge
        assert_eq!(
            detector.detect_zone(Vec2::new(90.0, 50.0), bounds),
            Some(DockZone::Right)
        );
    }

    #[test]
    fn test_detect_center_zone() {
        let detector = DropZoneDetector::new();
        let bounds = LayoutRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        };

        // Center
        assert_eq!(
            detector.detect_zone(Vec2::new(50.0, 50.0), bounds),
            Some(DockZone::Center)
        );
    }

    #[test]
    fn test_detect_outside_bounds() {
        let detector = DropZoneDetector::new();
        let bounds = LayoutRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        };

        // Outside bounds
        assert_eq!(detector.detect_zone(Vec2::new(150.0, 50.0), bounds), None);
    }

    #[test]
    fn test_preview_bounds_left() {
        let detector = DropZoneDetector::new();
        let target = LayoutRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        };

        let preview = detector.preview_bounds(DockZone::Left, target);
        assert_eq!(preview.x, 0.0);
        assert_eq!(preview.y, 0.0);
        assert_eq!(preview.width, 50.0); // 25% * 2
        assert_eq!(preview.height, 100.0);
    }

    #[test]
    fn test_preview_bounds_center() {
        let detector = DropZoneDetector::new();
        let target = LayoutRect {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 80.0,
        };

        let preview = detector.preview_bounds(DockZone::Center, target);
        assert_eq!(preview.x, target.x);
        assert_eq!(preview.y, target.y);
        assert_eq!(preview.width, target.width);
        assert_eq!(preview.height, target.height);
    }

    #[test]
    fn test_custom_edge_threshold() {
        let detector = DropZoneDetector::new().with_edge_threshold(0.3);
        let bounds = LayoutRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        };

        // At 28% should be left zone
        assert_eq!(
            detector.detect_zone(Vec2::new(28.0, 50.0), bounds),
            Some(DockZone::Left)
        );

        // At 35% should be center zone
        assert_eq!(
            detector.detect_zone(Vec2::new(35.0, 50.0), bounds),
            Some(DockZone::Center)
        );
    }
}
