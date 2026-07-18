//! Retained, backend-independent UI tree, layout, events, semantics, and paint.

#![warn(missing_docs)]

use std::{
    any::Any,
    collections::{HashMap, HashSet, VecDeque},
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
    Affinity, CaretMovement, FontDatabase, ParagraphStyle, TextLayout, TextLayoutContext,
    TextLayoutRequest, TextPosition, TextWrap,
};
use bitflags::bitflags;
use taffy::prelude::{
    AlignContent, AlignItems, AvailableSpace, Dimension, Display, FlexDirection,
    FlexWrap as TaffyFlexWrap, JustifyContent, LengthPercentage, LengthPercentageAuto, NodeId,
    Position as TaffyPosition, Rect as TaffyRect, Size as TaffySize, Style, TaffyTree,
};
use taffy::style::Overflow as TaffyOverflow;
use unicode_segmentation::UnicodeSegmentation;

mod a11y;
mod controls;
mod drag;
mod error;
mod event;
mod input;
mod inspect;
mod layout;
mod overlay;
mod paint;
mod props;
mod style;
mod text;
mod tree;
mod util;
mod widget;
mod window;

#[cfg(test)]
mod tests;

pub use a11y::*;
pub use error::*;
pub use event::*;
pub use inspect::*;
pub use layout::*;
pub use overlay::*;
pub use style::*;
pub use tree::*;
pub use widget::*;

pub(crate) use util::*;

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) struct Dirty: u8 {
        const MEASURE = 1 << 0;
        const LAYOUT = 1 << 1;
        const PAINT = 1 << 2;
        const SEMANTICS = 1 << 3;
    }
}

/// Persistent UI tree associated with one native window.
pub struct Ui<Message = ()> {
    pub(crate) slots: Vec<Slot>,
    pub(crate) free: Vec<u32>,
    pub(crate) taffy_cache: TaffyCache,
    pub(crate) root: ElementId,
    pub(crate) theme: Theme,
    pub(crate) fonts: FontDatabase,
    pub(crate) text_context: TextLayoutContext,
    pub(crate) viewport: LogicalSize,
    pub(crate) scale_factor: f32,
    pub(crate) dirty: Dirty,
    /// Nodes whose text or layout style changed since the last layout pass, so
    /// the measure-input sweeps (text shaping, Taffy style reconciliation) can
    /// revisit only these instead of the whole tree. Ignored when
    /// `measure_resweep` is set. Keyed by generational id, so a recycled slot
    /// never inherits a stale entry.
    pub(crate) dirty_nodes: HashSet<ElementId>,
    /// Forces the measure-input sweeps to revisit every node. Set by changes
    /// that can affect many nodes at once (theme, viewport) or a node the
    /// caller cannot cheaply name (a custom widget resizing itself).
    pub(crate) measure_resweep: bool,
    pub(crate) focus: Option<ElementId>,
    pub(crate) hover: Option<ElementId>,
    pub(crate) hover_paths: HashMap<DeviceId, Vec<ElementId>>,
    pub(crate) capture: HashMap<DeviceId, ElementId>,
    pub(crate) pointer_positions: HashMap<DeviceId, LogicalPoint>,
    pub(crate) modifiers: Modifiers,
    pub(crate) window_focused: bool,
    pub(crate) applied_cursor: Option<CursorIcon>,
    pub(crate) events: VecDeque<UiEvent>,
    pub(crate) messages: VecDeque<Message>,
    pub(crate) listeners: HashMap<ElementId, Vec<Listener<Message>>>,
    pub(crate) next_listener: u64,
    pub(crate) custom_widgets: HashMap<ElementId, Box<dyn Widget<Message>>>,
    pub(crate) semantic_roles: HashMap<ElementId, SemanticRole>,
    pub(crate) event_requests: Vec<EventRequest>,
    pub(crate) drag_sessions: HashMap<DeviceId, DragSession>,
    pub(crate) next_drag_session: u64,
    pub(crate) drop_acceptance: Option<(DeviceId, ElementId, DropOperation)>,
}

pub(crate) struct Listener<Message> {
    pub(crate) id: ListenerId,
    pub(crate) phase: Option<EventPhase>,
    pub(crate) filter: EventFilter,
    pub(crate) callback: Box<EventCallback<Message>>,
}

pub(crate) type EventCallback<Message> = dyn FnMut(&mut EventContext<'_, Message>, &RoutedEvent);

pub(crate) struct DispatchControl<'a> {
    pub(crate) route: &'a [ElementId],
    pub(crate) stopped: bool,
    pub(crate) default_prevented: bool,
}
