//! DockTabs widget - tabbed container showing one panel at a time.

use std::any::Any;

use astrelis_core::math::Vec2;
use astrelis_render::Color;
use astrelis_text::FontRenderer;

use crate::style::Style;
use crate::tree::{LayoutRect, NodeId};
use crate::widgets::Widget;

/// Default tab bar height in pixels.
pub const DEFAULT_TAB_BAR_HEIGHT: f32 = 28.0;

/// Default tab padding in pixels.
pub const DEFAULT_TAB_PADDING: f32 = 12.0;

/// Default tab close button size in pixels.
pub const DEFAULT_CLOSE_BUTTON_SIZE: f32 = 16.0;

/// Default tab bar color.
pub fn default_tab_bar_color() -> Color {
    Color::from_rgb_u8(40, 40, 50)
}

/// Default active tab color.
pub fn default_active_tab_color() -> Color {
    Color::from_rgb_u8(60, 60, 80)
}

/// Default inactive tab color.
pub fn default_inactive_tab_color() -> Color {
    Color::from_rgb_u8(50, 50, 60)
}

/// Default tab text color.
pub fn default_tab_text_color() -> Color {
    Color::WHITE
}

/// Default tab hover color.
pub fn default_tab_hover_color() -> Color {
    Color::from_rgb_u8(70, 70, 90)
}

/// DockTabs widget - a tabbed container.
///
/// Contains multiple children, showing one at a time with a tab bar.
#[derive(Clone)]
pub struct DockTabs {
    /// Widget style.
    pub style: Style,
    /// Child node IDs (tab content).
    pub children: Vec<NodeId>,
    /// Tab labels.
    pub tab_labels: Vec<String>,
    /// Currently active tab index.
    pub active_tab: usize,
    /// Height of the tab bar in pixels.
    pub tab_bar_height: f32,
    /// Tab bar background color.
    pub tab_bar_color: Color,
    /// Active tab background color.
    pub active_tab_color: Color,
    /// Inactive tab background color.
    pub inactive_tab_color: Color,
    /// Tab text color.
    pub tab_text_color: Color,
    /// Tab hover color.
    pub tab_hover_color: Color,
    /// Index of hovered tab (None if no hover).
    pub hovered_tab: Option<usize>,
    /// Whether to show close buttons on tabs.
    pub closable: bool,
    /// Tab font size.
    pub tab_font_size: f32,
    /// Cached tab widths (computed during rendering).
    pub(crate) tab_widths: Vec<f32>,
    /// Index of tab currently being dragged (None if no drag).
    pub dragging_tab_index: Option<usize>,
    /// Index where dragged tab will be dropped (insertion point).
    pub drag_drop_target: Option<usize>,
    /// Current cursor position during drag (for ghost rendering).
    pub drag_cursor_pos: Option<Vec2>,
}

impl DockTabs {
    /// Create a new empty tabs container.
    pub fn new() -> Self {
        Self {
            style: Style::new().display(taffy::Display::Flex),
            children: Vec::new(),
            tab_labels: Vec::new(),
            active_tab: 0,
            tab_bar_height: DEFAULT_TAB_BAR_HEIGHT,
            tab_bar_color: default_tab_bar_color(),
            active_tab_color: default_active_tab_color(),
            inactive_tab_color: default_inactive_tab_color(),
            tab_text_color: default_tab_text_color(),
            tab_hover_color: default_tab_hover_color(),
            hovered_tab: None,
            closable: false,
            tab_font_size: 13.0,
            tab_widths: Vec::new(),
            dragging_tab_index: None,
            drag_drop_target: None,
            drag_cursor_pos: None,
        }
    }

    /// Add a tab with a label and content node.
    pub fn add_tab(&mut self, label: impl Into<String>, content: NodeId) {
        self.tab_labels.push(label.into());
        self.children.push(content);
    }

    /// Set the active tab index.
    /// Returns the previous active tab index if changed.
    pub fn set_active_tab(&mut self, index: usize) -> Option<usize> {
        if index < self.children.len() && index != self.active_tab {
            let old_active = self.active_tab;
            self.active_tab = index;
            Some(old_active)
        } else {
            None
        }
    }

    /// Set the hovered tab index.
    pub fn set_hovered_tab(&mut self, index: Option<usize>) {
        self.hovered_tab = index;
    }

    /// Close a tab at the given index.
    ///
    /// Returns the removed content node ID, or None if index is invalid.
    pub fn close_tab(&mut self, index: usize) -> Option<NodeId> {
        if index >= self.children.len() {
            return None;
        }

        self.tab_labels.remove(index);
        let removed = self.children.remove(index);

        // Adjust active tab if needed
        if self.active_tab >= self.children.len() && !self.children.is_empty() {
            self.active_tab = self.children.len() - 1;
        } else if self.active_tab > index {
            self.active_tab -= 1;
        }

        Some(removed)
    }

    /// Get the number of tabs.
    pub fn tab_count(&self) -> usize {
        self.children.len()
    }

    /// Get the label for a tab.
    pub fn tab_label(&self, index: usize) -> Option<&str> {
        self.tab_labels.get(index).map(|s| s.as_str())
    }

    /// Set the tab bar height.
    pub fn tab_bar_height(mut self, height: f32) -> Self {
        self.tab_bar_height = height.max(16.0);
        self
    }

    /// Set tab colors.
    pub fn tab_colors(mut self, bar: Color, active: Color, inactive: Color) -> Self {
        self.tab_bar_color = bar;
        self.active_tab_color = active;
        self.inactive_tab_color = inactive;
        self
    }

    /// Set the tab text color.
    pub fn text_color(mut self, color: Color) -> Self {
        self.tab_text_color = color;
        self
    }

    /// Set the tab hover color.
    pub fn hover_color(mut self, color: Color) -> Self {
        self.tab_hover_color = color;
        self
    }

    /// Enable or disable close buttons on tabs.
    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }

    /// Set the tab font size.
    pub fn tab_font_size(mut self, size: f32) -> Self {
        self.tab_font_size = size;
        self
    }

    /// Get the tab bar bounds.
    pub fn tab_bar_bounds(&self, layout: &LayoutRect) -> LayoutRect {
        LayoutRect {
            x: layout.x,
            y: layout.y,
            width: layout.width,
            height: self.tab_bar_height,
        }
    }

    /// Get the content area bounds (below the tab bar).
    pub fn content_bounds(&self, layout: &LayoutRect) -> LayoutRect {
        LayoutRect {
            x: layout.x,
            y: layout.y + self.tab_bar_height,
            width: layout.width,
            height: (layout.height - self.tab_bar_height).max(0.0),
        }
    }

    /// Get the bounds for a specific tab button.
    ///
    /// Returns None if the tab index is out of bounds or widths haven't been computed.
    pub fn tab_bounds(&self, index: usize, layout: &LayoutRect) -> Option<LayoutRect> {
        if index >= self.tab_labels.len() {
            return None;
        }

        // Calculate tab x position
        let mut x = layout.x;
        for i in 0..index {
            x += self.estimate_tab_width(i);
        }

        Some(LayoutRect {
            x,
            y: layout.y,
            width: self.estimate_tab_width(index),
            height: self.tab_bar_height,
        })
    }

    /// Estimate the width of a tab (rough calculation).
    fn estimate_tab_width(&self, index: usize) -> f32 {
        let label = self.tab_labels.get(index).map(|s| s.as_str()).unwrap_or("");
        let char_width = self.tab_font_size * 0.6;
        let text_width = label.len() as f32 * char_width;
        let close_width = if self.closable {
            DEFAULT_CLOSE_BUTTON_SIZE + 4.0
        } else {
            0.0
        };
        text_width + DEFAULT_TAB_PADDING * 2.0 + close_width
    }

    /// Get the close button bounds for a tab.
    pub fn close_button_bounds(&self, index: usize, layout: &LayoutRect) -> Option<LayoutRect> {
        if !self.closable || index >= self.tab_labels.len() {
            return None;
        }

        let tab_bounds = self.tab_bounds(index, layout)?;
        let button_size = DEFAULT_CLOSE_BUTTON_SIZE;
        let margin = (self.tab_bar_height - button_size) / 2.0;

        Some(LayoutRect {
            x: tab_bounds.x + tab_bounds.width - button_size - margin,
            y: tab_bounds.y + margin,
            width: button_size,
            height: button_size,
        })
    }

    /// Hit test to find which tab is at a position.
    pub fn hit_test_tab(&self, pos: Vec2, layout: &LayoutRect) -> Option<usize> {
        let bar = self.tab_bar_bounds(layout);
        if pos.y < bar.y || pos.y > bar.y + bar.height {
            return None;
        }

        for i in 0..self.tab_labels.len() {
            if let Some(tab_rect) = self.tab_bounds(i, layout) {
                if pos.x >= tab_rect.x && pos.x <= tab_rect.x + tab_rect.width {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Hit test to check if position is on a close button.
    pub fn hit_test_close_button(&self, pos: Vec2, layout: &LayoutRect) -> Option<usize> {
        if !self.closable {
            return None;
        }

        for i in 0..self.tab_labels.len() {
            if let Some(close_rect) = self.close_button_bounds(i, layout) {
                if pos.x >= close_rect.x
                    && pos.x <= close_rect.x + close_rect.width
                    && pos.y >= close_rect.y
                    && pos.y <= close_rect.y + close_rect.height
                {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Get the background color for a tab.
    pub fn tab_background_color(&self, index: usize) -> Color {
        if index == self.active_tab {
            self.active_tab_color
        } else if self.hovered_tab == Some(index) {
            self.tab_hover_color
        } else {
            self.inactive_tab_color
        }
    }

    /// Get the active tab's content node.
    pub fn active_content(&self) -> Option<NodeId> {
        self.children.get(self.active_tab).copied()
    }

    /// Start dragging a tab.
    pub fn start_tab_drag(&mut self, tab_index: usize) {
        if tab_index < self.tab_labels.len() {
            self.dragging_tab_index = Some(tab_index);
        }
    }

    /// Update drop target based on cursor position.
    pub fn update_drop_target(&mut self, cursor_pos: Vec2, layout: &LayoutRect) {
        if self.dragging_tab_index.is_none() {
            self.drag_drop_target = None;
            self.drag_cursor_pos = None;
            return;
        }

        // Store cursor position for ghost rendering
        self.drag_cursor_pos = Some(cursor_pos);

        // Calculate which insertion point cursor is closest to
        let mut closest_index = 0;
        let mut closest_dist = f32::MAX;

        for i in 0..=self.tab_labels.len() {
            let insertion_x = if i == 0 {
                layout.x
            } else if let Some(prev_bounds) = self.tab_bounds(i - 1, layout) {
                prev_bounds.x + prev_bounds.width
            } else {
                layout.x
            };

            let dist = (cursor_pos.x - insertion_x).abs();
            if dist < closest_dist {
                closest_dist = dist;
                closest_index = i;
            }
        }

        self.drag_drop_target = Some(closest_index);
    }

    /// Complete tab drag by reordering.
    pub fn finish_tab_drag(&mut self) {
        if let (Some(from_index), Some(to_index)) = (self.dragging_tab_index, self.drag_drop_target) {
            // A tab at index i is between insertion points i and i+1
            // Only reorder if we're actually moving to a different position
            let is_moving_left = to_index < from_index;
            let is_moving_right = to_index > from_index + 1;

            if is_moving_left || is_moving_right {
                // Reorder tab_labels
                let label = self.tab_labels.remove(from_index);
                let insert_index = if to_index > from_index { to_index - 1 } else { to_index };
                self.tab_labels.insert(insert_index, label);

                // Reorder children
                let child = self.children.remove(from_index);
                self.children.insert(insert_index, child);

                // Update active_tab if needed
                if self.active_tab == from_index {
                    self.active_tab = insert_index;
                } else if self.active_tab > from_index && self.active_tab <= insert_index {
                    self.active_tab -= 1;
                } else if self.active_tab < from_index && self.active_tab >= insert_index {
                    self.active_tab += 1;
                }
            }
        }

        self.dragging_tab_index = None;
        self.drag_drop_target = None;
        self.drag_cursor_pos = None;
    }

    /// Cancel tab drag without reordering.
    pub fn cancel_tab_drag(&mut self) {
        self.dragging_tab_index = None;
        self.drag_drop_target = None;
        self.drag_cursor_pos = None;
    }

    /// Get bounds for drop indicator line.
    pub fn drop_indicator_bounds(&self, layout: &LayoutRect) -> Option<LayoutRect> {
        let drop_index = self.drag_drop_target?;

        let x = if drop_index == 0 {
            layout.x
        } else if let Some(prev_tab) = self.tab_bounds(drop_index - 1, layout) {
            prev_tab.x + prev_tab.width
        } else {
            layout.x
        };

        Some(LayoutRect {
            x,
            y: layout.y,
            width: 2.0,  // 2px wide line
            height: self.tab_bar_height,
        })
    }
}

impl Default for DockTabs {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for DockTabs {
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
        // Tabs typically fill their container, no intrinsic size
        Vec2::ZERO
    }

    fn clone_box(&self) -> Box<dyn Widget> {
        Box::new(self.clone())
    }
}
