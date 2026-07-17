//! Semantic tree exposed to accessibility adapters.

use super::*;

use astrelis_core::geometry::LogicalRect;

use crate::ElementId;

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

impl<Message: 'static> Ui<Message> {
    /// Returns a snapshot-friendly semantic tree.
    pub fn semantic_tree(&mut self) -> Result<SemanticNode, UiError> {
        self.ensure_layout()?;
        let tree = self.semantic_node(self.root)?;
        self.dirty.remove(Dirty::SEMANTICS);
        Ok(tree)
    }

    pub(crate) fn semantic_node(&self, id: ElementId) -> Result<SemanticNode, UiError> {
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
            Kind::Checkbox { checked, .. } => (
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
                        ..
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
}
