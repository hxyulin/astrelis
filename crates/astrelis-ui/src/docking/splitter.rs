//! DockSplitter widget - resizable split container with a draggable separator.

use std::any::Any;

use astrelis_core::math::Vec2;
use astrelis_render::Color;
use astrelis_text::FontRenderer;

use crate::style::Style;
use crate::tree::{LayoutRect, NodeId};
use crate::widgets::Widget;

use super::types::{PanelConstraints, SplitDirection, calculate_separator_bounds};

/// Default separator size in pixels.
pub const DEFAULT_SEPARATOR_SIZE: f32 = 6.0;

/// Default separator color.
pub fn default_separator_color() -> Color {
    Color::from_rgb_u8(60, 60, 70)
}

/// Default separator hover color.
pub fn default_separator_hover_color() -> Color {
    Color::from_rgb_u8(80, 100, 140)
}

/// DockSplitter widget - a resizable split container.
///
/// Contains exactly two children separated by a draggable separator.
/// The separator can be dragged to resize the children.
#[derive(Clone)]
pub struct DockSplitter {
    /// Widget style.
    pub style: Style,
    /// Child node IDs (always exactly 2).
    pub children: Vec<NodeId>,
    /// Direction of the split.
    pub direction: SplitDirection,
    /// Split ratio (0.0-1.0), how much the first child gets.
    pub split_ratio: f32,
    /// Width of the separator bar in pixels.
    pub separator_size: f32,
    /// Normal separator color.
    pub separator_color: Color,
    /// Separator color when hovered.
    pub separator_hover_color: Color,
    /// Whether the separator is currently hovered.
    pub is_separator_hovered: bool,
    /// Whether the separator is currently being dragged.
    pub is_separator_dragging: bool,
    /// Constraints for the first panel.
    pub first_constraints: PanelConstraints,
    /// Constraints for the second panel.
    pub second_constraints: PanelConstraints,
}

impl DockSplitter {
    /// Create a new horizontal split (left/right panels).
    pub fn horizontal() -> Self {
        Self::new(SplitDirection::Horizontal)
    }

    /// Create a new vertical split (top/bottom panels).
    pub fn vertical() -> Self {
        Self::new(SplitDirection::Vertical)
    }

    /// Create a new split with the given direction.
    pub fn new(direction: SplitDirection) -> Self {
        Self {
            style: Style::new().display(taffy::Display::Flex),
            children: Vec::new(),
            direction,
            split_ratio: 0.5,
            separator_size: DEFAULT_SEPARATOR_SIZE,
            separator_color: default_separator_color(),
            separator_hover_color: default_separator_hover_color(),
            is_separator_hovered: false,
            is_separator_dragging: false,
            first_constraints: PanelConstraints::default(),
            second_constraints: PanelConstraints::default(),
        }
    }

    /// Set the split ratio (0.0-1.0).
    pub fn split_ratio(mut self, ratio: f32) -> Self {
        self.split_ratio = ratio.clamp(0.0, 1.0);
        self
    }

    /// Set the separator size in pixels.
    pub fn separator_size(mut self, size: f32) -> Self {
        self.separator_size = size.max(1.0);
        self
    }

    /// Set the separator colors.
    pub fn separator_colors(mut self, normal: Color, hover: Color) -> Self {
        self.separator_color = normal;
        self.separator_hover_color = hover;
        self
    }

    /// Set constraints for the first panel.
    pub fn first_constraints(mut self, constraints: PanelConstraints) -> Self {
        self.first_constraints = constraints;
        self
    }

    /// Set constraints for the second panel.
    pub fn second_constraints(mut self, constraints: PanelConstraints) -> Self {
        self.second_constraints = constraints;
        self
    }

    /// Get the separator bounds for hit testing.
    pub fn separator_bounds(&self, layout: &LayoutRect) -> LayoutRect {
        calculate_separator_bounds(layout, self.direction, self.split_ratio, self.separator_size)
    }

    /// Check if a point is within the separator bounds.
    pub fn is_point_in_separator(&self, layout: &LayoutRect, point: Vec2) -> bool {
        let sep = self.separator_bounds(layout);
        point.x >= sep.x
            && point.x <= sep.x + sep.width
            && point.y >= sep.y
            && point.y <= sep.y + sep.height
    }

    /// Apply a drag delta to update the split ratio.
    ///
    /// Returns the new ratio clamped to constraints.
    pub fn apply_drag_delta(&mut self, delta: Vec2, layout: &LayoutRect) -> f32 {
        self.apply_drag_delta_from_original(delta, layout, self.split_ratio)
    }

    /// Apply a drag delta from an original ratio to update the split ratio.
    ///
    /// This should be used during dragging where the delta is calculated from
    /// the drag start position and should be applied to the original ratio.
    ///
    /// Returns the new ratio clamped to constraints.
    pub fn apply_drag_delta_from_original(
        &mut self,
        delta: Vec2,
        layout: &LayoutRect,
        original_ratio: f32,
    ) -> f32 {
        let total_size = match self.direction {
            SplitDirection::Horizontal => layout.width,
            SplitDirection::Vertical => layout.height,
        };

        if total_size <= 0.0 {
            return self.split_ratio;
        }

        let delta_component = match self.direction {
            SplitDirection::Horizontal => delta.x,
            SplitDirection::Vertical => delta.y,
        };

        // Convert delta to ratio change and apply to the ORIGINAL ratio
        let ratio_delta = delta_component / total_size;
        let new_ratio = (original_ratio + ratio_delta).clamp(0.0, 1.0);

        // Apply constraints
        let first_size = total_size * new_ratio - self.separator_size / 2.0;
        let second_size = total_size * (1.0 - new_ratio) - self.separator_size / 2.0;

        let first_clamped = self.first_constraints.clamp(first_size);
        let second_clamped = self.second_constraints.clamp(second_size);

        // Calculate the final ratio respecting constraints
        let final_ratio = if first_clamped != first_size {
            (first_clamped + self.separator_size / 2.0) / total_size
        } else if second_clamped != second_size {
            1.0 - (second_clamped + self.separator_size / 2.0) / total_size
        } else {
            new_ratio
        };

        self.split_ratio = final_ratio.clamp(0.0, 1.0);
        self.split_ratio
    }

    /// Get the current separator color based on hover/drag state.
    pub fn current_separator_color(&self) -> Color {
        if self.is_separator_dragging || self.is_separator_hovered {
            self.separator_hover_color
        } else {
            self.separator_color
        }
    }

    /// Set the hover state of the separator.
    pub fn set_separator_hovered(&mut self, hovered: bool) {
        self.is_separator_hovered = hovered;
    }

    /// Set the dragging state of the separator.
    pub fn set_separator_dragging(&mut self, dragging: bool) {
        self.is_separator_dragging = dragging;
    }

    /// Calculate the first child's layout bounds.
    pub fn first_panel_layout(&self, layout: &LayoutRect) -> LayoutRect {
        let half_sep = self.separator_size / 2.0;
        match self.direction {
            SplitDirection::Horizontal => {
                let width = (layout.width * self.split_ratio - half_sep).max(0.0);
                LayoutRect {
                    x: layout.x,
                    y: layout.y,
                    width,
                    height: layout.height,
                }
            }
            SplitDirection::Vertical => {
                let height = (layout.height * self.split_ratio - half_sep).max(0.0);
                LayoutRect {
                    x: layout.x,
                    y: layout.y,
                    width: layout.width,
                    height,
                }
            }
        }
    }

    /// Calculate the second child's layout bounds.
    pub fn second_panel_layout(&self, layout: &LayoutRect) -> LayoutRect {
        let half_sep = self.separator_size / 2.0;
        match self.direction {
            SplitDirection::Horizontal => {
                let split_x = layout.width * self.split_ratio;
                let x = layout.x + split_x + half_sep;
                let width = (layout.width - split_x - half_sep).max(0.0);
                LayoutRect {
                    x,
                    y: layout.y,
                    width,
                    height: layout.height,
                }
            }
            SplitDirection::Vertical => {
                let split_y = layout.height * self.split_ratio;
                let y = layout.y + split_y + half_sep;
                let height = (layout.height - split_y - half_sep).max(0.0);
                LayoutRect {
                    x: layout.x,
                    y,
                    width: layout.width,
                    height,
                }
            }
        }
    }
}

impl Widget for DockSplitter {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }

    fn children(&self) -> &[NodeId] {
        &self.children
    }

    fn children_mut(&mut self) -> Option<&mut Vec<NodeId>> {
        Some(&mut self.children)
    }

    fn measure(&self, _available_space: Vec2, _font_renderer: Option<&FontRenderer>) -> Vec2 {
        // Splitters typically fill their container, no intrinsic size
        Vec2::ZERO
    }

    fn clone_box(&self) -> Box<dyn Widget> {
        Box::new(self.clone())
    }
}
