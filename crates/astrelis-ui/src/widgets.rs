//! Widget system for UI components.

use crate::style::Style;
use crate::tree::NodeId;
use astrelis_core::math::Vec2;
use astrelis_render::Color;
use astrelis_text::{FontRenderer, FontWeight, Text as TextStyle, TextAlign, VerticalAlign};
use std::any::Any;
use std::rc::Rc;
use std::sync::Arc;

/// Base trait for all UI widgets.
pub trait Widget: Any {
    /// Get widget type as Any for downcasting.
    fn as_any(&self) -> &dyn Any;

    /// Get mutable widget type as Any for downcasting.
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Get the style for this widget.
    fn style(&self) -> &Style;

    /// Get mutable style for this widget.
    fn style_mut(&mut self) -> &mut Style;

    /// Get child widgets.
    fn children(&self) -> &[NodeId] {
        &[]
    }

    /// Get mutable child widgets.
    fn children_mut(&mut self) -> &mut Vec<NodeId> {
        // Default implementation returns empty slice
        // Widgets with children should override this
        panic!("children_mut() called on widget without children support")
    }

    /// Measure content size for layout (for intrinsic sizing).
    /// Returns (width, height) in pixels.
    fn measure(&self, _available_space: Vec2, _font_renderer: Option<&FontRenderer>) -> Vec2 {
        Vec2::ZERO
    }

    /// Clone the widget into a box.
    fn clone_box(&self) -> Box<dyn Widget>;
}

impl Clone for Box<dyn Widget> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// Handle to a texture for use in Image widgets.
///
/// This is an Arc-wrapped texture view that can be shared across widgets.
pub type ImageTexture = Arc<astrelis_render::wgpu::TextureView>;

/// UV coordinates for sprite/image regions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageUV {
    pub u_min: f32,
    pub v_min: f32,
    pub u_max: f32,
    pub v_max: f32,
}

impl Default for ImageUV {
    fn default() -> Self {
        Self {
            u_min: 0.0,
            v_min: 0.0,
            u_max: 1.0,
            v_max: 1.0,
        }
    }
}

impl ImageUV {
    /// Create UV coordinates for a full texture.
    pub fn full() -> Self {
        Self::default()
    }

    /// Create UV coordinates from sprite sheet coordinates.
    pub fn from_sprite(
        sprite_x: u32,
        sprite_y: u32,
        sprite_width: u32,
        sprite_height: u32,
        texture_width: u32,
        texture_height: u32,
    ) -> Self {
        Self {
            u_min: sprite_x as f32 / texture_width as f32,
            v_min: sprite_y as f32 / texture_height as f32,
            u_max: (sprite_x + sprite_width) as f32 / texture_width as f32,
            v_max: (sprite_y + sprite_height) as f32 / texture_height as f32,
        }
    }

    /// Create UV coordinates from normalized values.
    pub fn new(u_min: f32, v_min: f32, u_max: f32, v_max: f32) -> Self {
        Self { u_min, v_min, u_max, v_max }
    }

    /// Flip horizontally.
    pub fn flip_h(self) -> Self {
        Self {
            u_min: self.u_max,
            u_max: self.u_min,
            ..self
        }
    }

    /// Flip vertically.
    pub fn flip_v(self) -> Self {
        Self {
            v_min: self.v_max,
            v_max: self.v_min,
            ..self
        }
    }
}

/// Image fit mode - how to fit the image within the widget bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageFit {
    /// Fill the entire bounds, may distort the image.
    Fill,
    /// Scale to fit within bounds, preserving aspect ratio (letterbox).
    #[default]
    Contain,
    /// Scale to cover bounds, preserving aspect ratio (may crop).
    Cover,
    /// Don't scale the image, render at native size.
    None,
}

/// Image widget - displays a texture or sprite.
#[derive(Clone)]
pub struct Image {
    /// The texture to display
    pub texture: Option<ImageTexture>,
    /// UV coordinates (for sprites)
    pub uv: ImageUV,
    /// Tint color (multiplied with texture)
    pub tint: Color,
    /// How to fit the image within bounds
    pub fit: ImageFit,
    /// Natural width of the image (for sizing)
    pub natural_width: f32,
    /// Natural height of the image (for sizing)
    pub natural_height: f32,
    /// Border radius for rounded corners
    pub border_radius: f32,
    /// Style
    pub style: Style,
}

impl Image {
    /// Create a new image widget.
    pub fn new() -> Self {
        Self {
            texture: None,
            uv: ImageUV::default(),
            tint: Color::WHITE,
            fit: ImageFit::default(),
            natural_width: 0.0,
            natural_height: 0.0,
            border_radius: 0.0,
            style: Style::new(),
        }
    }

    /// Create an image widget with a texture.
    pub fn with_texture(texture: ImageTexture, width: f32, height: f32) -> Self {
        Self {
            texture: Some(texture),
            uv: ImageUV::default(),
            tint: Color::WHITE,
            fit: ImageFit::default(),
            natural_width: width,
            natural_height: height,
            border_radius: 0.0,
            style: Style::new().width(width).height(height),
        }
    }

    /// Set the texture.
    pub fn texture(mut self, texture: ImageTexture) -> Self {
        self.texture = Some(texture);
        self
    }

    /// Set UV coordinates (for sprites).
    pub fn uv(mut self, uv: ImageUV) -> Self {
        self.uv = uv;
        self
    }

    /// Set the tint color.
    pub fn tint(mut self, color: Color) -> Self {
        self.tint = color;
        self
    }

    /// Set the image fit mode.
    pub fn fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }

    /// Set the natural size (for sizing calculations).
    pub fn natural_size(mut self, width: f32, height: f32) -> Self {
        self.natural_width = width;
        self.natural_height = height;
        self
    }

    /// Set border radius for rounded corners.
    pub fn border_radius(mut self, radius: f32) -> Self {
        self.border_radius = radius;
        self
    }

    /// Update the texture at runtime.
    pub fn set_texture(&mut self, texture: ImageTexture) {
        self.texture = Some(texture);
    }

    /// Update UV coordinates at runtime.
    pub fn set_uv(&mut self, uv: ImageUV) {
        self.uv = uv;
    }

    /// Update tint color at runtime.
    pub fn set_tint(&mut self, color: Color) {
        self.tint = color;
    }
}

impl Default for Image {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for Image {
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
        Vec2::new(self.natural_width, self.natural_height)
    }

    fn clone_box(&self) -> Box<dyn Widget> {
        Box::new(self.clone())
    }
}

/// Container widget - holds other widgets with flexbox layout.
#[derive(Clone)]
pub struct Container {
    pub style: Style,
    pub children: Vec<NodeId>,
}

impl Container {
    pub fn new() -> Self {
        Self {
            style: Style::new().display(taffy::Display::Flex),
            children: Vec::new(),
        }
    }

    pub fn with_style(style: Style) -> Self {
        Self {
            style,
            children: Vec::new(),
        }
    }
}

impl Default for Container {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for Container {
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

    fn children_mut(&mut self) -> &mut Vec<NodeId> {
        &mut self.children
    }

    fn clone_box(&self) -> Box<dyn Widget> {
        Box::new(self.clone())
    }
}

/// Text widget - displays text.
#[derive(Clone)]
pub struct Text {
    pub content: String,
    pub font_size: f32,
    pub color: Color,
    pub weight: FontWeight,
    pub align: TextAlign,
    pub vertical_align: VerticalAlign,
    pub style: Style,
}

impl Text {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
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
    pub fn build_text_style(&self) -> TextStyle {
        let mut text = TextStyle::new(&self.content)
            .size(self.font_size)
            .color(self.color)
            .weight(self.weight)
            .align(self.align)
            .vertical_align(self.vertical_align);

        // Apply max width from style if set
        if let taffy::Dimension::Length(width) = self.style.layout.size.width {
            text = text.max_width(width);
        }

        text
    }

    /// Set the text content (for incremental updates).
    /// Returns true if the content changed.
    pub fn set_content(&mut self, content: impl Into<String>) -> bool {
        let new_content = content.into();
        if self.content != new_content {
            self.content = new_content;
            true
        } else {
            false
        }
    }

    /// Get the current text content.
    pub fn get_content(&self) -> &str {
        &self.content
    }

    /// Set the font size (for incremental updates).
    pub fn set_font_size(&mut self, size: f32) {
        self.font_size = size;
    }

    /// Set the text color (for incremental updates).
    pub fn set_color(&mut self, color: Color) {
        self.color = color;
    }
}

impl Widget for Text {
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

    fn measure(&self, _available_space: Vec2, font_renderer: Option<&FontRenderer>) -> Vec2 {
        // Use actual font renderer if available for accurate measurement
        if let Some(renderer) = font_renderer {
            let text_style = self.build_text_style();
            let (width, height) = renderer.measure_text(&text_style);
            return Vec2::new(width, height);
        }

        // Fallback: rough estimate based on font size
        let char_count = self.content.chars().count() as f32;
        let estimated_width = char_count * self.font_size * 0.6;
        let estimated_height = self.font_size * 1.2;
        Vec2::new(estimated_width, estimated_height)
    }

    fn clone_box(&self) -> Box<dyn Widget> {
        Box::new(self.clone())
    }
}

/// Callback type for button clicks.
pub type ButtonCallback = Rc<dyn Fn()>;

/// Button widget - clickable with label.
#[derive(Clone)]
pub struct Button {
    pub label: String,
    pub style: Style,
    pub hover_color: Color,
    pub active_color: Color,
    pub text_color: Color,
    pub font_size: f32,
    pub is_hovered: bool,
    pub is_pressed: bool,
    pub on_click: Option<ButtonCallback>,
}

impl Button {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            style: Style::new()
                .display(taffy::Display::Flex)
                .padding(10.0)
                .background_color(Color::from_rgb_u8(60, 60, 80))
                .border_radius(4.0),
            hover_color: Color::from_rgb_u8(80, 80, 100),
            active_color: Color::from_rgb_u8(40, 40, 60),
            text_color: Color::WHITE,
            font_size: 16.0,
            is_hovered: false,
            is_pressed: false,
            on_click: None,
        }
    }

    pub fn on_click<F>(mut self, callback: F) -> Self
    where
        F: Fn() + 'static,
    {
        self.on_click = Some(Rc::new(callback));
        self
    }

    pub fn background_color(mut self, color: Color) -> Self {
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
            self.style
                .background_color
                .unwrap_or(Color::from_rgb_u8(60, 60, 80))
        }
    }

    /// Set the button label (for incremental updates).
    /// Returns true if the label changed.
    pub fn set_label(&mut self, label: impl Into<String>) -> bool {
        let new_label = label.into();
        if self.label != new_label {
            self.label = new_label;
            true
        } else {
            false
        }
    }

    /// Get the current label.
    pub fn get_label(&self) -> &str {
        &self.label
    }

    /// Set the button hover color (for incremental updates).
    pub fn set_hover_color(&mut self, color: Color) {
        self.hover_color = color;
    }

    /// Set the button text color (for incremental updates).
    pub fn set_text_color(&mut self, color: Color) {
        self.text_color = color;
    }
}

impl Widget for Button {
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

    fn measure(&self, _available_space: Vec2, font_renderer: Option<&FontRenderer>) -> Vec2 {
        // Measure button label text
        if let Some(renderer) = font_renderer {
            let text_style = TextStyle::new(&self.label)
                .size(self.font_size)
                .color(self.text_color);
            let (text_width, text_height) = renderer.measure_text(&text_style);

            // Add padding from style
            let padding_x = match self.style.layout.padding.left {
                taffy::LengthPercentage::Length(l) => l,
                _ => 0.0,
            } + match self.style.layout.padding.right {
                taffy::LengthPercentage::Length(r) => r,
                _ => 0.0,
            };
            let padding_y = match self.style.layout.padding.top {
                taffy::LengthPercentage::Length(t) => t,
                _ => 0.0,
            } + match self.style.layout.padding.bottom {
                taffy::LengthPercentage::Length(b) => b,
                _ => 0.0,
            };

            return Vec2::new(text_width + padding_x, text_height + padding_y);
        }

        // Fallback: estimate
        let char_count = self.label.chars().count() as f32;
        let estimated_width = char_count * self.font_size * 0.6 + 20.0;
        let estimated_height = self.font_size * 1.2 + 20.0;
        Vec2::new(estimated_width, estimated_height)
    }

    fn clone_box(&self) -> Box<dyn Widget> {
        Box::new(self.clone())
    }
}

/// Row widget - horizontal layout.
#[derive(Clone)]
pub struct Row {
    pub style: Style,
    pub children: Vec<NodeId>,
}

impl Row {
    pub fn new() -> Self {
        Self {
            style: Style::new()
                .display(taffy::Display::Flex)
                .flex_direction(taffy::FlexDirection::Row),
            children: Vec::new(),
        }
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.style = self.style.gap(gap);
        self
    }
}

impl Default for Row {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for Row {
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

    fn children_mut(&mut self) -> &mut Vec<NodeId> {
        &mut self.children
    }

    fn clone_box(&self) -> Box<dyn Widget> {
        Box::new(self.clone())
    }
}

/// Text input widget - editable text field.
#[derive(Clone)]
pub struct TextInput {
    pub content: String,
    pub placeholder: String,
    pub font_size: f32,
    pub text_color: Color,
    pub placeholder_color: Color,
    pub style: Style,
    pub is_focused: bool,
    pub cursor_position: usize,
    pub max_length: Option<usize>,
    pub on_change: Option<Rc<dyn Fn(String)>>,
}

impl TextInput {
    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            content: String::new(),
            placeholder: placeholder.into(),
            font_size: 16.0,
            text_color: Color::WHITE,
            placeholder_color: Color::from_rgb_u8(120, 120, 120),
            style: Style::new()
                .display(taffy::Display::Flex)
                .padding(10.0)
                .background_color(Color::from_rgb_u8(40, 40, 50))
                .border_color(Color::from_rgb_u8(80, 80, 100))
                .border_width(1.0)
                .border_radius(4.0),
            is_focused: false,
            cursor_position: 0,
            max_length: None,
            on_change: None,
        }
    }

    pub fn content(mut self, content: impl Into<String>) -> Self {
        let content_str = content.into();
        self.cursor_position = content_str.len();
        self.content = content_str;
        self
    }

    /// Set the text input value (for incremental updates).
    /// Returns true if the value changed.
    pub fn set_value(&mut self, value: impl Into<String>) -> bool {
        let value_str = value.into();
        self.cursor_position = value_str.len();
        if self.content != value_str {
            self.content = value_str;
            true
        } else {
            false
        }
    }

    /// Get the current value.
    pub fn get_value(&self) -> &str {
        &self.content
    }

    /// Set the placeholder text (for incremental updates).
    pub fn set_placeholder(&mut self, placeholder: impl Into<String>) {
        self.placeholder = placeholder.into();
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn text_color(mut self, color: Color) -> Self {
        self.text_color = color;
        self
    }

    pub fn placeholder_color(mut self, color: Color) -> Self {
        self.placeholder_color = color;
        self
    }

    pub fn max_length(mut self, max: usize) -> Self {
        self.max_length = Some(max);
        self
    }

    pub fn on_change<F>(mut self, callback: F) -> Self
    where
        F: Fn(String) + 'static,
    {
        self.on_change = Some(Rc::new(callback));
        self
    }

    pub fn insert_char(&mut self, c: char) {
        if let Some(max) = self.max_length
            && self.content.len() >= max {
                return;
            }
        self.content.insert(self.cursor_position, c);
        self.cursor_position += 1;
        if let Some(ref callback) = self.on_change {
            callback(self.content.clone());
        }
    }

    pub fn delete_char(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
            self.content.remove(self.cursor_position);
            if let Some(ref callback) = self.on_change {
                callback(self.content.clone());
            }
        }
    }

    pub fn display_text(&self) -> &str {
        if self.content.is_empty() {
            &self.placeholder
        } else {
            &self.content
        }
    }

    pub fn display_color(&self) -> Color {
        if self.content.is_empty() {
            self.placeholder_color
        } else {
            self.text_color
        }
    }
}

impl Widget for TextInput {
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

    fn measure(&self, _available_space: Vec2, font_renderer: Option<&FontRenderer>) -> Vec2 {
        if let Some(renderer) = font_renderer {
            let text = if self.content.is_empty() {
                &self.placeholder
            } else {
                &self.content
            };
            let text_style = TextStyle::new(text)
                .size(self.font_size)
                .color(self.display_color());
            let (text_width, text_height) = renderer.measure_text(&text_style);

            let padding_x = match self.style.layout.padding.left {
                taffy::LengthPercentage::Length(l) => l,
                _ => 0.0,
            } + match self.style.layout.padding.right {
                taffy::LengthPercentage::Length(r) => r,
                _ => 0.0,
            };
            let padding_y = match self.style.layout.padding.top {
                taffy::LengthPercentage::Length(t) => t,
                _ => 0.0,
            } + match self.style.layout.padding.bottom {
                taffy::LengthPercentage::Length(b) => b,
                _ => 0.0,
            };

            return Vec2::new(text_width + padding_x + 20.0, text_height + padding_y);
        }

        Vec2::new(200.0, 40.0)
    }

    fn clone_box(&self) -> Box<dyn Widget> {
        Box::new(self.clone())
    }
}

/// Tooltip widget - shows on hover.
#[derive(Clone)]
pub struct Tooltip {
    pub text: String,
    pub style: Style,
    pub font_size: f32,
    pub text_color: Color,
    pub visible: bool,
}

impl Tooltip {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: Style::new()
                .display(taffy::Display::Flex)
                .padding(8.0)
                .background_color(Color::from_rgba_u8(30, 30, 40, 230))
                .border_color(Color::from_rgb_u8(100, 100, 120))
                .border_width(1.0)
                .border_radius(4.0)
                .position(taffy::Position::Absolute),
            font_size: 12.0,
            text_color: Color::WHITE,
            visible: false,
        }
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn text_color(mut self, color: Color) -> Self {
        self.text_color = color;
        self
    }

    pub fn background_color(mut self, color: Color) -> Self {
        self.style = self.style.background_color(color);
        self
    }
}

impl Widget for Tooltip {
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

    fn measure(&self, _available_space: Vec2, font_renderer: Option<&FontRenderer>) -> Vec2 {
        if let Some(renderer) = font_renderer {
            let text_style = TextStyle::new(&self.text)
                .size(self.font_size)
                .color(self.text_color);
            let (text_width, text_height) = renderer.measure_text(&text_style);

            let padding_x = match self.style.layout.padding.left {
                taffy::LengthPercentage::Length(l) => l,
                _ => 0.0,
            } + match self.style.layout.padding.right {
                taffy::LengthPercentage::Length(r) => r,
                _ => 0.0,
            };
            let padding_y = match self.style.layout.padding.top {
                taffy::LengthPercentage::Length(t) => t,
                _ => 0.0,
            } + match self.style.layout.padding.bottom {
                taffy::LengthPercentage::Length(b) => b,
                _ => 0.0,
            };

            return Vec2::new(text_width + padding_x, text_height + padding_y);
        }

        Vec2::new(100.0, 30.0)
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

    pub fn gap(mut self, gap: f32) -> Self {
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

    fn children_mut(&mut self) -> &mut Vec<NodeId> {
        &mut self.children
    }

    fn clone_box(&self) -> Box<dyn Widget> {
        Box::new(self.clone())
    }
}
