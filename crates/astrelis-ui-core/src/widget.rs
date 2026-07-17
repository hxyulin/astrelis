//! The application-defined widget lifecycle.

use super::*;

/// Child-layout defaults supplied by an application-defined widget.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WidgetContainerStyle {
    /// Padding applied inside the custom widget.
    pub padding: Insets,
    /// Gap inserted between retained children.
    pub gap: f32,
}

impl WidgetContainerStyle {
    /// Creates a structural container without implicit padding or gaps.
    pub const fn structural() -> Self {
        Self {
            padding: Insets::all(0.0),
            gap: 0.0,
        }
    }
}

/// Lifecycle implemented by application-defined retained widgets.
///
/// Custom widgets are retained as ordinary tree nodes. Their children use the
/// same layout, routing, semantics, and painting machinery as built-ins.
pub trait Widget<Message>: Any {
    /// Returns this widget for typed retained access.
    fn as_any(&self) -> &dyn Any;
    /// Returns this widget for typed retained updates.
    fn as_any_mut(&mut self) -> &mut dyn Any;
    /// Called after the widget is attached to a UI tree.
    fn mounted(&mut self, _context: &mut MountContext<'_, Message>) -> Result<(), UiError> {
        Ok(())
    }
    /// Called immediately before the widget is removed.
    fn unmounted(&mut self) {}
    /// Called after application code mutates the retained widget.
    fn updated(&mut self) {}
    /// Returns the widget's intrinsic leaf size before Taffy constraints.
    fn intrinsic_size(&self, _theme: &Theme) -> LogicalSize {
        Size::ZERO
    }
    /// Supplies padding and child gaps for this custom container.
    fn container_style(&self, theme: &Theme) -> WidgetContainerStyle {
        WidgetContainerStyle {
            padding: theme.control_padding,
            gap: theme.gap,
        }
    }
    /// Observes normalized events at each phase along this node's route.
    fn event(&mut self, _context: &mut EventContext<'_, Message>, _event: &RoutedEvent) {}
    /// Whether this node may be the target of pointer input.
    fn hit_testable(&self) -> bool {
        false
    }
    /// Tests the widget's custom local hit shape after layout and transforms.
    fn hit_test(&self, point: LogicalPoint, bounds: LogicalRect) -> bool {
        bounds.contains(point)
    }
    /// Whether this node participates in keyboard focus traversal.
    fn focusable(&self) -> bool {
        false
    }
    /// Preferred cursor while this widget is the deepest hovered node.
    fn cursor_icon(&self) -> Option<CursorIcon> {
        None
    }
    /// Paints behind the widget's retained children.
    fn paint(
        &self,
        _painter: &mut Painter,
        _bounds: LogicalRect,
        _theme: &Theme,
    ) -> Result<(), UiError> {
        Ok(())
    }
    /// Supplies semantics for this node.
    fn semantics(&self) -> Option<(SemanticRole, String, Option<String>)> {
        None
    }
    /// Lists semantic operations supported by this custom widget.
    fn semantic_actions(&self) -> Vec<SemanticActionKind> {
        Vec::new()
    }
    /// Handles one semantic operation, returning whether it was accepted.
    fn semantic_action(
        &mut self,
        _context: &mut EventContext<'_, Message>,
        _action: &SemanticAction,
    ) -> bool {
        false
    }
}

/// Restricted tree-building context available during custom-widget mounting.
pub struct MountContext<'a, Message: 'static> {
    pub(crate) ui: &'a mut Ui<Message>,
    pub(crate) parent: ElementId,
}

impl<Message: 'static> MountContext<'_, Message> {
    /// Adds a label owned by the mounting widget.
    pub fn add_label(&mut self, text: impl Into<String>) -> Result<ElementHandle<Label>, UiError> {
        self.ui
            .insert(self.parent, Kind::Label { text: text.into() })
    }
    /// Adds a column owned by the mounting widget.
    pub fn add_column(&mut self) -> Result<ElementHandle<Column>, UiError> {
        let flex = FlexStyle {
            row_gap: self.ui.theme.gap,
            ..Default::default()
        };
        self.ui.insert(self.parent, Kind::Column { flex })
    }
}
