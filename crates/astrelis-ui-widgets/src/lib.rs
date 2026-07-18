//! Reusable controls and compositions built exclusively on `astrelis-ui-core`.

#![warn(missing_docs)]

use std::{any::Any, cell::Cell, rc::Rc};

use astrelis_core::geometry::{LogicalRect, LogicalSize, Size};
use astrelis_paint::{Brush, CornerRadii, Painter, RoundedRect, StrokeStyle};
use astrelis_platform::{CursorIcon, DeviceId, ElementState, Key, NamedKey, PointerButton};
use astrelis_ui_core::{
    Alignment, Column, DragOperations, DragOptions, DragPayload, DropOperation, ElementHandle,
    ElementId, EventFilter, FlexStyle, LayoutStyle, Length, ListenerId, MountContext,
    RoutedEventKind, Row, SemanticAction, SemanticActionKind, SemanticRole, Theme, Ui, UiError,
    Widget,
};

type PayloadAcceptance = dyn Fn(&DragPayload) -> bool;
type DropMessage<Message> = dyn Fn(&DragPayload, DropOperation) -> Message;
type RatioChangeMessage<Message> = dyn Fn(f32) -> Message;

mod composites;
mod render_view;
mod virtual_list;

pub use composites::{Form, List, ListItem, Menu, MenuItem, Popover, Tabs, Tooltip};
pub use render_view::{
    RenderView, RenderViewContent, RenderViewEvent, RenderViewPointerPosition,
    RenderViewResizePolicy, RenderViewSnapshot, render_view_snapshot,
};
pub use virtual_list::{VirtualList, VirtualListItem, VirtualListOptions};

/// Installs drag-source behavior on an arbitrary retained element.
pub fn install_drag_source<Message: 'static, T>(
    ui: &mut Ui<Message>,
    source: ElementHandle<T>,
    options: DragOptions,
    mut payload: impl FnMut() -> DragPayload + 'static,
) -> Result<ListenerId, UiError> {
    ui.listen(source, None, EventFilter::Pointer, move |context, event| {
        if let RoutedEventKind::PointerButton {
            device_id,
            position,
            state: ElementState::Pressed,
            ..
        } = event.kind
        {
            context.begin_drag(device_id, position, payload(), options);
        }
    })
}

/// A painted drop target which accepts payloads selected by application code.
pub struct DropZone<Message> {
    label: String,
    operation: DropOperation,
    accepts: Box<PayloadAcceptance>,
    on_drop: Box<DropMessage<Message>>,
    hovering: bool,
}

impl<Message> DropZone<Message> {
    /// Creates a drop zone with typed acceptance and message callbacks.
    pub fn new(
        label: impl Into<String>,
        operation: DropOperation,
        accepts: impl Fn(&DragPayload) -> bool + 'static,
        on_drop: impl Fn(&DragPayload, DropOperation) -> Message + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            operation,
            accepts: Box::new(accepts),
            on_drop: Box::new(on_drop),
            hovering: false,
        }
    }

    /// Returns whether an accepted drag currently overlaps this zone.
    pub const fn is_hovering(&self) -> bool {
        self.hovering
    }
}

impl<Message: 'static> Widget<Message> for DropZone<Message> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn mounted(&mut self, context: &mut MountContext<'_, Message>) -> Result<(), UiError> {
        context.add_label(self.label.clone())?;
        Ok(())
    }

    fn intrinsic_size(&self, _theme: &Theme) -> LogicalSize {
        Size::new(220.0, 88.0)
    }

    fn event(
        &mut self,
        context: &mut astrelis_ui_core::EventContext<'_, Message>,
        event: &astrelis_ui_core::RoutedEvent,
    ) {
        match &event.kind {
            RoutedEventKind::DragEntered {
                device_id, payload, ..
            }
            | RoutedEventKind::DragOver {
                device_id, payload, ..
            } if (self.accepts)(payload) => {
                self.hovering = true;
                context.accept_drop(*device_id, self.operation);
                context.request_paint();
            }
            RoutedEventKind::DragLeft { .. } => {
                self.hovering = false;
                context.request_paint();
            }
            RoutedEventKind::Dropped {
                payload, operation, ..
            } => {
                self.hovering = false;
                context.emit((self.on_drop)(payload, *operation));
                context.request_paint();
            }
            _ => {}
        }
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn paint(
        &self,
        painter: &mut Painter,
        bounds: LogicalRect,
        theme: &Theme,
    ) -> Result<(), UiError> {
        let rounded = RoundedRect::new(bounds, CornerRadii::uniform(theme.radii.lg))?;
        let background = if self.hovering {
            theme.button.pressed
        } else {
            theme.field_background
        };
        painter.fill_rounded_rect(rounded, Brush::Solid(background))?;
        painter.stroke_rounded_rect(
            rounded,
            StrokeStyle {
                width: theme.border_width,
                ..Default::default()
            },
            Brush::Solid(if self.hovering {
                theme.accent
            } else {
                theme.border
            }),
        )?;
        Ok(())
    }

    fn semantics(&self) -> Option<(SemanticRole, String, Option<String>)> {
        Some((SemanticRole::Group, self.label.clone(), None))
    }
}

/// Convenience options for a move-only source.
pub fn move_drag_options() -> DragOptions {
    DragOptions {
        allowed: DragOperations::MOVE,
        ..Default::default()
    }
}

/// Direction in which a split pane arranges its two regions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitAxis {
    /// Regions are side by side and the divider moves horizontally.
    Horizontal,
    /// Regions are stacked and the divider moves vertically.
    Vertical,
}

/// Configuration for a resizable split pane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplitPaneOptions {
    /// Arrangement and resize direction.
    pub axis: SplitAxis,
    /// Initial first-region fraction.
    pub ratio: f32,
    /// Minimum logical extent of the first region.
    pub first_min: f32,
    /// Minimum logical extent of the second region.
    pub second_min: f32,
    /// Logical divider thickness.
    pub divider_size: f32,
    /// Painted line thickness inside the divider hit target.
    pub divider_visual_size: f32,
    /// Fraction changed by one Arrow-key press.
    pub keyboard_step: f32,
}

impl Default for SplitPaneOptions {
    fn default() -> Self {
        Self {
            axis: SplitAxis::Horizontal,
            ratio: 0.5,
            first_min: 80.0,
            second_min: 80.0,
            divider_size: 8.0,
            divider_visual_size: 2.0,
            keyboard_step: 0.02,
        }
    }
}

#[derive(Clone, Copy)]
enum SplitContainer {
    Row(ElementHandle<Row>),
    Column(ElementHandle<Column>),
}

impl SplitContainer {
    fn id(self) -> ElementId {
        match self {
            Self::Row(handle) => handle.id(),
            Self::Column(handle) => handle.id(),
        }
    }
}

/// Controller and content handles for one resizable split pane.
pub struct SplitPane<Message = ()> {
    container: SplitContainer,
    first: ElementHandle<Column>,
    second: ElementHandle<Column>,
    divider: ElementHandle<Splitter<Message>>,
    ratio: Rc<Cell<f32>>,
}

impl<Message: 'static> SplitPane<Message> {
    /// Builds a split pane beneath `parent`.
    pub fn new<T>(
        ui: &mut Ui<Message>,
        parent: ElementHandle<T>,
        options: SplitPaneOptions,
    ) -> Result<Self, UiError> {
        Self::new_inner(ui, parent, options, None)
    }

    /// Builds a split pane which emits a typed message whenever an interactive
    /// pointer, keyboard, or semantic operation changes its ratio.
    pub fn new_with_on_change<T>(
        ui: &mut Ui<Message>,
        parent: ElementHandle<T>,
        options: SplitPaneOptions,
        on_change: impl Fn(f32) -> Message + 'static,
    ) -> Result<Self, UiError> {
        Self::new_inner(ui, parent, options, Some(Box::new(on_change)))
    }

    fn new_inner<T>(
        ui: &mut Ui<Message>,
        parent: ElementHandle<T>,
        options: SplitPaneOptions,
        on_change: Option<Box<RatioChangeMessage<Message>>>,
    ) -> Result<Self, UiError> {
        validate_split_options(options)?;
        let ratio = Rc::new(Cell::new(options.ratio.clamp(0.0, 1.0)));
        let mut on_change = on_change;
        let (container, first, second, divider) = match options.axis {
            SplitAxis::Horizontal => {
                let container = ui.add_row(parent)?;
                ui.set_flex_style(
                    container,
                    FlexStyle {
                        align_items: Alignment::Stretch,
                        ..Default::default()
                    },
                )?;
                let first = ui.add_column(container)?;
                let divider = ui.add_widget(
                    container,
                    Splitter::new(first, options, ratio.clone(), on_change.take()),
                )?;
                let second = ui.add_column(container)?;
                ui.update_widget(divider, |splitter| splitter.second = Some(second))?;
                (SplitContainer::Row(container), first, second, divider)
            }
            SplitAxis::Vertical => {
                let container = ui.add_column(parent)?;
                ui.set_flex_style(
                    container,
                    FlexStyle {
                        align_items: Alignment::Stretch,
                        ..Default::default()
                    },
                )?;
                let first = ui.add_column(container)?;
                let divider = ui.add_widget(
                    container,
                    Splitter::new(first, options, ratio.clone(), on_change.take()),
                )?;
                let second = ui.add_column(container)?;
                ui.update_widget(divider, |splitter| splitter.second = Some(second))?;
                (SplitContainer::Column(container), first, second, divider)
            }
        };
        apply_split_layout(ui, first, second, divider, options, ratio.get())?;
        Ok(Self {
            container,
            first,
            second,
            divider,
            ratio,
        })
    }

    /// Returns the erased identity of the outer row or column.
    pub fn container_id(&self) -> ElementId {
        self.container.id()
    }

    /// Changes the outer split container's layout constraints.
    pub fn set_container_layout(
        &self,
        ui: &mut Ui<Message>,
        layout: LayoutStyle,
    ) -> Result<(), UiError> {
        match self.container {
            SplitContainer::Row(handle) => ui.set_layout(handle, layout),
            SplitContainer::Column(handle) => ui.set_layout(handle, layout),
        }
    }

    /// Returns the first region's content column.
    pub const fn first(&self) -> ElementHandle<Column> {
        self.first
    }

    /// Returns the second region's content column.
    pub const fn second(&self) -> ElementHandle<Column> {
        self.second
    }

    /// Returns the interactive divider.
    pub const fn divider(&self) -> ElementHandle<Splitter<Message>> {
        self.divider
    }

    /// Returns the current first-region fraction.
    pub fn ratio(&self) -> f32 {
        self.ratio.get()
    }

    /// Changes the split fraction and immediately updates both regions.
    pub fn set_ratio(&self, ui: &mut Ui<Message>, ratio: f32) -> Result<(), UiError> {
        ui.update_widget(self.divider, |splitter| splitter.set_ratio(ratio))?;
        let options = ui.widget(self.divider)?.options;
        apply_split_layout(
            ui,
            self.first,
            self.second,
            self.divider,
            options,
            self.ratio.get(),
        )
    }
}

fn validate_split_options(options: SplitPaneOptions) -> Result<(), UiError> {
    let values = [
        options.ratio,
        options.first_min,
        options.second_min,
        options.divider_size,
        options.divider_visual_size,
        options.keyboard_step,
    ];
    if values.into_iter().any(|value| !value.is_finite())
        || options.first_min < 0.0
        || options.second_min < 0.0
        || options.divider_size <= 0.0
        || options.divider_visual_size <= 0.0
        || options.divider_visual_size > options.divider_size
        || options.keyboard_step <= 0.0
    {
        return Err(UiError::from_message(
            "split pane values must be finite and sizes/steps valid",
        ));
    }
    Ok(())
}

fn pane_layout(axis: SplitAxis, ratio: f32, minimum: f32) -> LayoutStyle {
    let mut layout = LayoutStyle {
        grow: ratio.max(0.0),
        shrink: 1.0,
        basis: Length::Px(0.0),
        ..Default::default()
    };
    match axis {
        SplitAxis::Horizontal => layout.min_width = Length::Px(minimum),
        SplitAxis::Vertical => layout.min_height = Length::Px(minimum),
    }
    layout
}

fn divider_layout(axis: SplitAxis, size: f32) -> LayoutStyle {
    match axis {
        SplitAxis::Horizontal => LayoutStyle {
            width: Length::Px(size),
            min_width: Length::Px(size),
            shrink: 0.0,
            ..Default::default()
        },
        SplitAxis::Vertical => LayoutStyle {
            height: Length::Px(size),
            min_height: Length::Px(size),
            shrink: 0.0,
            ..Default::default()
        },
    }
}

fn apply_split_layout<Message: 'static>(
    ui: &mut Ui<Message>,
    first: ElementHandle<Column>,
    second: ElementHandle<Column>,
    divider: ElementHandle<Splitter<Message>>,
    options: SplitPaneOptions,
    ratio: f32,
) -> Result<(), UiError> {
    ui.set_layout(first, pane_layout(options.axis, ratio, options.first_min))?;
    ui.set_layout(
        second,
        pane_layout(options.axis, 1.0 - ratio, options.second_min),
    )?;
    ui.set_layout(divider, divider_layout(options.axis, options.divider_size))
}

/// Interactive separator used by [`SplitPane`].
pub struct Splitter<Message = ()> {
    first: ElementHandle<Column>,
    second: Option<ElementHandle<Column>>,
    options: SplitPaneOptions,
    ratio: Rc<Cell<f32>>,
    drag: Option<(DeviceId, f32, f32)>,
    hovered: bool,
    focused: bool,
    on_change: Option<Box<RatioChangeMessage<Message>>>,
}

impl<Message> Splitter<Message> {
    fn new(
        first: ElementHandle<Column>,
        options: SplitPaneOptions,
        ratio: Rc<Cell<f32>>,
        on_change: Option<Box<RatioChangeMessage<Message>>>,
    ) -> Self {
        Self {
            first,
            second: None,
            options,
            ratio,
            drag: None,
            hovered: false,
            focused: false,
            on_change,
        }
    }

    fn set_ratio(&mut self, ratio: f32) {
        if ratio.is_finite() {
            self.ratio.set(ratio.clamp(0.0, 1.0));
        }
    }

    fn legal_ratio(&self, ratio: f32, parent_extent: f32) -> f32 {
        let available = (parent_extent - self.options.divider_size).max(1.0);
        let minimum = (self.options.first_min / available).clamp(0.0, 1.0);
        let maximum = (1.0 - self.options.second_min / available).clamp(0.0, 1.0);
        if minimum <= maximum {
            ratio.clamp(minimum, maximum)
        } else {
            (self.options.first_min / (self.options.first_min + self.options.second_min).max(1.0))
                .clamp(0.0, 1.0)
        }
    }

    fn update_layout(&self, context: &mut astrelis_ui_core::EventContext<'_, Message>) {
        let Some(second) = self.second else {
            return;
        };
        let ratio = self.ratio.get();
        context.set_layout(
            self.first,
            pane_layout(self.options.axis, ratio, self.options.first_min),
        );
        context.set_layout(
            second,
            pane_layout(self.options.axis, 1.0 - ratio, self.options.second_min),
        );
        context.request_paint();
    }

    fn emit_change(&self, context: &mut astrelis_ui_core::EventContext<'_, Message>) {
        if let Some(on_change) = &self.on_change {
            context.emit(on_change(self.ratio.get()));
        }
    }

    fn parent_extent(
        context: &astrelis_ui_core::EventContext<'_, Message>,
        axis: SplitAxis,
    ) -> f32 {
        context.parent_bounds().map_or(1.0, |bounds| match axis {
            SplitAxis::Horizontal => bounds.size.width,
            SplitAxis::Vertical => bounds.size.height,
        })
    }
}

impl<Message: 'static> Widget<Message> for Splitter<Message> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn event(
        &mut self,
        context: &mut astrelis_ui_core::EventContext<'_, Message>,
        event: &astrelis_ui_core::RoutedEvent,
    ) {
        match &event.kind {
            RoutedEventKind::PointerEntered { .. } => {
                self.hovered = true;
                context.request_paint();
            }
            RoutedEventKind::PointerLeft { .. } if self.drag.is_none() => {
                self.hovered = false;
                context.request_paint();
            }
            RoutedEventKind::FocusChanged(focused) => {
                self.focused = *focused;
                context.request_paint();
            }
            RoutedEventKind::PointerButton {
                device_id,
                position,
                button: PointerButton::Primary,
                state: ElementState::Pressed,
            } => {
                let coordinate = match self.options.axis {
                    SplitAxis::Horizontal => position.x,
                    SplitAxis::Vertical => position.y,
                };
                self.drag = Some((*device_id, coordinate, self.ratio.get()));
                context.request_focus();
                context.capture_pointer(*device_id);
                context.prevent_default();
                context.request_paint();
            }
            RoutedEventKind::PointerMoved {
                device_id,
                position,
            } if self.drag.is_some_and(|drag| drag.0 == *device_id) => {
                let (_, start, start_ratio) = self.drag.expect("drag was matched above");
                let coordinate = match self.options.axis {
                    SplitAxis::Horizontal => position.x,
                    SplitAxis::Vertical => position.y,
                };
                let extent = Self::parent_extent(context, self.options.axis);
                let available = (extent - self.options.divider_size).max(1.0);
                self.ratio
                    .set(self.legal_ratio(start_ratio + (coordinate - start) / available, extent));
                self.update_layout(context);
                self.emit_change(context);
            }
            RoutedEventKind::PointerButton {
                device_id,
                button: PointerButton::Primary,
                state: ElementState::Released,
                ..
            }
            | RoutedEventKind::PointerCancelled { device_id }
                if self.drag.is_some_and(|drag| drag.0 == *device_id) =>
            {
                self.drag = None;
                self.hovered = false;
                context.release_pointer(*device_id);
                context.prevent_default();
                context.request_paint();
            }
            RoutedEventKind::Keyboard(input) if input.state == ElementState::Pressed => {
                let Key::Named(NamedKey::Other(key)) = &input.logical_key else {
                    return;
                };
                let direction = match (self.options.axis, key.as_str()) {
                    (SplitAxis::Horizontal, "ArrowLeft") | (SplitAxis::Vertical, "ArrowUp") => {
                        Some(-1.0)
                    }
                    (SplitAxis::Horizontal, "ArrowRight") | (SplitAxis::Vertical, "ArrowDown") => {
                        Some(1.0)
                    }
                    _ => None,
                };
                let extent = Self::parent_extent(context, self.options.axis);
                let next = match key.as_str() {
                    "Home" => 0.0,
                    "End" => 1.0,
                    _ if direction.is_some() => {
                        let multiplier = if context.modifiers().shift { 5.0 } else { 1.0 };
                        self.ratio.get()
                            + direction.expect("direction was checked")
                                * self.options.keyboard_step
                                * multiplier
                    }
                    _ => return,
                };
                self.ratio.set(self.legal_ratio(next, extent));
                self.update_layout(context);
                self.emit_change(context);
                context.prevent_default();
            }
            _ => {}
        }
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn focusable(&self) -> bool {
        true
    }

    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(match self.options.axis {
            SplitAxis::Horizontal => CursorIcon::EwResize,
            SplitAxis::Vertical => CursorIcon::NsResize,
        })
    }

    fn paint(
        &self,
        painter: &mut Painter,
        bounds: LogicalRect,
        theme: &Theme,
    ) -> Result<(), UiError> {
        let visual_size = self
            .options
            .divider_visual_size
            .min(self.options.divider_size);
        let visual_bounds = match self.options.axis {
            SplitAxis::Horizontal => LogicalRect::from_xywh(
                bounds.origin.x + (bounds.size.width - visual_size) * 0.5,
                bounds.origin.y,
                visual_size,
                bounds.size.height,
            ),
            SplitAxis::Vertical => LogicalRect::from_xywh(
                bounds.origin.x,
                bounds.origin.y + (bounds.size.height - visual_size) * 0.5,
                bounds.size.width,
                visual_size,
            ),
        };
        painter.fill_rect(
            visual_bounds,
            Brush::Solid(if self.hovered || self.focused || self.drag.is_some() {
                theme.accent
            } else {
                theme.button.hovered
            }),
        )?;
        Ok(())
    }

    fn semantics(&self) -> Option<(SemanticRole, String, Option<String>)> {
        Some((
            SemanticRole::Separator,
            "Resize panes".into(),
            Some(format!("{:.0}%", self.ratio.get() * 100.0)),
        ))
    }

    fn semantic_actions(&self) -> Vec<SemanticActionKind> {
        vec![SemanticActionKind::Focus, SemanticActionKind::SetValue]
    }

    fn semantic_action(
        &mut self,
        context: &mut astrelis_ui_core::EventContext<'_, Message>,
        action: &SemanticAction,
    ) -> bool {
        match action {
            SemanticAction::Focus => {
                context.request_focus();
                true
            }
            SemanticAction::SetValue(value) if value.is_finite() => {
                let extent = Self::parent_extent(context, self.options.axis);
                self.ratio.set(self.legal_ratio(*value, extent));
                self.update_layout(context);
                self.emit_change(context);
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use astrelis_core::geometry::Size;
    use astrelis_text::FontDatabase;
    use astrelis_ui_core::{SemanticAction, SemanticActionKind, SemanticRole};

    use super::*;

    #[test]
    fn split_pane_semantics_resize_regions_and_respect_minima() {
        let mut ui: Ui = Ui::new(FontDatabase::default(), Theme::default());
        ui.set_viewport(Size::new(600.0, 300.0), 1.0);
        let root = ui.root();
        let split = SplitPane::new(
            &mut ui,
            root,
            SplitPaneOptions {
                first_min: 120.0,
                second_min: 150.0,
                divider_size: 10.0,
                ..Default::default()
            },
        )
        .unwrap();
        split
            .set_container_layout(
                &mut ui,
                LayoutStyle {
                    width: Length::Px(500.0),
                    height: Length::Px(200.0),
                    ..Default::default()
                },
            )
            .unwrap();
        ui.display_list().unwrap();

        let semantics = ui.semantic_tree().unwrap();
        let separator = semantics
            .children
            .iter()
            .flat_map(|node| &node.children)
            .find(|node| node.id == split.divider().id())
            .unwrap();
        assert_eq!(separator.role, SemanticRole::Separator);
        assert!(separator.actions.contains(&SemanticActionKind::SetValue));

        ui.perform_semantic_action(split.divider().id(), SemanticAction::SetValue(0.0))
            .unwrap();
        ui.display_list().unwrap();
        let first = ui.layout_bounds(split.first()).unwrap();
        let second = ui.layout_bounds(split.second()).unwrap();
        assert!(first.size.width >= 120.0);
        assert!(second.size.width >= 150.0);
        assert!(split.ratio() > 0.0);

        split.set_ratio(&mut ui, 0.7).unwrap();
        ui.display_list().unwrap();
        let resized = ui.layout_bounds(split.first()).unwrap();
        assert!(resized.size.width > first.size.width);
    }

    #[test]
    fn vertical_split_uses_height_constraints() {
        let mut ui: Ui = Ui::new(FontDatabase::default(), Theme::default());
        ui.set_viewport(Size::new(400.0, 500.0), 1.0);
        let root = ui.root();
        let split = SplitPane::new(
            &mut ui,
            root,
            SplitPaneOptions {
                axis: SplitAxis::Vertical,
                ratio: 0.6,
                first_min: 90.0,
                second_min: 110.0,
                ..Default::default()
            },
        )
        .unwrap();
        split
            .set_container_layout(
                &mut ui,
                LayoutStyle {
                    width: Length::Px(300.0),
                    height: Length::Px(400.0),
                    ..Default::default()
                },
            )
            .unwrap();
        ui.display_list().unwrap();
        assert!(ui.layout_bounds(split.first()).unwrap().size.height > 90.0);
        assert!(ui.layout_bounds(split.second()).unwrap().size.height > 110.0);
    }

    #[test]
    fn split_pane_emits_ratio_changes_from_semantic_resizing() {
        let mut ui: Ui<f32> = Ui::new(FontDatabase::default(), Theme::default());
        ui.set_viewport(Size::new(500.0, 300.0), 1.0);
        let root = ui.root();
        let split =
            SplitPane::new_with_on_change(&mut ui, root, SplitPaneOptions::default(), |ratio| {
                ratio
            })
            .unwrap();
        split
            .set_container_layout(
                &mut ui,
                LayoutStyle {
                    width: Length::Px(400.0),
                    height: Length::Px(200.0),
                    ..Default::default()
                },
            )
            .unwrap();
        ui.display_list().unwrap();
        ui.perform_semantic_action(split.divider().id(), SemanticAction::SetValue(0.7))
            .unwrap();
        let messages = ui.drain_messages().collect::<Vec<_>>();
        assert_eq!(messages, vec![split.ratio()]);
    }
}
