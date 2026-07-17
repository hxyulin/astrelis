//! Routed event types, listeners, and the listener-facing context.

use super::*;

/// Phase of a routed UI event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventPhase {
    /// Routing from the root toward the target.
    Capture,
    /// Dispatch at the target.
    Target,
    /// Routing from the target back toward the root.
    Bubble,
}

/// Public category used to filter routed events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventFilter {
    /// Every routed event.
    Any,
    /// A control was activated.
    Activate,
    /// A control value changed.
    ValueChanged,
    /// Keyboard focus changed.
    Focus,
    /// Pointer input.
    Pointer,
    /// Keyboard input.
    Keyboard,
    /// Wheel or trackpad scrolling.
    Scroll,
    /// In-process drag-and-drop lifecycle events.
    Drag,
}

bitflags! {
    /// Operations a drag source permits a drop target to select.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct DragOperations: u8 {
        /// Copy the dragged value.
        const COPY = 1 << 0;
        /// Move the dragged value.
        const MOVE = 1 << 1;
        /// Create a logical link to the dragged value.
        const LINK = 1 << 2;
    }
}

/// Operation selected by a drop target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropOperation {
    /// Copy the dragged value.
    Copy,
    /// Move the dragged value.
    Move,
    /// Create a logical link to the dragged value.
    Link,
}

impl DropOperation {
    pub(crate) fn flag(self) -> DragOperations {
        match self {
            Self::Copy => DragOperations::COPY,
            Self::Move => DragOperations::MOVE,
            Self::Link => DragOperations::LINK,
        }
    }
}

/// Cloneable, type-erased data carried by one in-process drag session.
#[derive(Clone)]
pub struct DragPayload(Arc<dyn Any>);

impl DragPayload {
    /// Erases a typed payload for transport through routed drag events.
    pub fn new<T: Any>(value: T) -> Self {
        Self(Arc::new(value))
    }

    /// Reads the payload when its concrete type is `T`.
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.0.downcast_ref()
    }
}

impl fmt::Debug for DragPayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DragPayload")
            .finish_non_exhaustive()
    }
}

impl PartialEq for DragPayload {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// Stable identity of one drag session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DragSessionId(pub(crate) u64);

/// Configuration supplied when a possible drag begins.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragOptions {
    /// Logical movement required before the drag becomes active.
    pub threshold: f32,
    /// Operations a target may select.
    pub allowed: DragOperations,
}

impl Default for DragOptions {
    fn default() -> Self {
        Self {
            threshold: 4.0,
            allowed: DragOperations::COPY | DragOperations::MOVE,
        }
    }
}

/// Completion state reported to the drag source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragOutcome {
    /// The drag was cancelled or released without an accepting target.
    Cancelled,
    /// A target accepted the drop using the contained operation.
    Dropped(DropOperation),
}

/// Data delivered to a routed listener.
#[derive(Clone, Debug, PartialEq)]
pub enum RoutedEventKind {
    /// A control was activated.
    Activate,
    /// A checkbox changed.
    CheckedChanged(bool),
    /// A slider changed.
    SliderChanged(f32),
    /// Editable text changed.
    TextChanged(String),
    /// Editable text was submitted.
    TextSubmitted(String),
    /// Keyboard focus changed.
    FocusChanged(bool),
    /// A pointer moved in logical coordinates.
    PointerMoved {
        /// Normalized pointer identity.
        device_id: DeviceId,
        /// Logical window position.
        position: LogicalPoint,
    },
    /// A pointer entered a target's hover path.
    PointerEntered {
        /// Normalized pointer identity.
        device_id: DeviceId,
        /// Logical window position.
        position: LogicalPoint,
        /// Previous deepest hovered target.
        related_target: Option<ElementId>,
    },
    /// A pointer left a target's hover path.
    PointerLeft {
        /// Normalized pointer identity.
        device_id: DeviceId,
        /// Logical window position.
        position: LogicalPoint,
        /// New deepest hovered target.
        related_target: Option<ElementId>,
    },
    /// A pointer button changed in logical coordinates.
    PointerButton {
        /// Normalized pointer identity.
        device_id: DeviceId,
        /// Logical window position.
        position: LogicalPoint,
        /// Changed button.
        button: PointerButton,
        /// New button state.
        state: ElementState,
    },
    /// A captured pointer or touch contact was cancelled.
    PointerCancelled {
        /// Cancelled pointer identity.
        device_id: DeviceId,
    },
    /// Keyboard input.
    Keyboard(KeyboardInput),
    /// Input-method composition.
    Ime(ImeEvent),
    /// Scrolling input.
    Scroll {
        /// Scrolling device.
        device_id: DeviceId,
        /// Logical pixel displacement.
        delta: LogicalPoint,
    },
    /// A drag crossed its activation threshold.
    DragStarted {
        /// Drag identity.
        session: DragSessionId,
        /// Normalized pointer identity.
        device_id: DeviceId,
        /// Logical pointer position.
        position: LogicalPoint,
        /// Type-erased application payload.
        payload: DragPayload,
        /// Operations permitted by the source.
        allowed: DragOperations,
    },
    /// An active drag entered a candidate target route.
    DragEntered {
        /// Drag identity.
        session: DragSessionId,
        /// Normalized pointer identity.
        device_id: DeviceId,
        /// Logical pointer position.
        position: LogicalPoint,
        /// Type-erased application payload.
        payload: DragPayload,
        /// Operations permitted by the source.
        allowed: DragOperations,
    },
    /// An active drag moved over a candidate target route.
    DragOver {
        /// Drag identity.
        session: DragSessionId,
        /// Normalized pointer identity.
        device_id: DeviceId,
        /// Logical pointer position.
        position: LogicalPoint,
        /// Type-erased application payload.
        payload: DragPayload,
        /// Operations permitted by the source.
        allowed: DragOperations,
    },
    /// An active drag left its previous candidate target route.
    DragLeft {
        /// Drag identity.
        session: DragSessionId,
        /// Normalized pointer identity.
        device_id: DeviceId,
        /// Logical pointer position.
        position: LogicalPoint,
        /// Type-erased application payload.
        payload: DragPayload,
    },
    /// An accepted payload was dropped on a target.
    Dropped {
        /// Drag identity.
        session: DragSessionId,
        /// Normalized pointer identity.
        device_id: DeviceId,
        /// Logical pointer position.
        position: LogicalPoint,
        /// Type-erased application payload.
        payload: DragPayload,
        /// Operation selected by the target.
        operation: DropOperation,
    },
    /// A drag finished and reports its outcome to the source.
    DragEnded {
        /// Drag identity.
        session: DragSessionId,
        /// Normalized pointer identity.
        device_id: DeviceId,
        /// Final outcome.
        outcome: DragOutcome,
    },
}

impl RoutedEventKind {
    pub(crate) fn matches(&self, filter: EventFilter) -> bool {
        filter == EventFilter::Any
            || matches!(
                (filter, self),
                (EventFilter::Activate, Self::Activate)
                    | (
                        EventFilter::ValueChanged,
                        Self::CheckedChanged(_) | Self::SliderChanged(_) | Self::TextChanged(_)
                    )
                    | (EventFilter::Focus, Self::FocusChanged(_))
                    | (
                        EventFilter::Pointer,
                        Self::PointerMoved { .. }
                            | Self::PointerEntered { .. }
                            | Self::PointerLeft { .. }
                            | Self::PointerButton { .. }
                            | Self::PointerCancelled { .. }
                    )
                    | (EventFilter::Keyboard, Self::Keyboard(_) | Self::Ime(_))
                    | (EventFilter::Scroll, Self::Scroll { .. })
                    | (
                        EventFilter::Drag,
                        Self::DragStarted { .. }
                            | Self::DragEntered { .. }
                            | Self::DragOver { .. }
                            | Self::DragLeft { .. }
                            | Self::Dropped { .. }
                            | Self::DragEnded { .. }
                    )
            )
    }
}

/// One event as observed at a node along its route.
#[derive(Clone, Debug, PartialEq)]
pub struct RoutedEvent {
    /// Original target.
    pub target: ElementId,
    /// Node currently receiving the event.
    pub current_target: ElementId,
    /// Current routing phase.
    pub phase: EventPhase,
    /// Event payload.
    pub kind: RoutedEventKind,
}

/// Opaque identity for an installed event listener.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ListenerId(pub(crate) u64);

/// Context passed to routed listeners.
pub struct EventContext<'a, Message> {
    pub(crate) messages: &'a mut VecDeque<Message>,
    pub(crate) stopped: &'a mut bool,
    pub(crate) default_prevented: &'a mut bool,
    pub(crate) current_target: ElementId,
    pub(crate) current_bounds: LogicalRect,
    pub(crate) current_world_transform: Affine2,
    pub(crate) parent_bounds: Option<LogicalRect>,
    pub(crate) modifiers: Modifiers,
    pub(crate) route: &'a [ElementId],
    pub(crate) requests: &'a mut Vec<EventRequest>,
}

pub(crate) enum EventRequest {
    Focus(ElementId),
    Capture(DeviceId, ElementId),
    Release(DeviceId),
    Layout,
    Paint,
    SetLayout(ElementId, LayoutStyle),
    SetVisibility(ElementId, Visibility),
    SetScrollOffset(ElementId, f32),
    BeginDrag {
        device_id: DeviceId,
        source: ElementId,
        position: LogicalPoint,
        payload: DragPayload,
        options: DragOptions,
    },
    AcceptDrop {
        device_id: DeviceId,
        target: ElementId,
        operation: DropOperation,
    },
    CancelDrag(DeviceId),
}

impl<Message> EventContext<'_, Message> {
    /// Returns the current listener node's logical layout bounds.
    pub const fn bounds(&self) -> LogicalRect {
        self.current_bounds
    }
    /// Converts a logical window position into coordinates relative to this element.
    pub fn window_to_local(&self, position: LogicalPoint) -> Option<LogicalPoint> {
        let determinant = self.current_world_transform.matrix2.determinant();
        if !determinant.is_finite() || determinant.abs() <= f32::EPSILON {
            return None;
        }
        let point = self
            .current_world_transform
            .inverse()
            .transform_point2(Vec2::new(position.x, position.y));
        Some(Point::new(
            point.x - self.current_bounds.origin.x,
            point.y - self.current_bounds.origin.y,
        ))
    }
    /// Returns the current listener node's parent layout bounds.
    pub const fn parent_bounds(&self) -> Option<LogicalRect> {
        self.parent_bounds
    }
    /// Returns the modifier state current during dispatch.
    pub const fn modifiers(&self) -> Modifiers {
        self.modifiers
    }
    /// Returns whether the current event route passes through `handle`.
    pub fn route_contains<T>(&self, handle: ElementHandle<T>) -> bool {
        self.route.contains(&handle.id)
    }
    /// Emits an application message after dispatch.
    pub fn emit(&mut self, message: Message) {
        self.messages.push_back(message);
    }
    /// Stops delivery to the rest of the route.
    pub fn stop_propagation(&mut self) {
        *self.stopped = true;
    }
    /// Prevents the control's queued default action.
    pub fn prevent_default(&mut self) {
        *self.default_prevented = true;
    }
    /// Requests keyboard focus for the listener's current node.
    pub fn request_focus(&mut self) {
        self.requests.push(EventRequest::Focus(self.current_target));
    }
    /// Requests keyboard focus for another retained element after dispatch.
    pub fn request_focus_for<T>(&mut self, handle: ElementHandle<T>) {
        self.requests.push(EventRequest::Focus(handle.id));
    }
    /// Captures a normalized pointer for the listener's current node.
    pub fn capture_pointer(&mut self, device_id: DeviceId) {
        self.requests
            .push(EventRequest::Capture(device_id, self.current_target));
    }
    /// Releases a normalized pointer capture.
    pub fn release_pointer(&mut self, device_id: DeviceId) {
        self.requests.push(EventRequest::Release(device_id));
    }
    /// Invalidates measurement and layout after dispatch.
    pub fn request_layout(&mut self) {
        self.requests.push(EventRequest::Layout);
    }
    /// Invalidates painting after dispatch.
    pub fn request_paint(&mut self) {
        self.requests.push(EventRequest::Paint);
    }
    /// Defers a layout-style change until routed dispatch completes.
    pub fn set_layout<T>(&mut self, handle: ElementHandle<T>, style: LayoutStyle) {
        self.requests
            .push(EventRequest::SetLayout(handle.id, style));
    }
    /// Defers a layout-style change for the listener's current node.
    pub fn set_current_layout(&mut self, style: LayoutStyle) {
        self.requests
            .push(EventRequest::SetLayout(self.current_target, style));
    }
    /// Defers a visibility change until routed dispatch completes.
    pub fn set_visibility<T>(&mut self, handle: ElementHandle<T>, visibility: Visibility) {
        self.requests
            .push(EventRequest::SetVisibility(handle.id, visibility));
    }
    /// Defers a vertical scroll-position change until routed dispatch completes.
    pub fn set_scroll_offset<T>(&mut self, handle: ElementHandle<T>, offset: f32) {
        self.requests
            .push(EventRequest::SetScrollOffset(handle.id, offset));
    }
    /// Arms an in-process drag which activates after its movement threshold.
    pub fn begin_drag(
        &mut self,
        device_id: DeviceId,
        position: LogicalPoint,
        payload: DragPayload,
        options: DragOptions,
    ) {
        self.requests.push(EventRequest::BeginDrag {
            device_id,
            source: self.current_target,
            position,
            payload,
            options,
        });
    }
    /// Accepts the current drag using one of its allowed operations.
    pub fn accept_drop(&mut self, device_id: DeviceId, operation: DropOperation) {
        self.requests.push(EventRequest::AcceptDrop {
            device_id,
            target: self.current_target,
            operation,
        });
    }
    /// Cancels a pending or active drag for a normalized pointer.
    pub fn cancel_drag(&mut self, device_id: DeviceId) {
        self.requests.push(EventRequest::CancelDrag(device_id));
    }
}

impl<Message: 'static> Ui<Message> {
    /// Returns whether painting is currently invalidated.
    pub fn needs_redraw(&self) -> bool {
        self.dirty
            .intersects(Dirty::MEASURE | Dirty::LAYOUT | Dirty::PAINT)
    }

    /// Drains application-visible UI events.
    pub fn drain_events(&mut self) -> impl Iterator<Item = UiEvent> + '_ {
        self.events.drain(..)
    }

    /// Drains typed application messages emitted by routed listeners.
    pub fn drain_messages(&mut self) -> impl Iterator<Item = Message> + '_ {
        self.messages.drain(..)
    }

    /// Installs a listener on one retained node.
    pub fn listen<T>(
        &mut self,
        handle: ElementHandle<T>,
        phase: Option<EventPhase>,
        filter: EventFilter,
        listener: impl FnMut(&mut EventContext<'_, Message>, &RoutedEvent) + 'static,
    ) -> Result<ListenerId, UiError> {
        self.node(handle.id)?;
        let id = ListenerId(self.next_listener);
        self.next_listener = self.next_listener.wrapping_add(1).max(1);
        self.listeners.entry(handle.id).or_default().push(Listener {
            id,
            phase,
            filter,
            callback: Box::new(listener),
        });
        Ok(id)
    }

    /// Removes a previously installed listener.
    pub fn remove_listener(&mut self, id: ListenerId) -> bool {
        let mut removed = false;
        for listeners in self.listeners.values_mut() {
            let before = listeners.len();
            listeners.retain(|listener| listener.id != id);
            removed |= listeners.len() != before;
        }
        removed
    }

    pub(crate) fn dispatch_routed(
        &mut self,
        target: ElementId,
        kind: RoutedEventKind,
    ) -> Result<bool, UiError> {
        astrelis_profiling::profile_scope!("ui.dispatch");
        let mut route = Vec::new();
        let mut cursor = Some(target);
        while let Some(id) = cursor {
            route.push(id);
            cursor = self.node(id)?.parent;
        }
        route.reverse();
        let mut control = DispatchControl {
            route: &route,
            stopped: false,
            default_prevented: false,
        };
        let last = route.len().saturating_sub(1);
        for (index, current) in route.iter().copied().enumerate() {
            let phase = if index == last {
                EventPhase::Target
            } else {
                EventPhase::Capture
            };
            self.deliver(current, target, phase, &kind, &mut control);
            if control.stopped {
                break;
            }
        }
        if !control.stopped {
            for current in route[..last].iter().rev().copied() {
                self.deliver(current, target, EventPhase::Bubble, &kind, &mut control);
                if control.stopped {
                    break;
                }
            }
        }
        self.apply_event_requests()?;
        Ok(control.default_prevented)
    }

    pub(crate) fn deliver(
        &mut self,
        current: ElementId,
        target: ElementId,
        phase: EventPhase,
        kind: &RoutedEventKind,
        control: &mut DispatchControl<'_>,
    ) {
        let current_bounds = self
            .node(current)
            .map_or(LogicalRect::default(), |node| node.bounds);
        let parent_bounds = self
            .node(current)
            .ok()
            .and_then(|node| node.parent)
            .and_then(|parent| self.node(parent).ok())
            .map(|parent| parent.bounds);
        let current_world_transform = self
            .world_transform_for(current)
            .unwrap_or(Affine2::IDENTITY);
        let event = RoutedEvent {
            target,
            current_target: current,
            phase,
            kind: kind.clone(),
        };
        if let Some(mut widget) = self.custom_widgets.remove(&current) {
            let mut context = EventContext {
                messages: &mut self.messages,
                stopped: &mut control.stopped,
                default_prevented: &mut control.default_prevented,
                current_target: current,
                current_bounds,
                current_world_transform,
                parent_bounds,
                modifiers: self.modifiers,
                route: control.route,
                requests: &mut self.event_requests,
            };
            widget.event(&mut context, &event);
            self.custom_widgets.insert(current, widget);
            if control.stopped {
                return;
            }
        }
        let Some(mut listeners) = self.listeners.remove(&current) else {
            return;
        };
        for listener in &mut listeners {
            if listener.phase.is_none_or(|wanted| wanted == phase) && kind.matches(listener.filter)
            {
                let mut context = EventContext {
                    messages: &mut self.messages,
                    stopped: &mut control.stopped,
                    default_prevented: &mut control.default_prevented,
                    current_target: current,
                    current_bounds,
                    current_world_transform,
                    parent_bounds,
                    modifiers: self.modifiers,
                    route: control.route,
                    requests: &mut self.event_requests,
                };
                (listener.callback)(&mut context, &event);
                if control.stopped {
                    break;
                }
            }
        }
        self.listeners.insert(current, listeners);
    }

    pub(crate) fn apply_event_requests(&mut self) -> Result<(), UiError> {
        let mut cancellations = Vec::new();
        for request in std::mem::take(&mut self.event_requests) {
            match request {
                EventRequest::Focus(id) => self.set_focus(Some(id))?,
                EventRequest::Capture(device, id) => {
                    self.capture.insert(device, id);
                }
                EventRequest::Release(device) => {
                    self.capture.remove(&device);
                }
                EventRequest::Layout => self.invalidate_layout(),
                EventRequest::Paint => self.dirty |= Dirty::PAINT,
                EventRequest::SetLayout(id, style) => {
                    self.set_layout(
                        ElementHandle::<()> {
                            id,
                            marker: PhantomData,
                        },
                        style,
                    )?;
                }
                EventRequest::SetVisibility(id, visibility) => {
                    self.set_visibility(
                        ElementHandle::<()> {
                            id,
                            marker: PhantomData,
                        },
                        visibility,
                    )?;
                }
                EventRequest::SetScrollOffset(id, offset) => {
                    self.set_scroll_offset_id(id, offset)?;
                }
                EventRequest::BeginDrag {
                    device_id,
                    source,
                    position,
                    payload,
                    mut options,
                } => {
                    options.threshold = options.threshold.max(0.0);
                    if !options.allowed.is_empty() {
                        let id = DragSessionId(self.next_drag_session);
                        self.next_drag_session = self.next_drag_session.wrapping_add(1).max(1);
                        self.drag_sessions.insert(
                            device_id,
                            DragSession {
                                id,
                                source,
                                payload,
                                options,
                                start: position,
                                active: false,
                                candidate: None,
                                accepted: None,
                            },
                        );
                    }
                }
                EventRequest::AcceptDrop {
                    device_id,
                    target,
                    operation,
                } => {
                    if self.drop_acceptance.is_none() {
                        self.drop_acceptance = Some((device_id, target, operation));
                    }
                }
                EventRequest::CancelDrag(device_id) => cancellations.push(device_id),
            }
        }
        for device_id in cancellations {
            self.cancel_drag_id(device_id)?;
        }
        Ok(())
    }
}
