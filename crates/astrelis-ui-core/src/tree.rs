//! Retained element arena: identities, typed handles, and node storage.

use super::*;

/// Erased generational element identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ElementId {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

/// Typed generational handle to a retained element.
pub struct ElementHandle<T> {
    pub(crate) id: ElementId,
    pub(crate) marker: PhantomData<fn() -> T>,
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

#[derive(Clone, Debug)]
pub(crate) enum Kind {
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
        style: CheckboxStyle,
    },
    Slider {
        min: f32,
        max: f32,
        step: f32,
        value: f32,
        style: SliderStyle,
    },
    ScrollView {
        offset: f32,
        content_height: f32,
        style: ScrollViewStyle,
    },
    Custom,
}

#[derive(Clone, Debug)]
pub(crate) struct TextFieldState {
    pub(crate) text: String,
    pub(crate) placeholder: String,
    pub(crate) caret: TextPosition,
    pub(crate) anchor: TextPosition,
    pub(crate) preedit: String,
    pub(crate) password: bool,
    pub(crate) horizontal_offset: f32,
}

impl TextFieldState {
    pub(crate) fn new(text: String) -> Self {
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

    pub(crate) fn selection(&self) -> (usize, usize) {
        let a = self.anchor.byte_index.min(self.text.len());
        let b = self.caret.byte_index.min(self.text.len());
        (a.min(b), a.max(b))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Node {
    pub(crate) parent: Option<ElementId>,
    pub(crate) children: Vec<ElementId>,
    pub(crate) kind: Kind,
    pub(crate) style: LayoutStyle,
    pub(crate) visual: WidgetStyle,
    /// Whether text content wraps within the node's max width.
    pub(crate) wrap: bool,
    pub(crate) enabled: bool,
    pub(crate) visibility: Visibility,
    pub(crate) overflow: Overflow,
    pub(crate) z_index: i32,
    pub(crate) transform: Affine2,
    pub(crate) transform_origin: LogicalPoint,
    pub(crate) cursor: Option<CursorIcon>,
    pub(crate) bounds: LogicalRect,
    pub(crate) text_layout: Option<TextLayout>,
    /// The request that produced `text_layout`, retained so an unchanged node
    /// can skip reshaping even when a global dirty pass revisits it.
    pub(crate) text_request: Option<TextLayoutRequest>,
    /// A reshape in flight for this node, if any: the request being shaped and
    /// its id. `text_layout` keeps showing the previous result until this
    /// resolves, so layout uses the old extent while a reshape is pending.
    pub(crate) pending: Option<(RequestId, TextLayoutRequest)>,
    pub(crate) hovered: bool,
    pub(crate) pressed: bool,
}

pub(crate) struct Slot {
    pub(crate) generation: u32,
    pub(crate) node: Option<Node>,
}

pub(crate) struct DragSession {
    pub(crate) id: DragSessionId,
    pub(crate) source: ElementId,
    pub(crate) payload: DragPayload,
    pub(crate) options: DragOptions,
    pub(crate) start: LogicalPoint,
    pub(crate) active: bool,
    pub(crate) candidate: Option<ElementId>,
    pub(crate) accepted: Option<(ElementId, DropOperation)>,
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
                    wrap: false,
                    enabled: true,
                    visibility: Visibility::Visible,
                    overflow: Overflow::Visible,
                    z_index: 0,
                    transform: Affine2::IDENTITY,
                    transform_origin: LogicalPoint::ZERO,
                    cursor: None,
                    bounds: Rect::default(),
                    text_layout: None,
                    text_request: None,
                    pending: None,
                    hovered: false,
                    pressed: false,
                }),
            }],
            free: Vec::new(),
            taffy_cache: TaffyCache::default(),
            root,
            theme,
            fonts,
            text_context: TextLayoutContext::new(),
            shape_policy: ShapePolicy::default(),
            request_id_counter: 0,
            worker: None,
            async_outstanding: 0,
            viewport: Size::ZERO,
            scale_factor: 1.0,
            dirty: Dirty::all(),
            dirty_nodes: HashSet::new(),
            measure_resweep: true,
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
            semantic_roles: HashMap::new(),
            semantic_descriptions: HashMap::new(),
            semantic_invalid: HashSet::new(),
            semantic_live: HashMap::new(),
            semantic_selected: HashMap::new(),
            semantic_expanded: HashMap::new(),
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
            // Wrap widths track the viewport, so every wrapped label may reshape.
            self.invalidate_layout();
        }
    }

    /// Returns the active theme.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Replaces the active theme.
    pub fn set_theme(&mut self, theme: Theme) {
        if self.theme != theme {
            self.theme = theme;
            // Theme drives font sizes, colors, and spacing, so everything
            // reshapes and re-reconciles.
            self.invalidate_layout();
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
        self.insert(
            parent.id,
            Kind::Checkbox {
                checked,
                style: CheckboxStyle::default(),
            },
        )
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
        self.insert(
            parent.id,
            Kind::Slider {
                min,
                max,
                step,
                value,
                style: SliderStyle::default(),
            },
        )
    }

    /// Adds a vertically scrolling retained container.
    pub fn add_scroll_view<T>(
        &mut self,
        parent: ElementHandle<T>,
    ) -> Result<ElementHandle<ScrollView>, UiError> {
        self.insert(
            parent.id,
            Kind::ScrollView {
                offset: 0.0,
                content_height: 0.0,
                style: ScrollViewStyle::default(),
            },
        )
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
        // Only this widget's node changed, and its intrinsic size may have
        // moved, so enqueue it for the measure-input sweeps rather than forcing
        // a whole-tree resweep.
        self.invalidate_node(handle.id, Dirty::all());
        Ok(())
    }

    pub(crate) fn insert<T>(
        &mut self,
        parent: ElementId,
        kind: Kind,
    ) -> Result<ElementHandle<T>, UiError> {
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
            wrap: false,
            enabled: true,
            visibility: Visibility::Visible,
            overflow: Overflow::Visible,
            z_index: 0,
            transform: Affine2::IDENTITY,
            transform_origin: LogicalPoint::ZERO,
            cursor: None,
            bounds: Rect::default(),
            text_layout: None,
            text_request: None,
            pending: None,
            hovered: false,
            pressed: false,
        });
        self.node_mut(parent)?.children.push(id);
        self.taffy_cache.structure_dirty = true;
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
        for index in 0..self.slots.len() {
            let Some(id) = self.id_at(index) else {
                continue;
            };
            let hovered = self.hover_paths.values().any(|path| path.contains(&id));
            self.node_mut(id)?.hovered = hovered;
        }
        self.taffy_cache.structure_dirty = true;
        self.invalidate_layout();
        if let Some(restore) = restore_focus.filter(|id| self.node(*id).is_ok()) {
            self.set_focus(Some(restore))?;
        }
        Ok(())
    }

    pub(crate) fn is_descendant_of(&self, child: ElementId, ancestor: ElementId) -> bool {
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
        self.taffy_cache.structure_dirty = true;
        self.invalidate_layout();
        Ok(())
    }

    pub(crate) fn node(&self, id: ElementId) -> Result<&Node, UiError> {
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

    pub(crate) fn node_mut(&mut self, id: ElementId) -> Result<&mut Node, UiError> {
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

    /// Reconstructs the live identity occupying an arena slot, if any.
    pub(crate) fn id_at(&self, index: usize) -> Option<ElementId> {
        let slot = self.slots.get(index)?;
        slot.node.as_ref().map(|_| ElementId {
            index: index as u32,
            generation: slot.generation,
        })
    }

    /// Returns a node's children in ascending z-order, breaking ties by
    /// insertion order to match the CSS stable-paint rule.
    ///
    /// The overwhelmingly common case is a container whose children all share
    /// one z-index, where the stable sort is a no-op and insertion order is
    /// already correct. That case returns `None`, letting the caller iterate
    /// `node.children` directly and skip both the temporary `Vec` and the
    /// sort — paint, paint-order, and hit testing each ran that allocation and
    /// sort at every node, the last of them on every pointer move.
    pub(crate) fn z_sorted_children(&self, node: &Node) -> Option<Vec<ElementId>> {
        let reference = node
            .children
            .first()
            .map_or(0, |child| self.node(*child).map_or(0, |node| node.z_index));
        let uniform = node.children.iter().all(|child| {
            self.node(*child)
                .map_or(true, |node| node.z_index == reference)
        });
        if uniform {
            return None;
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
        Some(children.into_iter().map(|(_, child)| child).collect())
    }

    /// Iterates every live element identity in arena order without allocating.
    ///
    /// This borrows `self` immutably for the lifetime of the iterator, so it
    /// suits read-only sweeps. Loops that mutate a node per iteration should
    /// instead range over `0..self.slots.len()` and resolve each index with
    /// [`Self::id_at`], which re-borrows fresh on every step.
    pub(crate) fn ids(&self) -> impl Iterator<Item = ElementId> + '_ {
        (0..self.slots.len()).filter_map(|index| self.id_at(index))
    }
}
