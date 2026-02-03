//! Widget type registry for dispatching render/measure/event operations.
//!
//! Instead of O(N) downcast chains, the registry provides O(1) lookup of
//! per-widget-type handler functions via `TypeId`.

use crate::clip::ClipRect;
use crate::draw_list::DrawCommand;
use crate::style::Overflow;
use crate::theme::ColorPalette;
use astrelis_core::alloc::HashMap;
use astrelis_core::math::Vec2;
use astrelis_text::TextPipeline;
use std::any::TypeId;

/// How a tree traversal should proceed for a widget's children.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraversalBehavior {
    /// Recurse into all children normally.
    Normal,
    /// Only recurse into the child at the given index (e.g. DockTabs active tab).
    OnlyChild(usize),
    /// Skip all children entirely.
    Skip,
}

/// Context provided to widget render functions.
///
/// Contains all data needed to generate draw commands for a widget.
pub struct WidgetRenderContext<'a> {
    /// Absolute position of the widget (after walking up the tree).
    pub abs_position: Vec2,
    /// Layout size from Taffy.
    pub layout_size: Vec2,
    /// Current clip rect for this widget.
    pub clip_rect: ClipRect,
    /// Theme color palette for resolving defaults.
    pub theme_colors: &'a ColorPalette,
    /// Text shaping pipeline (mutable for caching).
    pub text_pipeline: &'a mut TextPipeline,
}

/// Overflow behavior for a widget (used by clipping system).
pub struct WidgetOverflow {
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,
}

/// Response from a widget event handler to communicate back to the event system.
///
/// Handlers return this to request changes to event system state (e.g. focus)
/// that they can't modify directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResponse {
    /// No special action needed.
    None,
    /// Widget requests keyboard focus.
    RequestFocus,
    /// Widget releases keyboard focus.
    ReleaseFocus,
}

/// Descriptor holding per-widget-type handler functions.
///
/// Each function takes `&dyn Any` (or `&mut dyn Any`) and internally does a single
/// `downcast_ref::<T>().unwrap()` — this is safe because the `TypeId` key guarantees
/// the correct concrete type.
pub struct WidgetTypeDescriptor {
    /// Human-readable name for debugging/inspector.
    pub name: &'static str,

    /// Render the widget into draw commands.
    /// Arguments: (widget, render_context)
    pub render: Option<for<'a> fn(&dyn std::any::Any, &mut WidgetRenderContext<'a>) -> Vec<DrawCommand>>,

    /// Measure intrinsic content size.
    /// Arguments: (widget, available_space, font_renderer)
    pub measure: Option<fn(&dyn std::any::Any, Vec2, Option<&astrelis_text::FontRenderer>) -> Vec2>,

    /// Determine tree traversal behavior for this widget.
    pub traversal: Option<fn(&dyn std::any::Any) -> TraversalBehavior>,

    /// Get scroll offset (for widgets that scroll their children).
    pub scroll_offset: Option<fn(&dyn std::any::Any) -> Vec2>,

    /// Whether this widget clips its children.
    pub clips_children: Option<fn(&dyn std::any::Any) -> bool>,

    /// Get overflow behavior (for clipping system).
    pub overflow: Option<fn(&dyn std::any::Any) -> WidgetOverflow>,

    /// Whether measurement results should be cached (e.g. text shaping results).
    pub caches_measurement: bool,

    // -- Event handlers --

    /// Called when mouse enters (`true`) or leaves (`false`) the widget.
    pub on_hover: Option<fn(&mut dyn std::any::Any, bool)>,

    /// Called when widget is pressed (`true`) or released (`false`).
    pub on_press: Option<fn(&mut dyn std::any::Any, bool)>,

    /// Called when widget is clicked. Returns an [`EventResponse`] to request
    /// event system state changes (e.g. focus).
    pub on_click: Option<fn(&mut dyn std::any::Any) -> EventResponse>,

    /// Called on keyboard input when widget has focus.
    /// Arguments: (widget, physical_key)
    pub on_key_input:
        Option<fn(&mut dyn std::any::Any, &astrelis_winit::event::PhysicalKey) -> EventResponse>,

    /// Called on character input when widget has focus.
    pub on_char_input: Option<fn(&mut dyn std::any::Any, char)>,
}

impl WidgetTypeDescriptor {
    /// Create a minimal descriptor with just a name.
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            render: None,
            measure: None,
            traversal: None,
            scroll_offset: None,
            clips_children: None,
            overflow: None,
            caches_measurement: false,
            on_hover: None,
            on_press: None,
            on_click: None,
            on_key_input: None,
            on_char_input: None,
        }
    }

    /// Set the render function.
    pub fn with_render(
        mut self,
        f: for<'a> fn(&dyn std::any::Any, &mut WidgetRenderContext<'a>) -> Vec<DrawCommand>,
    ) -> Self {
        self.render = Some(f);
        self
    }

    /// Set the measure function.
    pub fn with_measure(
        mut self,
        f: fn(&dyn std::any::Any, Vec2, Option<&astrelis_text::FontRenderer>) -> Vec2,
    ) -> Self {
        self.measure = Some(f);
        self
    }

    /// Set the traversal function.
    pub fn with_traversal(mut self, f: fn(&dyn std::any::Any) -> TraversalBehavior) -> Self {
        self.traversal = Some(f);
        self
    }

    /// Set the scroll offset function.
    pub fn with_scroll_offset(mut self, f: fn(&dyn std::any::Any) -> Vec2) -> Self {
        self.scroll_offset = Some(f);
        self
    }

    /// Set the clips_children function.
    pub fn with_clips_children(mut self, f: fn(&dyn std::any::Any) -> bool) -> Self {
        self.clips_children = Some(f);
        self
    }

    /// Set the overflow function.
    pub fn with_overflow(mut self, f: fn(&dyn std::any::Any) -> WidgetOverflow) -> Self {
        self.overflow = Some(f);
        self
    }

    /// Mark this widget type as caching measurement results.
    pub fn with_caches_measurement(mut self) -> Self {
        self.caches_measurement = true;
        self
    }

    /// Set the hover handler (called on mouse enter/leave).
    pub fn with_on_hover(mut self, f: fn(&mut dyn std::any::Any, bool)) -> Self {
        self.on_hover = Some(f);
        self
    }

    /// Set the press handler (called on mouse press/release).
    pub fn with_on_press(mut self, f: fn(&mut dyn std::any::Any, bool)) -> Self {
        self.on_press = Some(f);
        self
    }

    /// Set the click handler.
    pub fn with_on_click(mut self, f: fn(&mut dyn std::any::Any) -> EventResponse) -> Self {
        self.on_click = Some(f);
        self
    }

    /// Set the key input handler.
    pub fn with_on_key_input(
        mut self,
        f: fn(&mut dyn std::any::Any, &astrelis_winit::event::PhysicalKey) -> EventResponse,
    ) -> Self {
        self.on_key_input = Some(f);
        self
    }

    /// Set the character input handler.
    pub fn with_on_char_input(mut self, f: fn(&mut dyn std::any::Any, char)) -> Self {
        self.on_char_input = Some(f);
        self
    }
}

/// Registry mapping `TypeId` → `WidgetTypeDescriptor` for O(1) dispatch.
pub struct WidgetTypeRegistry {
    descriptors: HashMap<TypeId, WidgetTypeDescriptor>,
}

impl WidgetTypeRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            descriptors: HashMap::default(),
        }
    }

    /// Register a descriptor for a concrete widget type `W`.
    pub fn register<W: 'static>(&mut self, descriptor: WidgetTypeDescriptor) {
        self.descriptors.insert(TypeId::of::<W>(), descriptor);
    }

    /// Look up the descriptor for the given `TypeId`.
    pub fn get(&self, type_id: TypeId) -> Option<&WidgetTypeDescriptor> {
        self.descriptors.get(&type_id)
    }

    /// Check whether a descriptor is registered for the given `TypeId`.
    pub fn contains(&self, type_id: TypeId) -> bool {
        self.descriptors.contains_key(&type_id)
    }

    /// Check whether the widget type identified by `type_id` caches measurements.
    pub fn caches_measurement(&self, type_id: TypeId) -> bool {
        self.descriptors
            .get(&type_id)
            .is_some_and(|desc| desc.caches_measurement)
    }

    /// Number of registered widget types.
    pub fn len(&self) -> usize {
        self.descriptors.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.descriptors.is_empty()
    }
}

impl Default for WidgetTypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}
