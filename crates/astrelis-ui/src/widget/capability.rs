//! Widget capability traits for type-safe widget operations.
//!
//! This module defines a capability-based trait system for widgets, allowing
//! compile-time verification of widget capabilities instead of runtime downcasting.

use astrelis_core::math::Vec2;
use astrelis_render::Color;
use astrelis_text::{Text as TextStyle, TextAlign, VerticalAlign};

/// Unique identifier for a widget instance.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct WidgetId(pub u64);

/// Base trait for all widgets.
///
/// # Design Pattern: Capability-Based Traits
///
/// Instead of downcasting with `as_any()`, widgets expose their capabilities
/// through trait query methods. This provides compile-time type safety.
///
/// # Borrow Checking Pattern
///
/// Query methods return `Option<&dyn CapabilityTrait>` to indicate whether
/// a widget supports a particular capability. This is safe because:
/// - The returned reference has the same lifetime as `&self`
/// - Rust's borrow checker ensures no aliasing violations
/// - Multiple immutable queries can coexist
///
/// # Example
///
/// ```rust,ignore
/// fn update_text(widget: &mut dyn Widget, text: &str) {
///     if let Some(text_widget) = widget.as_text_widget_mut() {
///         text_widget.set_text(text);
///     }
/// }
/// ```
pub trait Widget: Send + Sync {
    /// Get the widget's unique identifier.
    fn id(&self) -> WidgetId;

    /// Get the widget's layout node ID.
    ///
    /// This is used by the Taffy layout engine.
    fn layout_node(&self) -> taffy::NodeId;

    /// Get the widget's name for debugging.
    fn debug_name(&self) -> &str {
        "Widget"
    }

    // Capability query methods (immutable)

    /// Query if this widget supports text operations.
    fn as_text_widget(&self) -> Option<&dyn TextWidget> {
        None
    }

    /// Query if this widget is a container.
    fn as_container(&self) -> Option<&dyn ParentWidget> {
        None
    }

    /// Query if this widget supports color operations.
    fn as_color_widget(&self) -> Option<&dyn ColorWidget> {
        None
    }

    /// Query if this widget supports size operations.
    fn as_sized_widget(&self) -> Option<&dyn SizedWidget> {
        None
    }

    // Capability query methods (mutable)

    /// Query if this widget supports text operations (mutable).
    fn as_text_widget_mut(&mut self) -> Option<&mut dyn TextWidget> {
        None
    }

    /// Query if this widget is a container (mutable).
    fn as_container_mut(&mut self) -> Option<&mut dyn ParentWidget> {
        None
    }

    /// Query if this widget supports color operations (mutable).
    fn as_color_widget_mut(&mut self) -> Option<&mut dyn ColorWidget> {
        None
    }

    /// Query if this widget supports size operations (mutable).
    fn as_sized_widget_mut(&mut self) -> Option<&mut dyn SizedWidget> {
        None
    }
}

/// Capability: Widget that can contain children.
///
/// # Borrow Checking: Children Access
///
/// ## Why `&[Box<dyn Widget>]` not `&[&dyn Widget]`?
///
/// We need `Box` ownership for:
/// - Adding/removing children
/// - Moving widgets between containers
/// - Storing heterogeneous widget types
///
/// ## Mutable Access Pattern
///
/// ```rust,ignore
/// let children = container.children_mut();
/// for child in children.iter_mut() {
///     let widget: &mut dyn Widget = &mut **child;
///     // Double deref: Box -> dyn Widget
/// }
/// ```
pub trait ParentWidget: Widget {
    /// Get children (immutable).
    fn children(&self) -> &[Box<dyn Widget>];

    /// Get children (mutable).
    fn children_mut(&mut self) -> &mut [Box<dyn Widget>];

    /// Add a child widget.
    ///
    /// Takes ownership of the child widget.
    fn add_child(&mut self, child: Box<dyn Widget>);

    /// Remove a child by ID.
    ///
    /// Returns the removed child if found.
    fn remove_child(&mut self, id: WidgetId) -> Option<Box<dyn Widget>>;

    /// Find a child by ID.
    fn find_child(&self, id: WidgetId) -> Option<&dyn Widget> {
        self.children()
            .iter()
            .find(|child| child.id() == id)
            .map(|b| &**b)
    }

    /// Find a child by ID (mutable).
    fn find_child_mut(&mut self, id: WidgetId) -> Option<&mut dyn Widget> {
        self.children_mut()
            .iter_mut()
            .find(|child| child.id() == id)
            .map(|b| &mut **b as &mut dyn Widget)
    }
}

/// Capability: Widget that displays text.
pub trait TextWidget: Widget {
    /// Get the current text.
    fn text(&self) -> &str;

    /// Set the text.
    ///
    /// This will invalidate text shaping cache.
    /// Note: Takes &str for object-safety (dyn compatibility).
    fn set_text(&mut self, text: &str);

    /// Build a text style for rendering.
    ///
    /// Returns an owned TextStyle built from the widget's current state.
    /// This is called when rendering text.
    fn build_text_style(&self) -> TextStyle;

    /// Get horizontal text alignment.
    fn text_align(&self) -> TextAlign;

    /// Set horizontal text alignment.
    fn set_text_align(&mut self, align: TextAlign);

    /// Get vertical text alignment.
    fn vertical_align(&self) -> VerticalAlign;

    /// Set vertical text alignment.
    fn set_vertical_align(&mut self, align: VerticalAlign);
}

/// Capability: Widget with a background color.
pub trait ColorWidget: Widget {
    /// Get the current color.
    fn color(&self) -> Color;

    /// Set the color.
    ///
    /// This only invalidates color, not layout or geometry.
    fn set_color(&mut self, color: Color);
}

/// Capability: Widget with explicit size control.
pub trait SizedWidget: Widget {
    /// Get the widget's size.
    fn size(&self) -> Vec2;

    /// Set the widget's size.
    ///
    /// This will trigger layout recalculation.
    fn set_size(&mut self, size: Vec2);

    /// Get the widget's minimum size.
    fn min_size(&self) -> Option<Vec2> {
        None
    }

    /// Set the widget's minimum size.
    fn set_min_size(&mut self, size: Option<Vec2>);

    /// Get the widget's maximum size.
    fn max_size(&self) -> Option<Vec2> {
        None
    }

    /// Set the widget's maximum size.
    fn set_max_size(&mut self, size: Option<Vec2>);
}

/// Capability: Widget that can be clicked.
pub trait ClickableWidget: Widget {
    /// Check if the widget is currently pressed.
    fn is_pressed(&self) -> bool;

    /// Check if the widget is currently hovered.
    fn is_hovered(&self) -> bool;

    /// Set click callback.
    ///
    /// Note: This uses dynamic dispatch since callbacks can't be in traits easily.
    /// Implementers should store callbacks internally.
    fn on_click(&mut self, callback: Box<dyn FnMut() + Send + Sync>);
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock widget for testing capability queries
    struct MockButton {
        id: WidgetId,
        node: taffy::NodeId,
        text: String,
        color: Color,
    }

    impl Widget for MockButton {
        fn id(&self) -> WidgetId {
            self.id
        }

        fn layout_node(&self) -> taffy::NodeId {
            self.node
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

    impl TextWidget for MockButton {
        fn text(&self) -> &str {
            &self.text
        }

        fn set_text(&mut self, text: &str) {
            self.text = text.to_string();
        }

        fn build_text_style(&self) -> TextStyle {
            TextStyle::new(&self.text)
        }

        fn text_align(&self) -> TextAlign {
            TextAlign::Left
        }

        fn set_text_align(&mut self, _align: TextAlign) {
            // No-op for mock
        }

        fn vertical_align(&self) -> VerticalAlign {
            VerticalAlign::Top
        }

        fn set_vertical_align(&mut self, _align: VerticalAlign) {
            // No-op for mock
        }
    }

    impl ColorWidget for MockButton {
        fn color(&self) -> Color {
            self.color
        }

        fn set_color(&mut self, color: Color) {
            self.color = color;
        }
    }

    #[test]
    fn test_capability_query() {
        let mut button = MockButton {
            id: WidgetId(1),
            node: taffy::NodeId::from(0u64),
            text: "Click me".to_string(),
            color: Color::RED,
        };

        // Test immutable query
        assert!(button.as_text_widget().is_some());
        assert!(button.as_color_widget().is_some());
        assert!(button.as_container().is_none());

        // Test mutable query
        if let Some(text_widget) = button.as_text_widget_mut() {
            text_widget.set_text("Updated");
        }
        assert_eq!(button.text(), "Updated");
    }

    #[test]
    fn test_multiple_capabilities() {
        let button = MockButton {
            id: WidgetId(1),
            node: taffy::NodeId::from(0u64),
            text: "Button".to_string(),
            color: Color::BLUE,
        };

        // Can query multiple capabilities
        let text_cap = button.as_text_widget();
        let color_cap = button.as_color_widget();

        assert!(text_cap.is_some());
        assert!(color_cap.is_some());
        assert_eq!(text_cap.unwrap().text(), "Button");
        assert_eq!(color_cap.unwrap().color(), Color::BLUE);
    }
}
