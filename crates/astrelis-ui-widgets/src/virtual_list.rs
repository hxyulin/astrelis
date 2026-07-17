use std::{any::Any, cell::Cell, collections::BTreeMap, ops::Range, rc::Rc};

use astrelis_core::geometry::LogicalRect;
use astrelis_paint::{Brush, Painter};
use astrelis_platform::{CursorIcon, ElementState, Key, NamedKey, PointerButton};
use astrelis_ui_core::{
    ElementHandle, EventContext, LayoutStyle, Length, Positioning, RoutedEvent, RoutedEventKind,
    ScrollView, SemanticAction, SemanticActionKind, SemanticRole, Stack, Theme, Ui, UiError,
    Widget,
};

/// Fixed-extent virtualization policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VirtualListOptions {
    /// Logical height of every item.
    pub item_extent: f32,
    /// Extra item count retained before and after the visible range.
    pub overscan: usize,
}

impl Default for VirtualListOptions {
    fn default() -> Self {
        Self {
            item_extent: 36.0,
            overscan: 3,
        }
    }
}

/// Focusable retained root representing one realized virtual-list item.
pub struct VirtualListItem {
    index: usize,
    item_count: usize,
    requested: Rc<Cell<Option<usize>>>,
    selected: Rc<Cell<Option<usize>>>,
    hovered: bool,
    focused: bool,
}

impl VirtualListItem {
    fn new(
        index: usize,
        item_count: usize,
        requested: Rc<Cell<Option<usize>>>,
        selected: Rc<Cell<Option<usize>>>,
    ) -> Self {
        Self {
            index,
            item_count,
            requested,
            selected,
            hovered: false,
            focused: false,
        }
    }

    /// Returns this row's data index.
    pub const fn index(&self) -> usize {
        self.index
    }

    fn request_navigation(&self, key: &str) -> bool {
        let next = match key {
            "ArrowUp" => self.index.saturating_sub(1),
            "ArrowDown" => (self.index + 1).min(self.item_count.saturating_sub(1)),
            "PageUp" => self.index.saturating_sub(10),
            "PageDown" => (self.index + 10).min(self.item_count.saturating_sub(1)),
            "Home" => 0,
            "End" => self.item_count.saturating_sub(1),
            _ => return false,
        };
        self.requested.set(Some(next));
        true
    }
}

impl<Message: 'static> Widget<Message> for VirtualListItem {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn event(&mut self, context: &mut EventContext<'_, Message>, event: &RoutedEvent) {
        match &event.kind {
            RoutedEventKind::PointerEntered { .. } => {
                self.hovered = true;
                context.request_paint();
            }
            RoutedEventKind::PointerLeft { .. } => {
                self.hovered = false;
                context.request_paint();
            }
            RoutedEventKind::FocusChanged(focused) => {
                self.focused = *focused;
                context.request_paint();
            }
            RoutedEventKind::PointerButton {
                button: PointerButton::Primary,
                state: ElementState::Pressed,
                ..
            } => {
                self.selected.set(Some(self.index));
                context.request_focus();
                context.request_paint();
            }
            RoutedEventKind::Keyboard(input) if input.state == ElementState::Pressed => {
                match &input.logical_key {
                    Key::Named(NamedKey::Other(key)) if self.request_navigation(key) => {
                        context.prevent_default();
                    }
                    Key::Named(NamedKey::Enter | NamedKey::Space) => {
                        self.selected.set(Some(self.index));
                        context.request_paint();
                        context.prevent_default();
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn focusable(&self) -> bool {
        true
    }

    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(CursorIcon::Pointer)
    }

    fn paint(
        &self,
        painter: &mut Painter,
        bounds: LogicalRect,
        theme: &Theme,
    ) -> Result<(), UiError> {
        let selected = self.selected.get() == Some(self.index);
        painter
            .fill_rect(
                bounds,
                Brush::Solid(if selected {
                    theme.button.pressed
                } else if self.focused || self.hovered {
                    theme.button.hovered
                } else {
                    theme.field_background
                }),
            )
            .map_err(|error| UiError::from_message(error.to_string()))
    }

    fn semantics(&self) -> Option<(SemanticRole, String, Option<String>)> {
        Some((
            SemanticRole::ListItem,
            format!("Item {}", self.index + 1),
            Some(self.index.to_string()),
        ))
    }

    fn semantic_actions(&self) -> Vec<SemanticActionKind> {
        vec![SemanticActionKind::Focus, SemanticActionKind::Activate]
    }

    fn semantic_action(
        &mut self,
        context: &mut EventContext<'_, Message>,
        action: &SemanticAction,
    ) -> bool {
        match action {
            SemanticAction::Focus => {
                context.request_focus();
                true
            }
            SemanticAction::Activate => {
                self.selected.set(Some(self.index));
                context.request_paint();
                true
            }
            _ => false,
        }
    }
}

/// Fixed-extent virtual list which retains only visible items plus overscan.
pub struct VirtualList {
    scroll: ElementHandle<ScrollView>,
    content: ElementHandle<Stack>,
    options: VirtualListOptions,
    realized: BTreeMap<usize, ElementHandle<VirtualListItem>>,
    requested: Rc<Cell<Option<usize>>>,
    selected: Rc<Cell<Option<usize>>>,
    item_count: usize,
}

impl VirtualList {
    /// Creates an empty virtual list. Call [`Self::sync`] after assigning its size.
    pub fn new<Message: 'static, T>(
        ui: &mut Ui<Message>,
        parent: ElementHandle<T>,
        options: VirtualListOptions,
    ) -> Result<Self, UiError> {
        if !options.item_extent.is_finite() || options.item_extent <= 0.0 {
            return Err(UiError::from_message(
                "virtual-list item extent must be finite and positive",
            ));
        }
        let scroll = ui.add_scroll_view(parent)?;
        ui.set_semantic_role(scroll, SemanticRole::List)?;
        let content = ui.add_stack(scroll)?;
        ui.set_layout(
            content,
            LayoutStyle {
                width: Length::Percent(1.0),
                height: Length::Px(0.0),
                shrink: 0.0,
                ..Default::default()
            },
        )?;
        Ok(Self {
            scroll,
            content,
            options,
            realized: BTreeMap::new(),
            requested: Rc::new(Cell::new(None)),
            selected: Rc::new(Cell::new(None)),
            item_count: 0,
        })
    }

    /// Returns the underlying scroll-view handle.
    pub const fn scroll_view(&self) -> ElementHandle<ScrollView> {
        self.scroll
    }

    /// Returns the currently selected data index.
    pub fn selected(&self) -> Option<usize> {
        self.selected.get()
    }

    /// Queues an index to be realized, revealed, and focused by the next sync.
    pub fn request_focus(&self, index: usize) {
        self.requested.set(Some(index));
    }

    /// Returns the number of retained item roots.
    pub fn realized_count(&self) -> usize {
        self.realized.len()
    }

    /// Returns the half-open range currently retained.
    pub fn realized_range(&self) -> Range<usize> {
        let start = self
            .realized
            .first_key_value()
            .map_or(0, |(index, _)| *index);
        let end = self
            .realized
            .last_key_value()
            .map_or(start, |(index, _)| index + 1);
        start..end
    }

    /// Removes every realized item so changed data is rebuilt on the next sync.
    pub fn invalidate_all<Message: 'static>(
        &mut self,
        ui: &mut Ui<Message>,
    ) -> Result<(), UiError> {
        for (_, handle) in std::mem::take(&mut self.realized) {
            ui.remove(handle)?;
        }
        Ok(())
    }

    /// Reconciles retained items after input, viewport, scroll, or data changes.
    pub fn sync<Message: 'static>(
        &mut self,
        ui: &mut Ui<Message>,
        item_count: usize,
        mut build: impl FnMut(
            &mut Ui<Message>,
            ElementHandle<VirtualListItem>,
            usize,
        ) -> Result<(), UiError>,
    ) -> Result<(), UiError> {
        let item_count_changed = self.item_count != item_count;
        self.item_count = item_count;
        let content_height = self.options.item_extent * item_count as f32;
        ui.set_layout(
            self.content,
            LayoutStyle {
                width: Length::Percent(1.0),
                height: Length::Px(content_height),
                shrink: 0.0,
                ..Default::default()
            },
        )?;
        let viewport = ui.layout_bounds(self.scroll)?.size.height;
        let requested = self
            .requested
            .take()
            .filter(|_| item_count > 0)
            .map(|index| index.min(item_count - 1));
        if let Some(index) = requested {
            let top = index as f32 * self.options.item_extent;
            let bottom = top + self.options.item_extent;
            let offset = ui.scroll_offset(self.scroll)?;
            let next = if top < offset {
                top
            } else if bottom > offset + viewport {
                bottom - viewport
            } else {
                offset
            };
            ui.set_scroll_offset(self.scroll, next)?;
        }
        let offset = ui.scroll_offset(self.scroll)?;
        let visible_start = (offset / self.options.item_extent).floor().max(0.0) as usize;
        let visible_end = ((offset + viewport) / self.options.item_extent).ceil() as usize;
        let start = visible_start.saturating_sub(self.options.overscan);
        let end = visible_end
            .saturating_add(self.options.overscan)
            .min(item_count);

        let stale = self
            .realized
            .range(..start)
            .chain(self.realized.range(end..))
            .map(|(index, _)| *index)
            .collect::<Vec<_>>();
        for index in stale {
            if let Some(handle) = self.realized.remove(&index) {
                ui.remove(handle)?;
            }
        }
        for index in start..end {
            if let Some(handle) = self.realized.get(&index).copied() {
                if item_count_changed {
                    ui.update_widget(handle, |item| item.item_count = item_count)?;
                }
                continue;
            }
            let item = ui.add_widget(
                self.content,
                VirtualListItem::new(
                    index,
                    item_count,
                    self.requested.clone(),
                    self.selected.clone(),
                ),
            )?;
            ui.set_layout(
                item,
                LayoutStyle {
                    width: Length::Percent(1.0),
                    height: Length::Px(self.options.item_extent),
                    positioning: Positioning::Absolute,
                    inset: astrelis_ui_core::Edges {
                        left: Length::Px(0.0),
                        top: Length::Px(index as f32 * self.options.item_extent),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )?;
            build(ui, item, index)?;
            self.realized.insert(index, item);
        }
        if let Some(index) = requested
            && let Some(handle) = self.realized.get(&index).copied()
        {
            ui.layout_bounds(handle)?;
            ui.perform_semantic_action(handle.id(), SemanticAction::Focus)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use astrelis_core::geometry::Size;
    use astrelis_text::FontDatabase;
    use astrelis_ui_core::{SemanticAction, Theme};

    use super::*;

    fn build_row(
        ui: &mut Ui,
        item: ElementHandle<VirtualListItem>,
        index: usize,
    ) -> Result<(), UiError> {
        ui.add_label(item, format!("Item {index}"))?;
        Ok(())
    }

    #[test]
    fn ten_thousand_items_keep_retention_bounded_while_scrolling() {
        let mut ui: Ui = Ui::new(FontDatabase::default(), Theme::default());
        ui.set_viewport(Size::new(500.0, 300.0), 1.0);
        let root = ui.root();
        let mut list = VirtualList::new(
            &mut ui,
            root,
            VirtualListOptions {
                item_extent: 40.0,
                overscan: 3,
            },
        )
        .unwrap();
        ui.set_layout(
            list.scroll_view(),
            LayoutStyle {
                width: Length::Px(400.0),
                height: Length::Px(200.0),
                ..Default::default()
            },
        )
        .unwrap();
        list.sync(&mut ui, 10_000, build_row).unwrap();
        assert!(list.realized_count() <= 11);
        assert_eq!(list.realized_range().start, 0);
        ui.display_list().unwrap();
        list.sync(&mut ui, 10_000, build_row).unwrap();
        assert!(!ui.needs_redraw(), "unchanged sync invalidated the UI");

        ui.set_scroll_offset(list.scroll_view(), 200_000.0).unwrap();
        list.sync(&mut ui, 10_000, build_row).unwrap();
        assert!(list.realized_count() <= 11);
        assert!(list.realized_range().start > 4_900);
        assert!(ui.inspect().unwrap().nodes.len() < 30);
    }

    #[test]
    fn requested_offscreen_index_is_realized_revealed_and_focused() {
        let mut ui: Ui = Ui::new(FontDatabase::default(), Theme::default());
        ui.set_viewport(Size::new(400.0, 240.0), 1.0);
        let root = ui.root();
        let mut list = VirtualList::new(&mut ui, root, VirtualListOptions::default()).unwrap();
        ui.set_layout(
            list.scroll_view(),
            LayoutStyle {
                height: Length::Px(180.0),
                ..Default::default()
            },
        )
        .unwrap();
        list.sync(&mut ui, 10_000, build_row).unwrap();
        list.request_focus(9_999);
        list.sync(&mut ui, 10_000, build_row).unwrap();
        let item = list.realized[&9_999];
        assert!(ui.is_focused(item).unwrap());
        let offset = ui.scroll_offset(list.scroll_view()).unwrap();
        assert!(offset > 350_000.0, "unexpected final offset: {offset}");
        ui.perform_semantic_action(item.id(), SemanticAction::Activate)
            .unwrap();
        assert_eq!(list.selected(), Some(9_999));
    }
}
