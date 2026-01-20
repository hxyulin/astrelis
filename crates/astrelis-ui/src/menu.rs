//! Context menu system with nested submenu support.
//!
//! Provides:
//! - Hierarchical menu items with unlimited nesting
//! - Separator support
//! - Checkbox and radio items
//! - Keyboard navigation
//! - Automatic positioning to stay on screen
//!
//! # Example
//!
//! ```ignore
//! use astrelis_ui::menu::{ContextMenu, MenuItem};
//!
//! let menu = ContextMenu::new(vec![
//!     MenuItem::action("Cut", || println!("Cut!")),
//!     MenuItem::action("Copy", || println!("Copy!")),
//!     MenuItem::action("Paste", || println!("Paste!")),
//!     MenuItem::separator(),
//!     MenuItem::submenu("More", vec![
//!         MenuItem::action("Option A", || {}),
//!         MenuItem::action("Option B", || {}),
//!         MenuItem::submenu("Even More", vec![
//!             MenuItem::action("Deep Option", || {}),
//!         ]),
//!     ]),
//!     MenuItem::checkbox("Auto-save", true, |checked| println!("Auto-save: {}", checked)),
//! ]);
//!
//! menu.show(&mut overlays, &mut tree, position);
//! ```

use std::sync::Arc;

use astrelis_core::math::Vec2;
use astrelis_render::Color;

use crate::overlay::{OverlayConfig, OverlayId, OverlayManager, OverlayPosition, ZLayer};
use crate::tree::{NodeId, UiTree};
use crate::widgets::Container;

/// Callback type for menu item actions.
pub type MenuCallback = Arc<dyn Fn() + Send + Sync>;

/// Callback type for checkbox/toggle items.
pub type ToggleCallback = Arc<dyn Fn(bool) + Send + Sync>;

/// A single menu item.
#[derive(Clone)]
pub enum MenuItem {
    /// Standard action item.
    Action {
        label: String,
        shortcut: Option<String>,
        enabled: bool,
        on_click: MenuCallback,
    },
    /// Submenu that opens another menu.
    Submenu {
        label: String,
        enabled: bool,
        items: Vec<MenuItem>,
    },
    /// Visual separator line.
    Separator,
    /// Checkbox item with toggle state.
    Checkbox {
        label: String,
        checked: bool,
        enabled: bool,
        on_toggle: ToggleCallback,
    },
    /// Radio item (part of a group).
    Radio {
        label: String,
        group: String,
        selected: bool,
        enabled: bool,
        on_select: MenuCallback,
    },
    /// Custom content widget.
    Custom {
        node_id: NodeId,
        height: f32,
    },
}

impl MenuItem {
    /// Create a simple action item.
    pub fn action<F>(label: impl Into<String>, on_click: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        Self::Action {
            label: label.into(),
            shortcut: None,
            enabled: true,
            on_click: Arc::new(on_click),
        }
    }

    /// Create an action item with keyboard shortcut display.
    pub fn action_with_shortcut<F>(
        label: impl Into<String>,
        shortcut: impl Into<String>,
        on_click: F,
    ) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        Self::Action {
            label: label.into(),
            shortcut: Some(shortcut.into()),
            enabled: true,
            on_click: Arc::new(on_click),
        }
    }

    /// Create a submenu item.
    pub fn submenu(label: impl Into<String>, items: Vec<MenuItem>) -> Self {
        Self::Submenu {
            label: label.into(),
            enabled: true,
            items,
        }
    }

    /// Create a separator.
    pub fn separator() -> Self {
        Self::Separator
    }

    /// Create a checkbox item.
    pub fn checkbox<F>(label: impl Into<String>, checked: bool, on_toggle: F) -> Self
    where
        F: Fn(bool) + Send + Sync + 'static,
    {
        Self::Checkbox {
            label: label.into(),
            checked,
            enabled: true,
            on_toggle: Arc::new(on_toggle),
        }
    }

    /// Create a radio item.
    pub fn radio<F>(
        label: impl Into<String>,
        group: impl Into<String>,
        selected: bool,
        on_select: F,
    ) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        Self::Radio {
            label: label.into(),
            group: group.into(),
            selected,
            enabled: true,
            on_select: Arc::new(on_select),
        }
    }

    /// Set enabled state.
    pub fn enabled(mut self, enabled: bool) -> Self {
        match &mut self {
            MenuItem::Action { enabled: e, .. } => *e = enabled,
            MenuItem::Submenu { enabled: e, .. } => *e = enabled,
            MenuItem::Checkbox { enabled: e, .. } => *e = enabled,
            MenuItem::Radio { enabled: e, .. } => *e = enabled,
            _ => {}
        }
        self
    }

    /// Check if this item is a submenu.
    pub fn is_submenu(&self) -> bool {
        matches!(self, MenuItem::Submenu { .. })
    }

    /// Check if this item is enabled.
    pub fn is_enabled(&self) -> bool {
        match self {
            MenuItem::Action { enabled, .. } => *enabled,
            MenuItem::Submenu { enabled, .. } => *enabled,
            MenuItem::Checkbox { enabled, .. } => *enabled,
            MenuItem::Radio { enabled, .. } => *enabled,
            MenuItem::Separator => true,
            MenuItem::Custom { .. } => true,
        }
    }

    /// Get the label (if any).
    pub fn label(&self) -> Option<&str> {
        match self {
            MenuItem::Action { label, .. } => Some(label),
            MenuItem::Submenu { label, .. } => Some(label),
            MenuItem::Checkbox { label, .. } => Some(label),
            MenuItem::Radio { label, .. } => Some(label),
            _ => None,
        }
    }
}

impl std::fmt::Debug for MenuItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MenuItem::Action { label, shortcut, enabled, .. } => {
                f.debug_struct("Action")
                    .field("label", label)
                    .field("shortcut", shortcut)
                    .field("enabled", enabled)
                    .finish()
            }
            MenuItem::Submenu { label, enabled, items } => {
                f.debug_struct("Submenu")
                    .field("label", label)
                    .field("enabled", enabled)
                    .field("items", &items.len())
                    .finish()
            }
            MenuItem::Separator => write!(f, "Separator"),
            MenuItem::Checkbox { label, checked, enabled, .. } => {
                f.debug_struct("Checkbox")
                    .field("label", label)
                    .field("checked", checked)
                    .field("enabled", enabled)
                    .finish()
            }
            MenuItem::Radio { label, group, selected, enabled, .. } => {
                f.debug_struct("Radio")
                    .field("label", label)
                    .field("group", group)
                    .field("selected", selected)
                    .field("enabled", enabled)
                    .finish()
            }
            MenuItem::Custom { node_id, height } => {
                f.debug_struct("Custom")
                    .field("node_id", node_id)
                    .field("height", height)
                    .finish()
            }
        }
    }
}

/// Menu styling configuration.
#[derive(Debug, Clone)]
pub struct MenuStyle {
    /// Background color.
    pub background_color: Color,
    /// Border color.
    pub border_color: Color,
    /// Border width.
    pub border_width: f32,
    /// Border radius.
    pub border_radius: f32,
    /// Item text color.
    pub text_color: Color,
    /// Disabled item text color.
    pub disabled_color: Color,
    /// Highlighted item background.
    pub highlight_color: Color,
    /// Separator color.
    pub separator_color: Color,
    /// Item height.
    pub item_height: f32,
    /// Horizontal padding.
    pub padding_x: f32,
    /// Vertical padding.
    pub padding_y: f32,
    /// Gap between icon/checkbox and label.
    pub icon_gap: f32,
    /// Shortcut text color.
    pub shortcut_color: Color,
    /// Minimum menu width.
    pub min_width: f32,
    /// Submenu arrow indicator.
    pub submenu_indicator: String,
    /// Checkbox checked indicator.
    pub checkbox_checked: String,
    /// Checkbox unchecked indicator.
    pub checkbox_unchecked: String,
    /// Radio selected indicator.
    pub radio_selected: String,
    /// Radio unselected indicator.
    pub radio_unselected: String,
}

impl Default for MenuStyle {
    fn default() -> Self {
        Self {
            background_color: Color::rgba(0.15, 0.15, 0.15, 0.98),
            border_color: Color::rgba(0.3, 0.3, 0.3, 1.0),
            border_width: 1.0,
            border_radius: 4.0,
            text_color: Color::WHITE,
            disabled_color: Color::rgba(0.5, 0.5, 0.5, 1.0),
            highlight_color: Color::rgba(0.2, 0.4, 0.8, 0.8),
            separator_color: Color::rgba(0.3, 0.3, 0.3, 1.0),
            item_height: 28.0,
            padding_x: 12.0,
            padding_y: 4.0,
            icon_gap: 8.0,
            shortcut_color: Color::rgba(0.6, 0.6, 0.6, 1.0),
            min_width: 150.0,
            submenu_indicator: "\u{25B6}".to_string(), // Right-pointing triangle
            checkbox_checked: "\u{2713}".to_string(),  // Check mark
            checkbox_unchecked: " ".to_string(),
            radio_selected: "\u{25CF}".to_string(), // Filled circle
            radio_unselected: "\u{25CB}".to_string(), // Empty circle
        }
    }
}

/// Active menu state.
#[derive(Debug)]
struct ActiveMenu {
    /// Overlay ID for this menu.
    overlay_id: OverlayId,
    /// Root node of the menu in the tree.
    root_node: NodeId,
    /// Item nodes for hit testing.
    item_nodes: Vec<(NodeId, usize)>,
    /// Parent menu (if this is a submenu).
    parent_menu: Option<OverlayId>,
    /// Currently active submenu.
    active_submenu: Option<OverlayId>,
    /// Currently hovered item index.
    hovered_item: Option<usize>,
    /// The menu items.
    items: Vec<MenuItem>,
}

/// Context menu system.
pub struct ContextMenu {
    /// Menu items.
    items: Vec<MenuItem>,
    /// Styling.
    style: MenuStyle,
    /// Currently active menus (root + submenus).
    active_menus: Vec<ActiveMenu>,
    /// Root overlay ID.
    root_overlay: Option<OverlayId>,
}

impl std::fmt::Debug for ContextMenu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContextMenu")
            .field("item_count", &self.items.len())
            .field("active_menu_count", &self.active_menus.len())
            .field("root_overlay", &self.root_overlay)
            .finish()
    }
}

/// Internal action type for deferred menu operations.
enum MenuAction {
    CloseSubmenu {
        sub_id: OverlayId,
    },
    OpenSubmenu {
        menu_idx: usize,
        items: Vec<MenuItem>,
        position: Vec2,
        parent_overlay: OverlayId,
        close_first: Option<OverlayId>,
    },
}

impl ContextMenu {
    /// Create a new context menu with items.
    pub fn new(items: Vec<MenuItem>) -> Self {
        Self {
            items,
            style: MenuStyle::default(),
            active_menus: Vec::new(),
            root_overlay: None,
        }
    }

    /// Create with custom style.
    pub fn with_style(items: Vec<MenuItem>, style: MenuStyle) -> Self {
        Self {
            items,
            style,
            active_menus: Vec::new(),
            root_overlay: None,
        }
    }

    /// Set menu style.
    pub fn set_style(&mut self, style: MenuStyle) {
        self.style = style;
    }

    /// Get menu style.
    pub fn style(&self) -> &MenuStyle {
        &self.style
    }

    /// Show the context menu at a position.
    pub fn show(
        &mut self,
        overlays: &mut OverlayManager,
        tree: &mut UiTree,
        position: Vec2,
    ) -> OverlayId {
        // Hide any existing menu
        self.hide(overlays, tree);

        // Build menu widget tree
        let (root_node, item_nodes) = self.build_menu_tree(tree, &self.items);

        // Show as overlay
        let overlay_id = overlays.show(
            tree,
            root_node,
            OverlayConfig {
                layer: ZLayer::Popover,
                position: OverlayPosition::Absolute {
                    x: position.x,
                    y: position.y,
                },
                close_on_outside_click: true,
                close_on_escape: true,
                trap_focus: true,
                show_backdrop: false,
                backdrop_color: Color::TRANSPARENT,
                animate_in: false,
                animate_out: false,
                auto_dismiss: None,
            },
        );

        self.root_overlay = Some(overlay_id);
        self.active_menus.push(ActiveMenu {
            overlay_id,
            root_node,
            item_nodes,
            parent_menu: None,
            active_submenu: None,
            hovered_item: None,
            items: self.items.clone(),
        });

        overlay_id
    }

    /// Show context menu at cursor position.
    pub fn show_at_cursor(
        &mut self,
        overlays: &mut OverlayManager,
        tree: &mut UiTree,
    ) -> OverlayId {
        self.hide(overlays, tree);

        let (root_node, item_nodes) = self.build_menu_tree(tree, &self.items);

        let overlay_id = overlays.show(
            tree,
            root_node,
            OverlayConfig::context_menu(),
        );

        self.root_overlay = Some(overlay_id);
        self.active_menus.push(ActiveMenu {
            overlay_id,
            root_node,
            item_nodes,
            parent_menu: None,
            active_submenu: None,
            hovered_item: None,
            items: self.items.clone(),
        });

        overlay_id
    }

    /// Hide the context menu and all submenus.
    pub fn hide(&mut self, overlays: &mut OverlayManager, tree: &mut UiTree) {
        // Hide all menus from innermost to outermost
        while let Some(menu) = self.active_menus.pop() {
            overlays.hide(tree, menu.overlay_id);
        }
        self.root_overlay = None;
    }

    /// Check if menu is currently visible.
    pub fn is_visible(&self) -> bool {
        self.root_overlay.is_some()
    }

    /// Handle mouse movement for highlighting and submenu opening.
    pub fn handle_mouse_move(
        &mut self,
        overlays: &mut OverlayManager,
        tree: &mut UiTree,
        position: Vec2,
    ) {
        // Collect actions to take (to avoid borrow conflicts)
        let mut action: Option<MenuAction> = None;

        let item_height = self.style.item_height;

        // Find which menu and item the mouse is over
        for (menu_idx, menu) in self.active_menus.iter_mut().enumerate() {
            // Check if mouse is over this menu
            if let Some(overlay) = overlays.get(menu.overlay_id) {
                if overlay.contains_point(position) {
                    // Check which item
                    let local_y = position.y - overlay.computed_position.y;
                    let item_index = (local_y / item_height) as usize;

                    if item_index < menu.items.len() {
                        let old_hovered = menu.hovered_item;
                        menu.hovered_item = Some(item_index);

                        // If hovered item changed, determine what action to take
                        if old_hovered != menu.hovered_item {
                            let close_sub = menu.active_submenu.take();

                            if let Some(MenuItem::Submenu { items, enabled, .. }) =
                                menu.items.get(item_index)
                            {
                                if *enabled {
                                    let sub_pos = Vec2::new(
                                        overlay.computed_position.x + overlay.computed_size.x,
                                        overlay.computed_position.y
                                            + (item_index as f32 * item_height),
                                    );

                                    action = Some(MenuAction::OpenSubmenu {
                                        menu_idx,
                                        items: items.clone(),
                                        position: sub_pos,
                                        parent_overlay: menu.overlay_id,
                                        close_first: close_sub,
                                    });
                                } else if let Some(sub_id) = close_sub {
                                    action = Some(MenuAction::CloseSubmenu { sub_id });
                                }
                            } else if let Some(sub_id) = close_sub {
                                // Not a submenu, close any open submenu
                                action = Some(MenuAction::CloseSubmenu { sub_id });
                            }
                        }
                    }
                    break;
                }
            }
        }

        // Perform collected action
        match action {
            Some(MenuAction::CloseSubmenu { sub_id }) => {
                self.close_submenu(overlays, tree, sub_id);
            }
            Some(MenuAction::OpenSubmenu {
                menu_idx,
                items,
                position: sub_pos,
                parent_overlay,
                close_first,
            }) => {
                if let Some(sub_id) = close_first {
                    self.close_submenu(overlays, tree, sub_id);
                }
                let sub_id = self.open_submenu(overlays, tree, items, sub_pos, parent_overlay);
                if let Some(menu) = self.active_menus.get_mut(menu_idx) {
                    menu.active_submenu = Some(sub_id);
                }
            }
            None => {}
        }
    }

    /// Handle click on menu item.
    pub fn handle_click(
        &mut self,
        overlays: &mut OverlayManager,
        tree: &mut UiTree,
        position: Vec2,
    ) -> bool {
        // Find which menu and item was clicked
        for menu in &self.active_menus {
            if let Some(overlay) = overlays.get(menu.overlay_id) {
                if overlay.contains_point(position) {
                    let local_y = position.y - overlay.computed_position.y;
                    let item_index = (local_y / self.style.item_height) as usize;

                    if let Some(item) = menu.items.get(item_index) {
                        if item.is_enabled() {
                            match item {
                                MenuItem::Action { on_click, .. } => {
                                    on_click();
                                    self.hide(overlays, tree);
                                    return true;
                                }
                                MenuItem::Checkbox {
                                    checked, on_toggle, ..
                                } => {
                                    on_toggle(!*checked);
                                    self.hide(overlays, tree);
                                    return true;
                                }
                                MenuItem::Radio { on_select, .. } => {
                                    on_select();
                                    self.hide(overlays, tree);
                                    return true;
                                }
                                MenuItem::Submenu { .. } => {
                                    // Submenus don't close on click
                                    return true;
                                }
                                _ => {}
                            }
                        }
                    }
                    return true;
                }
            }
        }
        false
    }

    /// Open a submenu.
    fn open_submenu(
        &mut self,
        overlays: &mut OverlayManager,
        tree: &mut UiTree,
        items: Vec<MenuItem>,
        position: Vec2,
        parent_id: OverlayId,
    ) -> OverlayId {
        let (root_node, item_nodes) = self.build_menu_tree(tree, &items);

        let overlay_id = overlays.show(
            tree,
            root_node,
            OverlayConfig {
                layer: ZLayer::Popover,
                position: OverlayPosition::Absolute {
                    x: position.x,
                    y: position.y,
                },
                close_on_outside_click: false, // Parent controls closing
                close_on_escape: true,
                trap_focus: false,
                show_backdrop: false,
                backdrop_color: Color::TRANSPARENT,
                animate_in: false,
                animate_out: false,
                auto_dismiss: None,
            },
        );

        self.active_menus.push(ActiveMenu {
            overlay_id,
            root_node,
            item_nodes,
            parent_menu: Some(parent_id),
            active_submenu: None,
            hovered_item: None,
            items,
        });

        overlay_id
    }

    /// Close a submenu and its children.
    fn close_submenu(
        &mut self,
        overlays: &mut OverlayManager,
        tree: &mut UiTree,
        overlay_id: OverlayId,
    ) {
        // Find and remove the submenu
        if let Some(index) = self
            .active_menus
            .iter()
            .position(|m| m.overlay_id == overlay_id)
        {
            // First close any child submenus
            if let Some(child_id) = self.active_menus[index].active_submenu {
                self.close_submenu(overlays, tree, child_id);
            }

            // Then close this submenu
            let menu = self.active_menus.remove(index);
            overlays.hide(tree, menu.overlay_id);
        }
    }

    /// Build the widget tree for a menu.
    fn build_menu_tree(
        &self,
        tree: &mut UiTree,
        items: &[MenuItem],
    ) -> (NodeId, Vec<(NodeId, usize)>) {
        // Create container for the menu
        let mut container = Container::new();
        container.style.background_color = Some(self.style.background_color);
        container.style.border_color = Some(self.style.border_color);
        container.style.border_width = self.style.border_width;
        container.style.border_radius = self.style.border_radius;
        container.style.layout.flex_direction = taffy::FlexDirection::Column;
        container.style.layout.min_size.width = taffy::Dimension::Length(self.style.min_width);

        let padding_y = taffy::LengthPercentage::Length(self.style.padding_y);
        container.style.layout.padding = taffy::Rect {
            left: taffy::LengthPercentage::Length(0.0),
            right: taffy::LengthPercentage::Length(0.0),
            top: padding_y,
            bottom: padding_y,
        };

        let container_id = tree.add_widget(Box::new(container));

        let mut item_nodes = Vec::new();

        for (index, item) in items.iter().enumerate() {
            let item_node = self.build_menu_item(tree, item, index);
            tree.add_child(container_id, item_node);
            item_nodes.push((item_node, index));
        }

        (container_id, item_nodes)
    }

    /// Build a single menu item widget.
    fn build_menu_item(&self, tree: &mut UiTree, item: &MenuItem, _index: usize) -> NodeId {
        match item {
            MenuItem::Separator => {
                // Separator is a simple horizontal line
                let mut sep = Container::new();
                sep.style.background_color = Some(self.style.separator_color);
                sep.style.layout.size.height = taffy::Dimension::Length(1.0);
                sep.style.layout.size.width = taffy::Dimension::Percent(1.0);
                sep.style.layout.margin = taffy::Rect {
                    left: taffy::LengthPercentageAuto::Length(self.style.padding_x),
                    right: taffy::LengthPercentageAuto::Length(self.style.padding_x),
                    top: taffy::LengthPercentageAuto::Length(4.0),
                    bottom: taffy::LengthPercentageAuto::Length(4.0),
                };
                tree.add_widget(Box::new(sep))
            }

            MenuItem::Action {
                label,
                shortcut,
                enabled,
                ..
            } => {
                self.build_text_item(tree, label, shortcut.as_deref(), None, *enabled, false)
            }

            MenuItem::Submenu { label, enabled, .. } => {
                self.build_text_item(
                    tree,
                    label,
                    Some(&self.style.submenu_indicator),
                    None,
                    *enabled,
                    true,
                )
            }

            MenuItem::Checkbox {
                label,
                checked,
                enabled,
                ..
            } => {
                let indicator = if *checked {
                    &self.style.checkbox_checked
                } else {
                    &self.style.checkbox_unchecked
                };
                self.build_text_item(tree, label, None, Some(indicator), *enabled, false)
            }

            MenuItem::Radio {
                label,
                selected,
                enabled,
                ..
            } => {
                let indicator = if *selected {
                    &self.style.radio_selected
                } else {
                    &self.style.radio_unselected
                };
                self.build_text_item(tree, label, None, Some(indicator), *enabled, false)
            }

            MenuItem::Custom { node_id, .. } => {
                // Return the pre-built custom node
                *node_id
            }
        }
    }

    /// Build a standard text menu item.
    fn build_text_item(
        &self,
        tree: &mut UiTree,
        label: &str,
        right_text: Option<&str>,
        left_indicator: Option<&str>,
        enabled: bool,
        _is_submenu: bool,
    ) -> NodeId {
        // Item container
        let mut item_container = Container::new();
        item_container.style.layout.flex_direction = taffy::FlexDirection::Row;
        item_container.style.layout.align_items = Some(taffy::AlignItems::Center);
        item_container.style.layout.justify_content = Some(taffy::JustifyContent::SpaceBetween);
        item_container.style.layout.size.height =
            taffy::Dimension::Length(self.style.item_height);
        item_container.style.layout.size.width = taffy::Dimension::Percent(1.0);

        let padding = taffy::LengthPercentage::Length(self.style.padding_x);
        item_container.style.layout.padding = taffy::Rect {
            left: padding,
            right: padding,
            top: taffy::LengthPercentage::Length(0.0),
            bottom: taffy::LengthPercentage::Length(0.0),
        };

        let container_id = tree.add_widget(Box::new(item_container));

        // Left side (indicator + label)
        let mut left_container = Container::new();
        left_container.style.layout.flex_direction = taffy::FlexDirection::Row;
        left_container.style.layout.align_items = Some(taffy::AlignItems::Center);
        left_container.style.layout.gap = taffy::Size {
            width: taffy::LengthPercentage::Length(self.style.icon_gap),
            height: taffy::LengthPercentage::Length(0.0),
        };
        let left_id = tree.add_widget(Box::new(left_container));

        // Add indicator if present
        if let Some(indicator) = left_indicator {
            let text_color = if enabled {
                self.style.text_color
            } else {
                self.style.disabled_color
            };
            let indicator_widget = crate::widgets::Text::new(indicator.to_string())
                .color(text_color)
                .size(12.0);
            let indicator_id = tree.add_widget(Box::new(indicator_widget));
            tree.add_child(left_id, indicator_id);
        }

        // Add label
        let text_color = if enabled {
            self.style.text_color
        } else {
            self.style.disabled_color
        };
        let label_widget = crate::widgets::Text::new(label.to_string())
            .color(text_color)
            .size(14.0);
        let label_id = tree.add_widget(Box::new(label_widget));
        tree.add_child(left_id, label_id);

        tree.add_child(container_id, left_id);

        // Right side (shortcut or submenu indicator)
        if let Some(right) = right_text {
            let right_color = if enabled {
                self.style.shortcut_color
            } else {
                self.style.disabled_color
            };
            let right_widget = crate::widgets::Text::new(right.to_string())
                .color(right_color)
                .size(12.0);
            let right_id = tree.add_widget(Box::new(right_widget));
            tree.add_child(container_id, right_id);
        }

        container_id
    }

    /// Get the number of items.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Get items.
    pub fn items(&self) -> &[MenuItem] {
        &self.items
    }
}

impl Default for ContextMenu {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

/// Menu bar for application menus.
#[derive(Debug)]
pub struct MenuBar {
    /// Menu bar items (each opens a dropdown).
    menus: Vec<(String, Vec<MenuItem>)>,
    /// Currently open menu index.
    open_menu: Option<usize>,
    /// Active context menu.
    active_menu: Option<ContextMenu>,
    /// Style for dropdown menus.
    style: MenuStyle,
}

impl MenuBar {
    /// Create a new menu bar.
    pub fn new() -> Self {
        Self {
            menus: Vec::new(),
            open_menu: None,
            active_menu: None,
            style: MenuStyle::default(),
        }
    }

    /// Add a menu to the bar.
    pub fn add_menu(&mut self, label: impl Into<String>, items: Vec<MenuItem>) {
        self.menus.push((label.into(), items));
    }

    /// Get the number of menus.
    pub fn menu_count(&self) -> usize {
        self.menus.len()
    }

    /// Get menu labels.
    pub fn menu_labels(&self) -> impl Iterator<Item = &str> {
        self.menus.iter().map(|(label, _)| label.as_str())
    }

    /// Open a menu by index.
    pub fn open_menu(
        &mut self,
        index: usize,
        overlays: &mut OverlayManager,
        tree: &mut UiTree,
        position: Vec2,
    ) {
        self.close_menu(overlays, tree);

        if let Some((_, items)) = self.menus.get(index) {
            let mut menu = ContextMenu::with_style(items.clone(), self.style.clone());
            menu.show(overlays, tree, position);
            self.active_menu = Some(menu);
            self.open_menu = Some(index);
        }
    }

    /// Close the currently open menu.
    pub fn close_menu(&mut self, overlays: &mut OverlayManager, tree: &mut UiTree) {
        if let Some(menu) = &mut self.active_menu {
            menu.hide(overlays, tree);
        }
        self.active_menu = None;
        self.open_menu = None;
    }

    /// Check if a menu is open.
    pub fn is_open(&self) -> bool {
        self.open_menu.is_some()
    }

    /// Get the currently open menu index.
    pub fn open_index(&self) -> Option<usize> {
        self.open_menu
    }
}

impl Default for MenuBar {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_menu_item_action() {
        let item = MenuItem::action("Test", || {});
        assert_eq!(item.label(), Some("Test"));
        assert!(item.is_enabled());
        assert!(!item.is_submenu());
    }

    #[test]
    fn test_menu_item_submenu() {
        let item = MenuItem::submenu("Sub", vec![MenuItem::action("Child", || {})]);
        assert!(item.is_submenu());
        if let MenuItem::Submenu { items, .. } = &item {
            assert_eq!(items.len(), 1);
        }
    }

    #[test]
    fn test_menu_item_checkbox() {
        let item = MenuItem::checkbox("Option", true, |_| {});
        assert_eq!(item.label(), Some("Option"));
        if let MenuItem::Checkbox { checked, .. } = &item {
            assert!(*checked);
        }
    }

    #[test]
    fn test_menu_item_enabled() {
        let item = MenuItem::action("Test", || {}).enabled(false);
        assert!(!item.is_enabled());
    }

    #[test]
    fn test_context_menu() {
        let menu = ContextMenu::new(vec![
            MenuItem::action("Item 1", || {}),
            MenuItem::separator(),
            MenuItem::action("Item 2", || {}),
        ]);
        assert_eq!(menu.item_count(), 3);
        assert!(!menu.is_visible());
    }

    #[test]
    fn test_menu_style_default() {
        let style = MenuStyle::default();
        assert!(style.item_height > 0.0);
        assert!(style.min_width > 0.0);
    }

    #[test]
    fn test_menu_bar() {
        let mut bar = MenuBar::new();
        bar.add_menu("File", vec![MenuItem::action("New", || {})]);
        bar.add_menu("Edit", vec![MenuItem::action("Undo", || {})]);

        assert_eq!(bar.menu_count(), 2);
        assert!(!bar.is_open());

        let labels: Vec<&str> = bar.menu_labels().collect();
        assert_eq!(labels, vec!["File", "Edit"]);
    }

    #[test]
    fn test_menu_item_separator() {
        let item = MenuItem::separator();
        assert!(matches!(item, MenuItem::Separator));
        assert_eq!(item.label(), None);
        assert!(!item.is_submenu());
    }

    #[test]
    fn test_menu_item_action_details() {
        let item = MenuItem::action("Save", || {});
        assert_eq!(item.label(), Some("Save"));
        assert!(item.is_enabled());
    }

    #[test]
    fn test_nested_submenu() {
        let inner_sub = MenuItem::submenu("More", vec![
            MenuItem::action("Option A", || {}),
            MenuItem::action("Option B", || {}),
        ]);

        let outer_sub = MenuItem::submenu("Settings", vec![
            MenuItem::action("Preferences", || {}),
            inner_sub,
        ]);

        assert!(outer_sub.is_submenu());
        if let MenuItem::Submenu { items, .. } = &outer_sub {
            assert_eq!(items.len(), 2);
            assert!(items[1].is_submenu());
        }
    }

    #[test]
    fn test_menu_style_custom() {
        let style = MenuStyle {
            item_height: 30.0,
            min_width: 200.0,
            padding_x: 10.0,
            padding_y: 5.0,
            ..Default::default()
        };

        assert_eq!(style.item_height, 30.0);
        assert_eq!(style.min_width, 200.0);
        assert_eq!(style.padding_x, 10.0);
        assert_eq!(style.padding_y, 5.0);
    }

    #[test]
    fn test_context_menu_items() {
        let menu = ContextMenu::new(vec![
            MenuItem::action("Cut", || {}),
            MenuItem::action("Copy", || {}),
            MenuItem::action("Paste", || {}),
            MenuItem::separator(),
            MenuItem::checkbox("Show Hidden", false, |_| {}),
        ]);

        assert_eq!(menu.item_count(), 5);
    }

    #[test]
    fn test_menu_item_checkbox_toggle() {
        // Initial state is checked
        let checked_item = MenuItem::checkbox("Option", true, |_| {});
        if let MenuItem::Checkbox { checked, .. } = &checked_item {
            assert!(*checked);
        }

        // Initial state is unchecked
        let unchecked_item = MenuItem::checkbox("Other", false, |_| {});
        if let MenuItem::Checkbox { checked, .. } = &unchecked_item {
            assert!(!*checked);
        }
    }

    #[test]
    fn test_menu_bar_empty() {
        let bar = MenuBar::new();
        assert_eq!(bar.menu_count(), 0);
        assert!(!bar.is_open());
        assert!(bar.menu_labels().next().is_none());
    }

    #[test]
    fn test_context_menu_empty() {
        let menu = ContextMenu::new(vec![]);
        assert_eq!(menu.item_count(), 0);
    }

    #[test]
    fn test_menu_item_enabled_chain() {
        let item = MenuItem::action("Test", || {}).enabled(false);
        assert!(!item.is_enabled());
        assert_eq!(item.label(), Some("Test"));
    }
}
