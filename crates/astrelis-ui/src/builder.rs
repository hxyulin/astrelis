//! Declarative builder API for constructing UI trees.

use crate::style::Style;
use crate::tree::{NodeId, UiTree};
use crate::widget_id::{WidgetId, WidgetIdRegistry};
use crate::widgets::{Button, Column, Container, Row, Text, TextInput, Tooltip, Widget};

/// Builder for constructing UI trees declaratively.
pub struct UiBuilder<'a> {
    tree: &'a mut UiTree,
    widget_registry: &'a mut WidgetIdRegistry,
    root: Option<NodeId>,
    is_root_builder: bool,
}

impl<'a> UiBuilder<'a> {
    /// Create a new UI builder.
    pub fn new(tree: &'a mut UiTree, widget_registry: &'a mut WidgetIdRegistry) -> Self {
        tree.clear();
        Self {
            tree,
            widget_registry,
            root: None,
            is_root_builder: true,
        }
    }

    /// Create a container widget.
    pub fn container(&mut self) -> ContainerBuilder<'_, 'a> {
        ContainerBuilder {
            builder: self,
            container: Container::new(),
            children: Vec::new(),
        }
    }

    /// Create a text widget.
    pub fn text(&mut self, content: impl Into<String>) -> TextBuilder<'_, 'a> {
        TextBuilder {
            builder: self,
            text: Text::new(content),
            widget_id: None,
        }
    }

    /// Create a button widget.
    pub fn button(&mut self, label: impl Into<String>) -> ButtonBuilder<'_, 'a> {
        ButtonBuilder {
            builder: self,
            button: Button::new(label),
            widget_id: None,
        }
    }

    /// Create a row layout widget.
    pub fn row(&mut self) -> RowBuilder<'_, 'a> {
        RowBuilder {
            builder: self,
            row: Row::new(),
            children: Vec::new(),
        }
    }

    /// Create a column layout widget.
    pub fn column(&mut self) -> ColumnBuilder<'_, 'a> {
        ColumnBuilder {
            builder: self,
            column: Column::new(),
            children: Vec::new(),
        }
    }

    /// Create a text input widget.
    pub fn text_input(&mut self, placeholder: impl Into<String>) -> TextInputBuilder<'_, 'a> {
        TextInputBuilder {
            builder: self,
            text_input: TextInput::new(placeholder),
            widget_id: None,
        }
    }

    /// Create a tooltip widget.
    pub fn tooltip(&mut self, text: impl Into<String>) -> TooltipBuilder<'_, 'a> {
        TooltipBuilder {
            builder: self,
            tooltip: Tooltip::new(text),
        }
    }

    /// Add a widget to the tree and return its node ID.
    fn add_widget(&mut self, widget: Box<dyn Widget>) -> NodeId {
        self.tree.add_widget(widget)
    }

    /// Set the root widget (only for top-level builder).
    fn set_root(&mut self, node_id: NodeId) {
        if self.is_root_builder && self.root.is_none() {
            self.root = Some(node_id);
            self.tree.set_root(node_id);
        }
    }

    /// Create a child builder that won't set root.
    fn child_builder(&mut self) -> UiBuilder<'_> {
        UiBuilder {
            tree: self.tree,
            widget_registry: self.widget_registry,
            root: None,
            is_root_builder: false,
        }
    }

    /// Finish building and set root if not already set.
    pub fn finish(self) {
        // Root is set automatically by first top-level widget
    }
}

/// Base trait for widget builders.
pub trait WidgetBuilder {
    /// Apply a style to the widget.
    fn style(self, style: Style) -> Self;

    /// Set width.
    fn width(self, width: f32) -> Self;

    /// Set height.
    fn height(self, height: f32) -> Self;

    /// Set padding.
    fn padding(self, padding: f32) -> Self;

    /// Set margin.
    fn margin(self, margin: f32) -> Self;
}

/// Builder for container widgets.
pub struct ContainerBuilder<'b, 'a> {
    builder: &'b mut UiBuilder<'a>,
    container: Container,
    children: Vec<NodeId>,
}

impl<'b, 'a> ContainerBuilder<'b, 'a> {
    /// Add a child widget.
    pub fn child<F>(mut self, build_child: F) -> Self
    where
        F: FnOnce(&mut UiBuilder) -> NodeId,
    {
        let mut child_builder = self.builder.child_builder();
        let child_id = build_child(&mut child_builder);
        self.children.push(child_id);
        self
    }

    /// Add multiple children.
    pub fn children<F>(mut self, build_children: F) -> Self
    where
        F: FnOnce(&mut UiBuilder) -> Vec<NodeId>,
    {
        let mut child_builder = self.builder.child_builder();
        let mut child_ids = build_children(&mut child_builder);
        self.children.append(&mut child_ids);
        self
    }

    /// Build the container and add it to the tree.
    pub fn build(mut self) -> NodeId {
        self.container.children = self.children.clone();
        let node_id = self.builder.add_widget(Box::new(self.container));

        // Set children in tree
        self.builder.tree.set_children(node_id, &self.children);

        // Set as root if this is the first widget
        self.builder.set_root(node_id);

        node_id
    }

    /// Set background color.
    pub fn background_color(mut self, color: astrelis_render::Color) -> Self {
        self.container.style = self.container.style.background_color(color);
        self
    }

    /// Set border color.
    pub fn border_color(mut self, color: astrelis_render::Color) -> Self {
        self.container.style = self.container.style.border_color(color);
        self
    }

    /// Set border width.
    pub fn border_width(mut self, width: f32) -> Self {
        self.container.style = self.container.style.border_width(width);
        self
    }

    /// Set border radius.
    pub fn border_radius(mut self, radius: f32) -> Self {
        self.container.style = self.container.style.border_radius(radius);
        self
    }

    /// Set flex direction.
    pub fn flex_direction(mut self, direction: taffy::FlexDirection) -> Self {
        self.container.style = self.container.style.flex_direction(direction);
        self
    }

    /// Set justify content.
    pub fn justify_content(mut self, justify: taffy::JustifyContent) -> Self {
        self.container.style = self.container.style.justify_content(justify);
        self
    }

    /// Set align items.
    pub fn align_items(mut self, align: taffy::AlignItems) -> Self {
        self.container.style = self.container.style.align_items(align);
        self
    }

    /// Set gap between items.
    pub fn gap(mut self, gap: f32) -> Self {
        self.container.style = self.container.style.gap(gap);
        self
    }

    /// Set flex wrap.
    pub fn flex_wrap(mut self, wrap: taffy::FlexWrap) -> Self {
        self.container.style = self.container.style.flex_wrap(wrap);
        self
    }

    // WidgetBuilder methods inlined
    pub fn style(mut self, style: Style) -> Self {
        self.container.style = style;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.container.style = self.container.style.width(width);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.container.style = self.container.style.height(height);
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.container.style = self.container.style.padding(padding);
        self
    }

    pub fn margin(mut self, margin: f32) -> Self {
        self.container.style = self.container.style.margin(margin);
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.container.style = self.container.style.min_width(width);
        self
    }

    pub fn min_height(mut self, height: f32) -> Self {
        self.container.style = self.container.style.min_height(height);
        self
    }

    pub fn max_width(mut self, width: f32) -> Self {
        self.container.style = self.container.style.max_width(width);
        self
    }

    pub fn max_height(mut self, height: f32) -> Self {
        self.container.style = self.container.style.max_height(height);
        self
    }
}

/// Builder for text widgets.
pub struct TextBuilder<'b, 'a> {
    builder: &'b mut UiBuilder<'a>,
    text: Text,
    widget_id: Option<WidgetId>,
}

impl<'b, 'a> TextBuilder<'b, 'a> {
    /// Set widget ID for later reference.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.widget_id = Some(id);
        self
    }

    /// Set font size.
    pub fn size(mut self, size: f32) -> Self {
        self.text = self.text.size(size);
        self
    }

    /// Set text color.
    pub fn color(mut self, color: astrelis_render::Color) -> Self {
        self.text = self.text.color(color);
        self
    }

    /// Set font weight.
    pub fn weight(mut self, weight: astrelis_text::FontWeight) -> Self {
        self.text = self.text.weight(weight);
        self
    }

    /// Make text bold.
    pub fn bold(self) -> Self {
        self.weight(astrelis_text::FontWeight::Bold)
    }

    /// Set text alignment.
    pub fn align(mut self, align: astrelis_text::TextAlign) -> Self {
        self.text = self.text.align(align);
        self
    }

    /// Build the text widget and add it to the tree.
    pub fn build(self) -> NodeId {
        let node_id = self.builder.add_widget(Box::new(self.text));
        if let Some(widget_id) = self.widget_id {
            self.builder.widget_registry.register(widget_id, node_id);
        }
        self.builder.set_root(node_id);
        node_id
    }
}

impl<'b, 'a> TextBuilder<'b, 'a> {
    // WidgetBuilder methods inlined
    pub fn style(mut self, style: Style) -> Self {
        self.text.style = style;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.text.style = self.text.style.width(width);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.text.style = self.text.style.height(height);
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.text.style = self.text.style.padding(padding);
        self
    }

    pub fn margin(mut self, margin: f32) -> Self {
        self.text.style = self.text.style.margin(margin);
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.text.style = self.text.style.min_width(width);
        self
    }

    pub fn min_height(mut self, height: f32) -> Self {
        self.text.style = self.text.style.min_height(height);
        self
    }

    pub fn max_width(mut self, width: f32) -> Self {
        self.text.style = self.text.style.max_width(width);
        self
    }

    pub fn max_height(mut self, height: f32) -> Self {
        self.text.style = self.text.style.max_height(height);
        self
    }
}

/// Builder for button widgets.
pub struct ButtonBuilder<'b, 'a> {
    builder: &'b mut UiBuilder<'a>,
    button: Button,
    widget_id: Option<WidgetId>,
}

impl<'b, 'a> ButtonBuilder<'b, 'a> {
    /// Set button background color.
    pub fn background_color(mut self, color: astrelis_render::Color) -> Self {
        self.button = self.button.background_color(color);
        self
    }

    /// Set hover color.
    pub fn hover_color(mut self, color: astrelis_render::Color) -> Self {
        self.button = self.button.hover_color(color);
        self
    }

    /// Set text color.
    pub fn text_color(mut self, color: astrelis_render::Color) -> Self {
        self.button = self.button.text_color(color);
        self
    }

    /// Set font size.
    pub fn font_size(mut self, size: f32) -> Self {
        self.button = self.button.font_size(size);
        self
    }

    /// Set widget ID for later reference.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.widget_id = Some(id);
        self
    }

    /// Set click callback.
    pub fn on_click<F>(mut self, callback: F) -> Self
    where
        F: Fn() + 'static,
    {
        self.button = self.button.on_click(callback);
        self
    }

    /// Build the button widget and add it to the tree.
    pub fn build(self) -> NodeId {
        let node_id = self.builder.add_widget(Box::new(self.button));
        if let Some(widget_id) = self.widget_id {
            self.builder.widget_registry.register(widget_id, node_id);
        }
        self.builder.set_root(node_id);
        node_id
    }
}

impl<'b, 'a> ButtonBuilder<'b, 'a> {
    // WidgetBuilder methods inlined
    pub fn style(mut self, style: Style) -> Self {
        self.button.style = style;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.button.style = self.button.style.width(width);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.button.style = self.button.style.height(height);
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.button.style = self.button.style.padding(padding);
        self
    }

    pub fn margin(mut self, margin: f32) -> Self {
        self.button.style = self.button.style.margin(margin);
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.button.style = self.button.style.min_width(width);
        self
    }

    pub fn min_height(mut self, height: f32) -> Self {
        self.button.style = self.button.style.min_height(height);
        self
    }

    pub fn max_width(mut self, width: f32) -> Self {
        self.button.style = self.button.style.max_width(width);
        self
    }

    pub fn max_height(mut self, height: f32) -> Self {
        self.button.style = self.button.style.max_height(height);
        self
    }
}

/// Builder for row layout widgets.
pub struct RowBuilder<'b, 'a> {
    builder: &'b mut UiBuilder<'a>,
    row: Row,
    children: Vec<NodeId>,
}

impl<'b, 'a> RowBuilder<'b, 'a> {
    /// Add a child widget.
    pub fn child<F>(mut self, build_child: F) -> Self
    where
        F: FnOnce(&mut UiBuilder) -> NodeId,
    {
        let mut child_builder = self.builder.child_builder();
        let child_id = build_child(&mut child_builder);
        self.children.push(child_id);
        self
    }

    /// Set gap between items.
    pub fn gap(mut self, gap: f32) -> Self {
        self.row = self.row.gap(gap);
        self
    }

    /// Set justify content.
    pub fn justify_content(mut self, justify: taffy::JustifyContent) -> Self {
        self.row.style = self.row.style.justify_content(justify);
        self
    }

    /// Set align items.
    pub fn align_items(mut self, align: taffy::AlignItems) -> Self {
        self.row.style = self.row.style.align_items(align);
        self
    }

    /// Build the row and add it to the tree.
    pub fn build(mut self) -> NodeId {
        self.row.children = self.children.clone();
        let node_id = self.builder.add_widget(Box::new(self.row));

        // Set children in tree
        self.builder.tree.set_children(node_id, &self.children);

        // Set as root if first widget
        self.builder.set_root(node_id);

        node_id
    }
}

impl<'b, 'a> RowBuilder<'b, 'a> {
    // WidgetBuilder methods inlined
    pub fn style(mut self, style: Style) -> Self {
        self.row.style = style;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.row.style = self.row.style.width(width);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.row.style = self.row.style.height(height);
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.row.style = self.row.style.padding(padding);
        self
    }

    pub fn margin(mut self, margin: f32) -> Self {
        self.row.style = self.row.style.margin(margin);
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.row.style = self.row.style.min_width(width);
        self
    }

    pub fn min_height(mut self, height: f32) -> Self {
        self.row.style = self.row.style.min_height(height);
        self
    }

    pub fn max_width(mut self, width: f32) -> Self {
        self.row.style = self.row.style.max_width(width);
        self
    }

    pub fn max_height(mut self, height: f32) -> Self {
        self.row.style = self.row.style.max_height(height);
        self
    }
}

/// Builder for column layout widgets.
pub struct ColumnBuilder<'b, 'a> {
    builder: &'b mut UiBuilder<'a>,
    column: Column,
    children: Vec<NodeId>,
}

impl<'b, 'a> ColumnBuilder<'b, 'a> {
    /// Add a child widget.
    pub fn child<F>(mut self, build_child: F) -> Self
    where
        F: FnOnce(&mut UiBuilder) -> NodeId,
    {
        let mut child_builder = self.builder.child_builder();
        let child_id = build_child(&mut child_builder);
        self.children.push(child_id);
        self
    }

    /// Set gap between items.
    pub fn gap(mut self, gap: f32) -> Self {
        self.column = self.column.gap(gap);
        self
    }

    /// Set justify content.
    pub fn justify_content(mut self, justify: taffy::JustifyContent) -> Self {
        self.column.style = self.column.style.justify_content(justify);
        self
    }

    /// Set align items.
    pub fn align_items(mut self, align: taffy::AlignItems) -> Self {
        self.column.style = self.column.style.align_items(align);
        self
    }

    /// Build the column and add it to the tree.
    pub fn build(mut self) -> NodeId {
        self.column.children = self.children.clone();
        let node_id = self.builder.add_widget(Box::new(self.column));

        // Set children in tree
        self.builder.tree.set_children(node_id, &self.children);

        // Set as root if first widget
        self.builder.set_root(node_id);

        node_id
    }
}

impl<'b, 'a> ColumnBuilder<'b, 'a> {
    // WidgetBuilder methods inlined
    pub fn style(mut self, style: Style) -> Self {
        self.column.style = style;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.column.style = self.column.style.width(width);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.column.style = self.column.style.height(height);
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.column.style = self.column.style.padding(padding);
        self
    }

    pub fn margin(mut self, margin: f32) -> Self {
        self.column.style = self.column.style.margin(margin);
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.column.style = self.column.style.min_width(width);
        self
    }

    pub fn min_height(mut self, height: f32) -> Self {
        self.column.style = self.column.style.min_height(height);
        self
    }

    pub fn max_width(mut self, width: f32) -> Self {
        self.column.style = self.column.style.max_width(width);
        self
    }

    pub fn max_height(mut self, height: f32) -> Self {
        self.column.style = self.column.style.max_height(height);
        self
    }
}

/// Builder for text input widgets.
pub struct TextInputBuilder<'b, 'a> {
    builder: &'b mut UiBuilder<'a>,
    text_input: TextInput,
    widget_id: Option<WidgetId>,
}

impl<'b, 'a> TextInputBuilder<'b, 'a> {
    /// Set widget ID for later reference.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.widget_id = Some(id);
        self
    }

    /// Set initial content.
    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.text_input = self.text_input.content(content);
        self
    }

    /// Set font size.
    pub fn font_size(mut self, size: f32) -> Self {
        self.text_input = self.text_input.font_size(size);
        self
    }

    /// Set text color.
    pub fn text_color(mut self, color: astrelis_render::Color) -> Self {
        self.text_input = self.text_input.text_color(color);
        self
    }

    /// Set placeholder color.
    pub fn placeholder_color(mut self, color: astrelis_render::Color) -> Self {
        self.text_input = self.text_input.placeholder_color(color);
        self
    }

    /// Set max length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.text_input = self.text_input.max_length(max);
        self
    }

    /// Set on change callback.
    pub fn on_change<F>(mut self, callback: F) -> Self
    where
        F: Fn(String) + 'static,
    {
        self.text_input = self.text_input.on_change(callback);
        self
    }

    /// Build the text input widget and add it to the tree.
    pub fn build(self) -> NodeId {
        let node_id = self.builder.add_widget(Box::new(self.text_input));
        if let Some(widget_id) = self.widget_id {
            self.builder.widget_registry.register(widget_id, node_id);
        }
        self.builder.set_root(node_id);
        node_id
    }

    // WidgetBuilder methods
    pub fn style(mut self, style: Style) -> Self {
        self.text_input.style = style;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.text_input.style = self.text_input.style.width(width);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.text_input.style = self.text_input.style.height(height);
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.text_input.style = self.text_input.style.padding(padding);
        self
    }

    pub fn margin(mut self, margin: f32) -> Self {
        self.text_input.style = self.text_input.style.margin(margin);
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.text_input.style = self.text_input.style.min_width(width);
        self
    }

    pub fn min_height(mut self, height: f32) -> Self {
        self.text_input.style = self.text_input.style.min_height(height);
        self
    }
}

/// Builder for tooltip widgets.
pub struct TooltipBuilder<'b, 'a> {
    builder: &'b mut UiBuilder<'a>,
    tooltip: Tooltip,
}

impl<'b, 'a> TooltipBuilder<'b, 'a> {
    /// Set font size.
    pub fn font_size(mut self, size: f32) -> Self {
        self.tooltip = self.tooltip.font_size(size);
        self
    }

    /// Set text color.
    pub fn text_color(mut self, color: astrelis_render::Color) -> Self {
        self.tooltip = self.tooltip.text_color(color);
        self
    }

    /// Set background color.
    pub fn background_color(mut self, color: astrelis_render::Color) -> Self {
        self.tooltip = self.tooltip.background_color(color);
        self
    }

    /// Build the tooltip and add it to the tree.
    pub fn build(self) -> NodeId {
        let node_id = self.builder.add_widget(Box::new(self.tooltip));
        self.builder.set_root(node_id);
        node_id
    }

    // WidgetBuilder methods
    pub fn style(mut self, style: Style) -> Self {
        self.tooltip.style = style;
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.tooltip.style = self.tooltip.style.padding(padding);
        self
    }

    pub fn margin(mut self, margin: f32) -> Self {
        self.tooltip.style = self.tooltip.style.margin(margin);
        self
    }
}
