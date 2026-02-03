//! ScrollContainer widget for scrollable content areas.

use crate::style::Style;
use crate::tree::{LayoutRect, NodeId};
use crate::widgets::{ScrollbarTheme, Widget};
use astrelis_core::math::Vec2;
use astrelis_text::FontRenderer;
use std::any::Any;

/// Which axes are scrollable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollAxis {
    /// Only vertical scrolling.
    #[default]
    Vertical,
    /// Only horizontal scrolling.
    Horizontal,
    /// Both axes scrollable.
    Both,
    /// No scrolling (content clips but cannot scroll).
    None,
}

/// When scrollbars are visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollbarVisibility {
    /// Show when content overflows (default).
    #[default]
    Auto,
    /// Always show scrollbars.
    Always,
    /// Never show scrollbars (still scrollable via wheel).
    Never,
}

/// A container that clips its children and supports scrolling.
#[derive(Clone)]
pub struct ScrollContainer {
    pub style: Style,
    pub children: Vec<NodeId>,
    /// Current scroll position in content pixels.
    pub scroll_offset: Vec2,
    /// Which axes are scrollable.
    pub scroll_axis: ScrollAxis,
    /// When scrollbars are shown.
    pub scrollbar_visibility: ScrollbarVisibility,
    /// Scrollbar visual theme.
    pub scrollbar_theme: ScrollbarTheme,
    /// Cached total content extent (computed after layout).
    pub content_size: Vec2,
    /// Cached viewport (container) size (computed after layout).
    pub viewport_size: Vec2,
    // -- Vertical scrollbar interaction state --
    pub v_thumb_hovered: bool,
    pub v_thumb_dragging: bool,
    pub v_drag_anchor: f32,
    // -- Horizontal scrollbar interaction state --
    pub h_thumb_hovered: bool,
    pub h_thumb_dragging: bool,
    pub h_drag_anchor: f32,
}

impl ScrollContainer {
    /// Create a new scroll container with default settings.
    pub fn new() -> Self {
        Self {
            style: Style::new()
                .display(taffy::Display::Flex)
                .flex_direction(taffy::FlexDirection::Column)
                .overflow(crate::style::Overflow::Scroll),
            children: Vec::new(),
            scroll_offset: Vec2::ZERO,
            scroll_axis: ScrollAxis::default(),
            scrollbar_visibility: ScrollbarVisibility::default(),
            scrollbar_theme: ScrollbarTheme::default(),
            content_size: Vec2::ZERO,
            viewport_size: Vec2::ZERO,
            v_thumb_hovered: false,
            v_thumb_dragging: false,
            v_drag_anchor: 0.0,
            h_thumb_hovered: false,
            h_thumb_dragging: false,
            h_drag_anchor: 0.0,
        }
    }

    // -- Scroll management --

    /// Maximum scroll offset for each axis.
    pub fn max_scroll_offset(&self) -> Vec2 {
        Vec2::new(
            (self.content_size.x - self.viewport_size.x).max(0.0),
            (self.content_size.y - self.viewport_size.y).max(0.0),
        )
    }

    /// Whether vertical scrolling is needed (content taller than viewport).
    pub fn needs_v_scroll(&self) -> bool {
        matches!(self.scroll_axis, ScrollAxis::Vertical | ScrollAxis::Both)
            && self.content_size.y > self.viewport_size.y
    }

    /// Whether horizontal scrolling is needed (content wider than viewport).
    pub fn needs_h_scroll(&self) -> bool {
        matches!(
            self.scroll_axis,
            ScrollAxis::Horizontal | ScrollAxis::Both
        ) && self.content_size.x > self.viewport_size.x
    }

    /// Whether the vertical scrollbar should be visible.
    pub fn should_show_v_scrollbar(&self) -> bool {
        match self.scrollbar_visibility {
            ScrollbarVisibility::Auto => self.needs_v_scroll(),
            ScrollbarVisibility::Always => {
                matches!(self.scroll_axis, ScrollAxis::Vertical | ScrollAxis::Both)
            }
            ScrollbarVisibility::Never => false,
        }
    }

    /// Whether the horizontal scrollbar should be visible.
    pub fn should_show_h_scrollbar(&self) -> bool {
        match self.scrollbar_visibility {
            ScrollbarVisibility::Auto => self.needs_h_scroll(),
            ScrollbarVisibility::Always => {
                matches!(
                    self.scroll_axis,
                    ScrollAxis::Horizontal | ScrollAxis::Both
                )
            }
            ScrollbarVisibility::Never => false,
        }
    }

    /// Clamp scroll offset to valid range.
    pub fn clamp_scroll(&mut self) {
        let max = self.max_scroll_offset();
        self.scroll_offset.x = self.scroll_offset.x.clamp(0.0, max.x);
        self.scroll_offset.y = self.scroll_offset.y.clamp(0.0, max.y);
    }

    /// Scroll by a delta in pixels.
    pub fn scroll_by(&mut self, delta: Vec2) {
        self.scroll_offset += delta;
        self.clamp_scroll();
    }

    /// Set absolute scroll offset.
    pub fn set_scroll_offset(&mut self, offset: Vec2) {
        self.scroll_offset = offset;
        self.clamp_scroll();
    }

    /// Scroll to a specific position, clamped.
    pub fn scroll_to(&mut self, position: Vec2) {
        self.set_scroll_offset(position);
    }

    // -- Vertical scrollbar geometry --

    /// Compute the vertical scrollbar track bounds (absolute coordinates).
    pub fn v_scrollbar_track(&self, abs_layout: &LayoutRect) -> LayoutRect {
        let thickness = self.scrollbar_theme.thickness;
        let h_bar_height = if self.should_show_h_scrollbar() {
            thickness
        } else {
            0.0
        };
        LayoutRect {
            x: abs_layout.x + abs_layout.width - thickness,
            y: abs_layout.y,
            width: thickness,
            height: (abs_layout.height - h_bar_height).max(0.0),
        }
    }

    /// Compute the vertical scrollbar thumb bounds (absolute coordinates).
    pub fn v_scrollbar_thumb(&self, abs_layout: &LayoutRect) -> LayoutRect {
        let track = self.v_scrollbar_track(abs_layout);
        let track_length = track.height;

        let ratio = if self.content_size.y > 0.0 {
            (self.viewport_size.y / self.content_size.y).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let thumb_len = (ratio * track_length).max(self.scrollbar_theme.min_thumb_length);

        let max_offset = self.max_scroll_offset().y;
        let scroll_frac = if max_offset > 0.0 {
            self.scroll_offset.y / max_offset
        } else {
            0.0
        };
        let thumb_y = track.y + scroll_frac * (track_length - thumb_len);

        LayoutRect {
            x: track.x,
            y: thumb_y,
            width: track.width,
            height: thumb_len,
        }
    }

    // -- Horizontal scrollbar geometry --

    /// Compute the horizontal scrollbar track bounds (absolute coordinates).
    pub fn h_scrollbar_track(&self, abs_layout: &LayoutRect) -> LayoutRect {
        let thickness = self.scrollbar_theme.thickness;
        let v_bar_width = if self.should_show_v_scrollbar() {
            thickness
        } else {
            0.0
        };
        LayoutRect {
            x: abs_layout.x,
            y: abs_layout.y + abs_layout.height - thickness,
            width: (abs_layout.width - v_bar_width).max(0.0),
            height: thickness,
        }
    }

    /// Compute the horizontal scrollbar thumb bounds (absolute coordinates).
    pub fn h_scrollbar_thumb(&self, abs_layout: &LayoutRect) -> LayoutRect {
        let track = self.h_scrollbar_track(abs_layout);
        let track_length = track.width;

        let ratio = if self.content_size.x > 0.0 {
            (self.viewport_size.x / self.content_size.x).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let thumb_len = (ratio * track_length).max(self.scrollbar_theme.min_thumb_length);

        let max_offset = self.max_scroll_offset().x;
        let scroll_frac = if max_offset > 0.0 {
            self.scroll_offset.x / max_offset
        } else {
            0.0
        };
        let thumb_x = track.x + scroll_frac * (track_length - thumb_len);

        LayoutRect {
            x: thumb_x,
            y: track.y,
            width: thumb_len,
            height: track.height,
        }
    }

    // -- Hit testing --

    fn rect_contains(rect: &LayoutRect, pos: Vec2) -> bool {
        pos.x >= rect.x
            && pos.x <= rect.x + rect.width
            && pos.y >= rect.y
            && pos.y <= rect.y + rect.height
    }

    /// Hit-test the vertical scrollbar thumb.
    pub fn hit_test_v_thumb(&self, pos: Vec2, abs_layout: &LayoutRect) -> bool {
        self.should_show_v_scrollbar() && Self::rect_contains(&self.v_scrollbar_thumb(abs_layout), pos)
    }

    /// Hit-test the horizontal scrollbar thumb.
    pub fn hit_test_h_thumb(&self, pos: Vec2, abs_layout: &LayoutRect) -> bool {
        self.should_show_h_scrollbar()
            && Self::rect_contains(&self.h_scrollbar_thumb(abs_layout), pos)
    }

    /// Hit-test the vertical scrollbar track (but not thumb).
    pub fn hit_test_v_track(&self, pos: Vec2, abs_layout: &LayoutRect) -> bool {
        self.should_show_v_scrollbar()
            && Self::rect_contains(&self.v_scrollbar_track(abs_layout), pos)
    }

    /// Hit-test the horizontal scrollbar track (but not thumb).
    pub fn hit_test_h_track(&self, pos: Vec2, abs_layout: &LayoutRect) -> bool {
        self.should_show_h_scrollbar()
            && Self::rect_contains(&self.h_scrollbar_track(abs_layout), pos)
    }

    // -- Drag handling --

    /// Start dragging the vertical scrollbar thumb.
    pub fn start_v_drag(&mut self, mouse_y: f32, abs_layout: &LayoutRect) {
        let thumb = self.v_scrollbar_thumb(abs_layout);
        self.v_drag_anchor = mouse_y - thumb.y;
        self.v_thumb_dragging = true;
    }

    /// Update vertical scrollbar drag.
    pub fn update_v_drag(&mut self, mouse_y: f32, abs_layout: &LayoutRect) {
        if !self.v_thumb_dragging {
            return;
        }
        let track = self.v_scrollbar_track(abs_layout);
        let track_length = track.height;

        let ratio = if self.content_size.y > 0.0 {
            (self.viewport_size.y / self.content_size.y).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let thumb_len = (ratio * track_length).max(self.scrollbar_theme.min_thumb_length);
        let available = track_length - thumb_len;
        if available <= 0.0 {
            return;
        }
        let thumb_pos = mouse_y - track.y - self.v_drag_anchor;
        let frac = (thumb_pos / available).clamp(0.0, 1.0);
        self.scroll_offset.y = frac * self.max_scroll_offset().y;
    }

    /// End vertical scrollbar drag.
    pub fn end_v_drag(&mut self) {
        self.v_thumb_dragging = false;
    }

    /// Start dragging the horizontal scrollbar thumb.
    pub fn start_h_drag(&mut self, mouse_x: f32, abs_layout: &LayoutRect) {
        let thumb = self.h_scrollbar_thumb(abs_layout);
        self.h_drag_anchor = mouse_x - thumb.x;
        self.h_thumb_dragging = true;
    }

    /// Update horizontal scrollbar drag.
    pub fn update_h_drag(&mut self, mouse_x: f32, abs_layout: &LayoutRect) {
        if !self.h_thumb_dragging {
            return;
        }
        let track = self.h_scrollbar_track(abs_layout);
        let track_length = track.width;

        let ratio = if self.content_size.x > 0.0 {
            (self.viewport_size.x / self.content_size.x).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let thumb_len = (ratio * track_length).max(self.scrollbar_theme.min_thumb_length);
        let available = track_length - thumb_len;
        if available <= 0.0 {
            return;
        }
        let thumb_pos = mouse_x - track.x - self.h_drag_anchor;
        let frac = (thumb_pos / available).clamp(0.0, 1.0);
        self.scroll_offset.x = frac * self.max_scroll_offset().x;
    }

    /// End horizontal scrollbar drag.
    pub fn end_h_drag(&mut self) {
        self.h_thumb_dragging = false;
    }

    /// Whether any scrollbar thumb is currently being dragged.
    pub fn is_any_thumb_dragging(&self) -> bool {
        self.v_thumb_dragging || self.h_thumb_dragging
    }

    /// Current vertical thumb color based on interaction state.
    pub fn v_thumb_color(&self) -> astrelis_render::Color {
        if self.v_thumb_dragging {
            self.scrollbar_theme.thumb_active_color
        } else if self.v_thumb_hovered {
            self.scrollbar_theme.thumb_hover_color
        } else {
            self.scrollbar_theme.thumb_color
        }
    }

    /// Current horizontal thumb color based on interaction state.
    pub fn h_thumb_color(&self) -> astrelis_render::Color {
        if self.h_thumb_dragging {
            self.scrollbar_theme.thumb_active_color
        } else if self.h_thumb_hovered {
            self.scrollbar_theme.thumb_hover_color
        } else {
            self.scrollbar_theme.thumb_color
        }
    }
}

impl Default for ScrollContainer {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for ScrollContainer {
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
        Vec2::ZERO
    }

    fn clone_box(&self) -> Box<dyn Widget> {
        Box::new(self.clone())
    }
}
