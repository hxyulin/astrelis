//! DockTabs widget - tabbed container showing one panel at a time.

use std::any::Any;

use astrelis_core::math::Vec2;
use astrelis_render::Color;
use astrelis_text::FontRenderer;

use crate::style::Style;
use crate::tree::{LayoutRect, NodeId};
use crate::widgets::{ScrollbarTheme, Widget};

/// Default tab bar height in pixels.
pub const DEFAULT_TAB_BAR_HEIGHT: f32 = 22.0;

/// Default tab padding in pixels.
pub const DEFAULT_TAB_PADDING: f32 = 8.0;

/// Default tab close button size in pixels.
pub const DEFAULT_CLOSE_BUTTON_SIZE: f32 = 12.0;

/// Character width factor for estimating tab text width.
pub(crate) const CHAR_WIDTH_FACTOR: f32 = 0.6;

/// Width of the drop indicator line in pixels.
pub(crate) const DROP_INDICATOR_WIDTH: f32 = 2.0;

/// Margin between tab text and close button in pixels.
pub(crate) const CLOSE_BUTTON_MARGIN: f32 = 4.0;

/// How overflow tabs are indicated in the tab bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabScrollIndicator {
    /// Show left/right arrow text indicators.
    #[default]
    Arrows,
    /// Show a thin horizontal scrollbar (track + thumb).
    Scrollbar,
    /// Show both arrows and a scrollbar.
    Both,
}

/// Vertical position of the tab bar scrollbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabScrollbarPosition {
    /// Scrollbar above the tabs.
    Top,
    /// Scrollbar below the tabs (between tab bar and content).
    #[default]
    Bottom,
}

/// Default tab bar color.
pub fn default_tab_bar_color() -> Color {
    Color::from_rgb_u8(14, 14, 19)
}

/// Default active tab color.
pub fn default_active_tab_color() -> Color {
    Color::from_rgb_u8(24, 24, 32)
}

/// Default inactive tab color.
pub fn default_inactive_tab_color() -> Color {
    Color::from_rgb_u8(14, 14, 19)
}

/// Default tab text color.
pub fn default_tab_text_color() -> Color {
    Color::from_rgb_u8(200, 200, 215)
}

/// Default tab hover color.
pub fn default_tab_hover_color() -> Color {
    Color::from_rgb_u8(30, 30, 40)
}

/// Visual theme properties for a DockTabs widget.
#[derive(Debug, Clone)]
pub struct DockTabsTheme {
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
    /// Tab font size.
    pub tab_font_size: f32,
    /// Whether to show close buttons on tabs.
    pub closable: bool,
    /// How overflow tabs are indicated (arrows, scrollbar, or both).
    pub scroll_indicator: TabScrollIndicator,
    /// Vertical position of the scrollbar within the tab bar.
    pub scrollbar_position: TabScrollbarPosition,
    /// Visual theme for the scrollbar track and thumb.
    pub scrollbar_theme: ScrollbarTheme,
}

impl Default for DockTabsTheme {
    fn default() -> Self {
        Self {
            tab_bar_height: DEFAULT_TAB_BAR_HEIGHT,
            tab_bar_color: default_tab_bar_color(),
            active_tab_color: default_active_tab_color(),
            inactive_tab_color: default_inactive_tab_color(),
            tab_text_color: default_tab_text_color(),
            tab_hover_color: default_tab_hover_color(),
            tab_font_size: 11.0,
            closable: false,
            scroll_indicator: TabScrollIndicator::default(),
            scrollbar_position: TabScrollbarPosition::default(),
            scrollbar_theme: ScrollbarTheme::default(),
        }
    }
}

/// Drag state for tab reordering within a DockTabs container.
#[derive(Debug, Clone, Default)]
pub struct TabDragState {
    /// Index of tab currently being dragged (None if no drag).
    pub dragging_tab_index: Option<usize>,
    /// Index where dragged tab will be dropped (insertion point).
    pub drag_drop_target: Option<usize>,
    /// Current cursor position during drag (for ghost rendering).
    pub drag_cursor_pos: Option<Vec2>,
}

impl TabDragState {
    /// Check if a tab drag is currently active.
    pub fn is_active(&self) -> bool {
        self.dragging_tab_index.is_some()
    }

    /// Clear all drag state.
    pub fn clear(&mut self) {
        *self = Self::default();
    }
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
    /// Visual theme properties.
    pub theme: DockTabsTheme,
    /// Index of hovered tab (None if no hover).
    pub hovered_tab: Option<usize>,
    /// Cached tab widths (computed during rendering).
    pub(crate) tab_widths: Vec<f32>,
    /// Whether tab widths need recomputation.
    pub(crate) tab_widths_dirty: bool,
    /// Tab drag state (reordering within this container).
    pub drag: TabDragState,
    /// Current horizontal scroll offset for the tab bar (pixels).
    pub tab_scroll_offset: f32,
    /// Whether the tab bar is scrollable (tabs overflow available width).
    pub(crate) tab_bar_scrollable: bool,
    /// Whether the scrollbar thumb is being dragged.
    pub(crate) scrollbar_thumb_dragging: bool,
    /// Anchor offset for scrollbar thumb drag (mouse offset from thumb left edge).
    pub(crate) scrollbar_drag_anchor: f32,
    /// Whether the scrollbar thumb is currently hovered.
    pub(crate) scrollbar_thumb_hovered: bool,
    /// Per-widget content padding override.
    ///
    /// When `None`, the global `DockingStyle.content_padding` is used.
    /// When `Some(px)`, this value is used instead.
    pub content_padding: Option<f32>,
}

impl DockTabs {
    /// Create a new empty tabs container.
    pub fn new() -> Self {
        Self {
            style: Style::new().display(taffy::Display::Flex),
            children: Vec::new(),
            tab_labels: Vec::new(),
            active_tab: 0,
            theme: DockTabsTheme::default(),
            hovered_tab: None,
            tab_widths: Vec::new(),
            tab_widths_dirty: true,
            drag: TabDragState::default(),
            tab_scroll_offset: 0.0,
            tab_bar_scrollable: false,
            scrollbar_thumb_dragging: false,
            scrollbar_drag_anchor: 0.0,
            scrollbar_thumb_hovered: false,
            content_padding: None,
        }
    }

    /// Add a tab with a label and content node.
    pub fn add_tab(&mut self, label: impl Into<String>, content: NodeId) {
        self.tab_labels.push(label.into());
        self.children.push(content);
        self.tab_widths_dirty = true;
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

    /// Insert a tab at a specific position.
    ///
    /// Adjusts `active_tab` to keep the same tab visually active.
    /// `index` is clamped to `[0, tab_count]`.
    pub fn insert_tab_at(&mut self, index: usize, label: impl Into<String>, content: NodeId) {
        let index = index.min(self.children.len());
        self.tab_labels.insert(index, label.into());
        self.children.insert(index, content);
        self.tab_widths_dirty = true;

        // Adjust active tab if insertion is before or at the active position
        if index <= self.active_tab
            && !self.children.is_empty()
            && self.active_tab + 1 < self.children.len()
        {
            self.active_tab += 1;
        }
    }

    /// Remove a tab at the given index.
    ///
    /// Returns the removed `(label, content)` pair, or `None` if the index is invalid.
    /// Adjusts `active_tab` to keep a valid selection.
    pub fn remove_tab(&mut self, index: usize) -> Option<(String, NodeId)> {
        if index >= self.children.len() {
            return None;
        }

        let label = self.tab_labels.remove(index);
        let content = self.children.remove(index);
        self.tab_widths_dirty = true;

        // Adjust active tab if needed
        if self.active_tab >= self.children.len() && !self.children.is_empty() {
            self.active_tab = self.children.len() - 1;
        } else if self.active_tab > index {
            self.active_tab -= 1;
        }

        Some((label, content))
    }

    /// Reorder a tab from `from_index` to `to_insertion` point.
    ///
    /// Uses insertion-point semantics: `to_insertion` is the index *between* tabs
    /// where the tab should land. A tab at index `i` occupies the space between
    /// insertion points `i` and `i+1`.
    ///
    /// Returns the new index of the moved tab, or `None` if no move occurred.
    pub fn reorder_tab(&mut self, from_index: usize, to_insertion: usize) -> Option<usize> {
        if from_index >= self.children.len() {
            return None;
        }

        let is_moving_left = to_insertion < from_index;
        let is_moving_right = to_insertion > from_index + 1;

        if !is_moving_left && !is_moving_right {
            return None;
        }

        let label = self.tab_labels.remove(from_index);
        let child = self.children.remove(from_index);

        let insert_index = if to_insertion > from_index {
            to_insertion - 1
        } else {
            to_insertion
        };

        self.tab_labels.insert(insert_index, label);
        self.children.insert(insert_index, child);
        self.tab_widths_dirty = true;

        // Update active_tab if needed
        if self.active_tab == from_index {
            self.active_tab = insert_index;
        } else if self.active_tab > from_index && self.active_tab <= insert_index {
            self.active_tab -= 1;
        } else if self.active_tab < from_index && self.active_tab >= insert_index {
            self.active_tab += 1;
        }

        Some(insert_index)
    }

    /// Close a tab at the given index.
    ///
    /// Returns the removed content node ID, or None if index is invalid.
    /// This is a convenience wrapper around `remove_tab`.
    pub fn close_tab(&mut self, index: usize) -> Option<NodeId> {
        self.remove_tab(index).map(|(_, content)| content)
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
        self.theme.tab_bar_height = height.max(16.0);
        self
    }

    /// Set tab colors.
    pub fn tab_colors(mut self, bar: Color, active: Color, inactive: Color) -> Self {
        self.theme.tab_bar_color = bar;
        self.theme.active_tab_color = active;
        self.theme.inactive_tab_color = inactive;
        self
    }

    /// Set the tab text color.
    pub fn text_color(mut self, color: Color) -> Self {
        self.theme.tab_text_color = color;
        self
    }

    /// Set the tab hover color.
    pub fn hover_color(mut self, color: Color) -> Self {
        self.theme.tab_hover_color = color;
        self
    }

    /// Enable or disable close buttons on tabs.
    pub fn closable(mut self, closable: bool) -> Self {
        self.theme.closable = closable;
        self
    }

    /// Set the tab font size.
    pub fn tab_font_size(mut self, size: f32) -> Self {
        self.theme.tab_font_size = size;
        self
    }

    /// Set how overflow tabs are indicated.
    pub fn scroll_indicator(mut self, mode: TabScrollIndicator) -> Self {
        self.theme.scroll_indicator = mode;
        self
    }

    /// Set the vertical position of the scrollbar.
    pub fn scrollbar_position(mut self, position: TabScrollbarPosition) -> Self {
        self.theme.scrollbar_position = position;
        self
    }

    /// Set the scrollbar visual theme.
    pub fn scrollbar_theme(mut self, theme: ScrollbarTheme) -> Self {
        self.theme.scrollbar_theme = theme;
        self
    }

    /// Set per-widget content padding override.
    ///
    /// When set, this value overrides the global `DockingStyle.content_padding`.
    pub fn content_padding(mut self, padding: f32) -> Self {
        self.content_padding = Some(padding);
        self
    }

    // -----------------------------------------------------------------------
    // Scrollbar query methods
    // -----------------------------------------------------------------------

    /// Whether the scrollbar should be shown.
    pub fn should_show_scrollbar(&self) -> bool {
        self.tab_bar_scrollable
            && matches!(
                self.theme.scroll_indicator,
                TabScrollIndicator::Scrollbar | TabScrollIndicator::Both
            )
    }

    /// Whether the arrow scroll indicators should be shown.
    pub fn should_show_arrows(&self) -> bool {
        self.tab_bar_scrollable
            && matches!(
                self.theme.scroll_indicator,
                TabScrollIndicator::Arrows | TabScrollIndicator::Both
            )
    }

    /// The thickness of the scrollbar strip (0.0 when hidden).
    pub fn scrollbar_thickness(&self) -> f32 {
        if self.should_show_scrollbar() {
            self.theme.scrollbar_theme.thickness
        } else {
            0.0
        }
    }

    // -----------------------------------------------------------------------
    // Scrollbar bounds methods
    // -----------------------------------------------------------------------

    /// Get the scrollbar track bounds (thin strip at top or bottom of the tab bar).
    pub fn scrollbar_track_bounds(&self, layout: &LayoutRect) -> LayoutRect {
        let thickness = self.scrollbar_thickness();
        let y = match self.theme.scrollbar_position {
            TabScrollbarPosition::Top => layout.y,
            TabScrollbarPosition::Bottom => layout.y + self.theme.tab_bar_height - thickness,
        };
        LayoutRect {
            x: layout.x,
            y,
            width: layout.width,
            height: thickness,
        }
    }

    /// Get the scrollbar thumb bounds within the track.
    pub fn scrollbar_thumb_bounds(&self, layout: &LayoutRect) -> LayoutRect {
        let track = self.scrollbar_track_bounds(layout);
        let total_width = self.total_tabs_width();
        if total_width <= 0.0 {
            return track;
        }

        let visible_ratio = (track.width / total_width).min(1.0);
        let thumb_width = (visible_ratio * track.width)
            .max(self.theme.scrollbar_theme.min_thumb_length)
            .min(track.width);

        let max_scroll = self.max_tab_scroll_offset(layout.width);
        let scroll_ratio = if max_scroll > 0.0 {
            self.tab_scroll_offset / max_scroll
        } else {
            0.0
        };

        let available_travel = track.width - thumb_width;
        let thumb_x = track.x + scroll_ratio * available_travel;

        LayoutRect {
            x: thumb_x,
            y: track.y,
            width: thumb_width,
            height: track.height,
        }
    }

    /// Get the tab row bounds (tab bar area minus scrollbar strip).
    pub fn tab_row_bounds(&self, layout: &LayoutRect) -> LayoutRect {
        let thickness = self.scrollbar_thickness();
        let y = match self.theme.scrollbar_position {
            TabScrollbarPosition::Top => layout.y + thickness,
            TabScrollbarPosition::Bottom => layout.y,
        };
        LayoutRect {
            x: layout.x,
            y,
            width: layout.width,
            height: self.theme.tab_bar_height - thickness,
        }
    }

    // -----------------------------------------------------------------------
    // Scrollbar hit testing
    // -----------------------------------------------------------------------

    /// Hit-test the scrollbar thumb.
    pub fn hit_test_scrollbar_thumb(&self, pos: Vec2, layout: &LayoutRect) -> bool {
        if !self.should_show_scrollbar() {
            return false;
        }
        let thumb = self.scrollbar_thumb_bounds(layout);
        pos.x >= thumb.x
            && pos.x <= thumb.x + thumb.width
            && pos.y >= thumb.y
            && pos.y <= thumb.y + thumb.height
    }

    /// Hit-test the scrollbar track.
    pub fn hit_test_scrollbar_track(&self, pos: Vec2, layout: &LayoutRect) -> bool {
        if !self.should_show_scrollbar() {
            return false;
        }
        let track = self.scrollbar_track_bounds(layout);
        pos.x >= track.x
            && pos.x <= track.x + track.width
            && pos.y >= track.y
            && pos.y <= track.y + track.height
    }

    // -----------------------------------------------------------------------
    // Scrollbar drag interaction
    // -----------------------------------------------------------------------

    /// Start dragging the scrollbar thumb.
    pub fn start_scrollbar_drag(&mut self, mouse_x: f32, layout: &LayoutRect) {
        let thumb = self.scrollbar_thumb_bounds(layout);
        self.scrollbar_drag_anchor = mouse_x - thumb.x;
        self.scrollbar_thumb_dragging = true;
    }

    /// Update scroll offset during scrollbar drag.
    pub fn update_scrollbar_drag(&mut self, mouse_x: f32, layout: &LayoutRect) {
        if !self.scrollbar_thumb_dragging {
            return;
        }
        let track = self.scrollbar_track_bounds(layout);
        let total_width = self.total_tabs_width();
        if total_width <= 0.0 {
            return;
        }

        let visible_ratio = (track.width / total_width).min(1.0);
        let thumb_width = (visible_ratio * track.width)
            .max(self.theme.scrollbar_theme.min_thumb_length)
            .min(track.width);
        let available_travel = track.width - thumb_width;

        if available_travel <= 0.0 {
            return;
        }

        let thumb_left = mouse_x - self.scrollbar_drag_anchor;
        let scroll_ratio = ((thumb_left - track.x) / available_travel).clamp(0.0, 1.0);
        let max_scroll = self.max_tab_scroll_offset(layout.width);
        self.tab_scroll_offset = scroll_ratio * max_scroll;
    }

    /// End scrollbar thumb drag.
    pub fn end_scrollbar_drag(&mut self) {
        self.scrollbar_thumb_dragging = false;
    }

    /// Get the current scrollbar thumb color based on interaction state.
    pub fn scrollbar_thumb_color(&self) -> Color {
        if self.scrollbar_thumb_dragging {
            self.theme.scrollbar_theme.thumb_active_color
        } else if self.scrollbar_thumb_hovered {
            self.theme.scrollbar_theme.thumb_hover_color
        } else {
            self.theme.scrollbar_theme.thumb_color
        }
    }

    /// Get the tab bar bounds.
    pub fn tab_bar_bounds(&self, layout: &LayoutRect) -> LayoutRect {
        LayoutRect {
            x: layout.x,
            y: layout.y,
            width: layout.width,
            height: self.theme.tab_bar_height,
        }
    }

    /// Get the content area bounds (below the tab bar).
    pub fn content_bounds(&self, layout: &LayoutRect) -> LayoutRect {
        LayoutRect {
            x: layout.x,
            y: layout.y + self.theme.tab_bar_height,
            width: layout.width,
            height: (layout.height - self.theme.tab_bar_height).max(0.0),
        }
    }

    /// Get the bounds for a specific tab button.
    ///
    /// Returns None if the tab index is out of bounds or widths haven't been computed.
    /// When the tab bar is scrollable, tab positions are offset by `tab_scroll_offset`.
    /// Tab bounds are positioned within the tab row area (excluding scrollbar strip).
    pub fn tab_bounds(&self, index: usize, layout: &LayoutRect) -> Option<LayoutRect> {
        if index >= self.tab_labels.len() {
            return None;
        }

        let row = self.tab_row_bounds(layout);

        // Calculate tab x position using cached or estimated widths
        let mut x = row.x - self.tab_scroll_offset;
        for i in 0..index {
            x += self.get_tab_width(i);
        }

        Some(LayoutRect {
            x,
            y: row.y,
            width: self.get_tab_width(index),
            height: row.height,
        })
    }

    /// Compute tab widths using the font renderer for accurate text measurement.
    ///
    /// Call this during the render pass when a `FontRenderer` is available.
    /// Results are cached in `tab_widths` and used by `get_tab_width`.
    pub fn compute_tab_widths(&mut self, font_renderer: &FontRenderer) {
        if !self.tab_widths_dirty {
            return;
        }

        self.tab_widths.clear();
        self.tab_widths.reserve(self.tab_labels.len());

        let close_width = if self.theme.closable {
            DEFAULT_CLOSE_BUTTON_SIZE + CLOSE_BUTTON_MARGIN
        } else {
            0.0
        };

        for label in &self.tab_labels {
            let text = astrelis_text::Text::new(label.as_str()).size(self.theme.tab_font_size);
            let (text_width, _) = font_renderer.measure_text(&text);
            let tab_width = text_width + DEFAULT_TAB_PADDING * 2.0 + close_width;
            self.tab_widths.push(tab_width);
        }

        self.tab_widths_dirty = false;
    }

    /// Get the width of a tab at the given index.
    ///
    /// Uses the cached width from `compute_tab_widths` if available,
    /// otherwise falls back to an estimate based on character count.
    pub fn get_tab_width(&self, index: usize) -> f32 {
        if let Some(&width) = self.tab_widths.get(index) {
            width
        } else {
            self.estimate_tab_width(index)
        }
    }

    /// Estimate the width of a tab (rough calculation).
    ///
    /// Used as a fallback when cached widths are not yet computed.
    fn estimate_tab_width(&self, index: usize) -> f32 {
        let label = self.tab_labels.get(index).map(|s| s.as_str()).unwrap_or("");
        let char_width = self.theme.tab_font_size * CHAR_WIDTH_FACTOR;
        let text_width = label.len() as f32 * char_width;
        let close_width = if self.theme.closable {
            DEFAULT_CLOSE_BUTTON_SIZE + CLOSE_BUTTON_MARGIN
        } else {
            0.0
        };
        text_width + DEFAULT_TAB_PADDING * 2.0 + close_width
    }

    /// Get the close button bounds for a tab.
    pub fn close_button_bounds(&self, index: usize, layout: &LayoutRect) -> Option<LayoutRect> {
        if !self.theme.closable || index >= self.tab_labels.len() {
            return None;
        }

        let tab_bounds = self.tab_bounds(index, layout)?;
        let button_size = DEFAULT_CLOSE_BUTTON_SIZE;
        let row_height = self.tab_row_bounds(layout).height;
        let margin = (row_height - button_size) / 2.0;

        Some(LayoutRect {
            x: tab_bounds.x + tab_bounds.width - button_size - margin,
            y: tab_bounds.y + margin,
            width: button_size,
            height: button_size,
        })
    }

    /// Hit test to find which tab is at a position.
    pub fn hit_test_tab(&self, pos: Vec2, layout: &LayoutRect) -> Option<usize> {
        let row = self.tab_row_bounds(layout);
        if pos.y < row.y || pos.y > row.y + row.height {
            return None;
        }

        for i in 0..self.tab_labels.len() {
            if let Some(tab_rect) = self.tab_bounds(i, layout)
                && pos.x >= tab_rect.x
                && pos.x <= tab_rect.x + tab_rect.width
            {
                return Some(i);
            }
        }
        None
    }

    /// Hit test to check if position is on a close button.
    pub fn hit_test_close_button(&self, pos: Vec2, layout: &LayoutRect) -> Option<usize> {
        if !self.theme.closable {
            return None;
        }

        for i in 0..self.tab_labels.len() {
            if let Some(close_rect) = self.close_button_bounds(i, layout)
                && pos.x >= close_rect.x
                && pos.x <= close_rect.x + close_rect.width
                && pos.y >= close_rect.y
                && pos.y <= close_rect.y + close_rect.height
            {
                return Some(i);
            }
        }
        None
    }

    /// Get the background color for a tab.
    pub fn tab_background_color(&self, index: usize) -> Color {
        if index == self.active_tab {
            self.theme.active_tab_color
        } else if self.hovered_tab == Some(index) {
            self.theme.tab_hover_color
        } else {
            self.theme.inactive_tab_color
        }
    }

    /// Get the active tab's content node.
    pub fn active_content(&self) -> Option<NodeId> {
        self.children.get(self.active_tab).copied()
    }

    /// Start dragging a tab.
    pub fn start_tab_drag(&mut self, tab_index: usize) {
        if tab_index < self.tab_labels.len() {
            self.drag.dragging_tab_index = Some(tab_index);
        }
    }

    /// Update drop target based on cursor position.
    pub fn update_drop_target(&mut self, cursor_pos: Vec2, layout: &LayoutRect) {
        if self.drag.dragging_tab_index.is_none() {
            self.drag.drag_drop_target = None;
            self.drag.drag_cursor_pos = None;
            return;
        }

        // Store cursor position for ghost rendering
        self.drag.drag_cursor_pos = Some(cursor_pos);

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

        self.drag.drag_drop_target = Some(closest_index);
    }

    /// Complete tab drag by reordering.
    pub fn finish_tab_drag(&mut self) {
        if let (Some(from_index), Some(to_index)) =
            (self.drag.dragging_tab_index, self.drag.drag_drop_target)
        {
            self.reorder_tab(from_index, to_index);
        }

        self.drag.clear();
    }

    /// Cancel tab drag without reordering.
    pub fn cancel_tab_drag(&mut self) {
        self.drag.clear();
    }

    // -----------------------------------------------------------------------
    // Tab group operations
    // -----------------------------------------------------------------------

    /// Returns true if `pos` is in the tab bar area but NOT on any tab label.
    ///
    /// This is used to detect clicks on the tab bar background for group drag.
    pub fn hit_test_tab_bar_background(&self, pos: Vec2, layout: &LayoutRect) -> bool {
        let bar = self.tab_bar_bounds(layout);
        if pos.y < bar.y || pos.y > bar.y + bar.height || pos.x < bar.x || pos.x > bar.x + bar.width
        {
            return false;
        }
        // Check that we're NOT on any tab label
        self.hit_test_tab(pos, layout).is_none()
    }

    /// Remove all tabs, returning `(label, content)` pairs. Resets `active_tab` to 0.
    pub fn remove_all_tabs(&mut self) -> Vec<(String, NodeId)> {
        let labels = std::mem::take(&mut self.tab_labels);
        let children = std::mem::take(&mut self.children);
        self.active_tab = 0;
        self.tab_widths_dirty = true;
        self.tab_widths.clear();
        self.tab_scroll_offset = 0.0;
        self.tab_bar_scrollable = false;
        labels.into_iter().zip(children).collect()
    }

    /// Bulk-insert tabs at a given position.
    ///
    /// `tabs` is a slice of `(label, content)` pairs. `start_index` is clamped
    /// to `[0, tab_count]`.
    pub fn insert_tabs_at(&mut self, start_index: usize, tabs: &[(String, NodeId)]) {
        let start = start_index.min(self.children.len());
        for (offset, (label, content)) in tabs.iter().enumerate() {
            let idx = start + offset;
            self.tab_labels.insert(idx, label.clone());
            self.children.insert(idx, *content);
        }
        self.tab_widths_dirty = true;

        // Adjust active_tab if insertion is before the active position
        if start <= self.active_tab && !self.children.is_empty() {
            self.active_tab = (self.active_tab + tabs.len()).min(self.children.len() - 1);
        }
    }

    // -----------------------------------------------------------------------
    // Tab bar scroll
    // -----------------------------------------------------------------------

    /// Total width of all tabs combined.
    pub fn total_tabs_width(&self) -> f32 {
        (0..self.tab_labels.len())
            .map(|i| self.get_tab_width(i))
            .sum()
    }

    /// Returns true if the tabs overflow the available width.
    pub fn tabs_overflow(&self, available_width: f32) -> bool {
        self.total_tabs_width() > available_width
    }

    /// Maximum scroll offset (0 if no overflow).
    pub fn max_tab_scroll_offset(&self, available_width: f32) -> f32 {
        (self.total_tabs_width() - available_width).max(0.0)
    }

    /// Clamp the current scroll offset to valid range.
    pub fn clamp_tab_scroll(&mut self, available_width: f32) {
        let max = self.max_tab_scroll_offset(available_width);
        self.tab_scroll_offset = self.tab_scroll_offset.clamp(0.0, max);
    }

    /// Scroll so that the tab at `index` is fully visible.
    pub fn scroll_to_tab(&mut self, index: usize, available_width: f32) {
        if index >= self.tab_labels.len() {
            return;
        }

        // Compute the start x of the tab (unscrolled)
        let mut tab_start: f32 = 0.0;
        for i in 0..index {
            tab_start += self.get_tab_width(i);
        }
        let tab_end = tab_start + self.get_tab_width(index);

        // If tab starts before the visible area, scroll left
        if tab_start < self.tab_scroll_offset {
            self.tab_scroll_offset = tab_start;
        }
        // If tab ends after the visible area, scroll right
        if tab_end > self.tab_scroll_offset + available_width {
            self.tab_scroll_offset = tab_end - available_width;
        }

        self.clamp_tab_scroll(available_width);
    }

    /// Scroll the tab bar by a pixel delta (positive = scroll right).
    pub fn scroll_tab_bar_by(&mut self, delta: f32, available_width: f32) {
        self.tab_scroll_offset += delta;
        self.clamp_tab_scroll(available_width);
    }

    /// Get bounds for drop indicator line.
    pub fn drop_indicator_bounds(&self, layout: &LayoutRect) -> Option<LayoutRect> {
        let drop_index = self.drag.drag_drop_target?;
        let row = self.tab_row_bounds(layout);

        let x = if drop_index == 0 {
            row.x
        } else if let Some(prev_tab) = self.tab_bounds(drop_index - 1, layout) {
            prev_tab.x + prev_tab.width
        } else {
            row.x
        };

        Some(LayoutRect {
            x,
            y: row.y,
            width: DROP_INDICATOR_WIDTH,
            height: row.height,
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

/// Compute tab widths for all DockTabs widgets in the tree.
///
/// Call this once per frame after layout computation and before rendering
/// to ensure accurate text-measured tab widths are cached.
/// Also computes whether the tab bar is scrollable and clamps the scroll offset.
pub fn compute_all_tab_widths(tree: &mut crate::tree::UiTree, font_renderer: &FontRenderer) {
    // Collect all DockTabs node IDs and their layout widths first to avoid borrow conflicts
    let all_ids = tree.node_ids();
    let tab_node_infos: Vec<(NodeId, f32)> = all_ids
        .into_iter()
        .filter_map(|id| {
            let is_tabs = tree
                .get_widget(id)
                .map(|w| w.as_any().downcast_ref::<DockTabs>().is_some())
                .unwrap_or(false);
            if is_tabs {
                let width = tree.get_layout(id).map(|l| l.width).unwrap_or(0.0);
                Some((id, width))
            } else {
                None
            }
        })
        .collect();

    for (node_id, layout_width) in tab_node_infos {
        if let Some(widget) = tree.get_widget_mut(node_id)
            && let Some(tabs) = widget.as_any_mut().downcast_mut::<DockTabs>()
        {
            tabs.compute_tab_widths(font_renderer);
            let new_scrollable = tabs.tabs_overflow(layout_width);
            let scrollable_changed = new_scrollable != tabs.tab_bar_scrollable;
            tabs.tab_bar_scrollable = new_scrollable;
            if tabs.tab_bar_scrollable {
                tabs.clamp_tab_scroll(layout_width);
            } else {
                tabs.tab_scroll_offset = 0.0;
            }
            // When scrollable state changes, mark dirty so draw commands
            // (including the scrollbar) are regenerated.
            if scrollable_changed {
                tree.mark_dirty_flags(node_id, crate::dirty::DirtyFlags::GEOMETRY);
            }
        }
    }
}
