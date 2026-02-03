//! Scrollbar and column layout widgets.

use crate::style::Style;
use crate::tree::NodeId;
use crate::widgets::Widget;
use astrelis_core::math::Vec2;
use astrelis_render::Color;
use astrelis_text::FontRenderer;
use std::any::Any;

// ---------------------------------------------------------------------------
// Scrollbar widgets
// ---------------------------------------------------------------------------

/// Scrollbar orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarOrientation {
    /// Horizontal scrollbar.
    Horizontal,
    /// Vertical scrollbar.
    Vertical,
}

/// Visual theme for scrollbar widgets.
#[derive(Debug, Clone)]
pub struct ScrollbarTheme {
    /// Track background color.
    pub track_color: Color,
    /// Thumb color (normal state).
    pub thumb_color: Color,
    /// Thumb color when hovered.
    pub thumb_hover_color: Color,
    /// Thumb color when being dragged.
    pub thumb_active_color: Color,
    /// Border radius for the thumb.
    pub thumb_border_radius: f32,
    /// Minimum thumb length in pixels.
    pub min_thumb_length: f32,
    /// Scrollbar thickness in pixels.
    pub thickness: f32,
}

impl Default for ScrollbarTheme {
    fn default() -> Self {
        Self {
            track_color: Color::from_rgba_u8(30, 30, 35, 100),
            thumb_color: Color::from_rgb_u8(80, 80, 90),
            thumb_hover_color: Color::from_rgb_u8(100, 100, 110),
            thumb_active_color: Color::from_rgb_u8(120, 120, 130),
            thumb_border_radius: 3.0,
            min_thumb_length: 20.0,
            thickness: 8.0,
        }
    }
}

/// Horizontal scrollbar widget.
#[derive(Clone)]
pub struct HScrollbar {
    /// Widget style.
    pub style: Style,
    /// Current scroll offset in content pixels.
    pub scroll_offset: f32,
    /// Total content width.
    pub content_width: f32,
    /// Visible viewport width.
    pub viewport_width: f32,
    /// Visual theme.
    pub theme: ScrollbarTheme,
    /// Whether the thumb is being dragged.
    pub is_thumb_dragging: bool,
    /// Anchor offset within the thumb when drag started.
    pub drag_anchor: f32,
    /// Whether the thumb is hovered.
    pub is_thumb_hovered: bool,
}

impl HScrollbar {
    /// Create a new horizontal scrollbar.
    pub fn new(content_width: f32, viewport_width: f32) -> Self {
        Self {
            style: Style::new(),
            scroll_offset: 0.0,
            content_width,
            viewport_width,
            theme: ScrollbarTheme::default(),
            is_thumb_dragging: false,
            drag_anchor: 0.0,
            is_thumb_hovered: false,
        }
    }

    /// Whether a scrollbar is needed (content exceeds viewport).
    pub fn needs_scrollbar(&self) -> bool {
        self.content_width > self.viewport_width
    }

    /// Maximum scroll offset.
    pub fn max_scroll_offset(&self) -> f32 {
        (self.content_width - self.viewport_width).max(0.0)
    }

    /// Ratio of viewport to content (0.0-1.0).
    pub fn thumb_ratio(&self) -> f32 {
        if self.content_width <= 0.0 {
            return 1.0;
        }
        (self.viewport_width / self.content_width).clamp(0.0, 1.0)
    }

    /// Thumb length in pixels along the track.
    pub fn thumb_length(&self, track_length: f32) -> f32 {
        (self.thumb_ratio() * track_length).max(self.theme.min_thumb_length)
    }

    /// Thumb bounds within the scrollbar layout.
    pub fn thumb_bounds(&self, layout: &crate::tree::LayoutRect) -> crate::tree::LayoutRect {
        let track_length = layout.width;
        let thumb_len = self.thumb_length(track_length);
        let max_offset = self.max_scroll_offset();
        let scroll_frac = if max_offset > 0.0 {
            self.scroll_offset / max_offset
        } else {
            0.0
        };
        let thumb_x = layout.x + scroll_frac * (track_length - thumb_len);

        crate::tree::LayoutRect {
            x: thumb_x,
            y: layout.y,
            width: thumb_len,
            height: layout.height,
        }
    }

    /// Set scroll offset, clamping to valid range.
    pub fn set_scroll_offset(&mut self, offset: f32) {
        self.scroll_offset = offset.clamp(0.0, self.max_scroll_offset());
    }

    /// Adjust scroll offset by a relative delta.
    pub fn scroll_by(&mut self, delta: f32) {
        self.set_scroll_offset(self.scroll_offset + delta);
    }

    /// Hit-test the thumb.
    pub fn hit_test_thumb(&self, pos: Vec2, layout: &crate::tree::LayoutRect) -> bool {
        let tb = self.thumb_bounds(layout);
        pos.x >= tb.x && pos.x <= tb.x + tb.width && pos.y >= tb.y && pos.y <= tb.y + tb.height
    }

    /// Start thumb drag. `mouse_x` is the x coordinate in the scrollbar's coordinate space.
    pub fn start_thumb_drag(&mut self, mouse_x: f32, layout: &crate::tree::LayoutRect) {
        let tb = self.thumb_bounds(layout);
        self.drag_anchor = mouse_x - tb.x;
        self.is_thumb_dragging = true;
    }

    /// Update thumb position during drag.
    pub fn update_thumb_drag(&mut self, mouse_x: f32, layout: &crate::tree::LayoutRect) {
        if !self.is_thumb_dragging {
            return;
        }
        let track_length = layout.width;
        let thumb_len = self.thumb_length(track_length);
        let available = track_length - thumb_len;
        if available <= 0.0 {
            return;
        }
        let thumb_pos = mouse_x - layout.x - self.drag_anchor;
        let frac = (thumb_pos / available).clamp(0.0, 1.0);
        self.scroll_offset = frac * self.max_scroll_offset();
    }

    /// End thumb drag.
    pub fn end_thumb_drag(&mut self) {
        self.is_thumb_dragging = false;
    }

    /// Current thumb color based on state.
    pub fn current_thumb_color(&self) -> Color {
        if self.is_thumb_dragging {
            self.theme.thumb_active_color
        } else if self.is_thumb_hovered {
            self.theme.thumb_hover_color
        } else {
            self.theme.thumb_color
        }
    }
}

impl Widget for HScrollbar {
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

    fn measure(&self, _available_space: Vec2, _font_renderer: Option<&FontRenderer>) -> Vec2 {
        Vec2::new(0.0, self.theme.thickness)
    }

    fn clone_box(&self) -> Box<dyn Widget> {
        Box::new(self.clone())
    }
}

/// Vertical scrollbar widget.
#[derive(Clone)]
pub struct VScrollbar {
    /// Widget style.
    pub style: Style,
    /// Current scroll offset in content pixels.
    pub scroll_offset: f32,
    /// Total content height.
    pub content_height: f32,
    /// Visible viewport height.
    pub viewport_height: f32,
    /// Visual theme.
    pub theme: ScrollbarTheme,
    /// Whether the thumb is being dragged.
    pub is_thumb_dragging: bool,
    /// Anchor offset within the thumb when drag started.
    pub drag_anchor: f32,
    /// Whether the thumb is hovered.
    pub is_thumb_hovered: bool,
}

impl VScrollbar {
    /// Create a new vertical scrollbar.
    pub fn new(content_height: f32, viewport_height: f32) -> Self {
        Self {
            style: Style::new(),
            scroll_offset: 0.0,
            content_height,
            viewport_height,
            theme: ScrollbarTheme::default(),
            is_thumb_dragging: false,
            drag_anchor: 0.0,
            is_thumb_hovered: false,
        }
    }

    /// Whether a scrollbar is needed.
    pub fn needs_scrollbar(&self) -> bool {
        self.content_height > self.viewport_height
    }

    /// Maximum scroll offset.
    pub fn max_scroll_offset(&self) -> f32 {
        (self.content_height - self.viewport_height).max(0.0)
    }

    /// Ratio of viewport to content.
    pub fn thumb_ratio(&self) -> f32 {
        if self.content_height <= 0.0 {
            return 1.0;
        }
        (self.viewport_height / self.content_height).clamp(0.0, 1.0)
    }

    /// Thumb length along the track.
    pub fn thumb_length(&self, track_length: f32) -> f32 {
        (self.thumb_ratio() * track_length).max(self.theme.min_thumb_length)
    }

    /// Thumb bounds within the scrollbar layout.
    pub fn thumb_bounds(&self, layout: &crate::tree::LayoutRect) -> crate::tree::LayoutRect {
        let track_length = layout.height;
        let thumb_len = self.thumb_length(track_length);
        let max_offset = self.max_scroll_offset();
        let scroll_frac = if max_offset > 0.0 {
            self.scroll_offset / max_offset
        } else {
            0.0
        };
        let thumb_y = layout.y + scroll_frac * (track_length - thumb_len);

        crate::tree::LayoutRect {
            x: layout.x,
            y: thumb_y,
            width: layout.width,
            height: thumb_len,
        }
    }

    /// Set scroll offset, clamping to valid range.
    pub fn set_scroll_offset(&mut self, offset: f32) {
        self.scroll_offset = offset.clamp(0.0, self.max_scroll_offset());
    }

    /// Adjust scroll offset by a relative delta.
    pub fn scroll_by(&mut self, delta: f32) {
        self.set_scroll_offset(self.scroll_offset + delta);
    }

    /// Hit-test the thumb.
    pub fn hit_test_thumb(&self, pos: Vec2, layout: &crate::tree::LayoutRect) -> bool {
        let tb = self.thumb_bounds(layout);
        pos.x >= tb.x && pos.x <= tb.x + tb.width && pos.y >= tb.y && pos.y <= tb.y + tb.height
    }

    /// Start thumb drag.
    pub fn start_thumb_drag(&mut self, mouse_y: f32, layout: &crate::tree::LayoutRect) {
        let tb = self.thumb_bounds(layout);
        self.drag_anchor = mouse_y - tb.y;
        self.is_thumb_dragging = true;
    }

    /// Update thumb position during drag.
    pub fn update_thumb_drag(&mut self, mouse_y: f32, layout: &crate::tree::LayoutRect) {
        if !self.is_thumb_dragging {
            return;
        }
        let track_length = layout.height;
        let thumb_len = self.thumb_length(track_length);
        let available = track_length - thumb_len;
        if available <= 0.0 {
            return;
        }
        let thumb_pos = mouse_y - layout.y - self.drag_anchor;
        let frac = (thumb_pos / available).clamp(0.0, 1.0);
        self.scroll_offset = frac * self.max_scroll_offset();
    }

    /// End thumb drag.
    pub fn end_thumb_drag(&mut self) {
        self.is_thumb_dragging = false;
    }

    /// Current thumb color based on state.
    pub fn current_thumb_color(&self) -> Color {
        if self.is_thumb_dragging {
            self.theme.thumb_active_color
        } else if self.is_thumb_hovered {
            self.theme.thumb_hover_color
        } else {
            self.theme.thumb_color
        }
    }
}

impl Widget for VScrollbar {
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

    fn measure(&self, _available_space: Vec2, _font_renderer: Option<&FontRenderer>) -> Vec2 {
        Vec2::new(self.theme.thickness, 0.0)
    }

    fn clone_box(&self) -> Box<dyn Widget> {
        Box::new(self.clone())
    }
}

/// Column widget - vertical layout.
#[derive(Clone)]
pub struct Column {
    pub style: Style,
    pub children: Vec<NodeId>,
}

impl Column {
    pub fn new() -> Self {
        Self {
            style: Style::new()
                .display(taffy::Display::Flex)
                .flex_direction(taffy::FlexDirection::Column),
            children: Vec::new(),
        }
    }

    pub fn gap(mut self, gap: impl Into<crate::constraint::Constraint> + Copy) -> Self {
        self.style = self.style.gap(gap);
        self
    }
}

impl Default for Column {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for Column {
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

    fn clone_box(&self) -> Box<dyn Widget> {
        Box::new(self.clone())
    }
}
