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

// ===== Slider Widget =====

/// Slider widget for selecting a numeric value within a range.
pub struct Slider {
    id: WidgetId,
    node: taffy::NodeId,
    value: f32,
    min: f32,
    max: f32,
    step: Option<f32>,
    style: Style,
    track_color: Color,
    thumb_color: Color,
    fill_color: Color,
    is_dragging: bool,
    is_hovered: bool,
    orientation: SliderOrientation,
}

/// Slider orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliderOrientation {
    Horizontal,
    Vertical,
}

impl Slider {
    pub fn new(id: WidgetId, node: taffy::NodeId, min: f32, max: f32) -> Self {
        Self {
            id,
            node,
            value: min,
            min,
            max,
            step: None,
            style: Style::new()
                .display(taffy::Display::Flex)
                .width(200.0)
                .height(20.0)
                .background_color(Color::from_rgb_u8(40, 40, 50)),
            track_color: Color::from_rgb_u8(40, 40, 50),
            thumb_color: Color::from_rgb_u8(100, 100, 120),
            fill_color: Color::from_rgb_u8(60, 120, 200),
            is_dragging: false,
            is_hovered: false,
            orientation: SliderOrientation::Horizontal,
        }
    }

    pub fn value(mut self, value: f32) -> Self {
        self.value = value.clamp(self.min, self.max);
        self
    }

    pub fn step(mut self, step: f32) -> Self {
        self.step = Some(step);
        self
    }

    pub fn track_color(mut self, color: Color) -> Self {
        self.track_color = color;
        self
    }

    pub fn thumb_color(mut self, color: Color) -> Self {
        self.thumb_color = color;
        self
    }

    pub fn fill_color(mut self, color: Color) -> Self {
        self.fill_color = color;
        self
    }

    pub fn orientation(mut self, orientation: SliderOrientation) -> Self {
        self.orientation = orientation;
        self
    }

    pub fn get_value(&self) -> f32 {
        self.value
    }

    pub fn set_value(&mut self, value: f32) {
        self.value = if let Some(step) = self.step {
            let steps = ((value - self.min) / step).round();
            (self.min + steps * step).clamp(self.min, self.max)
        } else {
            value.clamp(self.min, self.max)
        };
    }

    pub fn normalized_value(&self) -> f32 {
        if self.max == self.min {
            0.0
        } else {
            (self.value - self.min) / (self.max - self.min)
        }
    }

    pub fn set_is_dragging(&mut self, dragging: bool) {
        self.is_dragging = dragging;
    }

    pub fn set_is_hovered(&mut self, hovered: bool) {
        self.is_hovered = hovered;
    }
}

impl Widget for Slider {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout_node(&self) -> taffy::NodeId {
        self.node
    }

    fn debug_name(&self) -> &str {
        "Slider"
    }

    fn as_color_widget(&self) -> Option<&dyn ColorWidget> {
        Some(self)
    }

    fn as_color_widget_mut(&mut self) -> Option<&mut dyn ColorWidget> {
        Some(self)
    }
}

impl ColorWidget for Slider {
    fn color(&self) -> Color {
        self.track_color
    }

    fn set_color(&mut self, color: Color) {
        self.track_color = color;
        self.style.background_color = Some(color);
    }
}

// ===== Checkbox Widget =====

/// Checkbox widget for boolean selection.
pub struct Checkbox {
    id: WidgetId,
    node: taffy::NodeId,
    checked: bool,
    label: Option<String>,
    style: Style,
    box_color: Color,
    check_color: Color,
    text_color: Color,
    font_size: f32,
    is_hovered: bool,
}

impl Checkbox {
    pub fn new(id: WidgetId, node: taffy::NodeId) -> Self {
        Self {
            id,
            node,
            checked: false,
            label: None,
            style: Style::new()
                .display(taffy::Display::Flex)
                .width(20.0)
                .height(20.0)
                .background_color(Color::from_rgb_u8(40, 40, 50))
                .border_radius(3.0),
            box_color: Color::from_rgb_u8(40, 40, 50),
            check_color: Color::from_rgb_u8(60, 120, 200),
            text_color: Color::WHITE,
            font_size: 14.0,
            is_hovered: false,
        }
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn box_color(mut self, color: Color) -> Self {
        self.box_color = color;
        self
    }

    pub fn check_color(mut self, color: Color) -> Self {
        self.check_color = color;
        self
    }

    pub fn text_color(mut self, color: Color) -> Self {
        self.text_color = color;
        self
    }

    pub fn is_checked(&self) -> bool {
        self.checked
    }

    pub fn set_checked(&mut self, checked: bool) {
        self.checked = checked;
    }

    pub fn toggle(&mut self) {
        self.checked = !self.checked;
    }

    pub fn set_is_hovered(&mut self, hovered: bool) {
        self.is_hovered = hovered;
    }

    pub fn current_box_color(&self) -> Color {
        if self.checked {
            self.check_color
        } else if self.is_hovered {
            Color::from_rgb_u8(50, 50, 60)
        } else {
            self.box_color
        }
    }
}

impl Widget for Checkbox {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout_node(&self) -> taffy::NodeId {
        self.node
    }

    fn debug_name(&self) -> &str {
        "Checkbox"
    }

    fn as_color_widget(&self) -> Option<&dyn ColorWidget> {
        Some(self)
    }

    fn as_color_widget_mut(&mut self) -> Option<&mut dyn ColorWidget> {
        Some(self)
    }

    fn as_text_widget(&self) -> Option<&dyn TextWidget> {
        Some(self)
    }

    fn as_text_widget_mut(&mut self) -> Option<&mut dyn TextWidget> {
        Some(self)
    }
}

impl ColorWidget for Checkbox {
    fn color(&self) -> Color {
        self.current_box_color()
    }

    fn set_color(&mut self, color: Color) {
        self.box_color = color;
        self.style.background_color = Some(color);
    }
}

impl TextWidget for Checkbox {
    fn text(&self) -> &str {
        self.label.as_deref().unwrap_or("")
    }

    fn set_text(&mut self, text: &str) {
        self.label = Some(text.to_string());
    }

    fn build_text_style(&self) -> TextStyle {
        TextStyle::new(self.label.as_deref().unwrap_or(""))
            .size(self.font_size)
            .color(self.text_color)
    }

    fn text_align(&self) -> TextAlign {
        TextAlign::Left
    }

    fn set_text_align(&mut self, _align: TextAlign) {
        // Not applicable for checkbox
    }

    fn vertical_align(&self) -> VerticalAlign {
        VerticalAlign::Center
    }

    fn set_vertical_align(&mut self, _align: VerticalAlign) {
        // Not applicable for checkbox
    }
}

// ===== RadioButton Widget =====

/// RadioButton widget for mutually exclusive selection within a group.
pub struct RadioButton {
    id: WidgetId,
    node: taffy::NodeId,
    selected: bool,
    group: String,
    label: Option<String>,
    style: Style,
    circle_color: Color,
    selected_color: Color,
    text_color: Color,
    font_size: f32,
    is_hovered: bool,
}

impl RadioButton {
    pub fn new(id: WidgetId, node: taffy::NodeId, group: impl Into<String>) -> Self {
        Self {
            id,
            node,
            selected: false,
            group: group.into(),
            label: None,
            style: Style::new()
                .display(taffy::Display::Flex)
                .width(20.0)
                .height(20.0)
                .background_color(Color::from_rgb_u8(40, 40, 50))
                .border_radius(10.0), // Circular
            circle_color: Color::from_rgb_u8(40, 40, 50),
            selected_color: Color::from_rgb_u8(60, 120, 200),
            text_color: Color::WHITE,
            font_size: 14.0,
            is_hovered: false,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn circle_color(mut self, color: Color) -> Self {
        self.circle_color = color;
        self
    }

    pub fn selected_color(mut self, color: Color) -> Self {
        self.selected_color = color;
        self
    }

    pub fn text_color(mut self, color: Color) -> Self {
        self.text_color = color;
        self
    }

    pub fn is_selected(&self) -> bool {
        self.selected
    }

    pub fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
    }

    pub fn group(&self) -> &str {
        &self.group
    }

    pub fn set_is_hovered(&mut self, hovered: bool) {
        self.is_hovered = hovered;
    }

    pub fn current_circle_color(&self) -> Color {
        if self.selected {
            self.selected_color
        } else if self.is_hovered {
            Color::from_rgb_u8(50, 50, 60)
        } else {
            self.circle_color
        }
    }
}

impl Widget for RadioButton {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout_node(&self) -> taffy::NodeId {
        self.node
    }

    fn debug_name(&self) -> &str {
        "RadioButton"
    }

    fn as_color_widget(&self) -> Option<&dyn ColorWidget> {
        Some(self)
    }

    fn as_color_widget_mut(&mut self) -> Option<&mut dyn ColorWidget> {
        Some(self)
    }

    fn as_text_widget(&self) -> Option<&dyn TextWidget> {
        Some(self)
    }

    fn as_text_widget_mut(&mut self) -> Option<&mut dyn TextWidget> {
        Some(self)
    }
}

impl ColorWidget for RadioButton {
    fn color(&self) -> Color {
        self.current_circle_color()
    }

    fn set_color(&mut self, color: Color) {
        self.circle_color = color;
        self.style.background_color = Some(color);
    }
}

impl TextWidget for RadioButton {
    fn text(&self) -> &str {
        self.label.as_deref().unwrap_or("")
    }

    fn set_text(&mut self, text: &str) {
        self.label = Some(text.to_string());
    }

    fn build_text_style(&self) -> TextStyle {
        TextStyle::new(self.label.as_deref().unwrap_or(""))
            .size(self.font_size)
            .color(self.text_color)
    }

    fn text_align(&self) -> TextAlign {
        TextAlign::Left
    }

    fn set_text_align(&mut self, _align: TextAlign) {
        // Not applicable for radio button
    }

    fn vertical_align(&self) -> VerticalAlign {
        VerticalAlign::Center
    }

    fn set_vertical_align(&mut self, _align: VerticalAlign) {
        // Not applicable for radio button
    }
}

// ================================================================================================
// Dropdown Widget
// ================================================================================================

/// Dropdown/Select widget with expandable options.
pub struct Dropdown {
    id: WidgetId,
    node: taffy::NodeId,
    style: Style,
    options: Vec<String>,
    selected: Option<usize>,
    is_open: bool,
    background_color: Color,
    selected_bg_color: Color,
    hover_color: Color,
    text_color: Color,
    font_size: f32,
    hovered_option: Option<usize>,
}

impl Dropdown {
    pub fn new(id: WidgetId, node: taffy::NodeId, options: Vec<String>) -> Self {
        Self {
            id,
            node,
            style: Style::default(),
            options,
            selected: None,
            is_open: false,
            background_color: Color::from_rgb_u8(40, 40, 50),
            selected_bg_color: Color::from_rgb_u8(60, 120, 200),
            hover_color: Color::from_rgb_u8(60, 60, 70),
            text_color: Color::WHITE,
            font_size: 14.0,
            hovered_option: None,
        }
    }

    pub fn selected(mut self, index: Option<usize>) -> Self {
        if let Some(idx) = index {
            if idx < self.options.len() {
                self.selected = Some(idx);
            }
        }
        self
    }

    pub fn background_color(mut self, color: Color) -> Self {
        self.background_color = color;
        self
    }

    pub fn selected_bg_color(mut self, color: Color) -> Self {
        self.selected_bg_color = color;
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

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    pub fn toggle_open(&mut self) {
        self.is_open = !self.is_open;
    }

    pub fn set_open(&mut self, open: bool) {
        self.is_open = open;
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected
    }

    pub fn selected_text(&self) -> Option<&str> {
        self.selected.and_then(|idx| self.options.get(idx).map(|s| s.as_str()))
    }

    pub fn options(&self) -> &[String] {
        &self.options
    }

    pub fn select(&mut self, index: usize) {
        if index < self.options.len() {
            self.selected = Some(index);
            self.is_open = false;
        }
    }

    pub fn set_hovered_option(&mut self, index: Option<usize>) {
        self.hovered_option = index;
    }

    pub fn hovered_option(&self) -> Option<usize> {
        self.hovered_option
    }

    pub fn option_color(&self, index: usize) -> Color {
        if Some(index) == self.selected {
            self.selected_bg_color
        } else if Some(index) == self.hovered_option {
            self.hover_color
        } else {
            self.background_color
        }
    }

    pub fn current_background_color(&self) -> Color {
        if self.is_open {
            self.hover_color
        } else {
            self.background_color
        }
    }
}

impl Widget for Dropdown {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout_node(&self) -> taffy::NodeId {
        self.node
    }

    fn debug_name(&self) -> &str {
        "Dropdown"
    }

    fn as_color_widget(&self) -> Option<&dyn ColorWidget> {
        Some(self)
    }

    fn as_color_widget_mut(&mut self) -> Option<&mut dyn ColorWidget> {
        Some(self)
    }

    fn as_text_widget(&self) -> Option<&dyn TextWidget> {
        Some(self)
    }

    fn as_text_widget_mut(&mut self) -> Option<&mut dyn TextWidget> {
        Some(self)
    }
}

impl ColorWidget for Dropdown {
    fn color(&self) -> Color {
        self.current_background_color()
    }

    fn set_color(&mut self, color: Color) {
        self.background_color = color;
        self.style.background_color = Some(color);
    }
}

impl TextWidget for Dropdown {
    fn text(&self) -> &str {
        self.selected_text().unwrap_or("Select...")
    }

    fn set_text(&mut self, _text: &str) {
        // Not applicable for dropdown - use select() instead
    }

    fn build_text_style(&self) -> TextStyle {
        TextStyle::new(self.text())
            .size(self.font_size)
            .color(self.text_color)
    }

    fn text_align(&self) -> TextAlign {
        TextAlign::Left
    }

    fn set_text_align(&mut self, _align: TextAlign) {
        // Not applicable for dropdown
    }

    fn vertical_align(&self) -> VerticalAlign {
        VerticalAlign::Center
    }

    fn set_vertical_align(&mut self, _align: VerticalAlign) {
        // Not applicable for dropdown
    }
}

// ================================================================================================
// ProgressBar Widget
// ================================================================================================

/// Progress bar widget showing completion status.
pub struct ProgressBar {
    id: WidgetId,
    node: taffy::NodeId,
    style: Style,
    value: f32, // 0.0 to 1.0
    indeterminate: bool,
    show_label: bool,
    background_color: Color,
    fill_color: Color,
    text_color: Color,
    font_size: f32,
    border_radius: f32,
}

impl ProgressBar {
    pub fn new(id: WidgetId, node: taffy::NodeId) -> Self {
        Self {
            id,
            node,
            style: Style::default(),
            value: 0.0,
            indeterminate: false,
            show_label: false,
            background_color: Color::from_rgb_u8(40, 40, 50),
            fill_color: Color::from_rgb_u8(60, 120, 200),
            text_color: Color::WHITE,
            font_size: 12.0,
            border_radius: 4.0,
        }
    }

    pub fn value(mut self, value: f32) -> Self {
        self.value = value.clamp(0.0, 1.0);
        self
    }

    pub fn indeterminate(mut self, indeterminate: bool) -> Self {
        self.indeterminate = indeterminate;
        self
    }

    pub fn show_label(mut self, show: bool) -> Self {
        self.show_label = show;
        self
    }

    pub fn background_color(mut self, color: Color) -> Self {
        self.background_color = color;
        self
    }

    pub fn fill_color(mut self, color: Color) -> Self {
        self.fill_color = color;
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

    pub fn border_radius(mut self, radius: f32) -> Self {
        self.border_radius = radius;
        self
    }

    pub fn get_value(&self) -> f32 {
        self.value
    }

    pub fn set_value(&mut self, value: f32) {
        self.value = value.clamp(0.0, 1.0);
    }

    pub fn is_indeterminate(&self) -> bool {
        self.indeterminate
    }

    pub fn set_indeterminate(&mut self, indeterminate: bool) {
        self.indeterminate = indeterminate;
    }

    pub fn is_complete(&self) -> bool {
        self.value >= 1.0
    }

    pub fn percentage(&self) -> f32 {
        self.value * 100.0
    }

    pub fn label_text(&self) -> String {
        if self.indeterminate {
            "Loading...".to_string()
        } else {
            format!("{}%", self.percentage() as i32)
        }
    }

    pub fn current_fill_color(&self) -> Color {
        if self.is_complete() {
            Color::from_rgb_u8(80, 200, 120) // Green when complete
        } else {
            self.fill_color
        }
    }
}

impl Widget for ProgressBar {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout_node(&self) -> taffy::NodeId {
        self.node
    }

    fn debug_name(&self) -> &str {
        "ProgressBar"
    }

    fn as_color_widget(&self) -> Option<&dyn ColorWidget> {
        Some(self)
    }

    fn as_color_widget_mut(&mut self) -> Option<&mut dyn ColorWidget> {
        Some(self)
    }

    fn as_text_widget(&self) -> Option<&dyn TextWidget> {
        if self.show_label {
            Some(self)
        } else {
            None
        }
    }

    fn as_text_widget_mut(&mut self) -> Option<&mut dyn TextWidget> {
        if self.show_label {
            Some(self)
        } else {
            None
        }
    }
}

impl ColorWidget for ProgressBar {
    fn color(&self) -> Color {
        self.background_color
    }

    fn set_color(&mut self, color: Color) {
        self.background_color = color;
        self.style.background_color = Some(color);
    }
}

impl TextWidget for ProgressBar {
    fn text(&self) -> &str {
        // Return empty string since we generate the text dynamically
        ""
    }

    fn set_text(&mut self, _text: &str) {
        // Not applicable for progress bar
    }

    fn build_text_style(&self) -> TextStyle {
        TextStyle::new(&self.label_text())
            .size(self.font_size)
            .color(self.text_color)
    }

    fn text_align(&self) -> TextAlign {
        TextAlign::Center
    }

    fn set_text_align(&mut self, _align: TextAlign) {
        // Not applicable for progress bar
    }

    fn vertical_align(&self) -> VerticalAlign {
        VerticalAlign::Center
    }

    fn set_vertical_align(&mut self, _align: VerticalAlign) {
        // Not applicable for progress bar
    }
}

// ================================================================================================
// ScrollView Widget
// ================================================================================================

/// Scrollbar visibility mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarVisibility {
    /// Always show scrollbars
    Always,
    /// Show scrollbars only when content overflows
    Auto,
    /// Never show scrollbars
    Never,
}

/// ScrollView widget for scrollable content.
pub struct ScrollView {
    id: WidgetId,
    node: taffy::NodeId,
    style: Style,
    children: Vec<Box<dyn Widget>>,
    scroll_offset: astrelis_core::math::Vec2,
    viewport_size: astrelis_core::math::Vec2,
    content_size: astrelis_core::math::Vec2,
    scrollbars: ScrollbarVisibility,
    scrollbar_width: f32,
    background_color: Color,
    scrollbar_color: Color,
    scrollbar_hover_color: Color,
    horizontal_scroll: bool,
    vertical_scroll: bool,
    is_dragging_h_scrollbar: bool,
    is_dragging_v_scrollbar: bool,
    is_hovering_h_scrollbar: bool,
    is_hovering_v_scrollbar: bool,
}

impl ScrollView {
    pub fn new(id: WidgetId, node: taffy::NodeId) -> Self {
        Self {
            id,
            node,
            style: Style::default(),
            children: Vec::new(),
            scroll_offset: astrelis_core::math::Vec2::ZERO,
            viewport_size: astrelis_core::math::Vec2::new(100.0, 100.0),
            content_size: astrelis_core::math::Vec2::ZERO,
            scrollbars: ScrollbarVisibility::Auto,
            scrollbar_width: 8.0,
            background_color: Color::from_rgb_u8(30, 30, 35),
            scrollbar_color: Color::from_rgb_u8(80, 80, 90),
            scrollbar_hover_color: Color::from_rgb_u8(120, 120, 130),
            horizontal_scroll: true,
            vertical_scroll: true,
            is_dragging_h_scrollbar: false,
            is_dragging_v_scrollbar: false,
            is_hovering_h_scrollbar: false,
            is_hovering_v_scrollbar: false,
        }
    }

    pub fn scrollbars(mut self, visibility: ScrollbarVisibility) -> Self {
        self.scrollbars = visibility;
        self
    }

    pub fn scrollbar_width(mut self, width: f32) -> Self {
        self.scrollbar_width = width;
        self
    }

    pub fn background_color(mut self, color: Color) -> Self {
        self.background_color = color;
        self
    }

    pub fn scrollbar_color(mut self, color: Color) -> Self {
        self.scrollbar_color = color;
        self
    }

    pub fn horizontal_scroll(mut self, enabled: bool) -> Self {
        self.horizontal_scroll = enabled;
        self
    }

    pub fn vertical_scroll(mut self, enabled: bool) -> Self {
        self.vertical_scroll = enabled;
        self
    }

    pub fn scroll_offset(&self) -> astrelis_core::math::Vec2 {
        self.scroll_offset
    }

    pub fn set_scroll_offset(&mut self, offset: astrelis_core::math::Vec2) {
        self.scroll_offset = offset;
        self.clamp_scroll_offset();
    }

    pub fn scroll_by(&mut self, delta: astrelis_core::math::Vec2) {
        self.scroll_offset += delta;
        self.clamp_scroll_offset();
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll_offset.y = 0.0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset.y = (self.content_size.y - self.viewport_size.y).max(0.0);
    }

    pub fn scroll_to_left(&mut self) {
        self.scroll_offset.x = 0.0;
    }

    pub fn scroll_to_right(&mut self) {
        self.scroll_offset.x = (self.content_size.x - self.viewport_size.x).max(0.0);
    }

    fn clamp_scroll_offset(&mut self) {
        let max_scroll_x = (self.content_size.x - self.viewport_size.x).max(0.0);
        let max_scroll_y = (self.content_size.y - self.viewport_size.y).max(0.0);

        self.scroll_offset.x = self.scroll_offset.x.clamp(0.0, max_scroll_x);
        self.scroll_offset.y = self.scroll_offset.y.clamp(0.0, max_scroll_y);
    }

    pub fn viewport_size(&self) -> astrelis_core::math::Vec2 {
        self.viewport_size
    }

    pub fn set_viewport_size(&mut self, size: astrelis_core::math::Vec2) {
        self.viewport_size = size;
        self.clamp_scroll_offset();
    }

    pub fn content_size(&self) -> astrelis_core::math::Vec2 {
        self.content_size
    }

    pub fn set_content_size(&mut self, size: astrelis_core::math::Vec2) {
        self.content_size = size;
        self.clamp_scroll_offset();
    }

    pub fn can_scroll_horizontally(&self) -> bool {
        self.horizontal_scroll && self.content_size.x > self.viewport_size.x
    }

    pub fn can_scroll_vertically(&self) -> bool {
        self.vertical_scroll && self.content_size.y > self.viewport_size.y
    }

    pub fn should_show_horizontal_scrollbar(&self) -> bool {
        match self.scrollbars {
            ScrollbarVisibility::Always => self.horizontal_scroll,
            ScrollbarVisibility::Auto => self.can_scroll_horizontally(),
            ScrollbarVisibility::Never => false,
        }
    }

    pub fn should_show_vertical_scrollbar(&self) -> bool {
        match self.scrollbars {
            ScrollbarVisibility::Always => self.vertical_scroll,
            ScrollbarVisibility::Auto => self.can_scroll_vertically(),
            ScrollbarVisibility::Never => false,
        }
    }

    pub fn horizontal_scrollbar_rect(&self) -> Option<(f32, f32, f32, f32)> {
        if !self.should_show_horizontal_scrollbar() {
            return None;
        }

        let scrollbar_length = self.viewport_size.x;
        let content_ratio = self.viewport_size.x / self.content_size.x;
        let thumb_width = (scrollbar_length * content_ratio).max(20.0);

        let max_scroll = self.content_size.x - self.viewport_size.x;
        let scroll_ratio = if max_scroll > 0.0 {
            self.scroll_offset.x / max_scroll
        } else {
            0.0
        };
        let thumb_x = scroll_ratio * (scrollbar_length - thumb_width);

        let y = self.viewport_size.y - self.scrollbar_width;

        Some((thumb_x, y, thumb_width, self.scrollbar_width))
    }

    pub fn vertical_scrollbar_rect(&self) -> Option<(f32, f32, f32, f32)> {
        if !self.should_show_vertical_scrollbar() {
            return None;
        }

        let scrollbar_length = self.viewport_size.y;
        let content_ratio = self.viewport_size.y / self.content_size.y;
        let thumb_height = (scrollbar_length * content_ratio).max(20.0);

        let max_scroll = self.content_size.y - self.viewport_size.y;
        let scroll_ratio = if max_scroll > 0.0 {
            self.scroll_offset.y / max_scroll
        } else {
            0.0
        };
        let thumb_y = scroll_ratio * (scrollbar_length - thumb_height);

        let x = self.viewport_size.x - self.scrollbar_width;

        Some((x, thumb_y, self.scrollbar_width, thumb_height))
    }

    pub fn set_dragging_h_scrollbar(&mut self, dragging: bool) {
        self.is_dragging_h_scrollbar = dragging;
    }

    pub fn set_dragging_v_scrollbar(&mut self, dragging: bool) {
        self.is_dragging_v_scrollbar = dragging;
    }

    pub fn set_hovering_h_scrollbar(&mut self, hovering: bool) {
        self.is_hovering_h_scrollbar = hovering;
    }

    pub fn set_hovering_v_scrollbar(&mut self, hovering: bool) {
        self.is_hovering_v_scrollbar = hovering;
    }

    pub fn h_scrollbar_color(&self) -> Color {
        if self.is_dragging_h_scrollbar || self.is_hovering_h_scrollbar {
            self.scrollbar_hover_color
        } else {
            self.scrollbar_color
        }
    }

    pub fn v_scrollbar_color(&self) -> Color {
        if self.is_dragging_v_scrollbar || self.is_hovering_v_scrollbar {
            self.scrollbar_hover_color
        } else {
            self.scrollbar_color
        }
    }
}

impl Widget for ScrollView {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout_node(&self) -> taffy::NodeId {
        self.node
    }

    fn debug_name(&self) -> &str {
        "ScrollView"
    }

    fn as_color_widget(&self) -> Option<&dyn ColorWidget> {
        Some(self)
    }

    fn as_color_widget_mut(&mut self) -> Option<&mut dyn ColorWidget> {
        Some(self)
    }

    fn as_container(&self) -> Option<&dyn ParentWidget> {
        Some(self)
    }

    fn as_container_mut(&mut self) -> Option<&mut dyn ParentWidget> {
        Some(self)
    }
}

impl ColorWidget for ScrollView {
    fn color(&self) -> Color {
        self.background_color
    }

    fn set_color(&mut self, color: Color) {
        self.background_color = color;
        self.style.background_color = Some(color);
    }
}

impl ParentWidget for ScrollView {
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
        if let Some(index) = self.children.iter().position(|c| c.id() == id) {
            Some(self.children.remove(index))
        } else {
            None
        }
    }
}

// ================================================================================================
// Tabs Widget
// ================================================================================================

/// A single tab with label and content.
#[derive(Debug, Clone)]
pub struct Tab {
    pub label: String,
    pub enabled: bool,
}

impl Tab {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            enabled: true,
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// Tab view widget with multiple tabs.
pub struct TabView {
    id: WidgetId,
    node: taffy::NodeId,
    style: Style,
    tabs: Vec<Tab>,
    children: Vec<Box<dyn Widget>>, // One child per tab (content)
    active_tab: usize,
    tab_height: f32,
    background_color: Color,
    active_tab_color: Color,
    inactive_tab_color: Color,
    text_color: Color,
    active_text_color: Color,
    disabled_text_color: Color,
    font_size: f32,
    hovered_tab: Option<usize>,
}

impl TabView {
    pub fn new(id: WidgetId, node: taffy::NodeId) -> Self {
        Self {
            id,
            node,
            style: Style::default(),
            tabs: Vec::new(),
            children: Vec::new(),
            active_tab: 0,
            tab_height: 32.0,
            background_color: Color::from_rgb_u8(30, 30, 35),
            active_tab_color: Color::from_rgb_u8(50, 50, 60),
            inactive_tab_color: Color::from_rgb_u8(40, 40, 45),
            text_color: Color::from_rgb_u8(180, 180, 180),
            active_text_color: Color::WHITE,
            disabled_text_color: Color::from_rgb_u8(100, 100, 100),
            font_size: 14.0,
            hovered_tab: None,
        }
    }

    pub fn add_tab(&mut self, tab: Tab, content: Box<dyn Widget>) {
        self.tabs.push(tab);
        self.children.push(content);
    }

    pub fn with_tab(mut self, tab: Tab, content: Box<dyn Widget>) -> Self {
        self.add_tab(tab, content);
        self
    }

    pub fn tab_height(mut self, height: f32) -> Self {
        self.tab_height = height;
        self
    }

    pub fn background_color(mut self, color: Color) -> Self {
        self.background_color = color;
        self
    }

    pub fn active_tab_color(mut self, color: Color) -> Self {
        self.active_tab_color = color;
        self
    }

    pub fn inactive_tab_color(mut self, color: Color) -> Self {
        self.inactive_tab_color = color;
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

    pub fn active_tab(&self) -> usize {
        self.active_tab
    }

    pub fn set_active_tab(&mut self, index: usize) {
        if index < self.tabs.len() && self.tabs[index].enabled {
            self.active_tab = index;
        }
    }

    pub fn next_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }

        let start = (self.active_tab + 1) % self.tabs.len();
        for i in 0..self.tabs.len() {
            let idx = (start + i) % self.tabs.len();
            if self.tabs[idx].enabled {
                self.active_tab = idx;
                return;
            }
        }
    }

    pub fn previous_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }

        let start = if self.active_tab == 0 {
            self.tabs.len() - 1
        } else {
            self.active_tab - 1
        };

        for i in 0..self.tabs.len() {
            let idx = if start >= i { start - i } else { self.tabs.len() - (i - start) };
            if self.tabs[idx].enabled {
                self.active_tab = idx;
                return;
            }
        }
    }

    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub fn active_content(&self) -> Option<&dyn Widget> {
        self.children.get(self.active_tab).map(|b| &**b as &dyn Widget)
    }

    pub fn active_content_mut(&mut self) -> Option<&mut dyn Widget> {
        self.children.get_mut(self.active_tab).map(|b| &mut **b as &mut dyn Widget)
    }

    pub fn set_hovered_tab(&mut self, index: Option<usize>) {
        self.hovered_tab = index;
    }

    pub fn hovered_tab(&self) -> Option<usize> {
        self.hovered_tab
    }

    pub fn tab_color(&self, index: usize) -> Color {
        if index == self.active_tab {
            self.active_tab_color
        } else if Some(index) == self.hovered_tab {
            // Lighten the inactive color for hover
            let c = self.inactive_tab_color;
            Color::from_rgba_u8(
                (c.r * 255.0 + 20.0).min(255.0) as u8,
                (c.g * 255.0 + 20.0).min(255.0) as u8,
                (c.b * 255.0 + 20.0).min(255.0) as u8,
                (c.a * 255.0) as u8,
            )
        } else {
            self.inactive_tab_color
        }
    }

    pub fn tab_text_color(&self, index: usize) -> Color {
        if index >= self.tabs.len() {
            return self.disabled_text_color;
        }

        if !self.tabs[index].enabled {
            self.disabled_text_color
        } else if index == self.active_tab {
            self.active_text_color
        } else {
            self.text_color
        }
    }

    pub fn enable_tab(&mut self, index: usize, enabled: bool) {
        if let Some(tab) = self.tabs.get_mut(index) {
            tab.enabled = enabled;
            // If disabling the active tab, switch to the first enabled tab
            if !enabled && index == self.active_tab {
                self.set_active_tab(0);
            }
        }
    }
}

impl Widget for TabView {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout_node(&self) -> taffy::NodeId {
        self.node
    }

    fn debug_name(&self) -> &str {
        "TabView"
    }

    fn as_color_widget(&self) -> Option<&dyn ColorWidget> {
        Some(self)
    }

    fn as_color_widget_mut(&mut self) -> Option<&mut dyn ColorWidget> {
        Some(self)
    }

    fn as_container(&self) -> Option<&dyn ParentWidget> {
        Some(self)
    }

    fn as_container_mut(&mut self) -> Option<&mut dyn ParentWidget> {
        Some(self)
    }
}

impl ColorWidget for TabView {
    fn color(&self) -> Color {
        self.background_color
    }

    fn set_color(&mut self, color: Color) {
        self.background_color = color;
        self.style.background_color = Some(color);
    }
}

impl ParentWidget for TabView {
    fn children(&self) -> &[Box<dyn Widget>] {
        &self.children
    }

    fn children_mut(&mut self) -> &mut [Box<dyn Widget>] {
        &mut self.children
    }

    fn add_child(&mut self, child: Box<dyn Widget>) {
        // When adding a child without a tab, create a default tab
        self.tabs.push(Tab::new(format!("Tab {}", self.tabs.len() + 1)));
        self.children.push(child);
    }

    fn remove_child(&mut self, id: WidgetId) -> Option<Box<dyn Widget>> {
        if let Some(index) = self.children.iter().position(|c| c.id() == id) {
            self.tabs.remove(index);
            let child = self.children.remove(index);
            // Adjust active tab if needed
            if self.active_tab >= self.children.len() && !self.children.is_empty() {
                self.active_tab = self.children.len() - 1;
            }
            Some(child)
        } else {
            None
        }
    }
}

// ================================================================================================
// Modal Widget
// ================================================================================================

/// Modal/Dialog widget that overlays content.
pub struct Modal {
    id: WidgetId,
    node: taffy::NodeId,
    style: Style,
    content: Option<Box<dyn Widget>>,
    is_open: bool,
    close_on_outside_click: bool,
    close_on_escape: bool,
    overlay_color: Color,
    background_color: Color,
    border_radius: f32,
    padding: f32,
}

impl Modal {
    pub fn new(id: WidgetId, node: taffy::NodeId) -> Self {
        Self {
            id,
            node,
            style: Style::default(),
            content: None,
            is_open: false,
            close_on_outside_click: true,
            close_on_escape: true,
            overlay_color: Color::from_rgba_u8(0, 0, 0, 180),
            background_color: Color::from_rgb_u8(40, 40, 45),
            border_radius: 8.0,
            padding: 20.0,
        }
    }

    pub fn content(mut self, content: Box<dyn Widget>) -> Self {
        self.content = Some(content);
        self
    }

    pub fn open(mut self, open: bool) -> Self {
        self.is_open = open;
        self
    }

    pub fn close_on_outside_click(mut self, enabled: bool) -> Self {
        self.close_on_outside_click = enabled;
        self
    }

    pub fn close_on_escape(mut self, enabled: bool) -> Self {
        self.close_on_escape = enabled;
        self
    }

    pub fn overlay_color(mut self, color: Color) -> Self {
        self.overlay_color = color;
        self
    }

    pub fn background_color(mut self, color: Color) -> Self {
        self.background_color = color;
        self
    }

    pub fn border_radius(mut self, radius: f32) -> Self {
        self.border_radius = radius;
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    pub fn set_open(&mut self, open: bool) {
        self.is_open = open;
    }

    pub fn show(&mut self) {
        self.is_open = true;
    }

    pub fn hide(&mut self) {
        self.is_open = false;
    }

    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }

    pub fn should_close_on_outside_click(&self) -> bool {
        self.close_on_outside_click
    }

    pub fn should_close_on_escape(&self) -> bool {
        self.close_on_escape
    }

    pub fn get_content(&self) -> Option<&dyn Widget> {
        self.content.as_ref().map(|b| &**b as &dyn Widget)
    }

    pub fn get_content_mut(&mut self) -> Option<&mut dyn Widget> {
        self.content.as_mut().map(|b| &mut **b as &mut dyn Widget)
    }
}

impl Widget for Modal {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn layout_node(&self) -> taffy::NodeId {
        self.node
    }

    fn debug_name(&self) -> &str {
        "Modal"
    }

    fn as_color_widget(&self) -> Option<&dyn ColorWidget> {
        Some(self)
    }

    fn as_color_widget_mut(&mut self) -> Option<&mut dyn ColorWidget> {
        Some(self)
    }

    fn as_container(&self) -> Option<&dyn ParentWidget> {
        Some(self)
    }

    fn as_container_mut(&mut self) -> Option<&mut dyn ParentWidget> {
        Some(self)
    }
}

impl ColorWidget for Modal {
    fn color(&self) -> Color {
        self.background_color
    }

    fn set_color(&mut self, color: Color) {
        self.background_color = color;
        self.style.background_color = Some(color);
    }
}

impl ParentWidget for Modal {
    fn children(&self) -> &[Box<dyn Widget>] {
        if let Some(ref content) = self.content {
            std::slice::from_ref(content)
        } else {
            &[]
        }
    }

    fn children_mut(&mut self) -> &mut [Box<dyn Widget>] {
        if let Some(ref mut content) = self.content {
            std::slice::from_mut(content)
        } else {
            &mut []
        }
    }

    fn add_child(&mut self, child: Box<dyn Widget>) {
        self.content = Some(child);
    }

    fn remove_child(&mut self, id: WidgetId) -> Option<Box<dyn Widget>> {
        if let Some(ref content) = self.content {
            if content.id() == id {
                return self.content.take();
            }
        }
        None
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
