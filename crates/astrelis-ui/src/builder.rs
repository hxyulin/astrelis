//! Declarative builder API for constructing UI trees.
//!
//! # Generic `add()` API
//!
//! The builder provides a single generic entry point for adding widgets:
//!
//! ```ignore
//! root.add(Container::new()).width(100.0).child(|c| { ... }).build()
//! root.add(Text::new("Hello")).size(24.0).build()
//! ```
//!
//! Each widget implements [`IntoNodeBuilder`] to return a widget-specific builder
//! type with appropriate methods. Convenience methods like `text()`, `container()`,
//! and `button()` are provided for common widgets.

use crate::style::Style;
use crate::tree::{NodeId, UiTree};
use crate::widget_id::{WidgetId, WidgetIdRegistry};
#[cfg(feature = "docking")]
use crate::widgets::ScrollbarTheme;
#[cfg(feature = "docking")]
use crate::widgets::docking::{
    DockSplitter, DockTabs, PanelConstraints, TabScrollIndicator, TabScrollbarPosition,
};
use crate::widgets::scroll_container::{ScrollAxis, ScrollContainer, ScrollbarVisibility};
use crate::widgets::{
    Button, Column, Container, Image, ImageFit, ImageTexture, ImageUV, Row, Text, TextInput,
    Tooltip, Widget,
};
use astrelis_render::Color;

// ── Style macros (must be defined before use) ───────────────────────────────

/// Generate common style methods for node builders.
///
/// These methods use `Widget::style_mut()` through the `set_*` in-place setters on Style.
macro_rules! impl_node_style_methods {
    () => {
        /// Set the width of the widget.
        pub fn width(mut self, width: impl Into<crate::constraint::Constraint>) -> Self {
            self.widget.style_mut().set_width(width);
            self
        }

        /// Set the height of the widget.
        pub fn height(mut self, height: impl Into<crate::constraint::Constraint>) -> Self {
            self.widget.style_mut().set_height(height);
            self
        }

        /// Set uniform padding on all sides.
        pub fn padding(mut self, padding: impl Into<crate::constraint::Constraint> + Copy) -> Self {
            self.widget.style_mut().set_padding(padding);
            self
        }

        /// Set uniform margin on all sides.
        pub fn margin(mut self, margin: impl Into<crate::constraint::Constraint> + Copy) -> Self {
            self.widget.style_mut().set_margin(margin);
            self
        }

        // ── Per-side padding methods ─────────────────────────────────────────

        /// Set left padding only.
        pub fn padding_left(mut self, value: impl Into<crate::constraint::Constraint>) -> Self {
            self.widget.style_mut().set_padding_left(value);
            self
        }

        /// Set right padding only.
        pub fn padding_right(mut self, value: impl Into<crate::constraint::Constraint>) -> Self {
            self.widget.style_mut().set_padding_right(value);
            self
        }

        /// Set top padding only.
        pub fn padding_top(mut self, value: impl Into<crate::constraint::Constraint>) -> Self {
            self.widget.style_mut().set_padding_top(value);
            self
        }

        /// Set bottom padding only.
        pub fn padding_bottom(mut self, value: impl Into<crate::constraint::Constraint>) -> Self {
            self.widget.style_mut().set_padding_bottom(value);
            self
        }

        /// Set horizontal padding (left and right).
        pub fn padding_x(mut self, value: impl Into<crate::constraint::Constraint> + Copy) -> Self {
            self.widget.style_mut().set_padding_x(value);
            self
        }

        /// Set vertical padding (top and bottom).
        pub fn padding_y(mut self, value: impl Into<crate::constraint::Constraint> + Copy) -> Self {
            self.widget.style_mut().set_padding_y(value);
            self
        }

        // ── Per-side margin methods ──────────────────────────────────────────

        /// Set left margin only.
        pub fn margin_left(mut self, value: impl Into<crate::constraint::Constraint>) -> Self {
            self.widget.style_mut().set_margin_left(value);
            self
        }

        /// Set right margin only.
        pub fn margin_right(mut self, value: impl Into<crate::constraint::Constraint>) -> Self {
            self.widget.style_mut().set_margin_right(value);
            self
        }

        /// Set top margin only.
        pub fn margin_top(mut self, value: impl Into<crate::constraint::Constraint>) -> Self {
            self.widget.style_mut().set_margin_top(value);
            self
        }

        /// Set bottom margin only.
        pub fn margin_bottom(mut self, value: impl Into<crate::constraint::Constraint>) -> Self {
            self.widget.style_mut().set_margin_bottom(value);
            self
        }

        /// Set horizontal margin (left and right).
        pub fn margin_x(mut self, value: impl Into<crate::constraint::Constraint> + Copy) -> Self {
            self.widget.style_mut().set_margin_x(value);
            self
        }

        /// Set vertical margin (top and bottom).
        pub fn margin_y(mut self, value: impl Into<crate::constraint::Constraint> + Copy) -> Self {
            self.widget.style_mut().set_margin_y(value);
            self
        }

        /// Set minimum width constraint.
        pub fn min_width(mut self, width: impl Into<crate::constraint::Constraint>) -> Self {
            self.widget.style_mut().set_min_width(width);
            self
        }

        /// Set minimum height constraint.
        pub fn min_height(mut self, height: impl Into<crate::constraint::Constraint>) -> Self {
            self.widget.style_mut().set_min_height(height);
            self
        }

        /// Set maximum width constraint.
        pub fn max_width(mut self, width: impl Into<crate::constraint::Constraint>) -> Self {
            self.widget.style_mut().set_max_width(width);
            self
        }

        /// Set maximum height constraint.
        pub fn max_height(mut self, height: impl Into<crate::constraint::Constraint>) -> Self {
            self.widget.style_mut().set_max_height(height);
            self
        }

        /// Set the flex basis.
        pub fn flex_basis(mut self, basis: impl Into<crate::constraint::Constraint>) -> Self {
            self.widget.style_mut().set_flex_basis(basis);
            self
        }
    };
}

/// Generate layout-related style methods for container builders.
macro_rules! impl_node_layout_methods {
    () => {
        /// Set background color.
        pub fn background_color(mut self, color: astrelis_render::Color) -> Self {
            self.widget.style_mut().set_background_color(color);
            self
        }

        /// Set border color.
        pub fn border_color(mut self, color: astrelis_render::Color) -> Self {
            self.widget.style_mut().set_border_color(color);
            self
        }

        /// Set border width.
        pub fn border_width(mut self, width: f32) -> Self {
            self.widget.style_mut().set_border_width(width);
            self
        }

        /// Set border radius.
        pub fn border_radius(mut self, radius: f32) -> Self {
            self.widget.style_mut().set_border_radius(radius);
            self
        }

        /// Set overflow behavior for both axes.
        pub fn overflow(mut self, overflow: crate::Overflow) -> Self {
            self.widget.style_mut().set_overflow(overflow);
            self
        }

        /// Set overflow behavior for x-axis only.
        pub fn overflow_x(mut self, overflow: crate::Overflow) -> Self {
            self.widget.style_mut().set_overflow_x(overflow);
            self
        }

        /// Set overflow behavior for y-axis only.
        pub fn overflow_y(mut self, overflow: crate::Overflow) -> Self {
            self.widget.style_mut().set_overflow_y(overflow);
            self
        }

        /// Set the aspect ratio constraint.
        pub fn aspect_ratio(mut self, ratio: f32) -> Self {
            self.widget.style_mut().set_aspect_ratio(ratio);
            self
        }
    };
}

/// Generate flexbox-related style methods for container builders.
macro_rules! impl_node_flex_methods {
    () => {
        /// Set flex direction.
        pub fn flex_direction(mut self, direction: taffy::FlexDirection) -> Self {
            self.widget.style_mut().set_flex_direction(direction);
            self
        }

        /// Set justify content.
        pub fn justify_content(mut self, justify: taffy::JustifyContent) -> Self {
            self.widget.style_mut().set_justify_content(justify);
            self
        }

        /// Set align items.
        pub fn align_items(mut self, align: taffy::AlignItems) -> Self {
            self.widget.style_mut().set_align_items(align);
            self
        }

        /// Set gap between items.
        pub fn gap(mut self, gap: impl Into<crate::constraint::Constraint> + Copy) -> Self {
            self.widget.style_mut().set_gap(gap);
            self
        }

        /// Set flex wrap.
        pub fn flex_wrap(mut self, wrap: taffy::FlexWrap) -> Self {
            self.widget.style_mut().set_flex_wrap(wrap);
            self
        }
    };
}

// ── Trait: IntoNodeBuilder ──────────────────────────────────────────────────

/// Trait for widgets that can produce a widget-specific node builder.
///
/// Each widget implements this to return its own builder type, which provides
/// appropriate methods (e.g. children for containers, tabs for dock tabs).
///
/// # Example
///
/// ```ignore
/// // The generic add() method calls into_node_builder under the hood:
/// root.add(Text::new("Hello")).size(24.0).build()
/// ```
pub trait IntoNodeBuilder: Widget + Sized {
    /// The builder type returned by [`UiBuilder::add`].
    type Builder<'b, 'a: 'b>;

    /// Convert this widget into a node builder.
    fn into_node_builder<'b, 'a: 'b>(self, builder: &'b mut UiBuilder<'a>)
    -> Self::Builder<'b, 'a>;
}

// ── Node Builder: Leaf ──────────────────────────────────────────────────────

/// Node builder for leaf widgets (no children).
///
/// Used for: [`Text`], [`Button`], [`Image`], [`TextInput`], [`Tooltip`].
pub struct LeafNodeBuilder<'b, 'a: 'b, W: Widget> {
    builder: &'b mut UiBuilder<'a>,
    widget: W,
    widget_id: Option<WidgetId>,
}

impl<'b, 'a: 'b, W: Widget + 'static> LeafNodeBuilder<'b, 'a, W> {
    /// Create a new leaf node builder.
    pub fn new(builder: &'b mut UiBuilder<'a>, widget: W) -> Self {
        Self {
            builder,
            widget,
            widget_id: None,
        }
    }

    /// Set widget ID for later reference.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.widget_id = Some(id);
        self
    }

    /// Mutate the widget directly via a closure.
    ///
    /// Useful for third-party widgets or accessing widget-specific APIs
    /// not exposed through builder methods.
    pub fn configure<F: FnOnce(&mut W)>(mut self, f: F) -> Self {
        f(&mut self.widget);
        self
    }

    /// Apply a complete style to the widget.
    pub fn style(mut self, style: Style) -> Self {
        *self.widget.style_mut() = style;
        self
    }

    /// Build the widget and add it to the tree.
    pub fn build(self) -> NodeId {
        let node_id = self.builder.add_widget(Box::new(self.widget));
        if let Some(widget_id) = self.widget_id {
            self.builder.widget_registry.register(widget_id, node_id);
        }
        self.builder.set_root(node_id);
        node_id
    }

    // Style methods
    impl_node_style_methods!();
}

// ── Node Builder: Container ─────────────────────────────────────────────────

/// Node builder for container widgets (dynamic children).
///
/// Used for: [`Container`], [`Row`], [`Column`], [`ScrollContainer`].
pub struct ContainerNodeBuilder<'b, 'a: 'b, W: Widget> {
    builder: &'b mut UiBuilder<'a>,
    widget: W,
    children: Vec<NodeId>,
    widget_id: Option<WidgetId>,
}

impl<'b, 'a: 'b, W: Widget + 'static> ContainerNodeBuilder<'b, 'a, W> {
    /// Create a new container node builder.
    pub fn new(builder: &'b mut UiBuilder<'a>, widget: W) -> Self {
        Self {
            builder,
            widget,
            children: Vec::new(),
            widget_id: None,
        }
    }

    /// Set widget ID for later reference.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.widget_id = Some(id);
        self
    }

    /// Mutate the widget directly via a closure.
    pub fn configure<F: FnOnce(&mut W)>(mut self, f: F) -> Self {
        f(&mut self.widget);
        self
    }

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

    /// Apply a complete style to the widget.
    pub fn style(mut self, style: Style) -> Self {
        *self.widget.style_mut() = style;
        self
    }

    /// Build the container and add it to the tree.
    pub fn build(mut self) -> NodeId {
        let children = std::mem::take(&mut self.children);
        // Set children on the widget if it supports them
        if let Some(widget_children) = self.widget.children_mut() {
            *widget_children = children.clone();
        }
        let node_id = self.builder.add_widget(Box::new(self.widget));

        self.builder.tree.set_children(node_id, &children);
        if let Some(widget_id) = self.widget_id {
            self.builder.widget_registry.register(widget_id, node_id);
        }
        self.builder.set_root(node_id);
        node_id
    }

    // Style methods
    impl_node_style_methods!();
    impl_node_layout_methods!();
    impl_node_flex_methods!();
}

// ── Widget-specific methods on LeafNodeBuilder ──────────────────────────────

// Text-specific methods
impl<'b, 'a: 'b> LeafNodeBuilder<'b, 'a, Text> {
    /// Set font size.
    pub fn size(mut self, size: f32) -> Self {
        self.widget = self.widget.size(size);
        self
    }

    /// Set text color.
    pub fn color(mut self, color: Color) -> Self {
        self.widget = self.widget.color(color);
        self
    }

    /// Set font weight.
    pub fn weight(mut self, weight: astrelis_text::FontWeight) -> Self {
        self.widget = self.widget.weight(weight);
        self
    }

    /// Make text bold.
    pub fn bold(self) -> Self {
        self.weight(astrelis_text::FontWeight::Bold)
    }

    /// Set text alignment.
    pub fn align(mut self, align: astrelis_text::TextAlign) -> Self {
        self.widget = self.widget.align(align);
        self
    }

    /// Set font ID for font selection.
    pub fn font_id(mut self, font_id: u32) -> Self {
        self.widget = self.widget.font_id(font_id);
        self
    }

    /// Set the maximum wrap width for text.
    pub fn max_wrap_width(mut self, width: impl Into<crate::constraint::Constraint>) -> Self {
        self.widget = self.widget.max_wrap_width(width);
        self
    }
}

// Button-specific methods
impl<'b, 'a: 'b> LeafNodeBuilder<'b, 'a, Button> {
    /// Set button background color.
    pub fn background_color(mut self, color: Color) -> Self {
        self.widget = self.widget.background_color(color);
        self
    }

    /// Set hover color.
    pub fn hover_color(mut self, color: Color) -> Self {
        self.widget = self.widget.hover_color(color);
        self
    }

    /// Set text color.
    pub fn text_color(mut self, color: Color) -> Self {
        self.widget = self.widget.text_color(color);
        self
    }

    /// Set font size.
    pub fn font_size(mut self, size: f32) -> Self {
        self.widget = self.widget.font_size(size);
        self
    }

    /// Set font ID for font selection.
    pub fn font_id(mut self, font_id: u32) -> Self {
        self.widget = self.widget.font_id(font_id);
        self
    }

    /// Set click callback.
    pub fn on_click<F>(mut self, callback: F) -> Self
    where
        F: Fn() + 'static,
    {
        self.widget = self.widget.on_click(callback);
        self
    }
}

// Image-specific methods
impl<'b, 'a: 'b> LeafNodeBuilder<'b, 'a, Image> {
    /// Set UV coordinates (for sprites/atlases).
    pub fn uv(mut self, uv: ImageUV) -> Self {
        self.widget = self.widget.uv(uv);
        self
    }

    /// Set tint color (multiplied with texture).
    pub fn tint(mut self, color: Color) -> Self {
        self.widget = self.widget.tint(color);
        self
    }

    /// Set how the image fits within its bounds.
    pub fn fit(mut self, fit: ImageFit) -> Self {
        self.widget = self.widget.fit(fit);
        self
    }

    /// Set the natural size of the image (for sizing calculations).
    pub fn natural_size(mut self, width: f32, height: f32) -> Self {
        self.widget = self.widget.natural_size(width, height);
        self
    }

    /// Set border radius for rounded corners.
    pub fn border_radius(mut self, radius: f32) -> Self {
        self.widget = self.widget.border_radius(radius);
        self
    }

    /// Set the sampling mode for the image texture.
    pub fn sampling(mut self, sampling: astrelis_render::ImageSampling) -> Self {
        self.widget = self.widget.sampling(sampling);
        self
    }

    /// Shorthand for pixel-perfect rendering (Nearest filtering).
    pub fn pixel_perfect(self) -> Self {
        self.sampling(astrelis_render::ImageSampling::Nearest)
    }
}

// TextInput-specific methods
impl<'b, 'a: 'b> LeafNodeBuilder<'b, 'a, TextInput> {
    /// Set initial content.
    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.widget = self.widget.content(content);
        self
    }

    /// Set font size.
    pub fn font_size(mut self, size: f32) -> Self {
        self.widget = self.widget.font_size(size);
        self
    }

    /// Set text color.
    pub fn text_color(mut self, color: Color) -> Self {
        self.widget = self.widget.text_color(color);
        self
    }

    /// Set placeholder color.
    pub fn placeholder_color(mut self, color: Color) -> Self {
        self.widget = self.widget.placeholder_color(color);
        self
    }

    /// Set max length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.widget = self.widget.max_length(max);
        self
    }

    /// Set on change callback.
    pub fn on_change<F>(mut self, callback: F) -> Self
    where
        F: Fn(String) + 'static,
    {
        self.widget = self.widget.on_change(callback);
        self
    }
}

// Tooltip-specific methods
impl<'b, 'a: 'b> LeafNodeBuilder<'b, 'a, Tooltip> {
    /// Set font size.
    pub fn font_size(mut self, size: f32) -> Self {
        self.widget = self.widget.font_size(size);
        self
    }

    /// Set text color.
    pub fn text_color(mut self, color: Color) -> Self {
        self.widget = self.widget.text_color(color);
        self
    }

    /// Set background color.
    pub fn background_color(mut self, color: Color) -> Self {
        self.widget = self.widget.background_color(color);
        self
    }
}

// ── Widget-specific methods on ContainerNodeBuilder ─────────────────────────

// Row-specific methods
impl<'b, 'a: 'b> ContainerNodeBuilder<'b, 'a, Row> {
    // Row already gets gap(), justify_content(), align_items() from flex macro.
    // No additional methods needed.
}

// Column-specific methods
impl<'b, 'a: 'b> ContainerNodeBuilder<'b, 'a, Column> {
    // Column already gets gap(), justify_content(), align_items() from flex macro.
    // No additional methods needed.
}

// ScrollContainer-specific methods
impl<'b, 'a: 'b> ContainerNodeBuilder<'b, 'a, ScrollContainer> {
    /// Set which axes are scrollable.
    pub fn scroll_axis(mut self, axis: ScrollAxis) -> Self {
        self.widget.scroll_axis = axis;
        self
    }

    /// Set scrollbar visibility policy.
    pub fn scrollbar_visibility(mut self, visibility: ScrollbarVisibility) -> Self {
        self.widget.scrollbar_visibility = visibility;
        self
    }

    /// Set scrollbar visual theme.
    pub fn scrollbar_theme(mut self, theme: crate::widgets::ScrollbarTheme) -> Self {
        self.widget.scrollbar_theme = theme;
        self
    }
}

// ── IntoNodeBuilder implementations ─────────────────────────────────────────

impl IntoNodeBuilder for Text {
    type Builder<'b, 'a: 'b> = LeafNodeBuilder<'b, 'a, Text>;

    fn into_node_builder<'b, 'a: 'b>(
        self,
        builder: &'b mut UiBuilder<'a>,
    ) -> Self::Builder<'b, 'a> {
        LeafNodeBuilder::new(builder, self)
    }
}

impl IntoNodeBuilder for Button {
    type Builder<'b, 'a: 'b> = LeafNodeBuilder<'b, 'a, Button>;

    fn into_node_builder<'b, 'a: 'b>(
        self,
        builder: &'b mut UiBuilder<'a>,
    ) -> Self::Builder<'b, 'a> {
        LeafNodeBuilder::new(builder, self)
    }
}

impl IntoNodeBuilder for Image {
    type Builder<'b, 'a: 'b> = LeafNodeBuilder<'b, 'a, Image>;

    fn into_node_builder<'b, 'a: 'b>(
        self,
        builder: &'b mut UiBuilder<'a>,
    ) -> Self::Builder<'b, 'a> {
        LeafNodeBuilder::new(builder, self)
    }
}

impl IntoNodeBuilder for TextInput {
    type Builder<'b, 'a: 'b> = LeafNodeBuilder<'b, 'a, TextInput>;

    fn into_node_builder<'b, 'a: 'b>(
        self,
        builder: &'b mut UiBuilder<'a>,
    ) -> Self::Builder<'b, 'a> {
        LeafNodeBuilder::new(builder, self)
    }
}

impl IntoNodeBuilder for Tooltip {
    type Builder<'b, 'a: 'b> = LeafNodeBuilder<'b, 'a, Tooltip>;

    fn into_node_builder<'b, 'a: 'b>(
        self,
        builder: &'b mut UiBuilder<'a>,
    ) -> Self::Builder<'b, 'a> {
        LeafNodeBuilder::new(builder, self)
    }
}

impl IntoNodeBuilder for Container {
    type Builder<'b, 'a: 'b> = ContainerNodeBuilder<'b, 'a, Container>;

    fn into_node_builder<'b, 'a: 'b>(
        self,
        builder: &'b mut UiBuilder<'a>,
    ) -> Self::Builder<'b, 'a> {
        ContainerNodeBuilder::new(builder, self)
    }
}

impl IntoNodeBuilder for Row {
    type Builder<'b, 'a: 'b> = ContainerNodeBuilder<'b, 'a, Row>;

    fn into_node_builder<'b, 'a: 'b>(
        self,
        builder: &'b mut UiBuilder<'a>,
    ) -> Self::Builder<'b, 'a> {
        ContainerNodeBuilder::new(builder, self)
    }
}

impl IntoNodeBuilder for Column {
    type Builder<'b, 'a: 'b> = ContainerNodeBuilder<'b, 'a, Column>;

    fn into_node_builder<'b, 'a: 'b>(
        self,
        builder: &'b mut UiBuilder<'a>,
    ) -> Self::Builder<'b, 'a> {
        ContainerNodeBuilder::new(builder, self)
    }
}

impl IntoNodeBuilder for ScrollContainer {
    type Builder<'b, 'a: 'b> = ContainerNodeBuilder<'b, 'a, ScrollContainer>;

    fn into_node_builder<'b, 'a: 'b>(
        self,
        builder: &'b mut UiBuilder<'a>,
    ) -> Self::Builder<'b, 'a> {
        ContainerNodeBuilder::new(builder, self)
    }
}

// ── Docking builders ────────────────────────────────────────────────────────

#[cfg(feature = "docking")]
/// Node builder for dock splitter widgets (two named children).
pub struct DockSplitterNodeBuilder<'b, 'a: 'b> {
    builder: &'b mut UiBuilder<'a>,
    widget: DockSplitter,
    first_child: Option<NodeId>,
    second_child: Option<NodeId>,
}

#[cfg(feature = "docking")]
impl<'b, 'a: 'b> DockSplitterNodeBuilder<'b, 'a> {
    /// Set the split ratio (0.0-1.0, how much the first panel gets).
    pub fn split_ratio(mut self, ratio: f32) -> Self {
        self.widget = self.widget.split_ratio(ratio);
        self
    }

    /// Set the separator size in pixels.
    pub fn separator_size(mut self, size: f32) -> Self {
        self.widget = self.widget.separator_size(size);
        self
    }

    /// Set the separator colors (normal, hover).
    pub fn separator_colors(mut self, normal: Color, hover: Color) -> Self {
        self.widget = self.widget.separator_colors(normal, hover);
        self
    }

    /// Set minimum size constraint for the first panel.
    pub fn first_min_size(mut self, min_size: f32) -> Self {
        self.widget.first_constraints = PanelConstraints::min(min_size);
        self
    }

    /// Set minimum size constraint for the second panel.
    pub fn second_min_size(mut self, min_size: f32) -> Self {
        self.widget.second_constraints = PanelConstraints::min(min_size);
        self
    }

    /// Set constraints for the first panel.
    pub fn first_constraints(mut self, constraints: PanelConstraints) -> Self {
        self.widget = self.widget.first_constraints(constraints);
        self
    }

    /// Set constraints for the second panel.
    pub fn second_constraints(mut self, constraints: PanelConstraints) -> Self {
        self.widget = self.widget.second_constraints(constraints);
        self
    }

    /// Set per-widget separator hit-test tolerance (extra pixels per side).
    pub fn separator_tolerance(mut self, tolerance: f32) -> Self {
        self.widget = self.widget.separator_tolerance(tolerance);
        self
    }

    /// Build the first (left/top) panel content.
    pub fn first<F>(mut self, build_child: F) -> Self
    where
        F: FnOnce(&mut UiBuilder) -> NodeId,
    {
        let mut child_builder = self.builder.child_builder();
        let child_id = build_child(&mut child_builder);
        self.first_child = Some(child_id);
        self
    }

    /// Build the second (right/bottom) panel content.
    pub fn second<F>(mut self, build_child: F) -> Self
    where
        F: FnOnce(&mut UiBuilder) -> NodeId,
    {
        let mut child_builder = self.builder.child_builder();
        let child_id = build_child(&mut child_builder);
        self.second_child = Some(child_id);
        self
    }

    /// Build the splitter and add it to the tree.
    pub fn build(mut self) -> NodeId {
        let mut children = Vec::new();
        if let Some(first) = self.first_child {
            children.push(first);
        }
        if let Some(second) = self.second_child {
            children.push(second);
        }

        self.widget.children = children.clone();
        let node_id = self.builder.add_widget(Box::new(self.widget));

        self.builder.tree.set_children(node_id, &children);
        self.builder.set_root(node_id);

        node_id
    }

    // Style methods
    impl_node_style_methods!();
    impl_node_layout_methods!();
}

#[cfg(feature = "docking")]
impl IntoNodeBuilder for DockSplitter {
    type Builder<'b, 'a: 'b> = DockSplitterNodeBuilder<'b, 'a>;

    fn into_node_builder<'b, 'a: 'b>(
        self,
        builder: &'b mut UiBuilder<'a>,
    ) -> Self::Builder<'b, 'a> {
        DockSplitterNodeBuilder {
            builder,
            widget: self,
            first_child: None,
            second_child: None,
        }
    }
}

#[cfg(feature = "docking")]
/// Pending tab content builder.
struct TabContentBuilder {
    label: String,
    content: Option<NodeId>,
}

#[cfg(feature = "docking")]
/// Node builder for dock tabs widgets (labeled tabs).
pub struct DockTabsNodeBuilder<'b, 'a: 'b> {
    builder: &'b mut UiBuilder<'a>,
    widget: DockTabs,
    tab_builders: Vec<TabContentBuilder>,
}

#[cfg(feature = "docking")]
impl<'b, 'a: 'b> DockTabsNodeBuilder<'b, 'a> {
    /// Add a tab with a label and content builder.
    pub fn tab<F>(mut self, label: impl Into<String>, build_content: F) -> Self
    where
        F: FnOnce(&mut UiBuilder) -> NodeId,
    {
        let mut child_builder = self.builder.child_builder();
        let content_id = build_content(&mut child_builder);
        self.tab_builders.push(TabContentBuilder {
            label: label.into(),
            content: Some(content_id),
        });
        self
    }

    /// Set the initially active tab.
    pub fn active_tab(mut self, index: usize) -> Self {
        self.widget.active_tab = index;
        self
    }

    /// Set the tab bar height.
    pub fn tab_bar_height(mut self, height: f32) -> Self {
        self.widget = self.widget.tab_bar_height(height);
        self
    }

    /// Set tab colors (bar, active, inactive).
    pub fn tab_colors(mut self, bar: Color, active: Color, inactive: Color) -> Self {
        self.widget = self.widget.tab_colors(bar, active, inactive);
        self
    }

    /// Set the tab text color.
    pub fn text_color(mut self, color: Color) -> Self {
        self.widget = self.widget.text_color(color);
        self
    }

    /// Enable or disable close buttons on tabs.
    pub fn closable(mut self, closable: bool) -> Self {
        self.widget = self.widget.closable(closable);
        self
    }

    /// Set the tab font size.
    pub fn tab_font_size(mut self, size: f32) -> Self {
        self.widget = self.widget.tab_font_size(size);
        self
    }

    /// Set how overflow tabs are indicated (arrows, scrollbar, or both).
    pub fn scroll_indicator(mut self, mode: TabScrollIndicator) -> Self {
        self.widget = self.widget.scroll_indicator(mode);
        self
    }

    /// Set the vertical position of the tab bar scrollbar.
    pub fn scrollbar_position(mut self, position: TabScrollbarPosition) -> Self {
        self.widget = self.widget.scrollbar_position(position);
        self
    }

    /// Set the scrollbar visual theme.
    pub fn scrollbar_theme(mut self, theme: ScrollbarTheme) -> Self {
        self.widget = self.widget.scrollbar_theme(theme);
        self
    }

    /// Set per-widget content padding override.
    pub fn content_padding(mut self, padding: f32) -> Self {
        self.widget = self.widget.content_padding(padding);
        self
    }

    /// Build the tabs container and add it to the tree.
    pub fn build(mut self) -> NodeId {
        for tab_builder in self.tab_builders {
            if let Some(content) = tab_builder.content {
                self.widget.add_tab(tab_builder.label, content);
            }
        }

        let children = self.widget.children.clone();
        let node_id = self.builder.add_widget(Box::new(self.widget));

        self.builder.tree.set_children(node_id, &children);
        self.builder.set_root(node_id);

        node_id
    }

    // Style methods
    impl_node_style_methods!();
    impl_node_layout_methods!();
}

#[cfg(feature = "docking")]
impl IntoNodeBuilder for DockTabs {
    type Builder<'b, 'a: 'b> = DockTabsNodeBuilder<'b, 'a>;

    fn into_node_builder<'b, 'a: 'b>(
        self,
        builder: &'b mut UiBuilder<'a>,
    ) -> Self::Builder<'b, 'a> {
        DockTabsNodeBuilder {
            builder,
            widget: self,
            tab_builders: Vec::new(),
        }
    }
}

// ── UiBuilder ───────────────────────────────────────────────────────────────

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

    /// Add any widget that implements [`IntoNodeBuilder`].
    ///
    /// Returns a widget-specific builder with appropriate methods.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// root.add(Container::new()).width(100.0).child(|c| { ... }).build()
    /// root.add(Text::new("Hello")).size(24.0).build()
    /// root.add(DockSplitter::horizontal()).split_ratio(0.3).first(|l| { ... }).build()
    /// ```
    pub fn add<W: IntoNodeBuilder>(&mut self, widget: W) -> W::Builder<'_, 'a> {
        widget.into_node_builder(self)
    }

    // ── Convenience methods ─────────────────────────────────────────────

    /// Create a container widget.
    pub fn container(&mut self) -> ContainerNodeBuilder<'_, 'a, Container> {
        self.add(Container::new())
    }

    /// Create a text widget.
    pub fn text(&mut self, content: impl Into<String>) -> LeafNodeBuilder<'_, 'a, Text> {
        self.add(Text::new(content))
    }

    /// Create a button widget.
    pub fn button(&mut self, label: impl Into<String>) -> LeafNodeBuilder<'_, 'a, Button> {
        self.add(Button::new(label))
    }

    /// Create a row layout widget.
    pub fn row(&mut self) -> ContainerNodeBuilder<'_, 'a, Row> {
        self.add(Row::new())
    }

    /// Create a column layout widget.
    pub fn column(&mut self) -> ContainerNodeBuilder<'_, 'a, Column> {
        self.add(Column::new())
    }

    /// Create a text input widget.
    pub fn text_input(
        &mut self,
        placeholder: impl Into<String>,
    ) -> LeafNodeBuilder<'_, 'a, TextInput> {
        self.add(TextInput::new(placeholder))
    }

    /// Create a tooltip widget.
    pub fn tooltip(&mut self, text: impl Into<String>) -> LeafNodeBuilder<'_, 'a, Tooltip> {
        self.add(Tooltip::new(text))
    }

    /// Create an image widget with a texture.
    pub fn image(&mut self, texture: ImageTexture) -> LeafNodeBuilder<'_, 'a, Image> {
        self.add(Image::new().texture(texture))
    }

    /// Create an image widget without a texture (can be set later).
    pub fn image_placeholder(&mut self) -> LeafNodeBuilder<'_, 'a, Image> {
        self.add(Image::new())
    }

    /// Create a scroll container widget.
    pub fn scroll_container(&mut self) -> ContainerNodeBuilder<'_, 'a, ScrollContainer> {
        self.add(ScrollContainer::new())
    }

    /// Create a horizontal split (left/right panels).
    #[cfg(feature = "docking")]
    pub fn hsplit(&mut self) -> DockSplitterNodeBuilder<'_, 'a> {
        self.add(DockSplitter::horizontal())
    }

    /// Create a vertical split (top/bottom panels).
    #[cfg(feature = "docking")]
    pub fn vsplit(&mut self) -> DockSplitterNodeBuilder<'_, 'a> {
        self.add(DockSplitter::vertical())
    }

    /// Create a tabbed container.
    #[cfg(feature = "docking")]
    pub fn dock_tabs(&mut self) -> DockTabsNodeBuilder<'_, 'a> {
        self.add(DockTabs::new())
    }

    // ── Deprecated convenience methods (compile-time plugin proof) ──────

    /// Create a scroll container widget (requires [`ScrollPlugin`](crate::scroll_plugin::ScrollPlugin)).
    #[deprecated(note = "Use `scroll_container()` or `add(ScrollContainer::new())` instead")]
    pub fn scroll_container_with(
        &mut self,
        _handle: crate::plugin::PluginHandle<crate::scroll_plugin::ScrollPlugin>,
    ) -> ContainerNodeBuilder<'_, 'a, ScrollContainer> {
        self.scroll_container()
    }

    /// Create a horizontal split (left/right panels), requiring [`DockingPlugin`](crate::widgets::docking::plugin::DockingPlugin).
    #[cfg(feature = "docking")]
    #[deprecated(note = "Use `hsplit()` or `add(DockSplitter::horizontal())` instead")]
    pub fn hsplit_with(
        &mut self,
        _handle: crate::plugin::PluginHandle<crate::widgets::docking::plugin::DockingPlugin>,
    ) -> DockSplitterNodeBuilder<'_, 'a> {
        self.hsplit()
    }

    /// Create a vertical split (top/bottom panels), requiring [`DockingPlugin`](crate::widgets::docking::plugin::DockingPlugin).
    #[cfg(feature = "docking")]
    #[deprecated(note = "Use `vsplit()` or `add(DockSplitter::vertical())` instead")]
    pub fn vsplit_with(
        &mut self,
        _handle: crate::plugin::PluginHandle<crate::widgets::docking::plugin::DockingPlugin>,
    ) -> DockSplitterNodeBuilder<'_, 'a> {
        self.vsplit()
    }

    /// Create a tabbed container, requiring [`DockingPlugin`](crate::widgets::docking::plugin::DockingPlugin).
    #[cfg(feature = "docking")]
    #[deprecated(note = "Use `dock_tabs()` or `add(DockTabs::new())` instead")]
    pub fn dock_tabs_with(
        &mut self,
        _handle: crate::plugin::PluginHandle<crate::widgets::docking::plugin::DockingPlugin>,
    ) -> DockTabsNodeBuilder<'_, 'a> {
        self.dock_tabs()
    }

    // ── Internal methods ────────────────────────────────────────────────

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

// ── Legacy re-exports ───────────────────────────────────────────────────────
// These type aliases maintain backward compatibility for code that references
// the old builder types by name.

/// Legacy alias for [`LeafNodeBuilder<Text>`].
pub type TextBuilder<'b, 'a> = LeafNodeBuilder<'b, 'a, Text>;

/// Legacy alias for [`ContainerNodeBuilder<Container>`].
pub type ContainerBuilder<'b, 'a> = ContainerNodeBuilder<'b, 'a, Container>;

/// Legacy alias for [`LeafNodeBuilder<Button>`].
pub type ButtonBuilder<'b, 'a> = LeafNodeBuilder<'b, 'a, Button>;

/// Legacy alias for [`ContainerNodeBuilder<Row>`].
pub type RowBuilder<'b, 'a> = ContainerNodeBuilder<'b, 'a, Row>;

/// Legacy alias for [`ContainerNodeBuilder<Column>`].
pub type ColumnBuilder<'b, 'a> = ContainerNodeBuilder<'b, 'a, Column>;

/// Legacy alias for [`LeafNodeBuilder<TextInput>`].
pub type TextInputBuilder<'b, 'a> = LeafNodeBuilder<'b, 'a, TextInput>;

/// Legacy alias for [`LeafNodeBuilder<Tooltip>`].
pub type TooltipBuilder<'b, 'a> = LeafNodeBuilder<'b, 'a, Tooltip>;

/// Legacy alias for [`LeafNodeBuilder<Image>`].
pub type ImageBuilder<'b, 'a> = LeafNodeBuilder<'b, 'a, Image>;

/// Legacy alias for [`ContainerNodeBuilder<ScrollContainer>`].
pub type ScrollContainerBuilder<'b, 'a> = ContainerNodeBuilder<'b, 'a, ScrollContainer>;

/// Legacy alias for [`DockSplitterNodeBuilder`].
#[cfg(feature = "docking")]
pub type DockSplitterBuilder<'b, 'a> = DockSplitterNodeBuilder<'b, 'a>;

/// Legacy alias for [`DockTabsNodeBuilder`].
#[cfg(feature = "docking")]
pub type DockTabsBuilder<'b, 'a> = DockTabsNodeBuilder<'b, 'a>;

/// Base trait for widget builders (legacy).
///
/// This trait is no longer central to the builder API. All builder types
/// now use concrete methods via macros instead. Kept for backward compatibility.
pub trait WidgetBuilder {
    /// Apply a style to the widget.
    fn style(self, style: Style) -> Self;

    /// Set width. Accepts f32 (pixels), or any Constraint type.
    fn width(self, width: impl Into<crate::constraint::Constraint>) -> Self;

    /// Set height. Accepts f32 (pixels), or any Constraint type.
    fn height(self, height: impl Into<crate::constraint::Constraint>) -> Self;

    /// Set padding. Accepts f32 (pixels), or any Constraint type.
    fn padding(self, padding: impl Into<crate::constraint::Constraint> + Copy) -> Self;

    /// Set margin. Accepts f32 (pixels), or any Constraint type.
    fn margin(self, margin: impl Into<crate::constraint::Constraint> + Copy) -> Self;
}
