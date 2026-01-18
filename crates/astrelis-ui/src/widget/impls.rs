//! Widget implementations using the new capability-based system.
//!
//! This module provides concrete widget implementations that use the new
//! capability trait system instead of the old downcast-based approach.

use super::capability::*;
use crate::style::Style;
use astrelis_render::Color;
use astrelis_text::{FontWeight, Text as TextStyle, TextAlign, VerticalAlign};

/// Container widget - holds other widgets with flexbox layout.
///
/// Implements ParentWidget capability for containing children.
pub struct Container {
    id: WidgetId,
    node: taffy::NodeId,
    /// Style for future implementation (currently managed by UiTree)
    #[allow(dead_code)]
    style: Style,
    children: Vec<Box<dyn Widget>>,
}

impl Container {
    pub fn new(id: WidgetId, node: taffy::NodeId) -> Self {
        Self {
            id,
            node,
            style: Style::new().display(taffy::Display::Flex),
            children: Vec::new(),
        }
    }

    pub fn with_style(id: WidgetId, node: taffy::NodeId, style: Style) -> Self {
        Self {
            id,
            node,
            style,
            children: Vec::new(),
        }
    }
}

impl Widget for Container {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout_node(&self) -> taffy::NodeId {
        self.node
    }

    fn debug_name(&self) -> &str {
        "Container"
    }

    fn as_container(&self) -> Option<&dyn ParentWidget> {
        Some(self)
    }

    fn as_container_mut(&mut self) -> Option<&mut dyn ParentWidget> {
        Some(self)
    }
}

impl ParentWidget for Container {
    fn children(&self) -> &[Box<dyn Widget>] {
        &self.children
    }

    fn children_mut(&mut self) -> &mut [Box<dyn Widget>] {
        &mut self.children
    }

    fn add_child(&mut self, child: Box<dyn Widget>) {
        self.children.push(child);
    }

    fn remove_child(&mut self, id: WidgetId) -> Option<Box<dyn Widget>> {
        let pos = self.children.iter().position(|c| c.id() == id)?;
        Some(self.children.remove(pos))
    }
}

/// Text widget - displays text.
///
/// Implements TextWidget capability for text display and styling.
pub struct Text {
    id: WidgetId,
    node: taffy::NodeId,
    content: String,
    font_size: f32,
    color: Color,
    weight: FontWeight,
    align: TextAlign,
    vertical_align: VerticalAlign,
    /// Style for future implementation (currently managed by UiTree)
    #[allow(dead_code)]
    style: Style,
}

impl Text {
    pub fn new(id: WidgetId, node: taffy::NodeId, content: impl Into<String>) -> Self {
        Self {
            id,
            node,
            content: content.into(),
            font_size: 16.0,
            color: Color::WHITE,
            weight: FontWeight::Normal,
            align: TextAlign::Left,
            vertical_align: VerticalAlign::Top,
            style: Style::new(),
        }
    }

    pub fn size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn weight(mut self, weight: FontWeight) -> Self {
        self.weight = weight;
        self
    }

    pub fn bold(mut self) -> Self {
        self.weight = FontWeight::Bold;
        self
    }

    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    pub fn vertical_align(mut self, vertical_align: VerticalAlign) -> Self {
        self.vertical_align = vertical_align;
        self
    }

    /// Build a TextStyle for rendering.
    fn build_text_style(&self) -> TextStyle {
        TextStyle::new(&self.content)
            .size(self.font_size)
            .color(self.color)
            .weight(self.weight)
            .align(self.align)
            .vertical_align(self.vertical_align)
    }
}

impl Widget for Text {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout_node(&self) -> taffy::NodeId {
        self.node
    }

    fn debug_name(&self) -> &str {
        "Text"
    }

    fn as_text_widget(&self) -> Option<&dyn TextWidget> {
        Some(self)
    }

    fn as_text_widget_mut(&mut self) -> Option<&mut dyn TextWidget> {
        Some(self)
    }
}

impl TextWidget for Text {
    fn text(&self) -> &str {
        &self.content
    }

    fn set_text(&mut self, text: &str) {
        self.content = text.to_string();
    }

    fn build_text_style(&self) -> TextStyle {
        self.build_text_style()
    }

    fn text_align(&self) -> TextAlign {
        self.align
    }

    fn set_text_align(&mut self, align: TextAlign) {
        self.align = align;
    }

    fn vertical_align(&self) -> VerticalAlign {
        self.vertical_align
    }

    fn set_vertical_align(&mut self, align: VerticalAlign) {
        self.vertical_align = align;
    }
}

/// Button widget - clickable with label.
///
/// Implements TextWidget for label and ColorWidget for background color.
pub struct Button {
    id: WidgetId,
    node: taffy::NodeId,
    label: String,
    text_color: Color,
    font_size: f32,
    text_align: TextAlign,
    text_vertical_align: VerticalAlign,
    style: Style,
    background_color: Color,
    hover_color: Color,
    active_color: Color,
    is_hovered: bool,
    is_pressed: bool,
}

impl Button {
    pub fn new(id: WidgetId, node: taffy::NodeId, label: impl Into<String>) -> Self {
        Self {
            id,
            node,
            label: label.into(),
            text_color: Color::WHITE,
            font_size: 16.0,
            text_align: TextAlign::Left,
            text_vertical_align: VerticalAlign::Top,
            style: Style::new()
                .display(taffy::Display::Flex)
                .padding(10.0)
                .background_color(Color::from_rgb_u8(60, 60, 80))
                .border_radius(4.0),
            background_color: Color::from_rgb_u8(60, 60, 80),
            hover_color: Color::from_rgb_u8(80, 80, 100),
            active_color: Color::from_rgb_u8(40, 40, 60),
            is_hovered: false,
            is_pressed: false,
        }
    }

    pub fn background_color(mut self, color: Color) -> Self {
        self.background_color = color;
        self.style = self.style.background_color(color);
        self
    }

    pub fn hover_color(mut self, color: Color) -> Self {
        self.hover_color = color;
        self
    }

    pub fn text_color(mut self, color: Color) -> Self {
        self.text_color = color;
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Get current background color based on state.
    pub fn current_bg_color(&self) -> Color {
        if self.is_pressed {
            self.active_color
        } else if self.is_hovered {
            self.hover_color
        } else {
            self.background_color
        }
    }

    /// Build a TextStyle for rendering.
    fn build_text_style(&self) -> TextStyle {
        TextStyle::new(&self.label)
            .size(self.font_size)
            .color(self.text_color)
            .align(self.text_align)
            .vertical_align(self.text_vertical_align)
    }
}

impl Widget for Button {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout_node(&self) -> taffy::NodeId {
        self.node
    }

    fn debug_name(&self) -> &str {
        "Button"
    }

    fn as_text_widget(&self) -> Option<&dyn TextWidget> {
        Some(self)
    }

    fn as_text_widget_mut(&mut self) -> Option<&mut dyn TextWidget> {
        Some(self)
    }

    fn as_color_widget(&self) -> Option<&dyn ColorWidget> {
        Some(self)
    }

    fn as_color_widget_mut(&mut self) -> Option<&mut dyn ColorWidget> {
        Some(self)
    }
}

impl TextWidget for Button {
    fn text(&self) -> &str {
        &self.label
    }

    fn set_text(&mut self, text: &str) {
        self.label = text.to_string();
    }

    fn build_text_style(&self) -> TextStyle {
        self.build_text_style()
    }

    fn text_align(&self) -> TextAlign {
        self.text_align
    }

    fn set_text_align(&mut self, align: TextAlign) {
        self.text_align = align;
    }

    fn vertical_align(&self) -> VerticalAlign {
        self.text_vertical_align
    }

    fn set_vertical_align(&mut self, align: VerticalAlign) {
        self.text_vertical_align = align;
    }
}

impl ColorWidget for Button {
    fn color(&self) -> Color {
        self.current_bg_color()
    }

    fn set_color(&mut self, color: Color) {
        self.background_color = color;
        self.style.background_color = Some(color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_children() {
        let mut container = Container::new(WidgetId(0), taffy::NodeId::from(0u64));

        assert_eq!(container.children().len(), 0);

        let text = Text::new(WidgetId(1), taffy::NodeId::from(1u64), "Hello");
        container.add_child(Box::new(text));

        assert_eq!(container.children().len(), 1);

        let removed = container.remove_child(WidgetId(1));
        assert!(removed.is_some());
        assert_eq!(container.children().len(), 0);
    }

    #[test]
    fn test_text_widget() {
        let mut text = Text::new(WidgetId(0), taffy::NodeId::from(0u64), "Hello");

        assert_eq!(text.text(), "Hello");

        text.set_text("World");
        assert_eq!(text.text(), "World");
    }

    #[test]
    fn test_button_capabilities() {
        let mut button = Button::new(WidgetId(0), taffy::NodeId::from(0u64), "Click");

        // Test TextWidget capability
        assert_eq!(button.text(), "Click");
        button.set_text("Updated");
        assert_eq!(button.text(), "Updated");

        // Test ColorWidget capability
        let new_color = Color::RED;
        button.set_color(new_color);
        assert_eq!(button.color(), new_color);
    }

    #[test]
    fn test_widget_queries() {
        let button = Button::new(WidgetId(0), taffy::NodeId::from(0u64), "Test");

        // Should support text widget query
        assert!(button.as_text_widget().is_some());

        // Should support color widget query
        assert!(button.as_color_widget().is_some());

        // Should not support container query
        assert!(button.as_container().is_none());
    }
}
