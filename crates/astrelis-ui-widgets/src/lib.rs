//! Reusable controls and compositions built exclusively on `astrelis-ui-core`.

#![warn(missing_docs)]

use std::any::Any;

use astrelis_core::geometry::{LogicalRect, LogicalSize, Size};
use astrelis_paint::{Brush, CornerRadii, Painter, RoundedRect, StrokeStyle};
use astrelis_platform::ElementState;
use astrelis_ui_core::{
    DragOperations, DragOptions, DragPayload, DropOperation, ElementHandle, EventFilter,
    ListenerId, MountContext, RoutedEventKind, SemanticRole, Theme, Ui, UiError, Widget,
};

type PayloadAcceptance = dyn Fn(&DragPayload) -> bool;
type DropMessage<Message> = dyn Fn(&DragPayload, DropOperation) -> Message;

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
        let rounded = RoundedRect::new(bounds, CornerRadii::uniform(theme.corner_radius * 1.5))
            .map_err(|error| UiError::from_message(error.to_string()))?;
        let background = if self.hovering {
            theme.button.pressed
        } else {
            theme.field_background
        };
        painter
            .fill_rounded_rect(rounded, Brush::Solid(background))
            .map_err(|error| UiError::from_message(error.to_string()))?;
        painter
            .stroke_rounded_rect(
                rounded,
                StrokeStyle {
                    width: 2.0,
                    ..Default::default()
                },
                Brush::Solid(if self.hovering {
                    theme.accent
                } else {
                    theme.button.hovered
                }),
            )
            .map_err(|error| UiError::from_message(error.to_string()))
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
