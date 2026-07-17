//! Retained, backend-independent UI tree, layout, events, semantics, and paint.

#![warn(missing_docs)]

use std::{
    any::Any,
    collections::{HashMap, VecDeque},
    error::Error,
    fmt,
    marker::PhantomData,
    sync::Arc,
};

use astrelis_core::{
    color::Color,
    geometry::{LogicalPoint, LogicalRect, LogicalSize, PhysicalRect, Point, Rect, Size},
    math::{Affine2, Vec2},
};
use astrelis_paint::{Brush, CornerRadii, DisplayList, Painter, RoundedRect, StrokeStyle};
use astrelis_platform::{
    Clipboard, CursorIcon, DeviceId, ElementState, ImeEvent, ImePurpose, Key, KeyboardInput,
    Modifiers, NamedKey, PlatformError, PointerButton, ScrollDelta, TouchPhase, Window,
    WindowEvent,
};
use astrelis_text::{
    Affinity, CaretMovement, FontDatabase, FontFamily, ParagraphStyle, TextLayout,
    TextLayoutContext, TextLayoutRequest, TextPosition, TextWrap,
};
use bitflags::bitflags;
use taffy::prelude::{
    AlignContent, AlignItems, AvailableSpace, Dimension, Display, FlexDirection,
    FlexWrap as TaffyFlexWrap, JustifyContent, LengthPercentage, LengthPercentageAuto, NodeId,
    Position as TaffyPosition, Rect as TaffyRect, Size as TaffySize, Style, TaffyTree,
};
use taffy::style::Overflow as TaffyOverflow;
use unicode_segmentation::UnicodeSegmentation;

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Dirty: u8 {
        const MEASURE = 1 << 0;
        const LAYOUT = 1 << 1;
        const PAINT = 1 << 2;
        const SEMANTICS = 1 << 3;
    }
}

/// Error produced by tree, layout, text, or paint operations.
#[derive(Debug)]
pub struct UiError(String);

impl UiError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }

    /// Creates an error reported by an application-defined widget.
    pub fn from_message(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for UiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for UiError {}

/// Erased generational element identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ElementId {
    index: u32,
    generation: u32,
}

/// Typed generational handle to a retained element.
pub struct ElementHandle<T> {
    id: ElementId,
    marker: PhantomData<fn() -> T>,
}

impl<T> ElementHandle<T> {
    /// Returns the erased element identity.
    pub const fn id(self) -> ElementId {
        self.id
    }
}

impl<T> Clone for ElementHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for ElementHandle<T> {}

impl<T> fmt::Debug for ElementHandle<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ElementHandle")
            .field(&self.id)
            .finish()
    }
}

/// Label widget marker.
pub enum Label {}
/// Button widget marker.
pub enum Button {}
/// Horizontal flex container marker.
pub enum Row {}
/// Vertical flex container marker.
pub enum Column {}
/// Padding container marker.
pub enum Padding {}
/// Single-line editable text-field marker.
pub enum TextField {}

/// Checkbox widget marker.
pub enum Checkbox {}
/// Horizontal slider widget marker.
pub enum Slider {}
/// Vertically scrolling container marker.
pub enum ScrollView {}
/// Overlaying stack container marker.
pub enum Stack {}
/// Keyboard focus scope marker.
pub enum FocusScope {}
/// Viewport-hosted portal marker.
pub enum Overlay {}

/// Keyboard behavior of a focus scope.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FocusScopeOptions {
    /// Keep Tab traversal inside this scope.
    pub trapped: bool,
    /// Focus the first eligible descendant when mounted.
    pub autofocus: bool,
    /// Restore the prior focus when removed.
    pub restore_focus: bool,
}

/// Side of an anchor used to place an overlay.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverlaySide {
    /// Below the anchor.
    #[default]
    Below,
    /// Above the anchor.
    Above,
    /// Left of the anchor.
    Left,
    /// Right of the anchor.
    Right,
    /// Centered over the anchor.
    Center,
}

/// Alignment along an overlay anchor's side.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverlayAlignment {
    /// Align leading edges.
    #[default]
    Start,
    /// Center along the side.
    Center,
    /// Align trailing edges.
    End,
}

/// Viewport-hosted overlay configuration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlayOptions {
    /// Preferred side of the anchor.
    pub side: OverlaySide,
    /// Alignment along the chosen side.
    pub alignment: OverlayAlignment,
    /// Additional logical offset.
    pub offset: LogicalPoint,
    /// Keep the overlay inside the viewport.
    pub clamp_to_viewport: bool,
    /// Top-layer ordering.
    pub z_index: i32,
    /// Optional focus-scope behavior.
    pub focus: FocusScopeOptions,
}

impl Default for OverlayOptions {
    fn default() -> Self {
        Self {
            side: OverlaySide::Below,
            alignment: OverlayAlignment::Start,
            offset: LogicalPoint::ZERO,
            clamp_to_viewport: true,
            z_index: 0,
            focus: FocusScopeOptions::default(),
        }
    }
}

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
    fn flag(self) -> DragOperations {
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
pub struct DragSessionId(u64);

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
    fn matches(&self, filter: EventFilter) -> bool {
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
pub struct ListenerId(u64);

/// Context passed to routed listeners.
pub struct EventContext<'a, Message> {
    messages: &'a mut VecDeque<Message>,
    stopped: &'a mut bool,
    default_prevented: &'a mut bool,
    current_target: ElementId,
    current_bounds: LogicalRect,
    current_world_transform: Affine2,
    parent_bounds: Option<LogicalRect>,
    modifiers: Modifiers,
    route: &'a [ElementId],
    requests: &'a mut Vec<EventRequest>,
}

enum EventRequest {
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
    ui: &'a mut Ui<Message>,
    parent: ElementId,
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

/// Four-sided logical inset.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    /// Left inset.
    pub left: f32,
    /// Top inset.
    pub top: f32,
    /// Right inset.
    pub right: f32,
    /// Bottom inset.
    pub bottom: f32,
}

impl Insets {
    /// Creates equal insets on every side.
    pub const fn all(value: f32) -> Self {
        Self {
            left: value,
            top: value,
            right: value,
            bottom: value,
        }
    }
}

/// Cross-axis alignment for row and column containers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Alignment {
    /// Align children to the leading edge.
    Start,
    /// Center children.
    Center,
    /// Align children to the trailing edge.
    End,
    /// Stretch children across the available cross axis.
    #[default]
    Stretch,
}

/// A layout length resolved by Taffy.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Length {
    /// Let layout determine the value.
    #[default]
    Auto,
    /// Logical pixels.
    Px(f32),
    /// Fraction of the containing block (`1.0` is 100%).
    Percent(f32),
}

impl Length {
    /// Creates a logical-pixel length.
    pub const fn px(value: f32) -> Self {
        Self::Px(value)
    }
    /// Creates a fractional percentage length.
    pub const fn percent(value: f32) -> Self {
        Self::Percent(value)
    }
}

/// Four independently configurable edges.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edges<T> {
    /// Left edge.
    pub left: T,
    /// Top edge.
    pub top: T,
    /// Right edge.
    pub right: T,
    /// Bottom edge.
    pub bottom: T,
}

impl<T: Copy> Edges<T> {
    /// Uses one value for every edge.
    pub const fn all(value: T) -> Self {
        Self {
            left: value,
            top: value,
            right: value,
            bottom: value,
        }
    }
}

impl<T: Default> Default for Edges<T> {
    fn default() -> Self {
        Self {
            left: T::default(),
            top: T::default(),
            right: T::default(),
            bottom: T::default(),
        }
    }
}

/// Whether an element participates in normal flow.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Positioning {
    /// Normal flex layout.
    #[default]
    Flow,
    /// Positioned relative to the containing block.
    Absolute,
}

/// Flex line wrapping policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FlexWrap {
    /// Keep one line.
    #[default]
    NoWrap,
    /// Wrap onto additional lines.
    Wrap,
    /// Wrap in the reverse cross-axis direction.
    WrapReverse,
}

/// Main-axis distribution of flex children.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Justification {
    /// Pack at the start.
    #[default]
    Start,
    /// Pack at the center.
    Center,
    /// Pack at the end.
    End,
    /// Equal space between children.
    SpaceBetween,
    /// Equal space around children.
    SpaceAround,
    /// Equal space around and at the edges.
    SpaceEvenly,
}

/// Flex-container configuration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlexStyle {
    /// Horizontal gap.
    pub column_gap: f32,
    /// Vertical gap.
    pub row_gap: f32,
    /// Cross-axis child alignment.
    pub align_items: Alignment,
    /// Main-axis distribution.
    pub justify_content: Justification,
    /// Wrapped-line distribution.
    pub align_content: Alignment,
    /// Wrapping policy.
    pub wrap: FlexWrap,
}

impl Default for FlexStyle {
    fn default() -> Self {
        Self {
            column_gap: 0.0,
            row_gap: 0.0,
            align_items: Alignment::Stretch,
            justify_content: Justification::Start,
            align_content: Alignment::Stretch,
            wrap: FlexWrap::NoWrap,
        }
    }
}

/// Participation in layout, painting, semantics, and input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Visibility {
    /// Fully visible and interactive.
    #[default]
    Visible,
    /// Retains layout space but is not painted or interactive.
    Hidden,
    /// Removed from layout and interaction.
    Collapsed,
}

/// Child overflow policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Overflow {
    /// Permit descendants outside the element bounds.
    #[default]
    Visible,
    /// Clip descendants to the element bounds.
    Clip,
}

/// Optional per-element sizing constraints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutStyle {
    /// Preferred width.
    pub width: Length,
    /// Preferred height.
    pub height: Length,
    /// Minimum width.
    pub min_width: Length,
    /// Minimum height.
    pub min_height: Length,
    /// Maximum width.
    pub max_width: Length,
    /// Maximum height.
    pub max_height: Length,
    /// Outer spacing.
    pub margin: Edges<Length>,
    /// Flex growth factor.
    pub grow: f32,
    /// Flex shrink factor.
    pub shrink: f32,
    /// Initial main-axis size.
    pub basis: Length,
    /// Per-child cross-axis alignment override.
    pub align_self: Option<Alignment>,
    /// Flow or absolute positioning.
    pub positioning: Positioning,
    /// Absolute-position offsets.
    pub inset: Edges<Length>,
    /// Preferred width divided by height.
    pub aspect_ratio: Option<f32>,
}

impl Default for LayoutStyle {
    fn default() -> Self {
        Self {
            width: Length::Auto,
            height: Length::Auto,
            min_width: Length::Auto,
            min_height: Length::Auto,
            max_width: Length::Auto,
            max_height: Length::Auto,
            margin: Edges::all(Length::Px(0.0)),
            grow: 0.0,
            shrink: 1.0,
            basis: Length::Auto,
            align_self: None,
            positioning: Positioning::Flow,
            inset: Edges::default(),
            aspect_ratio: None,
        }
    }
}

/// Optional direct visual overrides for one widget.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WidgetStyle {
    /// Foreground/text color override.
    pub foreground: Option<Color>,
    /// Background color override.
    pub background: Option<Color>,
}

/// Typed visual style for a checkbox.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CheckboxStyle {
    /// Box background.
    pub background: Color,
    /// Checked indicator.
    pub indicator: Color,
    /// Corner radius.
    pub radius: f32,
}

/// Typed visual style for a horizontal slider.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderStyle {
    /// Track color.
    pub track: Color,
    /// Thumb color.
    pub thumb: Color,
    /// Thumb diameter.
    pub thumb_size: f32,
}

/// Typed visual style for a vertical scroll view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollViewStyle {
    /// Scrollbar track color.
    pub track: Color,
    /// Scrollbar thumb color.
    pub thumb: Color,
    /// Scrollbar width.
    pub width: f32,
}

/// Visual state colors for an interactive control.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlColors {
    /// Normal background.
    pub normal: Color,
    /// Hovered background.
    pub hovered: Color,
    /// Pressed background.
    pub pressed: Color,
    /// Disabled background.
    pub disabled: Color,
}

/// Typed visual tokens used by the built-in widgets.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    /// Window/background color.
    pub background: Color,
    /// Primary foreground color.
    pub foreground: Color,
    /// Muted foreground color.
    pub muted_foreground: Color,
    /// Text selection color.
    pub selection: Color,
    /// Caret and focus-ring color.
    pub accent: Color,
    /// Button state colors.
    pub button: ControlColors,
    /// Text-field background.
    pub field_background: Color,
    /// Default logical font size.
    pub font_size: f32,
    /// Ordered font families used by built-in widget text.
    pub font_families: Vec<FontFamily>,
    /// Default inter-widget gap.
    pub gap: f32,
    /// Default control padding.
    pub control_padding: Insets,
    /// Default corner radius.
    pub corner_radius: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::new(0.035, 0.045, 0.075, 1.0),
            foreground: Color::new(0.92, 0.94, 1.0, 1.0),
            muted_foreground: Color::new(0.58, 0.64, 0.75, 1.0),
            selection: Color::new(0.12, 0.38, 0.68, 0.85),
            accent: Color::new(0.25, 0.75, 1.0, 1.0),
            button: ControlColors {
                normal: Color::new(0.12, 0.16, 0.26, 1.0),
                hovered: Color::new(0.16, 0.22, 0.35, 1.0),
                pressed: Color::new(0.09, 0.35, 0.5, 1.0),
                disabled: Color::new(0.08, 0.09, 0.12, 1.0),
            },
            field_background: Color::new(0.065, 0.08, 0.13, 1.0),
            font_size: 16.0,
            font_families: vec![FontFamily::SansSerif],
            gap: 10.0,
            control_padding: Insets {
                left: 12.0,
                top: 8.0,
                right: 12.0,
                bottom: 8.0,
            },
            corner_radius: 6.0,
        }
    }
}

/// Semantic role exposed by one retained element.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticRole {
    /// Generic grouping container.
    Group,
    /// Static text.
    Label,
    /// Activatable button.
    Button,
    /// Editable single-line text field.
    TextField,
    /// Boolean checkbox.
    Checkbox,
    /// Numeric slider.
    Slider,
    /// Scrollable grouping container.
    ScrollView,
    /// Adjustable divider between two regions.
    Separator,
    /// Explanatory hover or focus content.
    Tooltip,
    /// Popup command collection.
    Menu,
    /// Command inside a menu.
    MenuItem,
    /// Container for tab selectors.
    TabList,
    /// One tab selector.
    Tab,
    /// Content controlled by a tab.
    TabPanel,
    /// Selectable item collection.
    List,
    /// One selectable list entry.
    ListItem,
    /// Group of labeled form controls.
    Form,
}

/// Semantic operation supported by a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticActionKind {
    /// Move keyboard focus to the node.
    Focus,
    /// Activate a button.
    Activate,
    /// Replace editable text.
    SetText,
    /// Change editable selection.
    SetSelection,
    /// Set a numeric value.
    SetValue,
    /// Scroll vertically.
    ScrollBy,
}

/// Requested semantic operation.
#[derive(Clone, Debug, PartialEq)]
pub enum SemanticAction {
    /// Move keyboard focus to the node.
    Focus,
    /// Activate a button.
    Activate,
    /// Replace editable text.
    SetText(String),
    /// Change editable selection using UTF-8 byte indices.
    SetSelection {
        /// Selection anchor byte index.
        anchor: usize,
        /// Selection focus byte index.
        focus: usize,
    },
    /// Sets a numeric control value.
    SetValue(f32),
    /// Scrolls a container by logical units.
    ScrollBy(f32),
}

/// Snapshot-friendly semantic node.
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticNode {
    /// Element identity.
    pub id: ElementId,
    /// Semantic role.
    pub role: SemanticRole,
    /// Logical bounds.
    pub bounds: LogicalRect,
    /// Accessible label.
    pub label: String,
    /// Accessible value.
    pub value: Option<String>,
    /// Whether the element accepts focus.
    pub focusable: bool,
    /// Whether the element currently has focus.
    pub focused: bool,
    /// Whether interaction is enabled.
    pub enabled: bool,
    /// Selected UTF-8 byte range for text fields.
    pub selection: Option<(usize, usize)>,
    /// Operations accepted by this semantic node.
    pub actions: Vec<SemanticActionKind>,
    /// Child semantic nodes.
    pub children: Vec<SemanticNode>,
}

/// Application-visible UI event category.
#[derive(Clone, Debug, PartialEq)]
pub enum UiEventKind {
    /// A button was activated.
    ButtonActivated,
    /// Editable text changed.
    TextChanged(String),
    /// Enter submitted a text field.
    TextSubmitted(String),
    /// Keyboard focus changed.
    FocusChanged(bool),
}

/// Queued application-visible UI event.
#[derive(Clone, Debug, PartialEq)]
pub struct UiEvent {
    /// Target element.
    pub target: ElementId,
    /// Event category.
    pub kind: UiEventKind,
}

impl UiEvent {
    /// Returns whether this event targets a typed handle.
    pub fn is_from<T>(&self, handle: ElementHandle<T>) -> bool {
        self.target == handle.id
    }
}

/// Summary of work caused by one input event or mutation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UiUpdate {
    /// A new display list is required.
    pub redraw: bool,
    /// Focus or IME platform state changed.
    pub platform_state_changed: bool,
}

/// Deterministic headless state for one retained element.
#[derive(Clone, Debug, PartialEq)]
pub struct ElementInspection {
    /// Element identity.
    pub id: ElementId,
    /// Logical parent.
    pub parent: Option<ElementId>,
    /// Untransformed layout bounds.
    pub layout_bounds: LogicalRect,
    /// Axis-aligned transformed bounds.
    pub world_bounds: LogicalRect,
    /// Axis-aligned transformed bounds in physical pixels.
    pub physical_bounds: PhysicalRect,
    /// Effective axis-aligned clip, when present.
    pub clip: Option<LogicalRect>,
    /// Effective clip in physical pixels.
    pub physical_clip: Option<PhysicalRect>,
    /// Composed visual transform.
    pub world_transform: Affine2,
    /// Stable paint rank.
    pub paint_rank: usize,
    /// Local visibility.
    pub visibility: Visibility,
    /// Whether this node and every ancestor are visible.
    pub effectively_visible: bool,
    /// Effective enabled and visible state.
    pub interactive: bool,
    /// Whether any pointer hover path contains this node.
    pub hovered: bool,
    /// Whether this node has keyboard focus.
    pub focused: bool,
    /// Whether this node can receive focus.
    pub focusable: bool,
    /// Whether this node can be a pointer target.
    pub hit_testable: bool,
}

/// Deterministic headless snapshot of a UI tree.
#[derive(Clone, Debug, PartialEq)]
pub struct UiInspection {
    /// Current logical viewport.
    pub viewport: LogicalSize,
    /// Current logical-to-physical scale.
    pub scale_factor: f32,
    /// Nodes in retained-tree order.
    pub nodes: Vec<ElementInspection>,
}

#[derive(Clone, Debug)]
enum Kind {
    Label {
        text: String,
    },
    Button {
        text: String,
    },
    Row {
        flex: FlexStyle,
    },
    Column {
        flex: FlexStyle,
    },
    Stack,
    FocusScope {
        options: FocusScopeOptions,
        restore: Option<ElementId>,
    },
    Overlay {
        owner: ElementId,
        options: OverlayOptions,
        restore: Option<ElementId>,
    },
    Padding {
        insets: Insets,
    },
    TextField(TextFieldState),
    Checkbox {
        checked: bool,
    },
    Slider {
        min: f32,
        max: f32,
        step: f32,
        value: f32,
    },
    ScrollView {
        offset: f32,
        content_height: f32,
    },
    Custom,
}

#[derive(Clone, Debug)]
struct TextFieldState {
    text: String,
    placeholder: String,
    caret: TextPosition,
    anchor: TextPosition,
    preedit: String,
    password: bool,
    horizontal_offset: f32,
}

impl TextFieldState {
    fn new(text: String) -> Self {
        let position = TextPosition {
            byte_index: text.len(),
            affinity: Affinity::Downstream,
        };
        Self {
            text,
            placeholder: String::new(),
            caret: position,
            anchor: position,
            preedit: String::new(),
            password: false,
            horizontal_offset: 0.0,
        }
    }

    fn selection(&self) -> (usize, usize) {
        let a = self.anchor.byte_index.min(self.text.len());
        let b = self.caret.byte_index.min(self.text.len());
        (a.min(b), a.max(b))
    }
}

#[derive(Clone, Debug)]
struct Node {
    parent: Option<ElementId>,
    children: Vec<ElementId>,
    kind: Kind,
    style: LayoutStyle,
    visual: WidgetStyle,
    enabled: bool,
    visibility: Visibility,
    overflow: Overflow,
    z_index: i32,
    transform: Affine2,
    transform_origin: LogicalPoint,
    cursor: Option<CursorIcon>,
    bounds: LogicalRect,
    text_layout: Option<TextLayout>,
    hovered: bool,
    pressed: bool,
}

struct Slot {
    generation: u32,
    node: Option<Node>,
}

struct DragSession {
    id: DragSessionId,
    source: ElementId,
    payload: DragPayload,
    options: DragOptions,
    start: LogicalPoint,
    active: bool,
    candidate: Option<ElementId>,
    accepted: Option<(ElementId, DropOperation)>,
}

/// Persistent UI tree associated with one native window.
pub struct Ui<Message = ()> {
    slots: Vec<Slot>,
    free: Vec<u32>,
    root: ElementId,
    theme: Theme,
    fonts: FontDatabase,
    text_context: TextLayoutContext,
    viewport: LogicalSize,
    scale_factor: f32,
    dirty: Dirty,
    focus: Option<ElementId>,
    hover: Option<ElementId>,
    hover_paths: HashMap<DeviceId, Vec<ElementId>>,
    capture: HashMap<DeviceId, ElementId>,
    pointer_positions: HashMap<DeviceId, LogicalPoint>,
    modifiers: Modifiers,
    window_focused: bool,
    applied_cursor: Option<CursorIcon>,
    events: VecDeque<UiEvent>,
    messages: VecDeque<Message>,
    listeners: HashMap<ElementId, Vec<Listener<Message>>>,
    next_listener: u64,
    custom_widgets: HashMap<ElementId, Box<dyn Widget<Message>>>,
    checkbox_styles: HashMap<ElementId, CheckboxStyle>,
    slider_styles: HashMap<ElementId, SliderStyle>,
    scroll_styles: HashMap<ElementId, ScrollViewStyle>,
    semantic_roles: HashMap<ElementId, SemanticRole>,
    event_requests: Vec<EventRequest>,
    drag_sessions: HashMap<DeviceId, DragSession>,
    next_drag_session: u64,
    drop_acceptance: Option<(DeviceId, ElementId, DropOperation)>,
}

struct Listener<Message> {
    id: ListenerId,
    phase: Option<EventPhase>,
    filter: EventFilter,
    callback: Box<EventCallback<Message>>,
}

type EventCallback<Message> = dyn FnMut(&mut EventContext<'_, Message>, &RoutedEvent);

struct DispatchControl<'a> {
    route: &'a [ElementId],
    stopped: bool,
    default_prevented: bool,
}

impl<Message: 'static> Ui<Message> {
    /// Creates a UI tree with a root column container.
    pub fn new(fonts: FontDatabase, theme: Theme) -> Self {
        let root = ElementId {
            index: 0,
            generation: 1,
        };
        Self {
            slots: vec![Slot {
                generation: 1,
                node: Some(Node {
                    parent: None,
                    children: Vec::new(),
                    kind: Kind::Column {
                        flex: FlexStyle {
                            row_gap: theme.gap,
                            ..Default::default()
                        },
                    },
                    style: LayoutStyle::default(),
                    visual: WidgetStyle::default(),
                    enabled: true,
                    visibility: Visibility::Visible,
                    overflow: Overflow::Visible,
                    z_index: 0,
                    transform: Affine2::IDENTITY,
                    transform_origin: LogicalPoint::ZERO,
                    cursor: None,
                    bounds: Rect::default(),
                    text_layout: None,
                    hovered: false,
                    pressed: false,
                }),
            }],
            free: Vec::new(),
            root,
            theme,
            fonts,
            text_context: TextLayoutContext::new(),
            viewport: Size::ZERO,
            scale_factor: 1.0,
            dirty: Dirty::all(),
            focus: None,
            hover: None,
            hover_paths: HashMap::new(),
            capture: HashMap::new(),
            pointer_positions: HashMap::new(),
            modifiers: Modifiers::default(),
            window_focused: true,
            applied_cursor: None,
            events: VecDeque::new(),
            messages: VecDeque::new(),
            listeners: HashMap::new(),
            next_listener: 1,
            custom_widgets: HashMap::new(),
            checkbox_styles: HashMap::new(),
            slider_styles: HashMap::new(),
            scroll_styles: HashMap::new(),
            semantic_roles: HashMap::new(),
            event_requests: Vec::new(),
            drag_sessions: HashMap::new(),
            next_drag_session: 1,
            drop_acceptance: None,
        }
    }

    /// Returns the typed root column handle.
    pub fn root(&self) -> ElementHandle<Column> {
        ElementHandle {
            id: self.root,
            marker: PhantomData,
        }
    }

    /// Changes the logical viewport and DPI scale.
    pub fn set_viewport(&mut self, viewport: LogicalSize, scale_factor: f32) {
        if self.viewport != viewport || self.scale_factor != scale_factor {
            self.viewport = viewport;
            self.scale_factor = scale_factor.max(f32::EPSILON);
            self.dirty |= Dirty::MEASURE | Dirty::LAYOUT | Dirty::PAINT | Dirty::SEMANTICS;
        }
    }

    /// Replaces the active theme.
    pub fn set_theme(&mut self, theme: Theme) {
        if self.theme != theme {
            self.theme = theme;
            self.dirty = Dirty::all();
        }
    }

    /// Adds a label.
    pub fn add_label<T>(
        &mut self,
        parent: ElementHandle<T>,
        text: impl Into<String>,
    ) -> Result<ElementHandle<Label>, UiError> {
        self.insert(parent.id, Kind::Label { text: text.into() })
    }

    /// Adds a button.
    pub fn add_button<T>(
        &mut self,
        parent: ElementHandle<T>,
        text: impl Into<String>,
    ) -> Result<ElementHandle<Button>, UiError> {
        self.insert(parent.id, Kind::Button { text: text.into() })
    }

    /// Adds a horizontal flex container.
    pub fn add_row<T>(&mut self, parent: ElementHandle<T>) -> Result<ElementHandle<Row>, UiError> {
        self.insert(
            parent.id,
            Kind::Row {
                flex: FlexStyle {
                    column_gap: self.theme.gap,
                    align_items: Alignment::Center,
                    ..Default::default()
                },
            },
        )
    }

    /// Adds a vertical flex container.
    pub fn add_column<T>(
        &mut self,
        parent: ElementHandle<T>,
    ) -> Result<ElementHandle<Column>, UiError> {
        self.insert(
            parent.id,
            Kind::Column {
                flex: FlexStyle {
                    row_gap: self.theme.gap,
                    ..Default::default()
                },
            },
        )
    }

    /// Adds an overlaying stack container.
    pub fn add_stack<T>(
        &mut self,
        parent: ElementHandle<T>,
    ) -> Result<ElementHandle<Stack>, UiError> {
        self.insert(parent.id, Kind::Stack)
    }

    /// Adds a keyboard focus scope.
    pub fn add_focus_scope<T>(
        &mut self,
        parent: ElementHandle<T>,
        options: FocusScopeOptions,
    ) -> Result<ElementHandle<FocusScope>, UiError> {
        let restore = options.restore_focus.then_some(self.focus).flatten();
        self.insert(parent.id, Kind::FocusScope { options, restore })
    }

    /// Adds a viewport-hosted portal logically owned by `owner`.
    pub fn add_overlay<T>(
        &mut self,
        owner: ElementHandle<T>,
        options: OverlayOptions,
    ) -> Result<ElementHandle<Overlay>, UiError> {
        let restore = options.focus.restore_focus.then_some(self.focus).flatten();
        let handle = self.insert(
            owner.id,
            Kind::Overlay {
                owner: owner.id,
                options,
                restore,
            },
        )?;
        self.node_mut(handle.id)?.z_index = options.z_index;
        Ok(handle)
    }

    /// Adds a one-child padding container.
    pub fn add_padding<T>(
        &mut self,
        parent: ElementHandle<T>,
        insets: Insets,
    ) -> Result<ElementHandle<Padding>, UiError> {
        self.insert(parent.id, Kind::Padding { insets })
    }

    /// Adds a complete single-line text field.
    pub fn add_text_field<T>(
        &mut self,
        parent: ElementHandle<T>,
        text: impl Into<String>,
    ) -> Result<ElementHandle<TextField>, UiError> {
        self.insert(parent.id, Kind::TextField(TextFieldState::new(text.into())))
    }

    /// Adds a retained checkbox.
    pub fn add_checkbox<T>(
        &mut self,
        parent: ElementHandle<T>,
        checked: bool,
    ) -> Result<ElementHandle<Checkbox>, UiError> {
        let handle = self.insert(parent.id, Kind::Checkbox { checked })?;
        self.checkbox_styles.insert(
            handle.id,
            CheckboxStyle {
                background: self.theme.button.normal,
                indicator: self.theme.accent,
                radius: self.theme.corner_radius,
            },
        );
        Ok(handle)
    }

    /// Adds a retained horizontal slider.
    pub fn add_slider<T>(
        &mut self,
        parent: ElementHandle<T>,
        min: f32,
        max: f32,
        step: f32,
        value: f32,
    ) -> Result<ElementHandle<Slider>, UiError> {
        if !min.is_finite() || !max.is_finite() || !step.is_finite() || min >= max || step <= 0.0 {
            return Err(UiError::new(
                "slider requires finite min < max and a positive step",
            ));
        }
        let value = snap_slider(value, min, max, step);
        let handle = self.insert(
            parent.id,
            Kind::Slider {
                min,
                max,
                step,
                value,
            },
        )?;
        self.slider_styles.insert(
            handle.id,
            SliderStyle {
                track: self.theme.button.normal,
                thumb: self.theme.accent,
                thumb_size: 16.0,
            },
        );
        Ok(handle)
    }

    /// Adds a vertically scrolling retained container.
    pub fn add_scroll_view<T>(
        &mut self,
        parent: ElementHandle<T>,
    ) -> Result<ElementHandle<ScrollView>, UiError> {
        let handle = self.insert(
            parent.id,
            Kind::ScrollView {
                offset: 0.0,
                content_height: 0.0,
            },
        )?;
        self.scroll_styles.insert(
            handle.id,
            ScrollViewStyle {
                track: self.theme.button.normal,
                thumb: self.theme.accent,
                width: 8.0,
            },
        );
        Ok(handle)
    }

    /// Adds an application-defined retained widget.
    pub fn add_widget<T, W: Widget<Message>>(
        &mut self,
        parent: ElementHandle<T>,
        mut widget: W,
    ) -> Result<ElementHandle<W>, UiError> {
        let handle = self.insert(parent.id, Kind::Custom)?;
        widget.mounted(&mut MountContext {
            ui: self,
            parent: handle.id,
        })?;
        self.custom_widgets.insert(handle.id, Box::new(widget));
        Ok(handle)
    }

    /// Reads an application-defined widget through its typed handle.
    pub fn widget<W: Widget<Message>>(&self, handle: ElementHandle<W>) -> Result<&W, UiError> {
        self.node(handle.id)?;
        self.custom_widgets
            .get(&handle.id)
            .and_then(|widget| widget.as_any().downcast_ref())
            .ok_or_else(|| UiError::new("handle has the wrong widget type"))
    }

    /// Mutates an application-defined widget and invalidates all dependent phases.
    pub fn update_widget<W: Widget<Message>>(
        &mut self,
        handle: ElementHandle<W>,
        update: impl FnOnce(&mut W),
    ) -> Result<(), UiError> {
        self.node(handle.id)?;
        let widget = self
            .custom_widgets
            .get_mut(&handle.id)
            .and_then(|widget| widget.as_any_mut().downcast_mut())
            .ok_or_else(|| UiError::new("handle has the wrong widget type"))?;
        update(widget);
        widget.updated();
        self.dirty = Dirty::all();
        Ok(())
    }

    fn insert<T>(&mut self, parent: ElementId, kind: Kind) -> Result<ElementHandle<T>, UiError> {
        self.node(parent)?;
        let id = if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            slot.generation = slot.generation.wrapping_add(1).max(1);
            ElementId {
                index,
                generation: slot.generation,
            }
        } else {
            let id = ElementId {
                index: self.slots.len() as u32,
                generation: 1,
            };
            self.slots.push(Slot {
                generation: 1,
                node: None,
            });
            id
        };
        self.slots[id.index as usize].node = Some(Node {
            parent: Some(parent),
            children: Vec::new(),
            kind,
            style: LayoutStyle::default(),
            visual: WidgetStyle::default(),
            enabled: true,
            visibility: Visibility::Visible,
            overflow: Overflow::Visible,
            z_index: 0,
            transform: Affine2::IDENTITY,
            transform_origin: LogicalPoint::ZERO,
            cursor: None,
            bounds: Rect::default(),
            text_layout: None,
            hovered: false,
            pressed: false,
        });
        self.node_mut(parent)?.children.push(id);
        self.invalidate_layout();
        Ok(ElementHandle {
            id,
            marker: PhantomData,
        })
    }

    /// Removes an element and its descendants.
    pub fn remove<T>(&mut self, handle: ElementHandle<T>) -> Result<(), UiError> {
        if handle.id == self.root {
            return Err(UiError::new("the root element cannot be removed"));
        }
        let affected_drags = self
            .drag_sessions
            .iter()
            .filter_map(|(device, session)| {
                (self.is_descendant_of(session.source, handle.id)
                    || session
                        .candidate
                        .is_some_and(|target| self.is_descendant_of(target, handle.id)))
                .then_some(*device)
            })
            .collect::<Vec<_>>();
        for device_id in affected_drags {
            self.cancel_drag_id(device_id)?;
        }
        let restore = match self.node(handle.id)?.kind {
            Kind::FocusScope { restore, .. } | Kind::Overlay { restore, .. } => restore,
            _ => None,
        };
        let restore_focus = self
            .focus
            .is_some_and(|focus| self.is_descendant_of(focus, handle.id))
            .then_some(restore)
            .flatten();
        let parent = self.node(handle.id)?.parent;
        if let Some(parent) = parent {
            self.node_mut(parent)?
                .children
                .retain(|child| *child != handle.id);
        }
        let leaving = self
            .hover_paths
            .iter()
            .filter_map(|(device, path)| {
                path.last()
                    .copied()
                    .filter(|leaf| self.is_descendant_of(*leaf, handle.id))
                    .map(|leaf| (*device, leaf))
            })
            .collect::<Vec<_>>();
        for (device, leaf) in leaving {
            let position = self
                .pointer_positions
                .get(&device)
                .copied()
                .unwrap_or(LogicalPoint::ZERO);
            self.dispatch_routed(
                leaf,
                RoutedEventKind::PointerLeft {
                    device_id: device,
                    position,
                    related_target: None,
                },
            )?;
            self.hover_paths.remove(&device);
        }
        self.remove_subtree(handle.id);
        for id in self.all_ids() {
            let hovered = self.hover_paths.values().any(|path| path.contains(&id));
            self.node_mut(id)?.hovered = hovered;
        }
        self.invalidate_layout();
        if let Some(restore) = restore_focus.filter(|id| self.node(*id).is_ok()) {
            self.set_focus(Some(restore))?;
        }
        Ok(())
    }

    fn is_descendant_of(&self, child: ElementId, ancestor: ElementId) -> bool {
        let mut current = Some(child);
        while let Some(id) = current {
            if id == ancestor {
                return true;
            }
            current = self.node(id).ok().and_then(|node| node.parent);
        }
        false
    }

    /// Moves an existing subtree beneath a different parent.
    pub fn reparent<T, P>(
        &mut self,
        handle: ElementHandle<T>,
        parent: ElementHandle<P>,
    ) -> Result<(), UiError> {
        if handle.id == self.root || handle.id == parent.id {
            return Err(UiError::new("invalid reparent operation"));
        }
        self.node(handle.id)?;
        self.node(parent.id)?;
        let mut ancestor = Some(parent.id);
        while let Some(id) = ancestor {
            if id == handle.id {
                return Err(UiError::new("reparenting would create a cycle"));
            }
            ancestor = self.node(id)?.parent;
        }
        if let Some(old_parent) = self.node(handle.id)?.parent {
            self.node_mut(old_parent)?
                .children
                .retain(|child| *child != handle.id);
        }
        self.node_mut(handle.id)?.parent = Some(parent.id);
        self.node_mut(parent.id)?.children.push(handle.id);
        self.invalidate_layout();
        Ok(())
    }

    /// Changes the gap and cross-axis alignment of a row or column.
    pub fn set_flex<T>(
        &mut self,
        handle: ElementHandle<T>,
        gap: f32,
        alignment: Alignment,
    ) -> Result<(), UiError> {
        let node = self.node_mut(handle.id)?;
        match &mut node.kind {
            Kind::Row { flex } | Kind::Column { flex } => {
                flex.column_gap = gap.max(0.0);
                flex.row_gap = gap.max(0.0);
                flex.align_items = alignment;
                self.invalidate_layout();
                Ok(())
            }
            _ => Err(UiError::new("element is not a row or column")),
        }
    }

    /// Replaces a row or column's complete flex-container configuration.
    pub fn set_flex_style<T>(
        &mut self,
        handle: ElementHandle<T>,
        style: FlexStyle,
    ) -> Result<(), UiError> {
        let node = self.node_mut(handle.id)?;
        match &mut node.kind {
            Kind::Row { flex } | Kind::Column { flex } => *flex = style,
            _ => return Err(UiError::new("element is not a row or column")),
        }
        self.invalidate_layout();
        Ok(())
    }

    fn remove_subtree(&mut self, id: ElementId) {
        let children = self
            .node(id)
            .map(|node| node.children.clone())
            .unwrap_or_default();
        for child in children {
            self.remove_subtree(child);
        }
        if self.focus == Some(id) {
            self.focus = None;
        }
        if self.hover == Some(id) {
            self.hover = None;
        }
        for path in self.hover_paths.values_mut() {
            path.retain(|hovered| *hovered != id);
        }
        self.capture.retain(|_, captured| *captured != id);
        self.listeners.remove(&id);
        self.checkbox_styles.remove(&id);
        self.slider_styles.remove(&id);
        self.scroll_styles.remove(&id);
        self.semantic_roles.remove(&id);
        if let Some(mut widget) = self.custom_widgets.remove(&id) {
            widget.unmounted();
        }
        let slot = &mut self.slots[id.index as usize];
        slot.node = None;
        self.free.push(id.index);
    }

    /// Changes an element's sizing constraints.
    pub fn set_layout<T>(
        &mut self,
        handle: ElementHandle<T>,
        style: LayoutStyle,
    ) -> Result<(), UiError> {
        let lengths = [
            style.width,
            style.height,
            style.min_width,
            style.min_height,
            style.max_width,
            style.max_height,
            style.margin.left,
            style.margin.top,
            style.margin.right,
            style.margin.bottom,
            style.basis,
            style.inset.left,
            style.inset.top,
            style.inset.right,
            style.inset.bottom,
        ];
        if lengths.into_iter().any(|value| match value {
            Length::Auto => false,
            Length::Px(value) | Length::Percent(value) => !value.is_finite(),
        }) || !style.grow.is_finite()
            || !style.shrink.is_finite()
            || style
                .aspect_ratio
                .is_some_and(|value| !value.is_finite() || value <= 0.0)
        {
            return Err(UiError::new(
                "layout values must be finite and aspect ratios positive",
            ));
        }
        let node = self.node_mut(handle.id)?;
        if node.style != style {
            node.style = style;
            self.invalidate_layout();
        }
        Ok(())
    }

    /// Applies direct foreground and background overrides to one widget.
    pub fn set_widget_style<T>(
        &mut self,
        handle: ElementHandle<T>,
        style: WidgetStyle,
    ) -> Result<(), UiError> {
        let node = self.node_mut(handle.id)?;
        if node.visual != style {
            node.visual = style;
            self.dirty |= Dirty::MEASURE | Dirty::PAINT;
        }
        Ok(())
    }

    /// Enables or disables an element.
    pub fn set_enabled<T>(
        &mut self,
        handle: ElementHandle<T>,
        enabled: bool,
    ) -> Result<(), UiError> {
        let changed = self.node(handle.id)?.enabled != enabled;
        if changed {
            self.node_mut(handle.id)?.enabled = enabled;
            self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
            if !enabled
                && self
                    .focus
                    .is_some_and(|focus| self.is_descendant_of(focus, handle.id))
            {
                self.set_focus(None)?;
            }
        }
        Ok(())
    }

    /// Changes layout/paint visibility for an element and its subtree.
    pub fn set_visibility<T>(
        &mut self,
        handle: ElementHandle<T>,
        visibility: Visibility,
    ) -> Result<(), UiError> {
        let changed = self.node(handle.id)?.visibility != visibility;
        if changed {
            if visibility == Visibility::Visible {
                let current_focus = self.focus;
                match &mut self.node_mut(handle.id)?.kind {
                    Kind::FocusScope { options, restore } if options.restore_focus => {
                        *restore = current_focus;
                    }
                    Kind::Overlay {
                        options, restore, ..
                    } if options.focus.restore_focus => {
                        *restore = current_focus;
                    }
                    _ => {}
                }
            }
            let restore = match self.node(handle.id)?.kind {
                Kind::FocusScope { restore, .. } | Kind::Overlay { restore, .. } => restore,
                _ => None,
            };
            let restore_focus = (visibility == Visibility::Hidden
                && self
                    .focus
                    .is_some_and(|focus| self.is_descendant_of(focus, handle.id)))
            .then_some(restore)
            .flatten();
            self.node_mut(handle.id)?.visibility = visibility;
            self.invalidate_layout();
            if visibility == Visibility::Hidden {
                self.set_focus(restore_focus)?;
            }
        }
        Ok(())
    }

    /// Changes whether descendants are clipped to an element's bounds.
    pub fn set_overflow<T>(
        &mut self,
        handle: ElementHandle<T>,
        overflow: Overflow,
    ) -> Result<(), UiError> {
        let node = self.node_mut(handle.id)?;
        if node.overflow != overflow {
            node.overflow = overflow;
            self.dirty |= Dirty::PAINT;
        }
        Ok(())
    }

    /// Changes stable paint and targeting order among siblings.
    pub fn set_z_index<T>(
        &mut self,
        handle: ElementHandle<T>,
        z_index: i32,
    ) -> Result<(), UiError> {
        let node = self.node_mut(handle.id)?;
        if node.z_index != z_index {
            node.z_index = z_index;
            self.dirty |= Dirty::PAINT;
        }
        Ok(())
    }

    /// Applies a visual transform around a logical origin without affecting layout.
    pub fn set_transform<T>(
        &mut self,
        handle: ElementHandle<T>,
        transform: Affine2,
        origin: LogicalPoint,
    ) -> Result<(), UiError> {
        let node = self.node_mut(handle.id)?;
        if node.transform != transform || node.transform_origin != origin {
            node.transform = transform;
            node.transform_origin = origin;
            self.dirty |= Dirty::PAINT;
        }
        Ok(())
    }

    /// Overrides the cursor selected while this element is hovered.
    pub fn set_cursor_icon<T>(
        &mut self,
        handle: ElementHandle<T>,
        cursor: Option<CursorIcon>,
    ) -> Result<(), UiError> {
        self.node_mut(handle.id)?.cursor = cursor;
        Ok(())
    }

    /// Replaces label text.
    pub fn set_label_text(
        &mut self,
        handle: ElementHandle<Label>,
        text: impl Into<String>,
    ) -> Result<(), UiError> {
        self.set_static_text(handle.id, text.into(), false)
    }

    /// Replaces button text.
    pub fn set_button_text(
        &mut self,
        handle: ElementHandle<Button>,
        text: impl Into<String>,
    ) -> Result<(), UiError> {
        self.set_static_text(handle.id, text.into(), true)
    }

    fn set_static_text(
        &mut self,
        id: ElementId,
        value: String,
        button: bool,
    ) -> Result<(), UiError> {
        let node = self.node_mut(id)?;
        let text = match &mut node.kind {
            Kind::Label { text } if !button => text,
            Kind::Button { text } if button => text,
            _ => return Err(UiError::new("handle has the wrong widget type")),
        };
        if *text != value {
            *text = value;
            self.invalidate_layout();
        }
        Ok(())
    }

    /// Replaces text-field content and collapses selection at the end.
    pub fn set_text(
        &mut self,
        handle: ElementHandle<TextField>,
        text: impl Into<String>,
    ) -> Result<(), UiError> {
        let text = text.into();
        let node = self.node_mut(handle.id)?;
        let Kind::TextField(field) = &mut node.kind else {
            return Err(UiError::new("handle has the wrong widget type"));
        };
        if field.text != text {
            field.text = text;
            field.caret.byte_index = field.text.len();
            field.anchor = field.caret;
            self.invalidate_layout();
        }
        Ok(())
    }

    /// Returns text-field content.
    pub fn text(&self, handle: ElementHandle<TextField>) -> Result<&str, UiError> {
        match &self.node(handle.id)?.kind {
            Kind::TextField(field) => Ok(&field.text),
            _ => Err(UiError::new("handle has the wrong widget type")),
        }
    }

    /// Changes text-field placeholder text.
    pub fn set_placeholder(
        &mut self,
        handle: ElementHandle<TextField>,
        placeholder: impl Into<String>,
    ) -> Result<(), UiError> {
        let placeholder = placeholder.into();
        let node = self.node_mut(handle.id)?;
        let Kind::TextField(field) = &mut node.kind else {
            return Err(UiError::new("handle has the wrong widget type"));
        };
        if field.placeholder != placeholder {
            field.placeholder = placeholder;
            self.invalidate_layout();
        }
        Ok(())
    }

    /// Selects password-purpose display and IME behavior.
    pub fn set_password(
        &mut self,
        handle: ElementHandle<TextField>,
        password: bool,
    ) -> Result<(), UiError> {
        let node = self.node_mut(handle.id)?;
        let Kind::TextField(field) = &mut node.kind else {
            return Err(UiError::new("handle has the wrong widget type"));
        };
        if field.password != password {
            field.password = password;
            self.invalidate_layout();
        }
        Ok(())
    }

    /// Returns a checkbox's retained value.
    pub fn checked(&self, handle: ElementHandle<Checkbox>) -> Result<bool, UiError> {
        match self.node(handle.id)?.kind {
            Kind::Checkbox { checked } => Ok(checked),
            _ => Err(UiError::new("handle has the wrong widget type")),
        }
    }

    /// Sets a checkbox's retained value.
    pub fn set_checked(
        &mut self,
        handle: ElementHandle<Checkbox>,
        checked: bool,
    ) -> Result<(), UiError> {
        let Kind::Checkbox { checked: current } = &mut self.node_mut(handle.id)?.kind else {
            return Err(UiError::new("handle has the wrong widget type"));
        };
        if *current != checked {
            *current = checked;
            self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
        }
        Ok(())
    }

    /// Returns a slider's retained value.
    pub fn slider_value(&self, handle: ElementHandle<Slider>) -> Result<f32, UiError> {
        match self.node(handle.id)?.kind {
            Kind::Slider { value, .. } => Ok(value),
            _ => Err(UiError::new("handle has the wrong widget type")),
        }
    }

    /// Sets and snaps a slider's retained value.
    pub fn set_slider_value(
        &mut self,
        handle: ElementHandle<Slider>,
        value: f32,
    ) -> Result<(), UiError> {
        let Kind::Slider {
            min,
            max,
            step,
            value: current,
        } = &mut self.node_mut(handle.id)?.kind
        else {
            return Err(UiError::new("handle has the wrong widget type"));
        };
        let value = snap_slider(value, *min, *max, *step);
        if *current != value {
            *current = value;
            self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
        }
        Ok(())
    }

    /// Returns a scroll view's vertical offset.
    pub fn scroll_offset(&self, handle: ElementHandle<ScrollView>) -> Result<f32, UiError> {
        match self.node(handle.id)?.kind {
            Kind::ScrollView { offset, .. } => Ok(offset),
            _ => Err(UiError::new("handle has the wrong widget type")),
        }
    }

    /// Sets a vertical scroll view's logical offset, clamped to its content.
    pub fn set_scroll_offset(
        &mut self,
        handle: ElementHandle<ScrollView>,
        offset: f32,
    ) -> Result<(), UiError> {
        self.set_scroll_offset_id(handle.id, offset)
    }

    fn set_scroll_offset_id(&mut self, id: ElementId, offset: f32) -> Result<(), UiError> {
        let (height, content_height) = {
            let node = self.node(id)?;
            let Kind::ScrollView { content_height, .. } = node.kind else {
                return Err(UiError::new("element is not a scroll view"));
            };
            (node.bounds.size.height, content_height)
        };
        let offset = offset.clamp(0.0, (content_height - height).max(0.0));
        let Kind::ScrollView {
            offset: current, ..
        } = &mut self.node_mut(id)?.kind
        else {
            unreachable!("kind was checked above")
        };
        if *current != offset {
            *current = offset;
            self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
        }
        Ok(())
    }

    /// Replaces a checkbox's typed visual style.
    pub fn set_checkbox_style(
        &mut self,
        handle: ElementHandle<Checkbox>,
        style: CheckboxStyle,
    ) -> Result<(), UiError> {
        self.node(handle.id)?;
        self.checkbox_styles.insert(handle.id, style);
        self.dirty |= Dirty::PAINT;
        Ok(())
    }
    /// Replaces a slider's typed visual style.
    pub fn set_slider_style(
        &mut self,
        handle: ElementHandle<Slider>,
        style: SliderStyle,
    ) -> Result<(), UiError> {
        self.node(handle.id)?;
        self.slider_styles.insert(handle.id, style);
        self.dirty |= Dirty::PAINT;
        Ok(())
    }
    /// Replaces a scroll view's typed visual style.
    pub fn set_scroll_view_style(
        &mut self,
        handle: ElementHandle<ScrollView>,
        style: ScrollViewStyle,
    ) -> Result<(), UiError> {
        self.node(handle.id)?;
        self.scroll_styles.insert(handle.id, style);
        self.dirty |= Dirty::PAINT;
        Ok(())
    }

    /// Overrides the semantic role reported for one retained element.
    pub fn set_semantic_role<T>(
        &mut self,
        handle: ElementHandle<T>,
        role: SemanticRole,
    ) -> Result<(), UiError> {
        self.node(handle.id)?;
        self.semantic_roles.insert(handle.id, role);
        self.dirty |= Dirty::SEMANTICS;
        Ok(())
    }

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

    fn dispatch_routed(
        &mut self,
        target: ElementId,
        kind: RoutedEventKind,
    ) -> Result<bool, UiError> {
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

    fn deliver(
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

    fn apply_event_requests(&mut self) -> Result<(), UiError> {
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

    fn invalidate_layout(&mut self) {
        self.dirty |= Dirty::MEASURE | Dirty::LAYOUT | Dirty::PAINT | Dirty::SEMANTICS;
    }

    fn node(&self, id: ElementId) -> Result<&Node, UiError> {
        let Some(slot) = self.slots.get(id.index as usize) else {
            return Err(UiError::new("stale element handle"));
        };
        if slot.generation != id.generation {
            return Err(UiError::new("stale element handle"));
        }
        slot.node
            .as_ref()
            .ok_or_else(|| UiError::new("stale element handle"))
    }

    fn node_mut(&mut self, id: ElementId) -> Result<&mut Node, UiError> {
        let Some(slot) = self.slots.get_mut(id.index as usize) else {
            return Err(UiError::new("stale element handle"));
        };
        if slot.generation != id.generation {
            return Err(UiError::new("stale element handle"));
        }
        slot.node
            .as_mut()
            .ok_or_else(|| UiError::new("stale element handle"))
    }

    fn all_ids(&self) -> Vec<ElementId> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| {
                slot.node.as_ref().map(|_| ElementId {
                    index: index as u32,
                    generation: slot.generation,
                })
            })
            .collect()
    }

    fn prepare_text_layouts(&mut self) -> Result<(), UiError> {
        let ids = self.all_ids();
        for id in ids {
            let request = match &self.node(id)?.kind {
                Kind::Label { text } | Kind::Button { text } => {
                    let mut request = TextLayoutRequest::new(text);
                    request.style.size = self.theme.font_size;
                    request.style.families = self.theme.font_families.clone();
                    request.style.color = self
                        .node(id)?
                        .visual
                        .foreground
                        .unwrap_or(self.theme.foreground);
                    request.paragraph = ParagraphStyle {
                        wrap: TextWrap::NoWrap,
                        ..Default::default()
                    };
                    Some(request)
                }
                Kind::TextField(field) => {
                    let display = if field.password {
                        "•".repeat(field.text.graphemes(true).count())
                    } else {
                        field.text.clone()
                    };
                    let mut shown = display;
                    if !field.preedit.is_empty() && !field.password {
                        let insert = field.caret.byte_index.min(shown.len());
                        if shown.is_char_boundary(insert) {
                            shown.insert_str(insert, &field.preedit);
                        }
                    }
                    if shown.is_empty() {
                        shown = field.placeholder.clone();
                    }
                    let mut request = TextLayoutRequest::new(shown);
                    request.style.size = self.theme.font_size;
                    request.style.families = self.theme.font_families.clone();
                    request.style.color = self.node(id)?.visual.foreground.unwrap_or(
                        if field.text.is_empty() && field.preedit.is_empty() {
                            self.theme.muted_foreground
                        } else {
                            self.theme.foreground
                        },
                    );
                    request.paragraph = ParagraphStyle {
                        wrap: TextWrap::NoWrap,
                        ..Default::default()
                    };
                    Some(request)
                }
                _ => None,
            };
            if let Some(request) = request {
                let layout = self
                    .text_context
                    .layout(&mut self.fonts, request)
                    .map_err(|error| UiError::new(error.to_string()))?;
                self.node_mut(id)?.text_layout = Some(layout);
            }
        }
        Ok(())
    }

    fn taffy_style(&self, node: &Node) -> Style {
        let dimension = |value: Length| match value {
            Length::Auto => Dimension::auto(),
            Length::Px(value) => Dimension::length(value.max(0.0)),
            Length::Percent(value) => Dimension::percent(value.max(0.0)),
        };
        let edge = |value: Length| match value {
            Length::Auto => LengthPercentageAuto::auto(),
            Length::Px(value) => LengthPercentageAuto::length(value),
            Length::Percent(value) => LengthPercentageAuto::percent(value),
        };
        let mut style = Style {
            display: if node.visibility == Visibility::Collapsed {
                Display::None
            } else {
                Display::Flex
            },
            size: TaffySize {
                width: dimension(node.style.width),
                height: dimension(node.style.height),
            },
            min_size: TaffySize {
                width: dimension(node.style.min_width),
                height: dimension(node.style.min_height),
            },
            max_size: TaffySize {
                width: dimension(node.style.max_width),
                height: dimension(node.style.max_height),
            },
            margin: TaffyRect {
                left: edge(node.style.margin.left),
                top: edge(node.style.margin.top),
                right: edge(node.style.margin.right),
                bottom: edge(node.style.margin.bottom),
            },
            inset: TaffyRect {
                left: edge(node.style.inset.left),
                top: edge(node.style.inset.top),
                right: edge(node.style.inset.right),
                bottom: edge(node.style.inset.bottom),
            },
            position: if node.style.positioning == Positioning::Absolute {
                TaffyPosition::Absolute
            } else {
                TaffyPosition::Relative
            },
            flex_grow: node.style.grow.max(0.0),
            flex_shrink: node.style.shrink.max(0.0),
            flex_basis: dimension(node.style.basis),
            align_self: node.style.align_self.map(map_alignment),
            aspect_ratio: node
                .style
                .aspect_ratio
                .filter(|value| value.is_finite() && *value > 0.0),
            ..Default::default()
        };
        if node.parent.is_none() {
            style.size = TaffySize {
                width: Dimension::length(self.viewport.width.max(0.0)),
                height: Dimension::length(self.viewport.height.max(0.0)),
            };
        }
        if node.style.grow > 0.0 {
            if node.style.min_width == Length::Auto {
                style.min_size.width = Dimension::length(0.0);
            }
            if node.style.min_height == Length::Auto {
                style.min_size.height = Dimension::length(0.0);
            }
        }
        match node.kind {
            Kind::Row { flex } => {
                style.flex_direction = FlexDirection::Row;
                apply_flex(&mut style, flex);
            }
            Kind::Column { flex } => {
                style.flex_direction = FlexDirection::Column;
                apply_flex(&mut style, flex);
            }
            Kind::Stack => {}
            Kind::FocusScope { .. } => {
                style.flex_direction = FlexDirection::Column;
            }
            Kind::Overlay { .. } => {
                style.flex_direction = FlexDirection::Column;
                style.position = TaffyPosition::Absolute;
            }
            Kind::Padding { insets } => {
                style.flex_direction = FlexDirection::Column;
                style.padding.left = LengthPercentage::length(insets.left.max(0.0));
                style.padding.top = LengthPercentage::length(insets.top.max(0.0));
                style.padding.right = LengthPercentage::length(insets.right.max(0.0));
                style.padding.bottom = LengthPercentage::length(insets.bottom.max(0.0));
            }
            Kind::Button { .. } | Kind::TextField(_) => {
                let insets = self.theme.control_padding;
                style.padding.left = LengthPercentage::length(insets.left);
                style.padding.top = LengthPercentage::length(insets.top);
                style.padding.right = LengthPercentage::length(insets.right);
                style.padding.bottom = LengthPercentage::length(insets.bottom);
                if node.children.iter().any(|child| {
                    self.node(*child)
                        .is_ok_and(|node| matches!(node.kind, Kind::Overlay { .. }))
                }) && let Some(size) = node.text_layout.as_ref().map(TextLayout::size)
                {
                    if node.style.min_width == Length::Auto {
                        style.min_size.width =
                            Dimension::length(size.width + insets.left + insets.right);
                    }
                    if node.style.min_height == Length::Auto {
                        style.min_size.height =
                            Dimension::length(size.height + insets.top + insets.bottom);
                    }
                }
            }
            Kind::Checkbox { .. } | Kind::Slider { .. } => {
                style.min_size.height = Dimension::length(28.0);
                style.min_size.width =
                    Dimension::length(if matches!(node.kind, Kind::Slider { .. }) {
                        160.0
                    } else {
                        28.0
                    });
            }
            Kind::ScrollView { .. } => {
                style.flex_direction = FlexDirection::Column;
                style.overflow.y = TaffyOverflow::Scroll;
                if node.style.min_height == Length::Auto {
                    style.min_size.height = Dimension::length(0.0);
                }
            }
            Kind::Custom => {
                style.flex_direction = FlexDirection::Column;
                style.gap.height = LengthPercentage::length(self.theme.gap);
                let insets = self.theme.control_padding;
                style.padding.left = LengthPercentage::length(insets.left);
                style.padding.top = LengthPercentage::length(insets.top);
                style.padding.right = LengthPercentage::length(insets.right);
                style.padding.bottom = LengthPercentage::length(insets.bottom);
            }
            Kind::Label { .. } => {}
        }
        if node
            .parent
            .and_then(|id| self.node(id).ok())
            .is_some_and(|parent| matches!(parent.kind, Kind::Stack))
        {
            style.position = TaffyPosition::Absolute;
            if node.style.inset == Edges::all(Length::Auto)
                && node.style.width == Length::Auto
                && node.style.height == Length::Auto
            {
                style.inset = TaffyRect {
                    left: LengthPercentageAuto::length(0.0),
                    top: LengthPercentageAuto::length(0.0),
                    right: LengthPercentageAuto::length(0.0),
                    bottom: LengthPercentageAuto::length(0.0),
                };
            }
        }
        style
    }

    fn build_taffy(
        &self,
        tree: &mut TaffyTree<ElementId>,
        id: ElementId,
        mapping: &mut HashMap<ElementId, NodeId>,
    ) -> Result<NodeId, UiError> {
        let node = self.node(id)?;
        let children = node
            .children
            .iter()
            .map(|child| self.build_taffy(tree, *child, mapping))
            .collect::<Result<Vec<_>, _>>()?;
        let style = self.taffy_style(node);
        let taffy_id = if children.is_empty() {
            tree.new_leaf_with_context(style, id)
        } else {
            let result = tree.new_with_children(style, &children);
            if let Ok(node_id) = result {
                tree.set_node_context(node_id, Some(id))
                    .map_err(|error| UiError::new(error.to_string()))?;
            }
            result
        }
        .map_err(|error| UiError::new(error.to_string()))?;
        mapping.insert(id, taffy_id);
        Ok(taffy_id)
    }

    fn ensure_layout(&mut self) -> Result<(), UiError> {
        if !self.dirty.intersects(Dirty::MEASURE | Dirty::LAYOUT) {
            return Ok(());
        }
        astrelis_profiling::profile_scope!("ui.layout");
        self.prepare_text_layouts()?;
        let mut tree = TaffyTree::<ElementId>::new();
        tree.disable_rounding();
        let mut mapping = HashMap::new();
        let root = self.build_taffy(&mut tree, self.root, &mut mapping)?;
        let mut layouts = self
            .all_ids()
            .into_iter()
            .filter_map(|id| {
                self.node(id)
                    .ok()
                    .and_then(|node| node.text_layout.as_ref().map(|layout| (id, layout.size())))
            })
            .collect::<HashMap<_, _>>();
        for (id, widget) in &self.custom_widgets {
            let size = widget.intrinsic_size(&self.theme);
            if size != Size::ZERO {
                layouts.insert(*id, size);
            }
        }
        tree.compute_layout_with_measure(
            root,
            TaffySize {
                width: AvailableSpace::Definite(self.viewport.width.max(0.0)),
                height: AvailableSpace::Definite(self.viewport.height.max(0.0)),
            },
            |known, _available, _node, context, _style| {
                let Some(id) = context.copied() else {
                    return TaffySize::ZERO;
                };
                let measured = layouts.get(&id).copied().unwrap_or(Size::ZERO);
                TaffySize {
                    width: known.width.unwrap_or(measured.width),
                    height: known.height.unwrap_or(measured.height),
                }
            },
        )
        .map_err(|error| UiError::new(error.to_string()))?;
        self.assign_layout(&tree, &mapping, self.root, LogicalPoint::ZERO)?;
        self.position_overlays()?;
        if self.focus.is_none() {
            let autofocus = self.all_ids().into_iter().find(|id| self.node(*id).is_ok_and(|node| matches!(node.kind, Kind::FocusScope { options, .. } if options.autofocus) || matches!(node.kind, Kind::Overlay { options, .. } if options.focus.autofocus)));
            if let Some(scope) = autofocus
                && let Some(target) = self.all_ids().into_iter().find(|id| {
                    self.is_descendant_of(*id, scope)
                        && self.is_effectively_interactive(*id)
                        && self.is_focusable_id(*id)
                })
            {
                self.set_focus(Some(target))?;
            }
        }
        self.ensure_caret_visible()?;
        self.dirty.remove(Dirty::MEASURE | Dirty::LAYOUT);
        self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
        if self
            .focus
            .is_some_and(|id| !self.is_effectively_interactive(id))
        {
            self.set_focus(None)?;
        }
        let hovered_devices = self.hover_paths.keys().copied().collect::<Vec<_>>();
        for device in hovered_devices {
            if let Some(position) = self.pointer_positions.get(&device).copied() {
                let target = self.hit_test(position);
                self.set_hover(device, position, target)?;
            }
        }
        Ok(())
    }

    fn position_overlays(&mut self) -> Result<(), UiError> {
        let overlays = self
            .all_ids()
            .into_iter()
            .filter(|id| {
                self.node(*id)
                    .is_ok_and(|node| matches!(node.kind, Kind::Overlay { .. }))
            })
            .collect::<Vec<_>>();
        for id in overlays {
            let (owner, options) = match self.node(id)?.kind {
                Kind::Overlay { owner, options, .. } => (owner, options),
                _ => continue,
            };
            let anchor = self.node(owner)?.bounds;
            let bounds = self.node(id)?.bounds;
            let mut x = match options.side {
                OverlaySide::Left => anchor.origin.x - bounds.size.width,
                OverlaySide::Right => anchor.max_x(),
                OverlaySide::Center => {
                    anchor.origin.x + (anchor.size.width - bounds.size.width) * 0.5
                }
                OverlaySide::Above | OverlaySide::Below => match options.alignment {
                    OverlayAlignment::Start => anchor.origin.x,
                    OverlayAlignment::Center => {
                        anchor.origin.x + (anchor.size.width - bounds.size.width) * 0.5
                    }
                    OverlayAlignment::End => anchor.max_x() - bounds.size.width,
                },
            };
            let mut y = match options.side {
                OverlaySide::Above => anchor.origin.y - bounds.size.height,
                OverlaySide::Below => anchor.max_y(),
                OverlaySide::Center => {
                    anchor.origin.y + (anchor.size.height - bounds.size.height) * 0.5
                }
                OverlaySide::Left | OverlaySide::Right => match options.alignment {
                    OverlayAlignment::Start => anchor.origin.y,
                    OverlayAlignment::Center => {
                        anchor.origin.y + (anchor.size.height - bounds.size.height) * 0.5
                    }
                    OverlayAlignment::End => anchor.max_y() - bounds.size.height,
                },
            };
            x += options.offset.x;
            y += options.offset.y;
            if options.clamp_to_viewport {
                x = x.clamp(0.0, (self.viewport.width - bounds.size.width).max(0.0));
                y = y.clamp(0.0, (self.viewport.height - bounds.size.height).max(0.0));
            }
            self.translate_subtree(id, x - bounds.origin.x, y - bounds.origin.y)?;
        }
        Ok(())
    }

    fn translate_subtree(&mut self, id: ElementId, x: f32, y: f32) -> Result<(), UiError> {
        let children = self.node(id)?.children.clone();
        let node = self.node_mut(id)?;
        node.bounds.origin.x += x;
        node.bounds.origin.y += y;
        for child in children {
            self.translate_subtree(child, x, y)?;
        }
        Ok(())
    }

    fn ensure_caret_visible(&mut self) -> Result<(), UiError> {
        let Some(focus) = self.focus else {
            return Ok(());
        };
        let node = self.node(focus)?;
        let Some(layout) = node.text_layout.clone() else {
            return Ok(());
        };
        let Kind::TextField(field) = &node.kind else {
            return Ok(());
        };
        let caret = layout.caret_rect(to_layout_position(field, field.caret), 1.0);
        let available = (node.bounds.size.width
            - self.theme.control_padding.left
            - self.theme.control_padding.right)
            .max(0.0);
        let mut offset = field.horizontal_offset;
        if caret.origin.x < offset {
            offset = caret.origin.x;
        } else if caret.origin.x + caret.size.width > offset + available {
            offset = (caret.origin.x + caret.size.width - available).max(0.0);
        }
        self.text_field_mut(focus)?.horizontal_offset = offset;
        Ok(())
    }

    fn assign_layout(
        &mut self,
        tree: &TaffyTree<ElementId>,
        mapping: &HashMap<ElementId, NodeId>,
        id: ElementId,
        parent_origin: LogicalPoint,
    ) -> Result<(), UiError> {
        let layout = tree
            .layout(mapping[&id])
            .map_err(|error| UiError::new(error.to_string()))?;
        let origin = Point::new(
            parent_origin.x + layout.location.x,
            parent_origin.y + layout.location.y,
        );
        let children = self.node(id)?.children.clone();
        self.node_mut(id)?.bounds =
            Rect::from_xywh(origin.x, origin.y, layout.size.width, layout.size.height);
        for child in children {
            self.assign_layout(tree, mapping, child, origin)?;
        }
        if matches!(self.node(id)?.kind, Kind::ScrollView { .. }) {
            let bottom = self
                .node(id)?
                .children
                .iter()
                .filter_map(|child| self.subtree_bottom(*child).ok())
                .fold(origin.y, f32::max);
            let content_height = (bottom - origin.y).max(self.node(id)?.bounds.size.height);
            if let Kind::ScrollView {
                content_height: current,
                offset,
            } = &mut self.node_mut(id)?.kind
            {
                *current = content_height;
                *offset = (*offset).clamp(0.0, (content_height - layout.size.height).max(0.0));
            }
        }
        Ok(())
    }

    fn subtree_bottom(&self, id: ElementId) -> Result<f32, UiError> {
        let node = self.node(id)?;
        let mut bottom = node.bounds.max_y();
        if matches!(node.kind, Kind::ScrollView { .. }) {
            return Ok(bottom);
        }
        for child in &node.children {
            if !matches!(self.node(*child)?.kind, Kind::Overlay { .. }) {
                bottom = bottom.max(self.subtree_bottom(*child)?);
            }
        }
        Ok(bottom)
    }

    /// Returns the pointer target at a logical viewport position.
    pub fn hit_test_at(&mut self, point: LogicalPoint) -> Result<Option<ElementId>, UiError> {
        self.ensure_layout()?;
        Ok(self.hit_test(point))
    }

    /// Returns the current untransformed logical layout bounds of an element.
    pub fn layout_bounds<T>(&mut self, handle: ElementHandle<T>) -> Result<LogicalRect, UiError> {
        self.ensure_layout()?;
        Ok(self.node(handle.id)?.bounds)
    }

    /// Returns whether `element` belongs to `ancestor`'s retained subtree.
    pub fn is_descendant<T, A>(
        &self,
        element: ElementHandle<T>,
        ancestor: ElementHandle<A>,
    ) -> Result<bool, UiError> {
        self.node(element.id)?;
        self.node(ancestor.id)?;
        Ok(self.is_descendant_of(element.id, ancestor.id))
    }

    /// Returns the active drag session associated with a pointer, when any.
    pub fn drag_session(&self, device_id: DeviceId) -> Option<DragSessionId> {
        self.drag_sessions.get(&device_id).map(|session| session.id)
    }

    /// Returns whether an element belongs to any active pointer hover path.
    pub fn is_hovered<T>(&self, handle: ElementHandle<T>) -> Result<bool, UiError> {
        Ok(self.node(handle.id)?.hovered)
    }

    /// Returns whether an element currently owns keyboard focus.
    pub fn is_focused<T>(&self, handle: ElementHandle<T>) -> Result<bool, UiError> {
        self.node(handle.id)?;
        Ok(self.focus == Some(handle.id))
    }

    /// Builds a deterministic headless layout and interaction snapshot.
    pub fn inspect(&mut self) -> Result<UiInspection, UiError> {
        self.ensure_layout()?;
        let mut paint = Vec::new();
        self.collect_paint_order(self.root, &mut paint)?;
        let mut overlays = self
            .all_ids()
            .into_iter()
            .filter(|id| {
                self.node(*id)
                    .is_ok_and(|node| matches!(node.kind, Kind::Overlay { .. }))
            })
            .collect::<Vec<_>>();
        overlays.sort_by_key(|id| self.node(*id).map_or(0, |node| node.z_index));
        for overlay in overlays {
            self.collect_paint_order(overlay, &mut paint)?;
        }
        let ranks = paint
            .into_iter()
            .enumerate()
            .map(|(rank, id)| (id, rank))
            .collect::<HashMap<_, _>>();
        let mut nodes = Vec::new();
        for id in self.all_ids() {
            let node = self.node(id)?;
            let route = self.route_to(id)?;
            let mut world = Affine2::IDENTITY;
            let mut clip: Option<LogicalRect> = None;
            for current in route {
                let current_node = self.node(current)?;
                world *= node_local_transform(current_node);
                if current_node.overflow == Overflow::Clip
                    || matches!(current_node.kind, Kind::ScrollView { .. })
                {
                    let bounds = transformed_bounds(current_node.bounds, world);
                    clip = Some(clip.map_or(bounds, |old| intersect_rect(old, bounds)));
                }
                if let Kind::ScrollView { offset, .. } = current_node.kind {
                    world *= Affine2::from_translation(Vec2::new(0.0, -offset));
                }
            }
            nodes.push(ElementInspection {
                id,
                parent: node.parent,
                layout_bounds: node.bounds,
                world_bounds: transformed_bounds(node.bounds, world),
                physical_bounds: scale_rect(
                    transformed_bounds(node.bounds, world),
                    self.scale_factor,
                ),
                clip,
                physical_clip: clip.map(|rect| scale_rect(rect, self.scale_factor)),
                world_transform: world,
                paint_rank: ranks.get(&id).copied().unwrap_or(0),
                visibility: node.visibility,
                effectively_visible: route_is_visible(self, id),
                interactive: self.is_effectively_interactive(id),
                hovered: node.hovered,
                focused: self.focus == Some(id),
                focusable: self.is_focusable_id(id),
                hit_testable: self.is_hit_testable_id(id),
            });
        }
        Ok(UiInspection {
            viewport: self.viewport,
            scale_factor: self.scale_factor,
            nodes,
        })
    }

    /// Inspects one retained element after ensuring layout is current.
    pub fn inspect_element<T>(
        &mut self,
        handle: ElementHandle<T>,
    ) -> Result<ElementInspection, UiError> {
        self.inspect()?
            .nodes
            .into_iter()
            .find(|node| node.id == handle.id)
            .ok_or_else(|| UiError::new("element is no longer retained"))
    }

    fn collect_paint_order(
        &self,
        id: ElementId,
        output: &mut Vec<ElementId>,
    ) -> Result<(), UiError> {
        let node = self.node(id)?;
        if node.visibility != Visibility::Visible {
            return Ok(());
        }
        output.push(id);
        let mut children = node
            .children
            .iter()
            .copied()
            .enumerate()
            .collect::<Vec<_>>();
        children.sort_by_key(|(index, child)| {
            (self.node(*child).map_or(0, |node| node.z_index), *index)
        });
        for (_, child) in children {
            if matches!(self.node(child)?.kind, Kind::Overlay { .. }) {
                continue;
            }
            self.collect_paint_order(child, output)?;
        }
        Ok(())
    }

    /// Generates the current backend-independent display list.
    pub fn display_list(&mut self) -> Result<DisplayList, UiError> {
        self.ensure_layout()?;
        astrelis_profiling::profile_scope!("ui.paint");
        let mut painter = Painter::new();
        painter
            .fill_rect(
                Rect::from_xywh(0.0, 0.0, self.viewport.width, self.viewport.height),
                Brush::Solid(self.theme.background),
            )
            .map_err(|error| UiError::new(error.to_string()))?;
        self.paint_node(self.root, &mut painter)?;
        let mut overlays = self
            .all_ids()
            .into_iter()
            .filter(|id| {
                self.node(*id)
                    .is_ok_and(|node| matches!(node.kind, Kind::Overlay { .. }))
            })
            .collect::<Vec<_>>();
        overlays.sort_by_key(|id| self.node(*id).map_or(0, |node| node.z_index));
        for overlay in overlays {
            self.paint_node(overlay, &mut painter)?;
        }
        let list = painter
            .finish()
            .map_err(|error| UiError::new(error.to_string()))?;
        self.dirty.remove(Dirty::PAINT);
        Ok(list)
    }

    fn paint_node(&self, id: ElementId, painter: &mut Painter) -> Result<(), UiError> {
        let node = self.node(id)?;
        if node.visibility != Visibility::Visible {
            return Ok(());
        }
        painter.save();
        let transform = node_local_transform(node);
        if transform != Affine2::IDENTITY {
            painter
                .transform(transform)
                .map_err(|error| UiError::new(error.to_string()))?;
        }
        if node.overflow == Overflow::Clip {
            painter
                .clip_rect(node.bounds)
                .map_err(|error| UiError::new(error.to_string()))?;
        }
        match &node.kind {
            Kind::Row { .. }
            | Kind::Column { .. }
            | Kind::Stack
            | Kind::FocusScope { .. }
            | Kind::Overlay { .. }
            | Kind::Padding { .. }
                if node.visual.background.is_some() =>
            {
                painter
                    .fill_rect(
                        node.bounds,
                        Brush::Solid(node.visual.background.expect("checked above")),
                    )
                    .map_err(|error| UiError::new(error.to_string()))?;
            }
            Kind::Button { .. } => {
                let color = if !node.enabled {
                    self.theme.button.disabled
                } else if node.pressed {
                    self.theme.button.pressed
                } else if node.hovered {
                    self.theme.button.hovered
                } else {
                    self.theme.button.normal
                };
                self.fill_control(
                    painter,
                    node.bounds,
                    node.visual.background.unwrap_or(color),
                )?;
            }
            Kind::TextField(_) => {
                self.fill_control(
                    painter,
                    node.bounds,
                    node.visual
                        .background
                        .unwrap_or(self.theme.field_background),
                )?;
            }
            Kind::Checkbox { checked } => {
                let style = self.checkbox_styles[&id];
                self.fill_control(
                    painter,
                    node.bounds,
                    node.visual.background.unwrap_or(style.background),
                )?;
                if *checked {
                    let inset = 6.0;
                    painter
                        .fill_rounded_rect(
                            RoundedRect::new(
                                Rect::from_xywh(
                                    node.bounds.origin.x + inset,
                                    node.bounds.origin.y + inset,
                                    (node.bounds.size.width - inset * 2.0).max(0.0),
                                    (node.bounds.size.height - inset * 2.0).max(0.0),
                                ),
                                CornerRadii::uniform(3.0),
                            )
                            .map_err(|error| UiError::new(error.to_string()))?,
                            Brush::Solid(style.indicator),
                        )
                        .map_err(|error| UiError::new(error.to_string()))?;
                }
            }
            Kind::Slider {
                min, max, value, ..
            } => {
                let style = self.slider_styles[&id];
                let center_y = node.bounds.origin.y + node.bounds.size.height * 0.5;
                let track = Rect::from_xywh(
                    node.bounds.origin.x,
                    center_y - 2.0,
                    node.bounds.size.width,
                    4.0,
                );
                painter
                    .fill_rounded_rect(
                        RoundedRect::new(track, CornerRadii::uniform(2.0))
                            .map_err(|error| UiError::new(error.to_string()))?,
                        Brush::Solid(style.track),
                    )
                    .map_err(|error| UiError::new(error.to_string()))?;
                let t = (*value - *min) / (*max - *min);
                let thumb = Rect::from_xywh(
                    node.bounds.origin.x + t * node.bounds.size.width - style.thumb_size * 0.5,
                    center_y - style.thumb_size * 0.5,
                    style.thumb_size,
                    style.thumb_size,
                );
                painter
                    .fill_ellipse(thumb, Brush::Solid(style.thumb))
                    .map_err(|error| UiError::new(error.to_string()))?;
                painter
                    .stroke_ellipse(
                        thumb,
                        StrokeStyle {
                            width: 1.0,
                            ..Default::default()
                        },
                        Brush::Solid(self.theme.foreground),
                    )
                    .map_err(|error| UiError::new(error.to_string()))?;
            }
            Kind::ScrollView { .. } => {}
            Kind::Custom => {
                if let Some(widget) = self.custom_widgets.get(&id) {
                    widget.paint(painter, node.bounds, &self.theme)?;
                }
            }
            _ => {}
        }

        if self.focus == Some(id) && self.is_focusable_id(id) {
            let thickness = 2.0;
            let bounds = Rect::from_xywh(
                node.bounds.origin.x,
                node.bounds.origin.y,
                node.bounds.size.width,
                thickness,
            );
            painter
                .fill_rect(bounds, Brush::Solid(self.theme.accent))
                .map_err(|error| UiError::new(error.to_string()))?;
        }

        if let Some(layout) = &node.text_layout {
            let mut origin = node.bounds.origin;
            if matches!(node.kind, Kind::Button { .. } | Kind::TextField(_)) {
                origin.x += self.theme.control_padding.left;
                origin.y += self.theme.control_padding.top;
            }
            if let Kind::TextField(field) = &node.kind {
                let content = Rect::from_xywh(
                    node.bounds.origin.x + self.theme.control_padding.left,
                    node.bounds.origin.y + self.theme.control_padding.top,
                    (node.bounds.size.width
                        - self.theme.control_padding.left
                        - self.theme.control_padding.right)
                        .max(0.0),
                    (node.bounds.size.height
                        - self.theme.control_padding.top
                        - self.theme.control_padding.bottom)
                        .max(0.0),
                );
                painter.save();
                painter
                    .clip_rect(content)
                    .map_err(|error| UiError::new(error.to_string()))?;
                origin.x -= field.horizontal_offset;
                if self.focus == Some(id) && !field.text.is_empty() {
                    let (start, end) = field.selection();
                    if start != end {
                        for rect in layout.selection_rects(
                            to_layout_position(
                                field,
                                TextPosition {
                                    byte_index: start,
                                    ..Default::default()
                                },
                            ),
                            to_layout_position(
                                field,
                                TextPosition {
                                    byte_index: end,
                                    ..Default::default()
                                },
                            ),
                        ) {
                            painter
                                .fill_rect(
                                    Rect::from_xywh(
                                        origin.x + rect.origin.x,
                                        origin.y + rect.origin.y,
                                        rect.size.width,
                                        rect.size.height,
                                    ),
                                    Brush::Solid(self.theme.selection),
                                )
                                .map_err(|error| UiError::new(error.to_string()))?;
                        }
                    }
                }
                painter
                    .draw_text(layout, origin, 1.0)
                    .map_err(|error| UiError::new(error.to_string()))?;
                if self.focus == Some(id) {
                    let caret = layout.caret_rect(to_layout_position(field, field.caret), 1.0);
                    painter
                        .fill_rect(
                            Rect::from_xywh(
                                origin.x + caret.origin.x,
                                origin.y + caret.origin.y,
                                caret.size.width.max(1.0),
                                caret.size.height,
                            ),
                            Brush::Solid(self.theme.accent),
                        )
                        .map_err(|error| UiError::new(error.to_string()))?;
                }
                painter
                    .restore()
                    .map_err(|error| UiError::new(error.to_string()))?;
            } else {
                let clipped_control = matches!(node.kind, Kind::Button { .. });
                if clipped_control {
                    painter.save();
                    painter
                        .clip_rect(node.bounds)
                        .map_err(|error| UiError::new(error.to_string()))?;
                }
                painter
                    .draw_text(layout, origin, 1.0)
                    .map_err(|error| UiError::new(error.to_string()))?;
                if clipped_control {
                    painter
                        .restore()
                        .map_err(|error| UiError::new(error.to_string()))?;
                }
            }
        }
        let scroll_offset = match node.kind {
            Kind::ScrollView { offset, .. } => Some(offset),
            _ => None,
        };
        if let Some(offset) = scroll_offset {
            painter.save();
            painter
                .clip_rect(node.bounds)
                .map_err(|error| UiError::new(error.to_string()))?;
            painter
                .transform(Affine2::from_translation(Vec2::new(0.0, -offset)))
                .map_err(|error| UiError::new(error.to_string()))?;
        }
        let mut children = node
            .children
            .iter()
            .copied()
            .enumerate()
            .collect::<Vec<_>>();
        children.sort_by_key(|(index, child)| {
            (self.node(*child).map_or(0, |node| node.z_index), *index)
        });
        for (_, child) in children {
            if matches!(self.node(child)?.kind, Kind::Overlay { .. }) {
                continue;
            }
            self.paint_node(child, painter)?;
        }
        if scroll_offset.is_some() {
            painter
                .restore()
                .map_err(|error| UiError::new(error.to_string()))?;
            if let Kind::ScrollView {
                offset,
                content_height,
            } = &node.kind
                && *content_height > node.bounds.size.height + f32::EPSILON
            {
                let style = self.scroll_styles[&id];
                let width = style.width.max(1.0);
                let track = Rect::from_xywh(
                    node.bounds.max_x() - width,
                    node.bounds.origin.y,
                    width,
                    node.bounds.size.height,
                );
                painter
                    .fill_rounded_rect(
                        RoundedRect::new(track, CornerRadii::uniform(width * 0.5))
                            .map_err(|error| UiError::new(error.to_string()))?,
                        Brush::Solid(style.track),
                    )
                    .map_err(|error| UiError::new(error.to_string()))?;
                let thumb_height = (node.bounds.size.height * node.bounds.size.height
                    / *content_height)
                    .max(24.0)
                    .min(node.bounds.size.height);
                let travel = node.bounds.size.height - thumb_height;
                let max_offset = *content_height - node.bounds.size.height;
                let thumb = Rect::from_xywh(
                    track.origin.x,
                    track.origin.y + travel * *offset / max_offset,
                    width,
                    thumb_height,
                );
                painter
                    .fill_rounded_rect(
                        RoundedRect::new(thumb, CornerRadii::uniform(width * 0.5))
                            .map_err(|error| UiError::new(error.to_string()))?,
                        Brush::Solid(style.thumb),
                    )
                    .map_err(|error| UiError::new(error.to_string()))?;
            }
        }
        painter
            .restore()
            .map_err(|error| UiError::new(error.to_string()))?;
        Ok(())
    }

    fn fill_control(
        &self,
        painter: &mut Painter,
        bounds: LogicalRect,
        color: Color,
    ) -> Result<(), UiError> {
        let rounded = RoundedRect::new(
            bounds,
            CornerRadii::uniform(self.theme.corner_radius.max(0.0)),
        )
        .map_err(|error| UiError::new(error.to_string()))?;
        painter
            .fill_rounded_rect(rounded, Brush::Solid(color))
            .map_err(|error| UiError::new(error.to_string()))
    }

    /// Returns a snapshot-friendly semantic tree.
    pub fn semantic_tree(&mut self) -> Result<SemanticNode, UiError> {
        self.ensure_layout()?;
        let tree = self.semantic_node(self.root)?;
        self.dirty.remove(Dirty::SEMANTICS);
        Ok(tree)
    }

    fn semantic_node(&self, id: ElementId) -> Result<SemanticNode, UiError> {
        let node = self.node(id)?;
        let (role, label, value, selection, actions) = match &node.kind {
            Kind::Label { text } => (SemanticRole::Label, text.clone(), None, None, vec![]),
            Kind::Button { text } => (
                SemanticRole::Button,
                text.clone(),
                None,
                None,
                vec![SemanticActionKind::Focus, SemanticActionKind::Activate],
            ),
            Kind::TextField(field) => (
                SemanticRole::TextField,
                field.placeholder.clone(),
                Some(if field.password {
                    "•".repeat(field.text.graphemes(true).count())
                } else {
                    field.text.clone()
                }),
                Some(field.selection()),
                vec![
                    SemanticActionKind::Focus,
                    SemanticActionKind::SetText,
                    SemanticActionKind::SetSelection,
                ],
            ),
            Kind::Checkbox { checked } => (
                SemanticRole::Checkbox,
                String::new(),
                Some(checked.to_string()),
                None,
                vec![SemanticActionKind::Focus, SemanticActionKind::Activate],
            ),
            Kind::Slider { value, .. } => (
                SemanticRole::Slider,
                String::new(),
                Some(value.to_string()),
                None,
                vec![SemanticActionKind::Focus, SemanticActionKind::SetValue],
            ),
            Kind::ScrollView { offset, .. } => (
                SemanticRole::ScrollView,
                String::new(),
                Some(offset.to_string()),
                None,
                vec![SemanticActionKind::Focus, SemanticActionKind::ScrollBy],
            ),
            Kind::Custom => self
                .custom_widgets
                .get(&id)
                .and_then(|widget| widget.semantics())
                .map_or(
                    (SemanticRole::Group, String::new(), None, None, vec![]),
                    |(role, label, value)| {
                        let actions = self
                            .custom_widgets
                            .get(&id)
                            .map_or_else(Vec::new, |widget| widget.semantic_actions());
                        (role, label, value, None, actions)
                    },
                ),
            _ => (SemanticRole::Group, String::new(), None, None, vec![]),
        };
        let role = self.semantic_roles.get(&id).copied().unwrap_or(role);
        Ok(SemanticNode {
            id,
            role,
            bounds: node.bounds,
            label,
            value,
            focusable: self.is_focusable_id(id),
            focused: self.focus == Some(id),
            enabled: self.is_effectively_interactive(id),
            selection,
            actions,
            children: node
                .children
                .iter()
                .filter(|child| {
                    self.node(**child)
                        .is_ok_and(|node| node.visibility == Visibility::Visible)
                })
                .map(|child| self.semantic_node(*child))
                .collect::<Result<_, _>>()?,
        })
    }

    /// Applies an accessibility/semantic action without a native adapter.
    pub fn perform_semantic_action(
        &mut self,
        target: ElementId,
        action: SemanticAction,
    ) -> Result<UiUpdate, UiError> {
        if matches!(self.node(target)?.kind, Kind::Custom) {
            let bounds = self.node(target)?.bounds;
            let parent_bounds = self
                .node(target)?
                .parent
                .and_then(|parent| self.node(parent).ok())
                .map(|parent| parent.bounds);
            let mut widget = self
                .custom_widgets
                .remove(&target)
                .ok_or_else(|| UiError::new("custom widget state is unavailable"))?;
            let mut stopped = false;
            let mut default_prevented = false;
            let current_world_transform = self.world_transform_for(target)?;
            let handled = widget.semantic_action(
                &mut EventContext {
                    messages: &mut self.messages,
                    stopped: &mut stopped,
                    default_prevented: &mut default_prevented,
                    current_target: target,
                    current_bounds: bounds,
                    current_world_transform,
                    parent_bounds,
                    modifiers: self.modifiers,
                    route: &[],
                    requests: &mut self.event_requests,
                },
                &action,
            );
            self.custom_widgets.insert(target, widget);
            if !handled {
                return Err(UiError::new(
                    "semantic action is unsupported by this widget",
                ));
            }
            self.apply_event_requests()?;
            return Ok(UiUpdate {
                redraw: self.needs_redraw(),
                platform_state_changed: true,
            });
        }
        match action {
            SemanticAction::Focus => self.set_focus(Some(target))?,
            SemanticAction::Activate => {
                if matches!(self.node(target)?.kind, Kind::Checkbox { .. }) {
                    self.toggle_checkbox_id(target)?;
                } else if matches!(self.node(target)?.kind, Kind::Button { .. }) {
                    if !self.dispatch_routed(target, RoutedEventKind::Activate)? {
                        self.events.push_back(UiEvent {
                            target,
                            kind: UiEventKind::ButtonActivated,
                        });
                    }
                } else {
                    return Err(UiError::new(
                        "semantic activation requires an activatable control",
                    ));
                }
            }
            SemanticAction::SetText(text) => {
                let length = self.text_field(target)?.text.len();
                self.replace_range(target, 0, length, &text)?;
            }
            SemanticAction::SetSelection { anchor, focus } => {
                let field = self.text_field(target)?;
                if anchor > field.text.len()
                    || focus > field.text.len()
                    || !field.text.is_char_boundary(anchor)
                    || !field.text.is_char_boundary(focus)
                {
                    return Err(UiError::new(
                        "semantic selection is not on valid UTF-8 boundaries",
                    ));
                }
                let field = self.text_field_mut(target)?;
                field.anchor.byte_index = anchor;
                field.caret.byte_index = focus;
                self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
            }
            SemanticAction::SetValue(value) => {
                let value = {
                    let Kind::Slider {
                        min,
                        max,
                        step,
                        value: current,
                    } = &mut self.node_mut(target)?.kind
                    else {
                        return Err(UiError::new("set-value semantics require a slider"));
                    };
                    *current = snap_slider(value, *min, *max, *step);
                    *current
                };
                self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
                self.dispatch_routed(target, RoutedEventKind::SliderChanged(value))?;
            }
            SemanticAction::ScrollBy(delta) => {
                self.scroll_by_id(target, delta)?;
            }
        }
        Ok(UiUpdate {
            redraw: self.needs_redraw(),
            platform_state_changed: true,
        })
    }

    fn update_drag(&mut self, device_id: DeviceId, position: LogicalPoint) -> Result<(), UiError> {
        let Some(mut session) = self.drag_sessions.remove(&device_id) else {
            return Ok(());
        };
        if !session.active {
            let delta = Vec2::new(position.x - session.start.x, position.y - session.start.y);
            if delta.length() < session.options.threshold {
                self.drag_sessions.insert(device_id, session);
                return Ok(());
            }
            session.active = true;
            self.dispatch_routed(
                session.source,
                RoutedEventKind::DragStarted {
                    session: session.id,
                    device_id,
                    position,
                    payload: session.payload.clone(),
                    allowed: session.options.allowed,
                },
            )?;
        }

        let candidate = self.hit_test(position);
        if candidate != session.candidate {
            if let Some(previous) = session.candidate {
                self.dispatch_routed(
                    previous,
                    RoutedEventKind::DragLeft {
                        session: session.id,
                        device_id,
                        position,
                        payload: session.payload.clone(),
                    },
                )?;
            }
            if let Some(candidate) = candidate {
                self.dispatch_routed(
                    candidate,
                    RoutedEventKind::DragEntered {
                        session: session.id,
                        device_id,
                        position,
                        payload: session.payload.clone(),
                        allowed: session.options.allowed,
                    },
                )?;
            }
            session.candidate = candidate;
        }

        self.drop_acceptance = None;
        if let Some(candidate) = candidate {
            self.dispatch_routed(
                candidate,
                RoutedEventKind::DragOver {
                    session: session.id,
                    device_id,
                    position,
                    payload: session.payload.clone(),
                    allowed: session.options.allowed,
                },
            )?;
        }
        session.accepted = self
            .drop_acceptance
            .take()
            .and_then(|(device, target, operation)| {
                (device == device_id && session.options.allowed.contains(operation.flag()))
                    .then_some((target, operation))
            });
        self.drag_sessions.insert(device_id, session);
        self.dirty |= Dirty::PAINT;
        Ok(())
    }

    fn finish_drag(
        &mut self,
        device_id: DeviceId,
        position: LogicalPoint,
    ) -> Result<bool, UiError> {
        let Some(session) = self.drag_sessions.remove(&device_id) else {
            return Ok(false);
        };
        if !session.active {
            return Ok(false);
        }
        if let Some(candidate) = session
            .candidate
            .filter(|target| self.node(*target).is_ok())
        {
            self.dispatch_routed(
                candidate,
                RoutedEventKind::DragLeft {
                    session: session.id,
                    device_id,
                    position,
                    payload: session.payload.clone(),
                },
            )?;
        }
        let outcome = if let Some((target, operation)) = session.accepted {
            self.dispatch_routed(
                target,
                RoutedEventKind::Dropped {
                    session: session.id,
                    device_id,
                    position,
                    payload: session.payload.clone(),
                    operation,
                },
            )?;
            DragOutcome::Dropped(operation)
        } else {
            DragOutcome::Cancelled
        };
        self.dispatch_routed(
            session.source,
            RoutedEventKind::DragEnded {
                session: session.id,
                device_id,
                outcome,
            },
        )?;
        if let Ok(node) = self.node_mut(session.source) {
            node.pressed = false;
        }
        self.capture.remove(&device_id);
        self.dirty |= Dirty::PAINT;
        Ok(true)
    }

    fn cancel_drag_id(&mut self, device_id: DeviceId) -> Result<(), UiError> {
        let Some(session) = self.drag_sessions.remove(&device_id) else {
            return Ok(());
        };
        if session.active
            && let Some(candidate) = session
                .candidate
                .filter(|target| self.node(*target).is_ok())
        {
            let position = self
                .pointer_positions
                .get(&device_id)
                .copied()
                .unwrap_or(session.start);
            self.dispatch_routed(
                candidate,
                RoutedEventKind::DragLeft {
                    session: session.id,
                    device_id,
                    position,
                    payload: session.payload.clone(),
                },
            )?;
        }
        if session.active && self.node(session.source).is_ok() {
            self.dispatch_routed(
                session.source,
                RoutedEventKind::DragEnded {
                    session: session.id,
                    device_id,
                    outcome: DragOutcome::Cancelled,
                },
            )?;
        }
        if let Ok(node) = self.node_mut(session.source) {
            node.pressed = false;
        }
        self.capture.remove(&device_id);
        self.dirty |= Dirty::PAINT;
        Ok(())
    }

    /// Routes one platform window event through the retained UI tree.
    pub fn handle_window_event(
        &mut self,
        window: &Window,
        clipboard: &Clipboard,
        event: &WindowEvent,
    ) -> Result<UiUpdate, UiError> {
        let was_dirty = self.needs_redraw();
        let mut platform_state_changed = false;
        match event {
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = *modifiers,
            WindowEvent::Focused(focused) => {
                self.window_focused = *focused;
                if !focused {
                    let dragging = self.drag_sessions.keys().copied().collect::<Vec<_>>();
                    for device_id in dragging {
                        self.cancel_drag_id(device_id)?;
                    }
                    self.capture.clear();
                    for id in self.all_ids() {
                        if let Ok(node) = self.node_mut(id) {
                            node.pressed = false;
                        }
                    }
                    self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
                }
                platform_state_changed = true;
            }
            WindowEvent::PointerMoved {
                device_id,
                position,
            } => {
                self.ensure_layout()?;
                let logical = Point::new(
                    position.x as f32 / self.scale_factor,
                    position.y as f32 / self.scale_factor,
                );
                self.pointer_positions.insert(*device_id, logical);
                let target = self
                    .capture
                    .get(device_id)
                    .copied()
                    .or_else(|| self.hit_test(logical));
                self.set_hover(*device_id, logical, self.hit_test(logical))?;
                if let Some(target) = target {
                    self.dispatch_routed(
                        target,
                        RoutedEventKind::PointerMoved {
                            device_id: *device_id,
                            position: logical,
                        },
                    )?;
                }
                self.update_drag(*device_id, logical)?;
                if let Some(target) = target
                    && self.capture.get(device_id) == Some(&target)
                {
                    if matches!(self.node(target)?.kind, Kind::TextField(_)) {
                        self.place_text_caret(target, logical, true)?;
                    } else if matches!(self.node(target)?.kind, Kind::Slider { .. }) {
                        self.set_slider_from_point(target, logical)?;
                    } else if matches!(self.node(target)?.kind, Kind::ScrollView { .. }) {
                        self.set_scroll_from_point(target, logical)?;
                    }
                }
            }
            WindowEvent::PointerLeft { device_id } => {
                let position = self
                    .pointer_positions
                    .get(device_id)
                    .copied()
                    .unwrap_or(LogicalPoint::ZERO);
                self.set_hover(*device_id, position, None)?;
            }
            WindowEvent::PointerButton {
                device_id,
                button: PointerButton::Primary,
                state,
            } => {
                self.ensure_layout()?;
                let position = self.pointer_positions.get(device_id).copied();
                match state {
                    ElementState::Pressed => {
                        let target = position.and_then(|point| self.hit_test(point));
                        if let Some(target) = target {
                            if self.dispatch_routed(
                                target,
                                RoutedEventKind::PointerButton {
                                    device_id: *device_id,
                                    position: position
                                        .expect("hit target requires a pointer position"),
                                    button: PointerButton::Primary,
                                    state: ElementState::Pressed,
                                },
                            )? {
                                return Ok(UiUpdate {
                                    redraw: self.needs_redraw(),
                                    platform_state_changed,
                                });
                            }
                            self.set_focus(Some(target))?;
                            self.capture.insert(*device_id, target);
                            self.node_mut(target)?.pressed = true;
                            if matches!(self.node(target)?.kind, Kind::TextField(_))
                                && let Some(position) = position
                            {
                                self.place_text_caret(target, position, self.modifiers.shift)?;
                            } else if matches!(self.node(target)?.kind, Kind::Slider { .. })
                                && let Some(position) = position
                            {
                                self.set_slider_from_point(target, position)?;
                            } else if matches!(self.node(target)?.kind, Kind::ScrollView { .. })
                                && let Some(position) = position
                            {
                                self.set_scroll_from_point(target, position)?;
                            }
                            self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
                        } else {
                            self.set_focus(None)?;
                        }
                        platform_state_changed = true;
                    }
                    ElementState::Released => {
                        if self.finish_drag(*device_id, position.unwrap_or(LogicalPoint::ZERO))? {
                            self.sync_platform_state(window)?;
                            return Ok(UiUpdate {
                                redraw: true,
                                platform_state_changed,
                            });
                        }
                        if let Some(target) = self.capture.remove(device_id) {
                            let prevented = self.dispatch_routed(
                                target,
                                RoutedEventKind::PointerButton {
                                    device_id: *device_id,
                                    position: position.unwrap_or(LogicalPoint::ZERO),
                                    button: PointerButton::Primary,
                                    state: ElementState::Released,
                                },
                            )?;
                            self.node_mut(target)?.pressed = false;
                            if !prevented
                                && position.and_then(|point| self.hit_test(point)) == Some(target)
                            {
                                if matches!(self.node(target)?.kind, Kind::Button { .. }) {
                                    if !self.dispatch_routed(target, RoutedEventKind::Activate)? {
                                        self.events.push_back(UiEvent {
                                            target,
                                            kind: UiEventKind::ButtonActivated,
                                        });
                                    }
                                } else if matches!(self.node(target)?.kind, Kind::Checkbox { .. }) {
                                    self.toggle_checkbox_id(target)?;
                                }
                            }
                            self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
                        }
                    }
                }
            }
            WindowEvent::KeyboardInput(input) if input.state == ElementState::Pressed => {
                if matches!(input.logical_key, Key::Named(NamedKey::Escape))
                    && !self.drag_sessions.is_empty()
                {
                    let dragging = self.drag_sessions.keys().copied().collect::<Vec<_>>();
                    for device_id in dragging {
                        self.cancel_drag_id(device_id)?;
                    }
                } else if matches!(input.logical_key, Key::Named(NamedKey::Tab)) {
                    self.move_focus(!self.modifiers.shift)?;
                    platform_state_changed = true;
                } else if let Some(focus) = self.focus {
                    if self.dispatch_routed(focus, RoutedEventKind::Keyboard(input.clone()))? {
                        self.sync_platform_state(window)?;
                        return Ok(UiUpdate {
                            redraw: self.needs_redraw(),
                            platform_state_changed,
                        });
                    }
                    match self.node(focus)?.kind {
                        Kind::Button { .. }
                            if matches!(
                                input.logical_key,
                                Key::Named(NamedKey::Enter | NamedKey::Space)
                            ) =>
                        {
                            if !self.dispatch_routed(focus, RoutedEventKind::Activate)? {
                                self.events.push_back(UiEvent {
                                    target: focus,
                                    kind: UiEventKind::ButtonActivated,
                                });
                            }
                            self.dirty |= Dirty::PAINT;
                        }
                        Kind::Checkbox { .. }
                            if matches!(input.logical_key, Key::Named(NamedKey::Space)) =>
                        {
                            self.toggle_checkbox_id(focus)?
                        }
                        Kind::Slider { .. } => self.handle_slider_key(focus, &input.logical_key)?,
                        Kind::ScrollView { .. } => {
                            self.handle_scroll_key(focus, &input.logical_key)?
                        }
                        Kind::TextField(_) => {
                            self.handle_text_key(focus, input, clipboard)?;
                            platform_state_changed = true;
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::PointerWheel {
                device_id, delta, ..
            } => {
                self.ensure_layout()?;
                if let Some(position) = self.pointer_positions.get(device_id).copied()
                    && let Some(target) = self.hit_test(position)
                {
                    let amount = match delta {
                        ScrollDelta::Lines { y, .. } => -*y * 40.0,
                        ScrollDelta::Pixels(point) => -(point.y as f32) / self.scale_factor,
                    };
                    if !self.dispatch_routed(
                        target,
                        RoutedEventKind::Scroll {
                            device_id: *device_id,
                            delta: Point::new(0.0, amount),
                        },
                    )? {
                        let mut current = Some(target);
                        while let Some(id) = current {
                            if matches!(self.node(id)?.kind, Kind::ScrollView { .. })
                                && self.scroll_by_id(id, amount)?
                            {
                                break;
                            }
                            current = self.node(id)?.parent;
                        }
                    }
                }
            }
            WindowEvent::Touch(touch) => {
                let device_id = DeviceId(touch.device_id.0 ^ touch.id.rotate_left(32));
                let logical = Point::new(
                    touch.position.x as f32 / self.scale_factor,
                    touch.position.y as f32 / self.scale_factor,
                );
                self.pointer_positions.insert(device_id, logical);
                match touch.phase {
                    TouchPhase::Started => {
                        if let Some(target) = self.hit_test(logical)
                            && !self.dispatch_routed(
                                target,
                                RoutedEventKind::PointerButton {
                                    device_id,
                                    position: logical,
                                    button: PointerButton::Primary,
                                    state: ElementState::Pressed,
                                },
                            )?
                        {
                            self.set_focus(Some(target))?;
                            self.capture.insert(device_id, target);
                            self.node_mut(target)?.pressed = true;
                            if matches!(self.node(target)?.kind, Kind::Slider { .. }) {
                                self.set_slider_from_point(target, logical)?;
                            } else if matches!(self.node(target)?.kind, Kind::ScrollView { .. }) {
                                self.set_scroll_from_point(target, logical)?;
                            }
                        }
                    }
                    TouchPhase::Moved => {
                        if let Some(target) = self.capture.get(&device_id).copied() {
                            self.dispatch_routed(
                                target,
                                RoutedEventKind::PointerMoved {
                                    device_id,
                                    position: logical,
                                },
                            )?;
                            self.update_drag(device_id, logical)?;
                            if matches!(self.node(target)?.kind, Kind::Slider { .. }) {
                                self.set_slider_from_point(target, logical)?;
                            } else if matches!(self.node(target)?.kind, Kind::ScrollView { .. }) {
                                self.set_scroll_from_point(target, logical)?;
                            }
                        }
                    }
                    TouchPhase::Ended => {
                        if self.finish_drag(device_id, logical)? {
                            self.dirty |= Dirty::PAINT;
                            self.sync_platform_state(window)?;
                            return Ok(UiUpdate {
                                redraw: true,
                                platform_state_changed,
                            });
                        }
                        if let Some(target) = self.capture.remove(&device_id) {
                            self.node_mut(target)?.pressed = false;
                            if !self.dispatch_routed(
                                target,
                                RoutedEventKind::PointerButton {
                                    device_id,
                                    position: logical,
                                    button: PointerButton::Primary,
                                    state: ElementState::Released,
                                },
                            )? && self.hit_test(logical) == Some(target)
                            {
                                if matches!(self.node(target)?.kind, Kind::Checkbox { .. }) {
                                    self.toggle_checkbox_id(target)?;
                                } else if matches!(self.node(target)?.kind, Kind::Button { .. }) {
                                    self.dispatch_routed(target, RoutedEventKind::Activate)?;
                                }
                            }
                        }
                    }
                    TouchPhase::Cancelled => {
                        self.cancel_drag_id(device_id)?;
                        if let Some(target) = self.capture.remove(&device_id) {
                            self.node_mut(target)?.pressed = false;
                            self.dispatch_routed(
                                target,
                                RoutedEventKind::PointerCancelled { device_id },
                            )?;
                        }
                    }
                }
                self.dirty |= Dirty::PAINT;
            }
            WindowEvent::Ime(ime) => {
                if let Some(focus) = self.focus
                    && matches!(self.node(focus)?.kind, Kind::TextField(_))
                {
                    if !self.dispatch_routed(focus, RoutedEventKind::Ime(ime.clone()))? {
                        self.handle_ime(focus, ime)?;
                    }
                    platform_state_changed = true;
                }
            }
            _ => {}
        }
        self.sync_platform_state(window)?;
        Ok(UiUpdate {
            redraw: self.needs_redraw() || (!was_dirty && self.dirty.contains(Dirty::PAINT)),
            platform_state_changed,
        })
    }

    fn hit_test(&self, point: LogicalPoint) -> Option<ElementId> {
        let mut overlays = self
            .all_ids()
            .into_iter()
            .filter(|id| {
                self.node(*id)
                    .is_ok_and(|node| matches!(node.kind, Kind::Overlay { .. }))
            })
            .collect::<Vec<_>>();
        overlays.sort_by_key(|id| self.node(*id).map_or(0, |node| node.z_index));
        for overlay in overlays.into_iter().rev() {
            let enabled = self
                .node(overlay)
                .ok()
                .and_then(|node| match node.kind {
                    Kind::Overlay { owner, .. } => Some(self.is_effectively_interactive(owner)),
                    _ => None,
                })
                .unwrap_or(true);
            if let Some(hit) = self.hit_test_node(overlay, point, enabled) {
                return Some(hit);
            }
        }
        self.hit_test_node(self.root, point, true)
    }

    fn hit_test_node(
        &self,
        id: ElementId,
        point: LogicalPoint,
        ancestors_enabled: bool,
    ) -> Option<ElementId> {
        let node = self.node(id).ok()?;
        if node.visibility != Visibility::Visible || !ancestors_enabled {
            return None;
        }
        let transform = node_local_transform(node);
        let determinant = transform.matrix2.determinant();
        if !determinant.is_finite() || determinant.abs() <= f32::EPSILON {
            return None;
        }
        let local = transform
            .inverse()
            .transform_point2(Vec2::new(point.x, point.y));
        let point = Point::new(local.x, local.y);
        if (node.overflow == Overflow::Clip || matches!(node.kind, Kind::ScrollView { .. }))
            && !node.bounds.contains(point)
        {
            return None;
        }
        if matches!(node.kind, Kind::ScrollView { content_height, .. } if content_height > node.bounds.size.height)
            && node.bounds.contains(point)
            && point.x >= node.bounds.max_x() - 12.0
        {
            return Some(id);
        }
        let child_point = match node.kind {
            Kind::ScrollView { offset, .. } => Point::new(point.x, point.y + offset),
            _ => point,
        };
        let mut children = node
            .children
            .iter()
            .copied()
            .enumerate()
            .collect::<Vec<_>>();
        children.sort_by_key(|(index, child)| {
            (self.node(*child).map_or(0, |node| node.z_index), *index)
        });
        for (_, child) in children.into_iter().rev() {
            if self
                .node(child)
                .is_ok_and(|node| matches!(node.kind, Kind::Overlay { .. }))
            {
                continue;
            }
            if let Some(hit) = self.hit_test_node(child, child_point, node.enabled) {
                return Some(hit);
            }
        }
        let shape_hit = self.custom_widgets.get(&id).map_or_else(
            || node.bounds.contains(point),
            |widget| widget.hit_test(point, node.bounds),
        );
        (node.enabled && shape_hit && self.is_hit_testable_id(id)).then_some(id)
    }

    fn set_hover(
        &mut self,
        device_id: DeviceId,
        position: LogicalPoint,
        target: Option<ElementId>,
    ) -> Result<(), UiError> {
        let old_path = self
            .hover_paths
            .get(&device_id)
            .cloned()
            .unwrap_or_default();
        let old = old_path.last().copied();
        if old == target {
            self.hover = target;
            return Ok(());
        }
        let new_path = target
            .map(|target| self.route_to(target))
            .transpose()?
            .unwrap_or_default();
        self.hover_paths.insert(device_id, new_path);
        self.hover = target;
        for id in self.all_ids() {
            let hovered = self.hover_paths.values().any(|path| path.contains(&id));
            self.node_mut(id)?.hovered = hovered;
        }
        if let Some(old) = old {
            self.dispatch_routed(
                old,
                RoutedEventKind::PointerLeft {
                    device_id,
                    position,
                    related_target: target,
                },
            )?;
        }
        if let Some(target) = target {
            self.dispatch_routed(
                target,
                RoutedEventKind::PointerEntered {
                    device_id,
                    position,
                    related_target: old,
                },
            )?;
        }
        self.dirty |= Dirty::PAINT;
        Ok(())
    }

    fn route_to(&self, target: ElementId) -> Result<Vec<ElementId>, UiError> {
        let mut route = Vec::new();
        let mut current = Some(target);
        while let Some(id) = current {
            route.push(id);
            current = self.node(id)?.parent;
        }
        route.reverse();
        Ok(route)
    }

    fn world_transform_for(&self, target: ElementId) -> Result<Affine2, UiError> {
        let mut world = Affine2::IDENTITY;
        for current in self.route_to(target)? {
            let node = self.node(current)?;
            world *= node_local_transform(node);
            if let Kind::ScrollView { offset, .. } = node.kind {
                world *= Affine2::from_translation(Vec2::new(0.0, -offset));
            }
        }
        Ok(world)
    }

    fn set_focus(&mut self, target: Option<ElementId>) -> Result<(), UiError> {
        let target = target.filter(|id| {
            self.node(*id)
                .is_ok_and(|_| self.is_effectively_interactive(*id) && self.is_focusable_id(*id))
        });
        if self.focus == target {
            return Ok(());
        }
        if let Some(old) = self.focus {
            self.events.push_back(UiEvent {
                target: old,
                kind: UiEventKind::FocusChanged(false),
            });
            if let Ok(node) = self.node_mut(old)
                && let Kind::TextField(field) = &mut node.kind
            {
                field.preedit.clear();
            }
            self.dispatch_routed(old, RoutedEventKind::FocusChanged(false))?;
        }
        self.focus = target;
        if let Some(target) = target {
            self.events.push_back(UiEvent {
                target,
                kind: UiEventKind::FocusChanged(true),
            });
            self.dispatch_routed(target, RoutedEventKind::FocusChanged(true))?;
            self.reveal_focused_descendant(target)?;
        }
        self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
        Ok(())
    }

    fn move_focus(&mut self, forward: bool) -> Result<(), UiError> {
        let trapped = self.focus.and_then(|focus| {
            self.route_to(focus).ok()?.into_iter().rev().find(|id| {
                self.node(*id).is_ok_and(|node| matches!(node.kind, Kind::FocusScope { options, .. } if options.trapped) || matches!(node.kind, Kind::Overlay { options, .. } if options.focus.trapped))
            })
        });
        let focusable = self
            .all_ids()
            .into_iter()
            .filter(|id| {
                self.is_effectively_interactive(*id)
                    && self.is_focusable_id(*id)
                    && trapped.is_none_or(|scope| self.is_descendant_of(*id, scope))
            })
            .collect::<Vec<_>>();
        if focusable.is_empty() {
            return self.set_focus(None);
        }
        let index = self
            .focus
            .and_then(|focus| focusable.iter().position(|id| *id == focus));
        let next = match (index, forward) {
            (Some(index), true) => focusable[(index + 1) % focusable.len()],
            (Some(0), false) | (None, false) => *focusable.last().expect("not empty"),
            (Some(index), false) => focusable[index - 1],
            (None, true) => focusable[0],
        };
        self.set_focus(Some(next))
    }

    fn is_effectively_interactive(&self, id: ElementId) -> bool {
        self.route_to(id).is_ok_and(|route| {
            route.into_iter().all(|id| {
                self.node(id)
                    .is_ok_and(|node| node.enabled && node.visibility == Visibility::Visible)
            })
        })
    }

    fn place_text_caret(
        &mut self,
        id: ElementId,
        point: LogicalPoint,
        extend: bool,
    ) -> Result<(), UiError> {
        let node = self.node(id)?;
        let layout = node
            .text_layout
            .clone()
            .ok_or_else(|| UiError::new("text field has not been measured"))?;
        let Kind::TextField(field) = &node.kind else {
            return Ok(());
        };
        let local = Point::new(
            point.x - node.bounds.origin.x - self.theme.control_padding.left
                + field.horizontal_offset,
            point.y - node.bounds.origin.y - self.theme.control_padding.top,
        );
        let position = from_layout_position(field, layout.hit_test(local).position);
        let node = self.node_mut(id)?;
        let Kind::TextField(field) = &mut node.kind else {
            return Ok(());
        };
        field.caret = position;
        if !extend {
            field.anchor = position;
        }
        self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
        Ok(())
    }

    fn handle_text_key(
        &mut self,
        id: ElementId,
        input: &astrelis_platform::KeyboardInput,
        clipboard: &Clipboard,
    ) -> Result<(), UiError> {
        let command = self.modifiers.control || self.modifiers.super_key;
        let character = match &input.logical_key {
            Key::Character(value) => Some(value.to_lowercase()),
            _ => None,
        };
        if command && character.as_deref() == Some("a") {
            let length = self.text_field(id)?.text.len();
            let field = self.text_field_mut(id)?;
            field.anchor.byte_index = 0;
            field.caret.byte_index = length;
            self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
            return Ok(());
        }
        if command && character.as_deref() == Some("c") {
            let field = self.text_field(id)?;
            if !field.password && clipboard.capabilities().write_text {
                let (start, end) = field.selection();
                if start != end {
                    clipboard
                        .write_text(field.text[start..end].to_owned())
                        .map_err(platform_error)?;
                }
            }
            return Ok(());
        }
        if command && character.as_deref() == Some("x") {
            let field = self.text_field(id)?.clone();
            if !field.password && clipboard.capabilities().write_text {
                let (start, end) = field.selection();
                if start != end {
                    clipboard
                        .write_text(field.text[start..end].to_owned())
                        .map_err(platform_error)?;
                    self.replace_selection(id, "")?;
                }
            }
            return Ok(());
        }
        if command && character.as_deref() == Some("v") {
            if clipboard.capabilities().read_text
                && let Some(text) = clipboard.read_text().map_err(platform_error)?
            {
                let text = text.replace(['\r', '\n'], " ");
                self.replace_selection(id, &text)?;
            }
            return Ok(());
        }

        match &input.logical_key {
            Key::Named(NamedKey::Backspace) => {
                let field = self.text_field(id)?.clone();
                let (start, end) = field.selection();
                if start != end {
                    self.replace_selection(id, "")?;
                } else if let Some(previous) = previous_grapheme(&field.text, start) {
                    self.replace_range(id, previous, start, "")?;
                }
            }
            Key::Named(NamedKey::Enter) => {
                let text = self.text_field(id)?.text.clone();
                self.dispatch_routed(id, RoutedEventKind::TextSubmitted(text.clone()))?;
                self.events.push_back(UiEvent {
                    target: id,
                    kind: UiEventKind::TextSubmitted(text),
                });
            }
            Key::Named(NamedKey::Other(name)) if name == "Delete" => {
                let field = self.text_field(id)?.clone();
                let (start, end) = field.selection();
                if start != end {
                    self.replace_selection(id, "")?;
                } else if let Some(next) = next_grapheme(&field.text, end) {
                    self.replace_range(id, end, next, "")?;
                }
            }
            Key::Named(NamedKey::Other(name))
                if matches!(name.as_str(), "ArrowLeft" | "ArrowRight" | "Home" | "End") =>
            {
                self.move_text_caret(id, name, self.modifiers.shift)?;
            }
            _ if !command && !self.modifiers.alt => {
                if let Some(text) = input.text.as_deref()
                    && !text.chars().any(char::is_control)
                {
                    self.replace_selection(id, text)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn move_text_caret(&mut self, id: ElementId, key: &str, extend: bool) -> Result<(), UiError> {
        self.ensure_layout()?;
        let node = self.node(id)?;
        let layout = node
            .text_layout
            .clone()
            .ok_or_else(|| UiError::new("text field has not been measured"))?;
        let field = self.text_field(id)?.clone();
        let movement = match key {
            "ArrowLeft" => CaretMovement::VisualLeft,
            "ArrowRight" => CaretMovement::VisualRight,
            "Home" => CaretMovement::LineStart,
            "End" => CaretMovement::LineEnd,
            _ => return Ok(()),
        };
        let position = from_layout_position(
            &field,
            layout.move_caret(to_layout_position(&field, field.caret), movement),
        );
        let field = self.text_field_mut(id)?;
        field.caret = position;
        if !extend {
            field.anchor = position;
        }
        self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
        Ok(())
    }

    fn handle_ime(&mut self, id: ElementId, event: &ImeEvent) -> Result<(), UiError> {
        match event {
            ImeEvent::Preedit(value, _) => {
                self.text_field_mut(id)?.preedit = value.clone();
                self.invalidate_layout();
            }
            ImeEvent::Commit(value) => {
                self.text_field_mut(id)?.preedit.clear();
                self.replace_selection(id, value)?;
            }
            ImeEvent::Disabled => {
                self.text_field_mut(id)?.preedit.clear();
                self.invalidate_layout();
            }
            ImeEvent::Enabled => {}
        }
        Ok(())
    }

    fn replace_selection(&mut self, id: ElementId, value: &str) -> Result<(), UiError> {
        let (start, end) = self.text_field(id)?.selection();
        self.replace_range(id, start, end, value)
    }

    fn replace_range(
        &mut self,
        id: ElementId,
        start: usize,
        end: usize,
        value: &str,
    ) -> Result<(), UiError> {
        let field = self.text_field_mut(id)?;
        field.text.replace_range(start..end, value);
        field.caret = TextPosition {
            byte_index: start + value.len(),
            ..Default::default()
        };
        field.anchor = field.caret;
        let text = field.text.clone();
        self.events.push_back(UiEvent {
            target: id,
            kind: UiEventKind::TextChanged(text.clone()),
        });
        self.dispatch_routed(id, RoutedEventKind::TextChanged(text))?;
        self.invalidate_layout();
        Ok(())
    }

    fn text_field(&self, id: ElementId) -> Result<&TextFieldState, UiError> {
        let Kind::TextField(field) = &self.node(id)?.kind else {
            return Err(UiError::new("element is not a text field"));
        };
        Ok(field)
    }

    fn text_field_mut(&mut self, id: ElementId) -> Result<&mut TextFieldState, UiError> {
        let Kind::TextField(field) = &mut self.node_mut(id)?.kind else {
            return Err(UiError::new("element is not a text field"));
        };
        Ok(field)
    }

    fn toggle_checkbox_id(&mut self, id: ElementId) -> Result<(), UiError> {
        let checked = {
            let Kind::Checkbox { checked } = &mut self.node_mut(id)?.kind else {
                return Err(UiError::new("element is not a checkbox"));
            };
            *checked = !*checked;
            *checked
        };
        self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
        self.dispatch_routed(id, RoutedEventKind::CheckedChanged(checked))?;
        Ok(())
    }

    fn set_slider_from_point(&mut self, id: ElementId, point: LogicalPoint) -> Result<(), UiError> {
        let bounds = self.node(id)?.bounds;
        let ratio = ((point.x - bounds.origin.x) / bounds.size.width.max(1.0)).clamp(0.0, 1.0);
        let value = {
            let Kind::Slider {
                min,
                max,
                step,
                value,
            } = &mut self.node_mut(id)?.kind
            else {
                return Err(UiError::new("element is not a slider"));
            };
            let next = snap_slider(*min + ratio * (*max - *min), *min, *max, *step);
            if *value == next {
                return Ok(());
            }
            *value = next;
            next
        };
        self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
        self.dispatch_routed(id, RoutedEventKind::SliderChanged(value))?;
        Ok(())
    }

    fn handle_slider_key(&mut self, id: ElementId, key: &Key) -> Result<(), UiError> {
        let name = match key {
            Key::Named(NamedKey::Other(name)) => name.as_str(),
            _ => return Ok(()),
        };
        let value = {
            let Kind::Slider {
                min,
                max,
                step,
                value,
            } = &mut self.node_mut(id)?.kind
            else {
                return Ok(());
            };
            let next = match name {
                "ArrowLeft" | "ArrowDown" => *value - *step,
                "ArrowRight" | "ArrowUp" => *value + *step,
                "Home" => *min,
                "End" => *max,
                _ => return Ok(()),
            };
            let next = snap_slider(next, *min, *max, *step);
            if next == *value {
                return Ok(());
            }
            *value = next;
            next
        };
        self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
        self.dispatch_routed(id, RoutedEventKind::SliderChanged(value))?;
        Ok(())
    }

    fn handle_scroll_key(&mut self, id: ElementId, key: &Key) -> Result<(), UiError> {
        let Key::Named(NamedKey::Other(name)) = key else {
            return Ok(());
        };
        let height = self.node(id)?.bounds.size.height;
        let delta = match name.as_str() {
            "ArrowUp" => -40.0,
            "ArrowDown" => 40.0,
            "PageUp" => -height * 0.9,
            "PageDown" => height * 0.9,
            "Home" => -f32::MAX,
            "End" => f32::MAX,
            _ => return Ok(()),
        };
        self.scroll_by_id(id, delta)?;
        Ok(())
    }

    fn set_scroll_from_point(&mut self, id: ElementId, point: LogicalPoint) -> Result<(), UiError> {
        let bounds = self.node(id)?.bounds;
        let (content_height, previous) = match self.node(id)?.kind {
            Kind::ScrollView {
                content_height,
                offset,
            } => (content_height, offset),
            _ => return Ok(()),
        };
        if content_height <= bounds.size.height || point.x < bounds.max_x() - 12.0 {
            return Ok(());
        }
        let thumb_height = (bounds.size.height * bounds.size.height / content_height)
            .max(24.0)
            .min(bounds.size.height);
        let travel = (bounds.size.height - thumb_height).max(1.0);
        let ratio = ((point.y - bounds.origin.y - thumb_height * 0.5) / travel).clamp(0.0, 1.0);
        let next = ratio * (content_height - bounds.size.height);
        if next != previous {
            if let Kind::ScrollView { offset, .. } = &mut self.node_mut(id)?.kind {
                *offset = next;
            }
            self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
        }
        Ok(())
    }

    fn scroll_by_id(&mut self, id: ElementId, delta: f32) -> Result<bool, UiError> {
        let viewport = self.node(id)?.bounds.size.height;
        let changed = {
            let Kind::ScrollView {
                offset,
                content_height,
            } = &mut self.node_mut(id)?.kind
            else {
                return Err(UiError::new("element is not a scroll view"));
            };
            let previous = *offset;
            *offset = (*offset + delta).clamp(0.0, (*content_height - viewport).max(0.0));
            previous != *offset
        };
        if changed {
            self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
        }
        Ok(changed)
    }

    fn reveal_focused_descendant(&mut self, target: ElementId) -> Result<(), UiError> {
        let target_bounds = self.node(target)?.bounds;
        let mut ancestor = self.node(target)?.parent;
        while let Some(id) = ancestor {
            if let Kind::ScrollView {
                offset,
                content_height,
            } = self.node(id)?.kind
            {
                let viewport = self.node(id)?.bounds;
                let mut next = offset;
                if target_bounds.origin.y < viewport.origin.y + offset {
                    next = target_bounds.origin.y - viewport.origin.y;
                } else if target_bounds.max_y() > viewport.max_y() + offset {
                    next = target_bounds.max_y() - viewport.max_y();
                }
                let max = (content_height - viewport.size.height).max(0.0);
                next = next.clamp(0.0, max);
                if next != offset {
                    if let Kind::ScrollView { offset, .. } = &mut self.node_mut(id)?.kind {
                        *offset = next;
                    }
                    self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
                }
            }
            ancestor = self.node(id)?.parent;
        }
        Ok(())
    }

    fn sync_platform_state(&mut self, window: &Window) -> Result<(), UiError> {
        self.ensure_layout()?;
        let drag_cursor = self
            .drag_sessions
            .values()
            .find(|session| session.active)
            .map(|session| {
                if session.accepted.is_some() {
                    CursorIcon::Move
                } else {
                    CursorIcon::NotAllowed
                }
            });
        let cursor = drag_cursor.unwrap_or_else(|| {
            self.hover
                .and_then(|leaf| self.route_to(leaf).ok())
                .and_then(|route| {
                    route.into_iter().rev().find_map(|id| {
                        let node = self.node(id).ok()?;
                        node.cursor
                            .or_else(|| {
                                self.custom_widgets
                                    .get(&id)
                                    .and_then(|widget| widget.cursor_icon())
                            })
                            .or(match node.kind {
                                Kind::Button { .. } => Some(CursorIcon::Pointer),
                                Kind::TextField(_) => Some(CursorIcon::Text),
                                _ => None,
                            })
                    })
                })
                .unwrap_or(CursorIcon::Default)
        });
        if self.applied_cursor != Some(cursor) {
            window.set_cursor_icon(cursor);
            self.applied_cursor = Some(cursor);
        }
        let Some(focus) = self.focus.filter(|_| self.window_focused) else {
            window.set_ime_allowed(false);
            return Ok(());
        };
        let node = self.node(focus)?;
        let Kind::TextField(field) = &node.kind else {
            window.set_ime_allowed(false);
            return Ok(());
        };
        window.set_ime_allowed(true);
        window.set_ime_purpose(if field.password {
            ImePurpose::Password
        } else {
            ImePurpose::Normal
        });
        if let Some(layout) = &node.text_layout {
            let caret = layout.caret_rect(to_layout_position(field, field.caret), 1.0);
            window.set_ime_cursor_area(Rect::from_xywh(
                (node.bounds.origin.x + self.theme.control_padding.left + caret.origin.x
                    - field.horizontal_offset) as f64,
                (node.bounds.origin.y + self.theme.control_padding.top + caret.origin.y) as f64,
                caret.size.width.max(1.0) as f64,
                caret.size.height as f64,
            ));
        }
        Ok(())
    }

    fn is_focusable_id(&self, id: ElementId) -> bool {
        self.node(id).is_ok_and(|node| match node.kind {
            Kind::Button { .. }
            | Kind::TextField(_)
            | Kind::Checkbox { .. }
            | Kind::Slider { .. }
            | Kind::ScrollView { .. } => true,
            Kind::Custom => self
                .custom_widgets
                .get(&id)
                .is_some_and(|widget| widget.focusable()),
            _ => false,
        })
    }

    fn is_hit_testable_id(&self, id: ElementId) -> bool {
        self.node(id).is_ok_and(|node| match node.kind {
            Kind::Button { .. }
            | Kind::TextField(_)
            | Kind::Checkbox { .. }
            | Kind::Slider { .. }
            | Kind::ScrollView { .. } => true,
            Kind::Custom => self
                .custom_widgets
                .get(&id)
                .is_some_and(|widget| widget.hit_testable()),
            _ => false,
        })
    }
}

fn map_alignment(alignment: Alignment) -> AlignItems {
    match alignment {
        Alignment::Start => AlignItems::FLEX_START,
        Alignment::Center => AlignItems::CENTER,
        Alignment::End => AlignItems::FLEX_END,
        Alignment::Stretch => AlignItems::STRETCH,
    }
}

fn node_local_transform(node: &Node) -> Affine2 {
    let pivot = Vec2::new(
        node.bounds.origin.x + node.transform_origin.x,
        node.bounds.origin.y + node.transform_origin.y,
    );
    Affine2::from_translation(pivot) * node.transform * Affine2::from_translation(-pivot)
}

fn transformed_bounds(rect: LogicalRect, transform: Affine2) -> LogicalRect {
    let points = [
        Vec2::new(rect.min_x(), rect.min_y()),
        Vec2::new(rect.max_x(), rect.min_y()),
        Vec2::new(rect.max_x(), rect.max_y()),
        Vec2::new(rect.min_x(), rect.max_y()),
    ]
    .map(|point| transform.transform_point2(point));
    let min_x = points
        .iter()
        .map(|point| point.x)
        .fold(f32::INFINITY, f32::min);
    let min_y = points
        .iter()
        .map(|point| point.y)
        .fold(f32::INFINITY, f32::min);
    let max_x = points
        .iter()
        .map(|point| point.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let max_y = points
        .iter()
        .map(|point| point.y)
        .fold(f32::NEG_INFINITY, f32::max);
    Rect::from_xywh(
        min_x,
        min_y,
        (max_x - min_x).max(0.0),
        (max_y - min_y).max(0.0),
    )
}

fn intersect_rect(a: LogicalRect, b: LogicalRect) -> LogicalRect {
    let x = a.min_x().max(b.min_x());
    let y = a.min_y().max(b.min_y());
    let max_x = a.max_x().min(b.max_x());
    let max_y = a.max_y().min(b.max_y());
    Rect::from_xywh(x, y, (max_x - x).max(0.0), (max_y - y).max(0.0))
}

fn scale_rect(rect: LogicalRect, scale: f32) -> PhysicalRect {
    Rect::from_xywh(
        rect.origin.x * scale,
        rect.origin.y * scale,
        rect.size.width * scale,
        rect.size.height * scale,
    )
}

fn route_is_visible<Message: 'static>(ui: &Ui<Message>, id: ElementId) -> bool {
    ui.route_to(id).is_ok_and(|route| {
        route.into_iter().all(|current| {
            ui.node(current)
                .is_ok_and(|node| node.visibility == Visibility::Visible)
        })
    })
}

fn apply_flex(style: &mut Style, flex: FlexStyle) {
    style.gap = TaffySize {
        width: LengthPercentage::length(flex.column_gap.max(0.0)),
        height: LengthPercentage::length(flex.row_gap.max(0.0)),
    };
    style.align_items = Some(map_alignment(flex.align_items));
    style.align_content = Some(match flex.align_content {
        Alignment::Start => AlignContent::FLEX_START,
        Alignment::Center => AlignContent::CENTER,
        Alignment::End => AlignContent::FLEX_END,
        Alignment::Stretch => AlignContent::STRETCH,
    });
    style.justify_content = Some(match flex.justify_content {
        Justification::Start => JustifyContent::FLEX_START,
        Justification::Center => JustifyContent::CENTER,
        Justification::End => JustifyContent::FLEX_END,
        Justification::SpaceBetween => JustifyContent::SPACE_BETWEEN,
        Justification::SpaceAround => JustifyContent::SPACE_AROUND,
        Justification::SpaceEvenly => JustifyContent::SPACE_EVENLY,
    });
    style.flex_wrap = match flex.wrap {
        FlexWrap::NoWrap => TaffyFlexWrap::NoWrap,
        FlexWrap::Wrap => TaffyFlexWrap::Wrap,
        FlexWrap::WrapReverse => TaffyFlexWrap::WrapReverse,
    };
}

fn snap_slider(value: f32, min: f32, max: f32, step: f32) -> f32 {
    let value = if value.is_finite() { value } else { min };
    (min + ((value.clamp(min, max) - min) / step).round() * step).clamp(min, max)
}

fn previous_grapheme(text: &str, index: usize) -> Option<usize> {
    text.grapheme_indices(true)
        .map(|(index, _)| index)
        .take_while(|candidate| *candidate < index)
        .last()
}

fn next_grapheme(text: &str, index: usize) -> Option<usize> {
    text.grapheme_indices(true)
        .map(|(index, _)| index)
        .find(|candidate| *candidate > index)
        .or_else(|| (index < text.len()).then_some(text.len()))
}

fn platform_error(error: PlatformError) -> UiError {
    UiError::new(format!("platform operation failed: {error}"))
}

fn to_layout_position(field: &TextFieldState, position: TextPosition) -> TextPosition {
    if !field.password {
        return TextPosition {
            byte_index: position.byte_index.min(field.text.len()),
            affinity: position.affinity,
        };
    }
    let graphemes = field.text[..position.byte_index.min(field.text.len())]
        .graphemes(true)
        .count();
    TextPosition {
        byte_index: graphemes * '•'.len_utf8(),
        affinity: position.affinity,
    }
}

fn from_layout_position(field: &TextFieldState, position: TextPosition) -> TextPosition {
    if !field.password {
        let mut index = position.byte_index.min(field.text.len());
        while !field.text.is_char_boundary(index) {
            index -= 1;
        }
        return TextPosition {
            byte_index: index,
            affinity: position.affinity,
        };
    }
    let target = position.byte_index / '•'.len_utf8();
    let byte_index = field
        .text
        .grapheme_indices(true)
        .nth(target)
        .map_or(field.text.len(), |(index, _)| index);
    TextPosition {
        byte_index,
        affinity: position.affinity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Default)]
    struct MemoryClipboard(Mutex<Option<String>>);

    impl astrelis_platform::backend::Clipboard for MemoryClipboard {
        fn capabilities(&self) -> astrelis_platform::ClipboardCapabilities {
            astrelis_platform::ClipboardCapabilities {
                read_text: true,
                write_text: true,
            }
        }

        fn read_text(&self) -> Result<Option<String>, PlatformError> {
            Ok(self.0.lock().unwrap().clone())
        }

        fn write_text(&self, text: String) -> Result<(), PlatformError> {
            *self.0.lock().unwrap() = Some(text);
            Ok(())
        }
    }

    #[derive(Debug)]
    struct UnsupportedClipboard;

    impl astrelis_platform::backend::Clipboard for UnsupportedClipboard {
        fn capabilities(&self) -> astrelis_platform::ClipboardCapabilities {
            astrelis_platform::ClipboardCapabilities::default()
        }

        fn read_text(&self) -> Result<Option<String>, PlatformError> {
            panic!("unsupported clipboard read should not be attempted")
        }

        fn write_text(&self, _text: String) -> Result<(), PlatformError> {
            panic!("unsupported clipboard write should not be attempted")
        }
    }

    fn key(logical_key: Key, text: Option<&str>) -> astrelis_platform::KeyboardInput {
        astrelis_platform::KeyboardInput {
            device_id: DeviceId(1),
            physical_key: astrelis_platform::PhysicalKey::Unidentified,
            logical_key,
            text: text.map(str::to_owned),
            location: astrelis_platform::KeyLocation::Standard,
            state: ElementState::Pressed,
            repeat: false,
            synthetic: false,
        }
    }

    fn ui() -> Ui {
        let mut ui = Ui::new(FontDatabase::default(), Theme::default());
        ui.set_viewport(Size::new(640.0, 480.0), 1.0);
        ui
    }

    #[test]
    fn handles_are_generational_and_subtree_removal_is_recursive() {
        let mut ui = ui();
        let root = ui.root();
        let row = ui.add_row(root).unwrap();
        let label = ui.add_label(row, "old").unwrap();
        ui.remove(row).unwrap();
        assert!(ui.set_label_text(label, "stale").is_err());
        let replacement = ui.add_label(root, "new").unwrap();
        assert_ne!(label.id(), replacement.id());
    }

    #[test]
    fn taffy_lays_out_rows_and_padding_without_leaking_types() {
        let mut ui = ui();
        let root = ui.root();
        let padding = ui.add_padding(root, Insets::all(20.0)).unwrap();
        let row = ui.add_row(padding).unwrap();
        ui.set_flex(row, 12.0, Alignment::Center).unwrap();
        let first = ui.add_button(row, "One").unwrap();
        let second = ui.add_button(row, "Two").unwrap();
        ui.ensure_layout().unwrap();
        let first_bounds = ui.node(first.id()).unwrap().bounds;
        let second_bounds = ui.node(second.id()).unwrap().bounds;
        assert!(first_bounds.origin.x >= 20.0);
        assert!(second_bounds.origin.x >= first_bounds.max_x() + 11.9);
    }

    #[test]
    fn semantic_tree_contains_roles_values_and_selection() {
        let mut ui = ui();
        let root = ui.root();
        let field = ui.add_text_field(root, "hello").unwrap();
        ui.set_placeholder(field, "Name").unwrap();
        ui.set_focus(Some(field.id())).unwrap();
        let tree = ui.semantic_tree().unwrap();
        let field_node = &tree.children[0];
        assert_eq!(field_node.role, SemanticRole::TextField);
        assert_eq!(field_node.label, "Name");
        assert_eq!(field_node.value.as_deref(), Some("hello"));
        assert!(field_node.focused);
        assert!(
            field_node
                .actions
                .contains(&SemanticActionKind::SetSelection)
        );
    }

    #[test]
    fn grapheme_deletion_does_not_split_unicode() {
        let mut ui = ui();
        let root = ui.root();
        let field = ui.add_text_field(root, "a👨‍👩‍👧‍👦").unwrap();
        let end = ui.text(field).unwrap().len();
        let previous = previous_grapheme(ui.text(field).unwrap(), end).unwrap();
        ui.replace_range(field.id(), previous, end, "").unwrap();
        assert_eq!(ui.text(field).unwrap(), "a");
        assert!(matches!(
            ui.drain_events().last().unwrap().kind,
            UiEventKind::TextChanged(ref value) if value == "a"
        ));
    }

    #[test]
    fn focus_traversal_and_button_activation_queue_events() {
        let mut ui = ui();
        let root = ui.root();
        let first = ui.add_button(root, "First").unwrap();
        let second = ui.add_button(root, "Second").unwrap();
        ui.move_focus(true).unwrap();
        assert_eq!(ui.focus, Some(first.id()));
        ui.move_focus(true).unwrap();
        assert_eq!(ui.focus, Some(second.id()));
        assert!(
            ui.drain_events()
                .any(|event| event.is_from(second) && event.kind == UiEventKind::FocusChanged(true))
        );
    }

    #[test]
    fn display_list_is_stable_when_read_repeatedly() {
        let mut ui = ui();
        let root = ui.root();
        ui.add_label(root, "Astrelis").unwrap();
        ui.add_button(root, "Save").unwrap();
        let first = ui.display_list().unwrap();
        assert!(!ui.needs_redraw());
        let second = ui.display_list().unwrap();
        assert_eq!(
            format!("{:?}", first.commands()),
            format!("{:?}", second.commands())
        );
    }

    #[test]
    fn clipboard_shortcuts_and_ime_replace_selection() {
        let mut ui = ui();
        let root = ui.root();
        let field = ui.add_text_field(root, "alpha").unwrap();
        ui.set_focus(Some(field.id())).unwrap();
        let clipboard = Clipboard::from_backend(Arc::new(MemoryClipboard::default()));
        {
            let state = ui.text_field_mut(field.id()).unwrap();
            state.anchor.byte_index = 0;
            state.caret.byte_index = state.text.len();
        }
        ui.modifiers.control = true;
        ui.handle_text_key(
            field.id(),
            &key(Key::Character("c".into()), None),
            &clipboard,
        )
        .unwrap();
        assert_eq!(clipboard.read_text().unwrap().as_deref(), Some("alpha"));
        clipboard.write_text("βeta").unwrap();
        ui.handle_text_key(
            field.id(),
            &key(Key::Character("v".into()), None),
            &clipboard,
        )
        .unwrap();
        assert_eq!(ui.text(field).unwrap(), "βeta");
        ui.modifiers.control = false;
        ui.handle_ime(field.id(), &ImeEvent::Preedit("中".into(), None))
            .unwrap();
        assert_eq!(ui.text_field(field.id()).unwrap().preedit, "中");
        ui.handle_ime(field.id(), &ImeEvent::Commit("中文".into()))
            .unwrap();
        assert_eq!(ui.text(field).unwrap(), "βeta中文");
    }

    #[test]
    fn unsupported_clipboard_shortcuts_are_noops() {
        let mut ui = ui();
        let root = ui.root();
        let field = ui.add_text_field(root, "alpha").unwrap();
        ui.set_focus(Some(field.id())).unwrap();
        {
            let state = ui.text_field_mut(field.id()).unwrap();
            state.anchor.byte_index = 0;
            state.caret.byte_index = state.text.len();
        }
        ui.modifiers.control = true;
        let clipboard = Clipboard::from_backend(Arc::new(UnsupportedClipboard));
        for shortcut in ["c", "x", "v"] {
            ui.handle_text_key(
                field.id(),
                &key(Key::Character(shortcut.into()), None),
                &clipboard,
            )
            .unwrap();
        }
        assert_eq!(ui.text(field).unwrap(), "alpha");
    }

    #[test]
    fn theme_font_family_resolves_an_embedded_font() {
        let mut fonts = FontDatabase::empty();
        fonts
            .register_font(Arc::<[u8]>::from(
                &include_bytes!("../assets/NotoSans.ttf")[..],
            ))
            .unwrap();
        let theme = Theme {
            font_families: vec![FontFamily::Named("Noto Sans".into())],
            ..Default::default()
        };
        let mut ui: Ui = Ui::new(fonts, theme);
        let root = ui.root();
        ui.add_label(root, "Astrelis on WebGPU").unwrap();
        assert!(!ui.display_list().unwrap().texts().is_empty());
    }

    #[test]
    fn bidi_caret_and_password_positions_round_trip() {
        let mut ui = ui();
        let root = ui.root();
        let field = ui.add_text_field(root, "hello אבג").unwrap();
        ui.set_focus(Some(field.id())).unwrap();
        ui.ensure_layout().unwrap();
        let before = ui.text_field(field.id()).unwrap().caret;
        ui.move_text_caret(field.id(), "ArrowLeft", false).unwrap();
        assert_ne!(ui.text_field(field.id()).unwrap().caret, before);
        ui.set_password(field, true).unwrap();
        ui.ensure_layout().unwrap();
        let state = ui.text_field(field.id()).unwrap();
        let layout_position = to_layout_position(state, state.caret);
        assert_eq!(from_layout_position(state, layout_position), state.caret);
    }

    #[derive(Debug, PartialEq)]
    enum TestMessage {
        Activated,
        Checked(bool),
    }

    struct Compound {
        content: Option<ElementHandle<Column>>,
        unmounted: Arc<Mutex<bool>>,
    }

    impl Widget<TestMessage> for Compound {
        fn as_any(&self) -> &dyn Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
        fn mounted(&mut self, context: &mut MountContext<'_, TestMessage>) -> Result<(), UiError> {
            context.add_label("Mounted")?;
            self.content = Some(context.add_column()?);
            Ok(())
        }
        fn unmounted(&mut self) {
            *self.unmounted.lock().unwrap() = true;
        }
        fn semantics(&self) -> Option<(SemanticRole, String, Option<String>)> {
            Some((SemanticRole::Group, "Compound".into(), None))
        }
    }

    #[test]
    fn custom_widget_mounts_children_and_unmounts_with_subtree() {
        let mut ui = Ui::<TestMessage>::new(FontDatabase::default(), Theme::default());
        let root = ui.root();
        let flag = Arc::new(Mutex::new(false));
        let widget = ui
            .add_widget(
                root,
                Compound {
                    content: None,
                    unmounted: flag.clone(),
                },
            )
            .unwrap();
        assert!(ui.widget(widget).unwrap().content.is_some());
        assert_eq!(ui.semantic_tree().unwrap().children[0].label, "Compound");
        ui.remove(widget).unwrap();
        assert!(*flag.lock().unwrap());
    }

    #[test]
    fn routed_listeners_emit_typed_messages_and_cancel_defaults() {
        let mut ui = Ui::<TestMessage>::new(FontDatabase::default(), Theme::default());
        let root = ui.root();
        let column = ui.add_column(root).unwrap();
        let checkbox = ui.add_checkbox(column, false).unwrap();
        ui.listen(
            column,
            Some(EventPhase::Capture),
            EventFilter::Activate,
            |context, _| {
                context.emit(TestMessage::Activated);
                context.prevent_default();
            },
        )
        .unwrap();
        assert!(
            ui.dispatch_routed(checkbox.id(), RoutedEventKind::Activate)
                .unwrap()
        );
        assert_eq!(
            ui.drain_messages().collect::<Vec<_>>(),
            vec![TestMessage::Activated]
        );
        assert!(!ui.checked(checkbox).unwrap());
        ui.listen(
            checkbox,
            None,
            EventFilter::ValueChanged,
            |context, event| {
                if let RoutedEventKind::CheckedChanged(value) = event.kind {
                    context.emit(TestMessage::Checked(value));
                }
            },
        )
        .unwrap();
        ui.toggle_checkbox_id(checkbox.id()).unwrap();
        assert_eq!(
            ui.drain_messages().collect::<Vec<_>>(),
            vec![TestMessage::Checked(true)]
        );
    }

    #[test]
    fn slider_snaps_and_scroll_view_clamps_and_reveals_focus() {
        let mut ui = Ui::<TestMessage>::new(FontDatabase::default(), Theme::default());
        ui.set_viewport(Size::new(320.0, 200.0), 1.0);
        let root = ui.root();
        let slider = ui.add_slider(root, 0.0, 1.0, 0.25, 0.6).unwrap();
        assert_eq!(ui.slider_value(slider).unwrap(), 0.5);
        let scroll = ui.add_scroll_view(root).unwrap();
        ui.set_layout(
            scroll,
            LayoutStyle {
                height: Length::Px(80.0),
                ..Default::default()
            },
        )
        .unwrap();
        let column = ui.add_column(scroll).unwrap();
        let mut last = None;
        for index in 0..8 {
            last = Some(ui.add_button(column, format!("Button {index}")).unwrap());
        }
        ui.ensure_layout().unwrap();
        assert!(
            matches!(ui.node(scroll.id()).unwrap().kind, Kind::ScrollView { content_height, .. } if content_height > 80.0)
        );
        ui.set_focus(last.map(|handle| handle.id())).unwrap();
        assert!(ui.scroll_offset(scroll).unwrap() > 0.0);
        ui.scroll_by_id(scroll.id(), f32::MAX).unwrap();
        let max = match ui.node(scroll.id()).unwrap().kind {
            Kind::ScrollView { content_height, .. } => content_height - 80.0,
            _ => unreachable!(),
        };
        assert!((ui.scroll_offset(scroll).unwrap() - max).abs() < 0.01);
        ui.set_scroll_offset(scroll, 0.0).unwrap();
        let bounds = ui.node(scroll.id()).unwrap().bounds;
        ui.set_scroll_from_point(
            scroll.id(),
            Point::new(bounds.origin.x + 20.0, bounds.origin.y + 60.0),
        )
        .unwrap();
        assert_eq!(ui.scroll_offset(scroll).unwrap(), 0.0);
        ui.set_scroll_from_point(
            scroll.id(),
            Point::new(bounds.max_x() - 2.0, bounds.origin.y + 60.0),
        )
        .unwrap();
        assert!(ui.scroll_offset(scroll).unwrap() > 0.0);
    }

    #[test]
    fn flex_scroll_view_shrinks_tracks_nested_overflow_and_clips_input() {
        let mut ui = Ui::<TestMessage>::new(FontDatabase::default(), Theme::default());
        ui.set_viewport(Size::new(320.0, 180.0), 1.0);
        let root = ui.root();
        let padding = ui.add_padding(root, Insets::all(12.0)).unwrap();
        ui.set_layout(
            padding,
            LayoutStyle {
                grow: 1.0,
                ..Default::default()
            },
        )
        .unwrap();
        let scroll = ui.add_scroll_view(padding).unwrap();
        ui.set_layout(
            scroll,
            LayoutStyle {
                grow: 1.0,
                ..Default::default()
            },
        )
        .unwrap();
        let content = ui.add_column(scroll).unwrap();
        let mut last = None;
        for index in 0..12 {
            last = Some(ui.add_button(content, format!("Item {index}")).unwrap());
        }
        ui.ensure_layout().unwrap();
        let scroll_bounds = ui.node(scroll.id()).unwrap().bounds;
        let last = last.unwrap();
        let last_bounds = ui.node(last.id()).unwrap().bounds;
        let content_height = match ui.node(scroll.id()).unwrap().kind {
            Kind::ScrollView { content_height, .. } => content_height,
            _ => unreachable!(),
        };
        assert!(
            scroll_bounds.size.height <= 156.0 + f32::EPSILON,
            "{scroll_bounds:?}"
        );
        assert!(content_height > scroll_bounds.size.height);
        assert!(last_bounds.origin.y > scroll_bounds.max_y());
        assert_ne!(
            ui.hit_test(Point::new(
                last_bounds.origin.x + 2.0,
                last_bounds.origin.y + 2.0
            )),
            Some(last.id())
        );
        assert!(ui.scroll_by_id(scroll.id(), 40.0).unwrap());
        assert!(ui.scroll_offset(scroll).unwrap() > 0.0);
        ui.scroll_by_id(scroll.id(), -f32::MAX).unwrap();
        for _ in 0..13 {
            ui.move_focus(true).unwrap();
        }
        assert_eq!(ui.focus, Some(last.id()));
        assert!(ui.scroll_offset(scroll).unwrap() > 0.0);
        assert!(
            ui.hit_test(Point::new(
                scroll_bounds.origin.x + 2.0,
                scroll_bounds.max_y() - 2.0
            ))
            .is_some()
        );
    }

    #[test]
    fn outer_scroll_extent_stops_at_nested_scroll_view_bounds() {
        let mut ui = ui();
        ui.set_viewport(Size::new(400.0, 260.0), 1.0);
        let root = ui.root();
        let outer = ui.add_scroll_view(root).unwrap();
        ui.set_layout(
            outer,
            LayoutStyle {
                height: Length::Px(200.0),
                ..Default::default()
            },
        )
        .unwrap();
        let column = ui.add_column(outer).unwrap();
        let inner = ui.add_scroll_view(column).unwrap();
        ui.set_layout(
            inner,
            LayoutStyle {
                height: Length::Px(100.0),
                shrink: 0.0,
                ..Default::default()
            },
        )
        .unwrap();
        let tall = ui.add_stack(inner).unwrap();
        ui.set_layout(
            tall,
            LayoutStyle {
                height: Length::Px(10_000.0),
                shrink: 0.0,
                ..Default::default()
            },
        )
        .unwrap();
        ui.add_label(column, "After nested scroll").unwrap();
        ui.ensure_layout().unwrap();
        let Kind::ScrollView { content_height, .. } = ui.node(outer.id()).unwrap().kind else {
            unreachable!()
        };
        assert!(
            content_height < 500.0,
            "outer extent leaked to {content_height}"
        );
    }

    #[test]
    fn rich_layout_supports_percent_constraints_wrapping_and_absolute_stack_children() {
        let mut ui = ui();
        let root = ui.root();
        let row = ui.add_row(root).unwrap();
        ui.set_layout(
            row,
            LayoutStyle {
                width: Length::Px(300.0),
                height: Length::Px(120.0),
                ..Default::default()
            },
        )
        .unwrap();
        ui.set_flex_style(
            row,
            FlexStyle {
                wrap: FlexWrap::Wrap,
                column_gap: 8.0,
                row_gap: 8.0,
                ..Default::default()
            },
        )
        .unwrap();
        for label in ["One", "Two", "Three"] {
            let button = ui.add_button(row, label).unwrap();
            ui.set_layout(
                button,
                LayoutStyle {
                    width: Length::Percent(0.48),
                    min_height: Length::Px(40.0),
                    ..Default::default()
                },
            )
            .unwrap();
        }
        let stack = ui.add_stack(root).unwrap();
        ui.set_layout(
            stack,
            LayoutStyle {
                width: Length::Px(100.0),
                height: Length::Px(60.0),
                ..Default::default()
            },
        )
        .unwrap();
        let back = ui.add_button(stack, "Back").unwrap();
        let front = ui.add_button(stack, "Front").unwrap();
        ui.set_z_index(front, 4).unwrap();
        ui.ensure_layout().unwrap();
        let third = ui.node(row.id()).unwrap().children[2];
        assert!(
            ui.node(third).unwrap().bounds.origin.y > ui.node(row.id()).unwrap().bounds.origin.y
        );
        let origin = ui.node(front.id()).unwrap().bounds.origin;
        let point = Point::new(origin.x + 5.0, origin.y + 5.0);
        assert_eq!(ui.hit_test(point), Some(front.id()));
        ui.set_visibility(front, Visibility::Hidden).unwrap();
        ui.ensure_layout().unwrap();
        assert_eq!(ui.hit_test(point), Some(back.id()));
    }

    #[test]
    fn transformed_hit_testing_clipping_and_effective_enablement_match_painting() {
        let mut ui = ui();
        let root = ui.root();
        let stack = ui.add_stack(root).unwrap();
        ui.set_layout(
            stack,
            LayoutStyle {
                width: Length::Px(100.0),
                height: Length::Px(100.0),
                ..Default::default()
            },
        )
        .unwrap();
        ui.set_overflow(stack, Overflow::Clip).unwrap();
        let button = ui.add_button(stack, "Moved").unwrap();
        ui.set_transform(
            button,
            Affine2::from_translation(Vec2::new(30.0, 0.0)),
            LogicalPoint::ZERO,
        )
        .unwrap();
        ui.ensure_layout().unwrap();
        let bounds = ui.node(button.id()).unwrap().bounds;
        assert_eq!(
            ui.hit_test(Point::new(bounds.origin.x + 35.0, bounds.origin.y + 5.0)),
            Some(button.id())
        );
        assert_eq!(
            ui.hit_test(Point::new(bounds.max_x() + 20.0, bounds.origin.y + 5.0)),
            None
        );
        ui.set_enabled(stack, false).unwrap();
        assert_eq!(
            ui.hit_test(Point::new(bounds.origin.x + 35.0, bounds.origin.y + 5.0)),
            None
        );
    }

    #[test]
    fn focus_scope_restores_focus_and_overlay_is_viewport_hosted() {
        let mut ui = ui();
        let root = ui.root();
        let owner = ui.add_button(root, "Owner").unwrap();
        ui.set_focus(Some(owner.id())).unwrap();
        let overlay = ui
            .add_overlay(
                owner,
                OverlayOptions {
                    focus: FocusScopeOptions {
                        trapped: true,
                        autofocus: false,
                        restore_focus: true,
                    },
                    ..Default::default()
                },
            )
            .unwrap();
        ui.set_layout(
            overlay,
            LayoutStyle {
                width: Length::Px(140.0),
                height: Length::Px(80.0),
                ..Default::default()
            },
        )
        .unwrap();
        let action = ui.add_button(overlay, "Action").unwrap();
        ui.set_focus(Some(action.id())).unwrap();
        ui.ensure_layout().unwrap();
        assert!(
            ui.node(overlay.id()).unwrap().bounds.origin.y
                >= ui.node(owner.id()).unwrap().bounds.max_y()
        );
        let inspection = ui.inspect().unwrap();
        assert!(
            inspection
                .nodes
                .iter()
                .any(|node| node.id == overlay.id() && !node.focused)
        );
        ui.remove(overlay).unwrap();
        assert_eq!(ui.focus, Some(owner.id()));
    }

    #[test]
    fn overlay_children_do_not_collapse_intrinsic_button_owners() {
        let mut ui = ui();
        let root = ui.root();
        let owner = ui.add_button(root, "A reasonably wide owner").unwrap();
        let before = ui.layout_bounds(owner).unwrap();
        let overlay = ui.add_overlay(owner, OverlayOptions::default()).unwrap();
        ui.add_label(overlay, "Overlay content").unwrap();
        let after = ui.layout_bounds(owner).unwrap();
        assert!(before.size.width > 100.0);
        assert!(after.size.width >= before.size.width);
        assert!(after.size.height >= before.size.height);
    }

    #[test]
    fn inspection_and_public_hit_test_are_deterministic() {
        let mut ui = ui();
        let root = ui.root();
        let button = ui.add_button(root, "Inspect").unwrap();
        let first = ui.inspect().unwrap();
        let second = ui.inspect().unwrap();
        assert_eq!(first, second);
        let bounds = first
            .nodes
            .iter()
            .find(|node| node.id == button.id())
            .unwrap()
            .world_bounds;
        assert_eq!(
            ui.hit_test_at(Point::new(bounds.origin.x + 1.0, bounds.origin.y + 1.0))
                .unwrap(),
            Some(button.id())
        );
    }

    #[test]
    fn drag_threshold_routes_drop_and_reports_outcome() {
        let mut ui = ui();
        ui.set_viewport(Size::new(500.0, 200.0), 1.0);
        let root = ui.root();
        let row = ui.add_row(root).unwrap();
        let source = ui.add_button(row, "source").unwrap();
        let target = ui.add_button(row, "target").unwrap();
        for handle in [source, target] {
            ui.set_layout(
                handle,
                LayoutStyle {
                    width: Length::Px(180.0),
                    height: Length::Px(80.0),
                    ..Default::default()
                },
            )
            .unwrap();
        }
        let device = DeviceId(9);
        ui.listen(source, None, EventFilter::Pointer, move |context, event| {
            if let RoutedEventKind::PointerButton {
                position,
                state: ElementState::Pressed,
                ..
            } = event.kind
            {
                context.begin_drag(
                    device,
                    position,
                    DragPayload::new(42_u32),
                    DragOptions {
                        threshold: 5.0,
                        allowed: DragOperations::MOVE,
                    },
                );
            }
        })
        .unwrap();
        let dropped = Arc::new(Mutex::new(Vec::new()));
        let dropped_listener = dropped.clone();
        ui.listen(
            target,
            None,
            EventFilter::Drag,
            move |context, event| match &event.kind {
                RoutedEventKind::DragOver {
                    device_id, payload, ..
                } if payload.downcast_ref::<u32>() == Some(&42) => {
                    context.accept_drop(*device_id, DropOperation::Move);
                }
                RoutedEventKind::Dropped {
                    payload, operation, ..
                } => dropped_listener
                    .lock()
                    .unwrap()
                    .push((*payload.downcast_ref::<u32>().unwrap(), *operation)),
                _ => {}
            },
        )
        .unwrap();
        let outcomes = Arc::new(Mutex::new(Vec::new()));
        let outcome_listener = outcomes.clone();
        ui.listen(source, None, EventFilter::Drag, move |_, event| {
            if let RoutedEventKind::DragEnded { outcome, .. } = event.kind {
                outcome_listener.lock().unwrap().push(outcome);
            }
        })
        .unwrap();

        ui.ensure_layout().unwrap();
        let source_point = ui.node(source.id()).unwrap().bounds.origin;
        let target_bounds = ui.node(target.id()).unwrap().bounds;
        let target_point = Point::new(target_bounds.origin.x + 20.0, target_bounds.origin.y + 20.0);
        ui.dispatch_routed(
            source.id(),
            RoutedEventKind::PointerButton {
                device_id: device,
                position: source_point,
                button: PointerButton::Primary,
                state: ElementState::Pressed,
            },
        )
        .unwrap();
        ui.update_drag(device, Point::new(source_point.x + 2.0, source_point.y))
            .unwrap();
        assert!(
            ui.drag_sessions
                .get(&device)
                .is_some_and(|drag| !drag.active)
        );
        ui.update_drag(device, target_point).unwrap();
        assert!(
            ui.drag_sessions
                .get(&device)
                .is_some_and(|drag| drag.active)
        );
        assert!(ui.finish_drag(device, target_point).unwrap());
        assert_eq!(*dropped.lock().unwrap(), vec![(42, DropOperation::Move)]);
        assert_eq!(
            *outcomes.lock().unwrap(),
            vec![DragOutcome::Dropped(DropOperation::Move)]
        );
    }

    #[test]
    fn default_root_button_targets_window_origin() {
        let mut ui = ui();
        let button = ui.add_button(ui.root(), "Save").unwrap();
        ui.ensure_layout().unwrap();
        assert_eq!(ui.hit_test(Point::new(5.0, 5.0)), Some(button.id()));
    }

    #[test]
    fn hover_paths_route_enter_leave_and_retarget_after_layout() {
        let mut ui = Ui::<TestMessage>::new(FontDatabase::default(), Theme::default());
        ui.set_viewport(Size::new(300.0, 200.0), 1.0);
        let root = ui.root();
        let parent = ui.add_column(root).unwrap();
        let button = ui.add_button(parent, "Hover").unwrap();
        let transitions = Arc::new(Mutex::new(Vec::new()));
        let observed = transitions.clone();
        ui.listen(button, None, EventFilter::Pointer, move |_, event| {
            if matches!(event.kind, RoutedEventKind::PointerEntered { .. }) {
                observed.lock().unwrap().push("enter");
            }
            if matches!(event.kind, RoutedEventKind::PointerLeft { .. }) {
                observed.lock().unwrap().push("leave");
            }
        })
        .unwrap();
        ui.ensure_layout().unwrap();
        let bounds = ui.node(button.id()).unwrap().bounds;
        let point = Point::new(bounds.origin.x + 2.0, bounds.origin.y + 2.0);
        let device = DeviceId(9);
        ui.pointer_positions.insert(device, point);
        ui.set_hover(device, point, Some(button.id())).unwrap();
        assert!(ui.is_hovered(parent).unwrap());
        ui.set_transform(
            button,
            Affine2::from_translation(Vec2::new(500.0, 0.0)),
            LogicalPoint::ZERO,
        )
        .unwrap();
        ui.invalidate_layout();
        ui.ensure_layout().unwrap();
        assert_eq!(&*transitions.lock().unwrap(), &["enter", "leave"]);
        assert!(!ui.is_hovered(parent).unwrap());
    }
}
