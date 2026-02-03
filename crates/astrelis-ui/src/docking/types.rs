//! Shared types for the docking system.

use astrelis_core::math::Vec2;
use crate::tree::{LayoutRect, NodeId};

/// Direction of a split container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SplitDirection {
    /// Left/Right panels (vertical separator line)
    #[default]
    Horizontal,
    /// Top/Bottom panels (horizontal separator line)
    Vertical,
}

impl SplitDirection {
    /// Get the perpendicular direction.
    pub fn perpendicular(&self) -> Self {
        match self {
            SplitDirection::Horizontal => SplitDirection::Vertical,
            SplitDirection::Vertical => SplitDirection::Horizontal,
        }
    }

    /// Check if this is a horizontal split.
    pub fn is_horizontal(&self) -> bool {
        matches!(self, SplitDirection::Horizontal)
    }

    /// Check if this is a vertical split.
    pub fn is_vertical(&self) -> bool {
        matches!(self, SplitDirection::Vertical)
    }
}

/// Size constraints for a panel.
#[derive(Debug, Clone, Copy)]
pub struct PanelConstraints {
    /// Minimum size in pixels.
    pub min_size: f32,
    /// Maximum size in pixels (None = unlimited).
    pub max_size: Option<f32>,
}

impl Default for PanelConstraints {
    fn default() -> Self {
        Self {
            min_size: 50.0,
            max_size: None,
        }
    }
}

impl PanelConstraints {
    /// Create constraints with a minimum size.
    pub fn min(min_size: f32) -> Self {
        Self {
            min_size,
            max_size: None,
        }
    }

    /// Create constraints with both min and max size.
    pub fn min_max(min_size: f32, max_size: f32) -> Self {
        Self {
            min_size,
            max_size: Some(max_size),
        }
    }

    /// Clamp a size value to the constraints.
    pub fn clamp(&self, size: f32) -> f32 {
        let mut result = size.max(self.min_size);
        if let Some(max) = self.max_size {
            result = result.min(max);
        }
        result
    }
}

/// Dock zone for drop preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockZone {
    /// Left side of the target.
    Left,
    /// Right side of the target.
    Right,
    /// Top side of the target.
    Top,
    /// Bottom side of the target.
    Bottom,
    /// Center (tabbed).
    Center,
}

impl DockZone {
    /// Get the split direction for this zone.
    pub fn split_direction(&self) -> Option<SplitDirection> {
        match self {
            DockZone::Left | DockZone::Right => Some(SplitDirection::Horizontal),
            DockZone::Top | DockZone::Bottom => Some(SplitDirection::Vertical),
            DockZone::Center => None,
        }
    }

    /// Check if this zone creates a new panel before the existing content.
    pub fn is_before(&self) -> bool {
        matches!(self, DockZone::Left | DockZone::Top)
    }
}

/// Type of drag operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragType {
    /// Dragging a splitter separator to resize panels.
    SplitterResize {
        /// The splitter node being resized.
        splitter_node: NodeId,
        /// Direction of the split.
        direction: SplitDirection,
    },
    /// Dragging a panel/tab to move it.
    PanelMove {
        /// The panel node being moved.
        panel_node: NodeId,
    },
    /// Dragging a tab to reorder or undock.
    TabDrag {
        /// The tabs container node.
        tabs_node: NodeId,
        /// The index of the tab being dragged.
        tab_index: usize,
    },
}

/// Drag threshold in pixels before a drag operation starts.
pub const DRAG_THRESHOLD: f32 = 5.0;

/// State of an active drag operation.
#[derive(Debug, Clone)]
pub struct DragState {
    /// Type of drag operation.
    pub drag_type: DragType,
    /// Position where the drag started.
    pub start_pos: Vec2,
    /// Current drag position.
    pub current_pos: Vec2,
    /// Whether the drag threshold has been exceeded.
    pub is_active: bool,
    /// Original value being dragged (e.g., split ratio).
    pub original_value: f32,
}

impl DragState {
    /// Create a new drag state.
    pub fn new(drag_type: DragType, start_pos: Vec2, original_value: f32) -> Self {
        Self {
            drag_type,
            start_pos,
            current_pos: start_pos,
            is_active: false,
            original_value,
        }
    }

    /// Update the current position and check if threshold exceeded.
    pub fn update(&mut self, pos: Vec2) {
        self.current_pos = pos;
        if !self.is_active {
            let delta = pos - self.start_pos;
            if delta.length() >= DRAG_THRESHOLD {
                self.is_active = true;
            }
        }
    }

    /// Get the drag delta from start.
    pub fn delta(&self) -> Vec2 {
        self.current_pos - self.start_pos
    }
}

/// Separator hit test result.
#[derive(Debug, Clone, Copy)]
pub struct SeparatorHit {
    /// The splitter node that owns the separator.
    pub splitter_node: NodeId,
    /// The direction of the split.
    pub direction: SplitDirection,
    /// The current split ratio.
    pub current_ratio: f32,
}

/// Calculate separator bounds from a layout rect and split parameters.
pub fn calculate_separator_bounds(
    layout: &LayoutRect,
    direction: SplitDirection,
    split_ratio: f32,
    separator_size: f32,
) -> LayoutRect {
    match direction {
        SplitDirection::Horizontal => {
            // Vertical separator line (left/right split)
            let split_x = layout.width * split_ratio;
            let sep_x = split_x - separator_size / 2.0;
            LayoutRect {
                x: layout.x + sep_x,
                y: layout.y,
                width: separator_size,
                height: layout.height,
            }
        }
        SplitDirection::Vertical => {
            // Horizontal separator line (top/bottom split)
            let split_y = layout.height * split_ratio;
            let sep_y = split_y - separator_size / 2.0;
            LayoutRect {
                x: layout.x,
                y: layout.y + sep_y,
                width: layout.width,
                height: separator_size,
            }
        }
    }
}

/// Calculate the layout for each child panel of a splitter.
pub fn calculate_panel_layouts(
    layout: &LayoutRect,
    direction: SplitDirection,
    split_ratio: f32,
    separator_size: f32,
) -> (LayoutRect, LayoutRect) {
    let half_sep = separator_size / 2.0;

    match direction {
        SplitDirection::Horizontal => {
            let split_x = layout.width * split_ratio;
            let first = LayoutRect {
                x: layout.x,
                y: layout.y,
                width: (split_x - half_sep).max(0.0),
                height: layout.height,
            };
            let second = LayoutRect {
                x: layout.x + split_x + half_sep,
                y: layout.y,
                width: (layout.width - split_x - half_sep).max(0.0),
                height: layout.height,
            };
            (first, second)
        }
        SplitDirection::Vertical => {
            let split_y = layout.height * split_ratio;
            let first = LayoutRect {
                x: layout.x,
                y: layout.y,
                width: layout.width,
                height: (split_y - half_sep).max(0.0),
            };
            let second = LayoutRect {
                x: layout.x,
                y: layout.y + split_y + half_sep,
                width: layout.width,
                height: (layout.height - split_y - half_sep).max(0.0),
            };
            (first, second)
        }
    }
}
