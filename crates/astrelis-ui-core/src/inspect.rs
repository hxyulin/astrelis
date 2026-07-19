//! Application-visible events and headless inspection output.

use super::*;

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

/// Stable category of one retained element for developer tooling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementKind {
    /// Static text.
    Label,
    /// Activatable button.
    Button,
    /// Horizontal flex container.
    Row,
    /// Vertical flex container.
    Column,
    /// Overlaying stack container.
    Stack,
    /// Keyboard focus scope.
    FocusScope,
    /// Viewport-hosted overlay.
    Overlay,
    /// Padding container.
    Padding,
    /// Single-line text editor.
    TextField,
    /// Boolean checkbox.
    Checkbox,
    /// Numeric slider.
    Slider,
    /// Vertically scrolling container.
    ScrollView,
    /// Application-defined widget.
    Custom,
}

impl ElementKind {
    fn from_kind(kind: &Kind) -> Self {
        match kind {
            Kind::Label { .. } => Self::Label,
            Kind::Button { .. } => Self::Button,
            Kind::Row { .. } => Self::Row,
            Kind::Column { .. } => Self::Column,
            Kind::Stack => Self::Stack,
            Kind::FocusScope { .. } => Self::FocusScope,
            Kind::Overlay { .. } => Self::Overlay,
            Kind::Padding { .. } => Self::Padding,
            Kind::TextField(_) => Self::TextField,
            Kind::Checkbox { .. } => Self::Checkbox,
            Kind::Slider { .. } => Self::Slider,
            Kind::ScrollView { .. } => Self::ScrollView,
            Kind::Custom => Self::Custom,
        }
    }
}

/// Deterministic headless state for one retained element.
#[derive(Clone, Debug, PartialEq)]
pub struct ElementInspection {
    /// Element identity.
    pub id: ElementId,
    /// Logical parent.
    pub parent: Option<ElementId>,
    /// Stable retained-element category.
    pub kind: ElementKind,
    /// Layout constraints declared on the element.
    pub declared_layout: LayoutStyle,
    /// Local enabled state before ancestor propagation.
    pub enabled: bool,
    /// Child overflow policy.
    pub overflow: Overflow,
    /// Local paint-order priority.
    pub z_index: i32,
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

impl<Message: 'static> Ui<Message> {
    /// Builds a deterministic headless layout and interaction snapshot.
    pub fn inspect(&mut self) -> Result<UiInspection, UiError> {
        self.ensure_layout()?;
        let mut paint = Vec::new();
        self.collect_paint_order(self.root, &mut paint)?;
        let mut overlays = self
            .ids()
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
        for id in self.ids() {
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
                kind: ElementKind::from_kind(&node.kind),
                declared_layout: node.style,
                enabled: node.enabled,
                overflow: node.overflow,
                z_index: node.z_index,
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
}
