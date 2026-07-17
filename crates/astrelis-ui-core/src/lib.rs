//! Retained, backend-independent UI tree, layout, events, semantics, and paint.

#![warn(missing_docs)]

use std::{
    any::Any,
    collections::{HashMap, VecDeque},
    error::Error,
    fmt,
    marker::PhantomData,
};

use astrelis_core::{
    color::Color,
    geometry::{LogicalPoint, LogicalRect, LogicalSize, Point, Rect, Size},
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
    AlignItems, AvailableSpace, Dimension, Display, FlexDirection, LengthPercentage, NodeId,
    Size as TaffySize, Style, TaffyTree,
};
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
                            | Self::PointerButton { .. }
                            | Self::PointerCancelled { .. }
                    )
                    | (EventFilter::Keyboard, Self::Keyboard(_) | Self::Ime(_))
                    | (EventFilter::Scroll, Self::Scroll { .. })
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
    requests: &'a mut Vec<EventRequest>,
}

enum EventRequest {
    Focus(ElementId),
    Capture(DeviceId, ElementId),
    Release(DeviceId),
    Layout,
    Paint,
}

impl<Message> EventContext<'_, Message> {
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
    /// Whether this node participates in keyboard focus traversal.
    fn focusable(&self) -> bool {
        false
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
        let gap = self.ui.theme.gap;
        self.ui.insert(
            self.parent,
            Kind::Column {
                gap,
                alignment: Alignment::Stretch,
            },
        )
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

/// Optional per-element sizing constraints.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LayoutStyle {
    /// Preferred width.
    pub width: Option<f32>,
    /// Preferred height.
    pub height: Option<f32>,
    /// Minimum width.
    pub min_width: Option<f32>,
    /// Minimum height.
    pub min_height: Option<f32>,
    /// Flex growth factor.
    pub grow: f32,
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

#[derive(Clone, Debug)]
enum Kind {
    Label {
        text: String,
    },
    Button {
        text: String,
    },
    Row {
        gap: f32,
        alignment: Alignment,
    },
    Column {
        gap: f32,
        alignment: Alignment,
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
    bounds: LogicalRect,
    text_layout: Option<TextLayout>,
    hovered: bool,
    pressed: bool,
}

struct Slot {
    generation: u32,
    node: Option<Node>,
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
    capture: HashMap<DeviceId, ElementId>,
    pointer_positions: HashMap<DeviceId, LogicalPoint>,
    modifiers: Modifiers,
    window_focused: bool,
    events: VecDeque<UiEvent>,
    messages: VecDeque<Message>,
    listeners: HashMap<ElementId, Vec<Listener<Message>>>,
    next_listener: u64,
    custom_widgets: HashMap<ElementId, Box<dyn Widget<Message>>>,
    checkbox_styles: HashMap<ElementId, CheckboxStyle>,
    slider_styles: HashMap<ElementId, SliderStyle>,
    scroll_styles: HashMap<ElementId, ScrollViewStyle>,
    event_requests: Vec<EventRequest>,
}

struct Listener<Message> {
    id: ListenerId,
    phase: Option<EventPhase>,
    filter: EventFilter,
    callback: Box<EventCallback<Message>>,
}

type EventCallback<Message> = dyn FnMut(&mut EventContext<'_, Message>, &RoutedEvent);

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
                        gap: theme.gap,
                        alignment: Alignment::Stretch,
                    },
                    style: LayoutStyle::default(),
                    visual: WidgetStyle::default(),
                    enabled: true,
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
            capture: HashMap::new(),
            pointer_positions: HashMap::new(),
            modifiers: Modifiers::default(),
            window_focused: true,
            events: VecDeque::new(),
            messages: VecDeque::new(),
            listeners: HashMap::new(),
            next_listener: 1,
            custom_widgets: HashMap::new(),
            checkbox_styles: HashMap::new(),
            slider_styles: HashMap::new(),
            scroll_styles: HashMap::new(),
            event_requests: Vec::new(),
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
        let gap = self.theme.gap;
        self.insert(
            parent.id,
            Kind::Row {
                gap,
                alignment: Alignment::Center,
            },
        )
    }

    /// Adds a vertical flex container.
    pub fn add_column<T>(
        &mut self,
        parent: ElementHandle<T>,
    ) -> Result<ElementHandle<Column>, UiError> {
        let gap = self.theme.gap;
        self.insert(
            parent.id,
            Kind::Column {
                gap,
                alignment: Alignment::Stretch,
            },
        )
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
        let parent = self.node(handle.id)?.parent;
        if let Some(parent) = parent {
            self.node_mut(parent)?
                .children
                .retain(|child| *child != handle.id);
        }
        self.remove_subtree(handle.id);
        self.invalidate_layout();
        Ok(())
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
            Kind::Row {
                gap: current_gap,
                alignment: current_alignment,
            }
            | Kind::Column {
                gap: current_gap,
                alignment: current_alignment,
            } => {
                *current_gap = gap.max(0.0);
                *current_alignment = alignment;
                self.invalidate_layout();
                Ok(())
            }
            _ => Err(UiError::new("element is not a row or column")),
        }
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
        self.capture.retain(|_, captured| *captured != id);
        self.listeners.remove(&id);
        self.checkbox_styles.remove(&id);
        self.slider_styles.remove(&id);
        self.scroll_styles.remove(&id);
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
        let node = self.node_mut(handle.id)?;
        if node.enabled != enabled {
            node.enabled = enabled;
            self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
        }
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
        let mut stopped = false;
        let mut default_prevented = false;
        let last = route.len().saturating_sub(1);
        for (index, current) in route.iter().copied().enumerate() {
            let phase = if index == last {
                EventPhase::Target
            } else {
                EventPhase::Capture
            };
            self.deliver(
                current,
                target,
                phase,
                &kind,
                &mut stopped,
                &mut default_prevented,
            );
            if stopped {
                break;
            }
        }
        if !stopped {
            for current in route[..last].iter().rev().copied() {
                self.deliver(
                    current,
                    target,
                    EventPhase::Bubble,
                    &kind,
                    &mut stopped,
                    &mut default_prevented,
                );
                if stopped {
                    break;
                }
            }
        }
        self.apply_event_requests()?;
        Ok(default_prevented)
    }

    fn deliver(
        &mut self,
        current: ElementId,
        target: ElementId,
        phase: EventPhase,
        kind: &RoutedEventKind,
        stopped: &mut bool,
        default_prevented: &mut bool,
    ) {
        let event = RoutedEvent {
            target,
            current_target: current,
            phase,
            kind: kind.clone(),
        };
        if let Some(mut widget) = self.custom_widgets.remove(&current) {
            let mut context = EventContext {
                messages: &mut self.messages,
                stopped,
                default_prevented,
                current_target: current,
                requests: &mut self.event_requests,
            };
            widget.event(&mut context, &event);
            self.custom_widgets.insert(current, widget);
            if *stopped {
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
                    stopped,
                    default_prevented,
                    current_target: current,
                    requests: &mut self.event_requests,
                };
                (listener.callback)(&mut context, &event);
                if *stopped {
                    break;
                }
            }
        }
        self.listeners.insert(current, listeners);
    }

    fn apply_event_requests(&mut self) -> Result<(), UiError> {
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
            }
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
        let dimension = |value: Option<f32>| {
            value.map_or(Dimension::auto(), |value| Dimension::length(value.max(0.0)))
        };
        let mut style = Style {
            display: Display::Flex,
            size: TaffySize {
                width: dimension(node.style.width),
                height: dimension(node.style.height),
            },
            min_size: TaffySize {
                width: dimension(node.style.min_width),
                height: dimension(node.style.min_height),
            },
            flex_grow: node.style.grow.max(0.0),
            ..Default::default()
        };
        if node.parent.is_none() {
            style.size = TaffySize {
                width: Dimension::length(self.viewport.width.max(0.0)),
                height: Dimension::length(self.viewport.height.max(0.0)),
            };
        }
        match node.kind {
            Kind::Row { gap, alignment } => {
                style.flex_direction = FlexDirection::Row;
                style.gap.width = LengthPercentage::length(gap.max(0.0));
                style.align_items = Some(map_alignment(alignment));
            }
            Kind::Column { gap, alignment } => {
                style.flex_direction = FlexDirection::Column;
                style.gap.height = LengthPercentage::length(gap.max(0.0));
                style.align_items = Some(map_alignment(alignment));
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
        self.ensure_caret_visible()?;
        self.dirty.remove(Dirty::MEASURE | Dirty::LAYOUT);
        self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
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
                .filter_map(|child| self.node(*child).ok().map(|node| node.bounds.max_y()))
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
        let list = painter
            .finish()
            .map_err(|error| UiError::new(error.to_string()))?;
        self.dirty.remove(Dirty::PAINT);
        Ok(list)
    }

    fn paint_node(&self, id: ElementId, painter: &mut Painter) -> Result<(), UiError> {
        let node = self.node(id)?;
        match &node.kind {
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
                painter
                    .draw_text(layout, origin, 1.0)
                    .map_err(|error| UiError::new(error.to_string()))?;
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
        for child in &node.children {
            self.paint_node(*child, painter)?;
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
                    |(role, label, value)| (role, label, value, None, vec![]),
                ),
            _ => (SemanticRole::Group, String::new(), None, None, vec![]),
        };
        Ok(SemanticNode {
            id,
            role,
            bounds: node.bounds,
            label,
            value,
            focusable: self.is_focusable_id(id),
            focused: self.focus == Some(id),
            enabled: node.enabled,
            selection,
            actions,
            children: node
                .children
                .iter()
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
                self.set_hover(self.hit_test(logical))?;
                if let Some(target) = target {
                    self.dispatch_routed(
                        target,
                        RoutedEventKind::PointerMoved {
                            device_id: *device_id,
                            position: logical,
                        },
                    )?;
                }
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
                if !self.capture.contains_key(device_id) {
                    self.set_hover(None)?;
                }
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
                if matches!(input.logical_key, Key::Named(NamedKey::Tab)) {
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
                            if matches!(self.node(target)?.kind, Kind::Slider { .. }) {
                                self.set_slider_from_point(target, logical)?;
                            } else if matches!(self.node(target)?.kind, Kind::ScrollView { .. }) {
                                self.set_scroll_from_point(target, logical)?;
                            }
                        }
                    }
                    TouchPhase::Ended => {
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
        self.hit_test_node(self.root, point)
    }

    fn hit_test_node(&self, id: ElementId, point: LogicalPoint) -> Option<ElementId> {
        let node = self.node(id).ok()?;
        if !node.bounds.contains(point) {
            return None;
        }
        if matches!(node.kind, Kind::ScrollView { content_height, .. } if content_height > node.bounds.size.height)
            && point.x >= node.bounds.max_x() - 12.0
        {
            return Some(id);
        }
        let child_point = match node.kind {
            Kind::ScrollView { offset, .. } => Point::new(point.x, point.y + offset),
            _ => point,
        };
        for child in node.children.iter().rev() {
            if let Some(hit) = self.hit_test_node(*child, child_point) {
                return Some(hit);
            }
        }
        (node.enabled && self.is_hit_testable_id(id)).then_some(id)
    }

    fn set_hover(&mut self, target: Option<ElementId>) -> Result<(), UiError> {
        if self.hover == target {
            return Ok(());
        }
        if let Some(old) = self.hover
            && let Ok(node) = self.node_mut(old)
        {
            node.hovered = false;
        }
        self.hover = target;
        if let Some(target) = target {
            self.node_mut(target)?.hovered = true;
        }
        self.dirty |= Dirty::PAINT;
        Ok(())
    }

    fn set_focus(&mut self, target: Option<ElementId>) -> Result<(), UiError> {
        let target = target.filter(|id| {
            self.node(*id)
                .is_ok_and(|node| node.enabled && self.is_focusable_id(*id))
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
        let focusable = self
            .all_ids()
            .into_iter()
            .filter(|id| {
                self.node(*id)
                    .is_ok_and(|node| node.enabled && self.is_focusable_id(*id))
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
        if content_height <= bounds.size.height {
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
        window.set_cursor_icon(
            if self.hover.is_some_and(|id| {
                matches!(
                    self.node(id).map(|node| &node.kind),
                    Ok(Kind::Button { .. })
                )
            }) {
                CursorIcon::Pointer
            } else if self.hover.is_some_and(|id| {
                matches!(self.node(id).map(|node| &node.kind), Ok(Kind::TextField(_)))
            }) {
                CursorIcon::Text
            } else {
                CursorIcon::Default
            },
        );
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
                height: Some(80.0),
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
    }
}
