use std::{cell::Cell, rc::Rc};

use astrelis_platform::{ElementState, Key, NamedKey, PointerButton};
use astrelis_ui_core::{
    Button, Checkbox, Column, ElementHandle, EventFilter, EventPhase, FocusScopeOptions,
    LayoutStyle, Length, Overlay, OverlayOptions, OverlaySide, RoutedEventKind, SemanticRole,
    Slider, TextField, Ui, UiError, Visibility,
};

/// Click-controlled arbitrary-content viewport overlay.
pub struct Popover {
    overlay: ElementHandle<Overlay>,
    open: Rc<Cell<bool>>,
}

impl Popover {
    /// Creates a hidden popover controlled by `owner` activation.
    pub fn new<Message: 'static, T: 'static>(
        ui: &mut Ui<Message>,
        owner: ElementHandle<T>,
        options: OverlayOptions,
    ) -> Result<Self, UiError> {
        let overlay = ui.add_overlay(owner, options)?;
        ui.set_visibility(overlay, Visibility::Hidden)?;
        // The overlay resolves its surface from the theme at paint time, so no
        // background is snapshotted here.
        let open = Rc::new(Cell::new(false));
        let toggle = open.clone();
        ui.listen(
            owner,
            Some(EventPhase::Target),
            EventFilter::Activate,
            move |context, _| {
                let next = !toggle.get();
                toggle.set(next);
                context.set_visibility(
                    overlay,
                    if next {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    },
                );
            },
        )?;
        let dismiss = open.clone();
        let root = ui.root();
        ui.listen(
            root,
            Some(EventPhase::Capture),
            EventFilter::Pointer,
            move |context, event| {
                if dismiss.get()
                    && matches!(
                        event.kind,
                        RoutedEventKind::PointerButton {
                            button: PointerButton::Primary,
                            state: ElementState::Pressed,
                            ..
                        }
                    )
                    && !context.route_contains(overlay)
                    && !context.route_contains(owner)
                {
                    dismiss.set(false);
                    context.set_visibility(overlay, Visibility::Hidden);
                }
            },
        )?;
        let escape = open.clone();
        ui.listen(
            overlay,
            None,
            EventFilter::Keyboard,
            move |context, event| {
                if matches!(
                    event.kind,
                    RoutedEventKind::Keyboard(ref input)
                        if input.state == ElementState::Pressed
                            && matches!(input.logical_key, Key::Named(NamedKey::Escape))
                ) {
                    escape.set(false);
                    context.set_visibility(overlay, Visibility::Hidden);
                    context.prevent_default();
                }
            },
        )?;
        Ok(Self { overlay, open })
    }

    /// Returns the overlay content parent.
    pub const fn content(&self) -> ElementHandle<Overlay> {
        self.overlay
    }

    /// Returns whether the popover is currently open.
    pub fn is_open(&self) -> bool {
        self.open.get()
    }

    /// Programmatically changes visibility.
    pub fn set_open<Message: 'static>(
        &self,
        ui: &mut Ui<Message>,
        open: bool,
    ) -> Result<(), UiError> {
        self.open.set(open);
        ui.set_visibility(
            self.overlay,
            if open {
                Visibility::Visible
            } else {
                Visibility::Hidden
            },
        )
    }
}

/// Immediate tooltip shown while its owner is hovered or focused.
pub struct Tooltip {
    overlay: ElementHandle<Overlay>,
}

impl Tooltip {
    /// Attaches a non-focusable tooltip to `owner`.
    pub fn new<Message: 'static, T: 'static>(
        ui: &mut Ui<Message>,
        owner: ElementHandle<T>,
        text: impl Into<String>,
    ) -> Result<Self, UiError> {
        let overlay = ui.add_overlay(
            owner,
            OverlayOptions {
                side: OverlaySide::Below,
                offset: astrelis_core::geometry::Point::new(0.0, 6.0),
                z_index: 100,
                ..Default::default()
            },
        )?;
        ui.set_visibility(overlay, Visibility::Hidden)?;
        ui.set_semantic_role(overlay, SemanticRole::Tooltip)?;
        // The overlay resolves its surface from the theme at paint time. Pad the
        // label off the rounded surface, and wrap it within a maximum width so a
        // long tooltip becomes several lines instead of one very wide line.
        let insets = ui.theme().control_padding;
        let content = ui.add_padding(overlay, insets)?;
        let label = ui.add_label(content, text)?;
        ui.set_layout(
            label,
            LayoutStyle {
                max_width: Length::Px(320.0),
                ..Default::default()
            },
        )?;
        ui.set_wrap(label, true)?;
        let hovered = Rc::new(Cell::new(false));
        let focused = Rc::new(Cell::new(false));
        let hover_state = hovered.clone();
        let focus_state = focused.clone();
        ui.listen(owner, None, EventFilter::Pointer, move |context, event| {
            match event.kind {
                RoutedEventKind::PointerEntered { .. } => hover_state.set(true),
                RoutedEventKind::PointerLeft { .. } => hover_state.set(false),
                _ => return,
            }
            context.set_visibility(
                overlay,
                if hover_state.get() || focus_state.get() {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                },
            );
        })?;
        let hover_state = hovered;
        let focus_state = focused;
        ui.listen(owner, None, EventFilter::Focus, move |context, event| {
            let RoutedEventKind::FocusChanged(value) = event.kind else {
                return;
            };
            focus_state.set(value);
            context.set_visibility(
                overlay,
                if hover_state.get() || focus_state.get() {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                },
            );
        })?;
        Ok(Self { overlay })
    }

    /// Returns the tooltip overlay handle.
    pub const fn overlay(&self) -> ElementHandle<Overlay> {
        self.overlay
    }
}

/// One entry supplied to a popup [`Menu`].
pub struct MenuItem<Message> {
    /// Visible item label.
    pub label: String,
    /// Message emitted when activated.
    pub message: Message,
    /// Whether the item can be focused and activated.
    pub enabled: bool,
}

/// Single-level keyboard-accessible popup menu.
pub struct Menu {
    popover: Popover,
    items: Vec<ElementHandle<Button>>,
}

impl Menu {
    /// Creates a popup menu controlled by `owner`.
    pub fn new<Message: Clone + 'static, T: 'static>(
        ui: &mut Ui<Message>,
        owner: ElementHandle<T>,
        entries: Vec<MenuItem<Message>>,
    ) -> Result<Self, UiError> {
        let popover = Popover::new(
            ui,
            owner,
            OverlayOptions {
                focus: FocusScopeOptions {
                    trapped: true,
                    autofocus: true,
                    restore_focus: true,
                },
                z_index: 80,
                ..Default::default()
            },
        )?;
        ui.set_semantic_role(popover.content(), SemanticRole::Menu)?;
        ui.set_layout(
            popover.content(),
            LayoutStyle {
                min_width: Length::Px(180.0),
                ..Default::default()
            },
        )?;
        let mut items = Vec::with_capacity(entries.len());
        let mut navigable = Vec::new();
        for entry in entries {
            let item = ui.add_button(popover.content(), entry.label)?;
            ui.set_semantic_role(item, SemanticRole::MenuItem)?;
            ui.set_enabled(item, entry.enabled)?;
            let message = entry.message;
            let open = popover.open.clone();
            let overlay = popover.overlay;
            ui.listen(item, None, EventFilter::Activate, move |context, _| {
                context.emit(message.clone());
                open.set(false);
                context.set_visibility(overlay, Visibility::Hidden);
            })?;
            items.push(item);
            if entry.enabled {
                navigable.push(item);
            }
        }
        if !navigable.is_empty() {
            install_linear_keyboard_navigation(ui, &navigable)?;
            let first = navigable[0];
            let open = popover.open.clone();
            ui.listen(
                owner,
                Some(EventPhase::Target),
                EventFilter::Activate,
                move |context, _| {
                    if open.get() {
                        context.request_focus_for(first);
                    }
                },
            )?;
        }
        Ok(Self { popover, items })
    }

    /// Returns the popup controller.
    pub const fn popover(&self) -> &Popover {
        &self.popover
    }

    /// Returns menu-item button handles in display order.
    pub fn items(&self) -> &[ElementHandle<Button>] {
        &self.items
    }
}

fn install_linear_keyboard_navigation<Message: 'static>(
    ui: &mut Ui<Message>,
    items: &[ElementHandle<Button>],
) -> Result<(), UiError> {
    for (index, item) in items.iter().copied().enumerate() {
        let all = items.to_vec();
        ui.listen(item, None, EventFilter::Keyboard, move |context, event| {
            let RoutedEventKind::Keyboard(input) = &event.kind else {
                return;
            };
            if input.state != ElementState::Pressed {
                return;
            }
            let Key::Named(NamedKey::Other(key)) = &input.logical_key else {
                return;
            };
            let next = match key.as_str() {
                "ArrowDown" | "ArrowRight" => (index + 1) % all.len(),
                "ArrowUp" | "ArrowLeft" => (index + all.len() - 1) % all.len(),
                "Home" => 0,
                "End" => all.len() - 1,
                _ => return,
            };
            context.request_focus_for(all[next]);
            context.prevent_default();
        })?;
    }
    Ok(())
}

/// Retained tab strip with one content column per tab.
pub struct Tabs {
    tabs: Vec<ElementHandle<Button>>,
    panels: Vec<ElementHandle<Column>>,
    selected: Rc<Cell<usize>>,
}

impl Tabs {
    /// Creates automatically activated tabs from their labels.
    pub fn new<Message: 'static, T>(
        ui: &mut Ui<Message>,
        parent: ElementHandle<T>,
        labels: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, UiError> {
        let root = ui.add_column(parent)?;
        let strip = ui.add_row(root)?;
        ui.set_semantic_role(strip, SemanticRole::TabList)?;
        let content = ui.add_column(root)?;
        let labels = labels.into_iter().map(Into::into).collect::<Vec<_>>();
        if labels.is_empty() {
            return Err(UiError::from_message("tabs require at least one label"));
        }
        let mut tabs = Vec::new();
        let mut panels = Vec::new();
        for (index, label) in labels.into_iter().enumerate() {
            tabs.push(ui.add_button(strip, label)?);
            ui.set_semantic_role(
                *tabs.last().expect("tab was just inserted"),
                SemanticRole::Tab,
            )?;
            let panel = ui.add_column(content)?;
            ui.set_semantic_role(panel, SemanticRole::TabPanel)?;
            ui.set_visibility(
                panel,
                if index == 0 {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                },
            )?;
            panels.push(panel);
        }
        let selected = Rc::new(Cell::new(0));
        for (index, tab) in tabs.iter().copied().enumerate() {
            let selected_state = selected.clone();
            let all_panels = panels.clone();
            ui.listen(tab, None, EventFilter::Activate, move |context, _| {
                selected_state.set(index);
                for (panel_index, panel) in all_panels.iter().copied().enumerate() {
                    context.set_visibility(
                        panel,
                        if panel_index == index {
                            Visibility::Visible
                        } else {
                            Visibility::Hidden
                        },
                    );
                }
            })?;
            let selected_state = selected.clone();
            let all_tabs = tabs.clone();
            let all_panels = panels.clone();
            ui.listen(tab, None, EventFilter::Keyboard, move |context, event| {
                let RoutedEventKind::Keyboard(input) = &event.kind else {
                    return;
                };
                if input.state != ElementState::Pressed {
                    return;
                }
                let Key::Named(NamedKey::Other(key)) = &input.logical_key else {
                    return;
                };
                let next = match key.as_str() {
                    "ArrowRight" | "ArrowDown" => (index + 1) % all_tabs.len(),
                    "ArrowLeft" | "ArrowUp" => (index + all_tabs.len() - 1) % all_tabs.len(),
                    "Home" => 0,
                    "End" => all_tabs.len() - 1,
                    _ => return,
                };
                selected_state.set(next);
                context.request_focus_for(all_tabs[next]);
                for (panel_index, panel) in all_panels.iter().copied().enumerate() {
                    context.set_visibility(
                        panel,
                        if panel_index == next {
                            Visibility::Visible
                        } else {
                            Visibility::Hidden
                        },
                    );
                }
                context.prevent_default();
            })?;
        }
        Ok(Self {
            tabs,
            panels,
            selected,
        })
    }

    /// Returns tab buttons.
    pub fn tabs(&self) -> &[ElementHandle<Button>] {
        &self.tabs
    }

    /// Returns panel content columns.
    pub fn panels(&self) -> &[ElementHandle<Column>] {
        &self.panels
    }

    /// Returns the selected tab index.
    pub fn selected(&self) -> usize {
        self.selected.get()
    }
}

/// One entry supplied to a selectable [`List`].
pub struct ListItem<Message> {
    /// Visible item label.
    pub label: String,
    /// Message emitted on selection.
    pub message: Message,
    /// Whether the item is enabled.
    pub enabled: bool,
}

/// Single-selection retained list with linear keyboard navigation.
pub struct List {
    items: Vec<ElementHandle<Button>>,
    selected: Rc<Cell<Option<usize>>>,
}

impl List {
    /// Creates a retained list.
    pub fn new<Message: Clone + 'static, T>(
        ui: &mut Ui<Message>,
        parent: ElementHandle<T>,
        entries: Vec<ListItem<Message>>,
    ) -> Result<Self, UiError> {
        let column = ui.add_column(parent)?;
        ui.set_semantic_role(column, SemanticRole::List)?;
        let selected = Rc::new(Cell::new(None));
        let mut items = Vec::new();
        let mut navigable = Vec::new();
        for (index, entry) in entries.into_iter().enumerate() {
            let item = ui.add_button(column, entry.label)?;
            ui.set_semantic_role(item, SemanticRole::ListItem)?;
            ui.set_enabled(item, entry.enabled)?;
            let state = selected.clone();
            let message = entry.message;
            ui.listen(item, None, EventFilter::Activate, move |context, _| {
                state.set(Some(index));
                context.emit(message.clone());
            })?;
            items.push(item);
            if entry.enabled {
                navigable.push(item);
            }
        }
        if !navigable.is_empty() {
            install_linear_keyboard_navigation(ui, &navigable)?;
        }
        Ok(Self { items, selected })
    }

    /// Returns retained item buttons.
    pub fn items(&self) -> &[ElementHandle<Button>] {
        &self.items
    }

    /// Returns the most recently selected index.
    pub fn selected(&self) -> Option<usize> {
        self.selected.get()
    }
}

/// Common labeled form-control compositions.
pub struct Form {
    content: ElementHandle<Column>,
}

impl Form {
    /// Creates a form column.
    pub fn new<Message: 'static, T>(
        ui: &mut Ui<Message>,
        parent: ElementHandle<T>,
    ) -> Result<Self, UiError> {
        let content = ui.add_column(parent)?;
        ui.set_semantic_role(content, SemanticRole::Form)?;
        Ok(Self { content })
    }

    /// Adds a labeled single-line text field and optional help text.
    pub fn add_text_field<Message: 'static>(
        &self,
        ui: &mut Ui<Message>,
        label: impl Into<String>,
        value: impl Into<String>,
        help: Option<&str>,
    ) -> Result<ElementHandle<TextField>, UiError> {
        ui.add_label(self.content, label)?;
        let field = ui.add_text_field(self.content, value)?;
        if let Some(help) = help {
            ui.add_label(self.content, help)?;
        }
        Ok(field)
    }

    /// Adds a label and checkbox.
    pub fn add_checkbox<Message: 'static>(
        &self,
        ui: &mut Ui<Message>,
        label: impl Into<String>,
        checked: bool,
    ) -> Result<ElementHandle<Checkbox>, UiError> {
        ui.add_label(self.content, label)?;
        ui.add_checkbox(self.content, checked)
    }

    /// Adds a label and horizontal slider.
    pub fn add_slider<Message: 'static>(
        &self,
        ui: &mut Ui<Message>,
        label: impl Into<String>,
        range: std::ops::RangeInclusive<f32>,
        step: f32,
        value: f32,
    ) -> Result<ElementHandle<Slider>, UiError> {
        ui.add_label(self.content, label)?;
        ui.add_slider(self.content, *range.start(), *range.end(), step, value)
    }

    /// Adds validation or other status text below the controls.
    pub fn add_status<Message: 'static>(
        &self,
        ui: &mut Ui<Message>,
        text: impl Into<String>,
    ) -> Result<(), UiError> {
        ui.add_label(self.content, text)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use astrelis_core::geometry::Size;
    use astrelis_text::FontDatabase;
    use astrelis_ui_core::{SemanticAction, Theme};

    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    enum Message {
        First,
        Second,
    }

    #[test]
    fn menu_activation_emits_and_restores_owner_focus() {
        let mut ui = Ui::new(FontDatabase::default(), Theme::default());
        ui.set_viewport(Size::new(500.0, 400.0), 1.0);
        let root = ui.root();
        let owner = ui.add_button(root, "Menu").unwrap();
        let menu = Menu::new(
            &mut ui,
            owner,
            vec![
                MenuItem {
                    label: "First".into(),
                    message: Message::First,
                    enabled: true,
                },
                MenuItem {
                    label: "Second".into(),
                    message: Message::Second,
                    enabled: true,
                },
            ],
        )
        .unwrap();
        ui.perform_semantic_action(owner.id(), SemanticAction::Focus)
            .unwrap();
        ui.perform_semantic_action(owner.id(), SemanticAction::Activate)
            .unwrap();
        ui.display_list().unwrap();
        assert!(menu.popover().is_open());
        assert!(ui.is_focused(menu.items()[0]).unwrap());

        ui.perform_semantic_action(menu.items()[1].id(), SemanticAction::Activate)
            .unwrap();
        ui.display_list().unwrap();
        assert_eq!(
            ui.drain_messages().collect::<Vec<_>>(),
            vec![Message::Second]
        );
        assert!(!menu.popover().is_open());
        assert!(ui.is_focused(owner).unwrap());
    }

    #[test]
    fn tabs_switch_panel_visibility_through_public_semantics() {
        let mut ui: Ui = Ui::new(FontDatabase::default(), Theme::default());
        ui.set_viewport(Size::new(500.0, 400.0), 1.0);
        let root = ui.root();
        let tabs = Tabs::new(&mut ui, root, ["One", "Two", "Three"]).unwrap();
        ui.add_label(tabs.panels()[0], "First").unwrap();
        ui.add_label(tabs.panels()[1], "Second").unwrap();
        ui.perform_semantic_action(tabs.tabs()[1].id(), SemanticAction::Activate)
            .unwrap();
        let inspection = ui.inspect().unwrap();
        let visibility = |id| {
            inspection
                .nodes
                .iter()
                .find(|node| node.id == id)
                .unwrap()
                .visibility
        };
        assert_eq!(tabs.selected(), 1);
        assert_eq!(visibility(tabs.panels()[0].id()), Visibility::Hidden);
        assert_eq!(visibility(tabs.panels()[1].id()), Visibility::Visible);
    }
}
