//! Event handling system for UI interactions.

use crate::tree::{NodeId, UiTree};
#[cfg(feature = "docking")]
use crate::widgets::docking::animation::GhostGroupAnimation;
#[cfg(feature = "docking")]
use crate::widgets::docking::operations::{
    DockOperation, MergeTabGroupOperation, MoveTabGroupOperation, SplitContainerOperation,
    TransferTabOperation,
};
#[cfg(feature = "docking")]
use crate::widgets::docking::{
    DockSplitter, DockTabs, DragType, DropPreviewAnimation, DropTarget, GhostTabAnimation,
};
use crate::widgets::scroll_container::ScrollContainer;
use astrelis_core::alloc::HashSet;
use astrelis_core::math::Vec2;
use astrelis_core::profiling::profile_function;
use astrelis_winit::event::{ElementState, Event, EventBatch, HandleStatus, PhysicalKey};

/// UI event types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiEvent {
    /// Mouse entered widget bounds.
    MouseEnter,
    /// Mouse left widget bounds.
    MouseLeave,
    /// Mouse button pressed on widget.
    MouseDown,
    /// Mouse button released on widget.
    MouseUp,
    /// Widget was clicked.
    Click,
    /// Focus gained.
    FocusGained,
    /// Focus lost.
    FocusLost,
}

/// Mouse button state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// UI event handling system.
pub struct UiEventSystem {
    /// Currently hovered node.
    hovered: Option<NodeId>,
    /// Currently focused node.
    focused: Option<NodeId>,
    /// Node with active tooltip (planned feature).
    #[allow(dead_code)]
    tooltip_node: Option<NodeId>,
    /// Current mouse position.
    mouse_pos: Vec2,
    /// Pressed mouse buttons.
    mouse_buttons: HashSet<MouseButton>,
    /// Nodes that were pressed this frame.
    pressed_nodes: HashSet<NodeId>,
}

/// Re-export `CrossContainerPreview` from the docking plugin.
#[cfg(feature = "docking")]
pub use crate::widgets::docking::plugin::CrossContainerPreview;

impl UiEventSystem {
    /// Create a new event system.
    pub fn new() -> Self {
        Self {
            hovered: None,
            focused: None,
            tooltip_node: None,
            mouse_pos: Vec2::ZERO,
            mouse_buttons: HashSet::new(),
            pressed_nodes: HashSet::new(),
        }
    }

    /// Get currently hovered node.
    pub fn hovered(&self) -> Option<NodeId> {
        self.hovered
    }

    /// Get currently focused node.
    pub fn focused(&self) -> Option<NodeId> {
        self.focused
    }

    /// Get current mouse position.
    pub fn mouse_position(&self) -> Vec2 {
        self.mouse_pos
    }

    /// Check if a mouse button is pressed.
    pub fn is_button_pressed(&self, button: MouseButton) -> bool {
        self.mouse_buttons.contains(&button)
    }

    /// Set focus to a specific node.
    pub fn set_focus(&mut self, node_id: Option<NodeId>) {
        if self.focused != node_id {
            self.focused = node_id;
        }
    }

    /// Invalidate any event system references to nodes that no longer exist in the tree.
    ///
    /// Called after operations that remove nodes (e.g., collapse_empty_container)
    /// to prevent stale NodeId references from causing lookups on deleted nodes.
    fn invalidate_removed_nodes(
        &mut self,
        tree: &UiTree,
        plugins: &mut crate::plugin::PluginManager,
    ) {
        if let Some(id) = self.hovered
            && !tree.node_exists(id)
        {
            self.hovered = None;
        }
        if let Some(id) = self.focused
            && !tree.node_exists(id)
        {
            self.focused = None;
        }
        #[cfg(feature = "docking")]
        if let Some(dp) = plugins.get_mut::<crate::widgets::docking::plugin::DockingPlugin>() {
            dp.invalidate_removed_nodes(tree);
        }
        if let Some(sp) = plugins.get_mut::<crate::scroll_plugin::ScrollPlugin>() {
            sp.invalidate_removed_nodes(tree);
        }
        self.pressed_nodes.retain(|id| tree.node_exists(*id));
    }

    /// Handle events from the event batch (without plugin access).
    ///
    /// Prefer [`handle_events_with_plugins`](Self::handle_events_with_plugins) when a PluginManager is available.
    pub fn handle_events(&mut self, events: &mut EventBatch, tree: &mut UiTree) {
        // When called without plugins, create a temporary PluginManager.
        // Docking features won't work without the DockingPlugin, but core events still function.
        let mut pm = crate::plugin::PluginManager::new();
        self.handle_events_with_plugins(events, tree, &mut pm);
    }

    /// Handle events from the event batch with plugin access.
    ///
    /// Docking state is accessed from the [`DockingPlugin`](crate::widgets::docking::plugin::DockingPlugin)
    /// in the plugin manager.
    pub fn handle_events_with_plugins(
        &mut self,
        events: &mut EventBatch,
        tree: &mut UiTree,
        plugins: &mut crate::plugin::PluginManager,
    ) {
        profile_function!();
        events.dispatch(|event| match event {
            Event::MouseMoved(pos) => {
                self.mouse_pos = Vec2::new(pos.x as f32, pos.y as f32);
                self.update_hover(tree, plugins);
                HandleStatus::consumed()
            }
            Event::MouseButtonDown(button) => {
                self.handle_mouse_input(*button, true, tree, plugins);
                HandleStatus::consumed()
            }
            Event::MouseButtonUp(button) => {
                self.handle_mouse_input(*button, false, tree, plugins);
                HandleStatus::consumed()
            }
            Event::MouseScrolled(delta) => {
                self.handle_scroll_event(delta, tree, plugins);
                HandleStatus::consumed()
            }
            Event::PanGesture(gesture) => {
                self.handle_pan_gesture(gesture, tree, plugins);
                HandleStatus::consumed()
            }
            Event::KeyInput(key_event) => {
                if key_event.state == ElementState::Pressed {
                    // Handle text input from key event
                    if let Some(ref text) = key_event.text {
                        for c in text.chars() {
                            self.handle_char_input(c, tree, plugins);
                        }
                    }
                    // Handle special keys
                    self.handle_key_input(&key_event.physical_key, tree, plugins);
                }
                HandleStatus::consumed()
            }
            _ => HandleStatus::ignored(),
        });
    }

    /// Handle mouse input events.
    fn handle_mouse_input(
        &mut self,
        button: astrelis_winit::event::MouseButton,
        pressed: bool,
        tree: &mut UiTree,
        plugins: &mut crate::plugin::PluginManager,
    ) {
        let mouse_button = match button {
            astrelis_winit::event::MouseButton::Left => MouseButton::Left,
            astrelis_winit::event::MouseButton::Right => MouseButton::Right,
            astrelis_winit::event::MouseButton::Middle => MouseButton::Middle,
            _ => return,
        };

        if pressed {
            self.mouse_buttons.insert(mouse_button);

            #[cfg(feature = "docking")]
            {
                let mut docking_needs_invalidate = false;
                if let Some(dp) =
                    plugins.get_mut::<crate::widgets::docking::plugin::DockingPlugin>()
                {
                    // Check for splitter separator press first
                    if mouse_button == MouseButton::Left {
                        if let Some(splitter_node) = dp.hovered_splitter {
                            // Start splitter drag
                            if let Some(widget) = tree.get_widget(splitter_node)
                                && let Some(splitter) =
                                    widget.as_any().downcast_ref::<DockSplitter>()
                            {
                                dp.drag_manager.start_splitter_drag(
                                    splitter_node,
                                    splitter.direction,
                                    self.mouse_pos,
                                    splitter.split_ratio,
                                );
                                // Mark the splitter as dragging
                                if let Some(widget) = tree.get_widget_mut(splitter_node)
                                    && let Some(splitter) =
                                        widget.as_any_mut().downcast_mut::<DockSplitter>()
                                {
                                    splitter.set_separator_dragging(true);
                                    tree.mark_dirty_flags(
                                        splitter_node,
                                        crate::dirty::DirtyFlags::COLOR,
                                    );
                                }
                                return; // Don't process further
                            }
                        }

                        // Check for scrollbar thumb click
                        if let Some(hovered_id) = self.hovered
                            && let Some(widget) = tree.get_widget(hovered_id)
                            && let Some(tabs) = widget.as_any().downcast_ref::<DockTabs>()
                            && tabs.should_show_scrollbar()
                        {
                            let layout = tree.get_layout(hovered_id).unwrap();
                            let abs_layout = self.get_absolute_layout(tree, hovered_id, layout);

                            if tabs.hit_test_scrollbar_thumb(self.mouse_pos, &abs_layout) {
                                // Start scrollbar drag
                                if let Some(widget) = tree.get_widget_mut(hovered_id)
                                    && let Some(tabs) =
                                        widget.as_any_mut().downcast_mut::<DockTabs>()
                                {
                                    tabs.start_scrollbar_drag(self.mouse_pos.x, &abs_layout);
                                    dp.scrollbar_drag_node = Some(hovered_id);
                                    tree.mark_dirty_flags(
                                        hovered_id,
                                        crate::dirty::DirtyFlags::GEOMETRY,
                                    );
                                }
                                return;
                            }
                        }

                        // Check for tab click
                        if let Some(hovered_id) = self.hovered
                            && let Some(widget) = tree.get_widget(hovered_id)
                            && let Some(tabs) = widget.as_any().downcast_ref::<DockTabs>()
                        {
                            let layout = tree.get_layout(hovered_id).unwrap();
                            let abs_layout = self.get_absolute_layout(tree, hovered_id, layout);

                            // Check for close button click first
                            if let Some(close_idx) =
                                tabs.hit_test_close_button(self.mouse_pos, &abs_layout)
                            {
                                // Close the tab
                                if let Some(widget) = tree.get_widget_mut(hovered_id)
                                    && let Some(tabs) =
                                        widget.as_any_mut().downcast_mut::<DockTabs>()
                                {
                                    tabs.close_tab(close_idx);
                                    tree.mark_dirty_flags(
                                        hovered_id,
                                        crate::dirty::DirtyFlags::LAYOUT,
                                    );
                                }

                                // Collapse empty container if last tab was closed
                                if let Err(e) =
                                    crate::widgets::docking::operations::collapse_empty_container(
                                        tree, hovered_id,
                                    )
                                {
                                    tracing::warn!("Failed to collapse empty container: {}", e);
                                }

                                // Invalidate references to removed nodes
                                docking_needs_invalidate = true;
                                dp.docking_context.invalidate();

                                // dp scope will end, invalidate_removed_nodes called below
                            } else {
                                // Check for tab click - start potential drag
                                if let Some(tab_idx) =
                                    tabs.hit_test_tab(self.mouse_pos, &abs_layout)
                                {
                                    // Start potential tab drag (will become active after threshold)
                                    dp.drag_manager.start_tab_drag(
                                        hovered_id,
                                        tab_idx,
                                        self.mouse_pos,
                                    );
                                    return;
                                }

                                // Check for tab bar background click - start potential group drag
                                if tabs.hit_test_tab_bar_background(self.mouse_pos, &abs_layout) {
                                    dp.drag_manager
                                        .start_tab_group_drag(hovered_id, self.mouse_pos);
                                    return;
                                }
                            }
                        }
                    }
                }
                if docking_needs_invalidate {
                    self.invalidate_removed_nodes(tree, plugins);
                    return;
                }
            }

            // Check for ScrollContainer scrollbar thumb click
            if mouse_button == MouseButton::Left
                && let Some(hovered_id) = self.hovered
                && let Some(widget) = tree.get_widget(hovered_id)
                && let Some(sc) = widget.as_any().downcast_ref::<ScrollContainer>()
                && (sc.should_show_v_scrollbar() || sc.should_show_h_scrollbar())
            {
                let layout = tree.get_layout(hovered_id).unwrap();
                let abs_layout = self.get_absolute_layout(tree, hovered_id, layout);

                if sc.hit_test_v_thumb(self.mouse_pos, &abs_layout) {
                    if let Some(widget) = tree.get_widget_mut(hovered_id)
                        && let Some(sc) = widget.as_any_mut().downcast_mut::<ScrollContainer>()
                    {
                        sc.start_v_drag(self.mouse_pos.y, &abs_layout);
                        if let Some(sp) = plugins.get_mut::<crate::scroll_plugin::ScrollPlugin>() {
                            sp.scroll_container_drag = Some((hovered_id, true));
                        }
                        tree.mark_dirty_flags(hovered_id, crate::dirty::DirtyFlags::GEOMETRY);
                    }
                    return;
                }

                if sc.hit_test_h_thumb(self.mouse_pos, &abs_layout) {
                    if let Some(widget) = tree.get_widget_mut(hovered_id)
                        && let Some(sc) = widget.as_any_mut().downcast_mut::<ScrollContainer>()
                    {
                        sc.start_h_drag(self.mouse_pos.x, &abs_layout);
                        if let Some(sp) = plugins.get_mut::<crate::scroll_plugin::ScrollPlugin>() {
                            sp.scroll_container_drag = Some((hovered_id, false));
                        }
                        tree.mark_dirty_flags(hovered_id, crate::dirty::DirtyFlags::GEOMETRY);
                    }
                    return;
                }
            }

            // Handle press on hovered widget
            if let Some(hovered_id) = self.hovered {
                self.pressed_nodes.insert(hovered_id);

                // Update widget press state via registry
                let on_press = tree
                    .get_widget(hovered_id)
                    .map(|w| w.as_any().type_id())
                    .and_then(|tid| plugins.widget_registry().get(tid))
                    .and_then(|desc| desc.on_press);
                if let Some(on_press) = on_press
                    && let Some(widget) = tree.get_widget_mut(hovered_id)
                {
                    on_press(widget.as_any_mut(), true);
                    tree.mark_dirty_flags(hovered_id, crate::dirty::DirtyFlags::COLOR);
                }
            }
        } else {
            self.mouse_buttons.remove(&mouse_button);

            // Handle scrollbar drag end
            #[cfg(feature = "docking")]
            {
                if let Some(dp) =
                    plugins.get_mut::<crate::widgets::docking::plugin::DockingPlugin>()
                    && mouse_button == MouseButton::Left
                    && let Some(node) = dp.scrollbar_drag_node.take()
                {
                    if let Some(widget) = tree.get_widget_mut(node)
                        && let Some(tabs) = widget.as_any_mut().downcast_mut::<DockTabs>()
                    {
                        tabs.end_scrollbar_drag();
                        tree.mark_dirty_flags(node, crate::dirty::DirtyFlags::GEOMETRY);
                    }
                    return;
                }
            }

            // Handle ScrollContainer scrollbar drag end
            if mouse_button == MouseButton::Left {
                let drag = plugins
                    .get_mut::<crate::scroll_plugin::ScrollPlugin>()
                    .and_then(|sp| sp.scroll_container_drag.take());
                if let Some((node, is_vertical)) = drag {
                    if let Some(widget) = tree.get_widget_mut(node)
                        && let Some(sc) = widget.as_any_mut().downcast_mut::<ScrollContainer>()
                    {
                        if is_vertical {
                            sc.end_v_drag();
                        } else {
                            sc.end_h_drag();
                        }
                        tree.mark_dirty_flags(node, crate::dirty::DirtyFlags::GEOMETRY);
                    }
                    return;
                }
            }

            // Handle drag end
            #[cfg(feature = "docking")]
            {
                let mut drag_needs_invalidate = false;
                let mut drag_handled = false;
                if let Some(dp) =
                    plugins.get_mut::<crate::widgets::docking::plugin::DockingPlugin>()
                    && mouse_button == MouseButton::Left
                    && (dp.drag_manager.is_dragging() || dp.drag_manager.has_pending_drag())
                {
                    drag_handled = true;
                    let (drag_state_opt, drop_target) = dp.drag_manager.end_drag();
                    if let Some(drag_state) = drag_state_opt {
                        match drag_state.drag_type {
                            DragType::SplitterResize { splitter_node, .. } => {
                                // Clear dragging state
                                if let Some(widget) = tree.get_widget_mut(splitter_node)
                                    && let Some(splitter) =
                                        widget.as_any_mut().downcast_mut::<DockSplitter>()
                                {
                                    splitter.set_separator_dragging(false);
                                    tree.mark_dirty_flags(
                                        splitter_node,
                                        crate::dirty::DirtyFlags::COLOR,
                                    );
                                }
                            }
                            DragType::TabGroupDrag { tabs_node } => {
                                if drag_state.is_active
                                    && let Some(drop_target) = drop_target
                                {
                                    if drop_target.container_id != tabs_node {
                                        if drop_target.is_center_drop() {
                                            // Merge all tabs into target
                                            let insert_index =
                                                drop_target.insert_index.unwrap_or(0);
                                            let mut op = MergeTabGroupOperation::new(
                                                tabs_node,
                                                drop_target.container_id,
                                                insert_index,
                                            );
                                            if let Err(e) = op.execute(tree) {
                                                tracing::warn!("Tab group merge failed: {}", e);
                                            } else {
                                                tracing::debug!(
                                                    "Merged tab group from {:?} into {:?} at index {}",
                                                    tabs_node,
                                                    drop_target.container_id,
                                                    insert_index
                                                );
                                            }

                                            // Collapse empty source container
                                            if let Err(e) =
                                                    crate::widgets::docking::operations::collapse_empty_container(
                                                        tree, tabs_node,
                                                    )
                                                {
                                                    tracing::warn!(
                                                        "Failed to collapse empty container: {}",
                                                        e
                                                    );
                                                }
                                        } else if drop_target.is_edge_drop() {
                                            // Move entire group to edge
                                            let mut op = MoveTabGroupOperation::new(
                                                tabs_node,
                                                drop_target.container_id,
                                                drop_target.zone,
                                            );
                                            if let Err(e) = op.execute(tree) {
                                                tracing::warn!("Tab group move failed: {}", e);
                                            } else {
                                                tracing::debug!(
                                                    "Moved tab group {:?} to {:?} edge {:?}",
                                                    tabs_node,
                                                    drop_target.container_id,
                                                    drop_target.zone
                                                );
                                            }
                                        }

                                        drag_needs_invalidate = true;
                                        dp.docking_context.invalidate();
                                    }

                                    // Clear cross-container preview
                                    if let Some(old_preview) = dp.cross_container_preview.take() {
                                        tree.mark_dirty_flags(
                                            old_preview.target_node,
                                            crate::dirty::DirtyFlags::GEOMETRY,
                                        );
                                    }
                                }
                                // If not active, it was a click on tab bar background — no action needed
                            }
                            DragType::TabDrag {
                                tabs_node,
                                tab_index,
                            } => {
                                if drag_state.is_active {
                                    // Check for cross-container drop first
                                    if let Some(drop_target) = drop_target {
                                        if drop_target.container_id != tabs_node
                                            && drop_target.is_center_drop()
                                        {
                                            // Cancel drag state BEFORE execute (node may be removed by collapse)
                                            if let Some(widget) = tree.get_widget_mut(tabs_node)
                                                && let Some(tabs) =
                                                    widget.as_any_mut().downcast_mut::<DockTabs>()
                                            {
                                                tabs.cancel_tab_drag();
                                            }

                                            // Execute cross-container transfer
                                            let insert_index =
                                                drop_target.insert_index.unwrap_or(0);
                                            let mut op = TransferTabOperation::new(
                                                tabs_node,
                                                drop_target.container_id,
                                                tab_index,
                                                insert_index,
                                            );

                                            if let Err(e) = op.execute(tree) {
                                                tracing::warn!(
                                                    "Cross-container tab transfer failed: {}",
                                                    e
                                                );
                                            } else {
                                                tracing::debug!(
                                                    "Transferred tab {} from {:?} to {:?} at index {}",
                                                    tab_index,
                                                    tabs_node,
                                                    drop_target.container_id,
                                                    insert_index
                                                );

                                                // Make the transferred tab active in the target
                                                if let Some(widget) =
                                                    tree.get_widget_mut(drop_target.container_id)
                                                    && let Some(target_tabs) = widget
                                                        .as_any_mut()
                                                        .downcast_mut::<DockTabs>(
                                                    )
                                                {
                                                    target_tabs.set_active_tab(insert_index);
                                                }
                                            }

                                            // Collapse empty source container after transfer
                                            if let Err(e) =
                                                    crate::widgets::docking::operations::collapse_empty_container(
                                                        tree, tabs_node,
                                                    )
                                                {
                                                    tracing::warn!(
                                                        "Failed to collapse empty container: {}",
                                                        e
                                                    );
                                                }

                                            // Invalidate references to removed nodes
                                            drag_needs_invalidate = true;
                                            dp.docking_context.invalidate();

                                            // Mark both containers dirty
                                            tree.mark_dirty_flags(
                                                drop_target.container_id,
                                                crate::dirty::DirtyFlags::LAYOUT,
                                            );
                                            // Only mark source if it still exists
                                            if tree.node_exists(tabs_node) {
                                                tree.mark_dirty_flags(
                                                    tabs_node,
                                                    crate::dirty::DirtyFlags::LAYOUT,
                                                );
                                            }
                                        } else if drop_target.is_edge_drop() {
                                            // Edge drop: create split (same or different container)
                                            // Cancel drag state BEFORE execute (node may be removed by collapse)
                                            if let Some(widget) = tree.get_widget_mut(tabs_node)
                                                && let Some(tabs) =
                                                    widget.as_any_mut().downcast_mut::<DockTabs>()
                                            {
                                                tabs.cancel_tab_drag();
                                            }

                                            let mut op = SplitContainerOperation::new(
                                                tabs_node,
                                                drop_target.container_id,
                                                tab_index,
                                                drop_target.zone,
                                            );

                                            if let Err(e) = op.execute(tree) {
                                                tracing::warn!("Edge-zone split failed: {}", e);
                                            } else {
                                                tracing::debug!(
                                                    "Split container: tab {} from {:?} to {:?} edge {:?}",
                                                    tab_index,
                                                    tabs_node,
                                                    drop_target.container_id,
                                                    drop_target.zone
                                                );
                                            }

                                            // Collapse empty source container after split
                                            // (no-op for same-container splits since source keeps N-1 >= 1 tabs)
                                            if tree.node_exists(tabs_node)
                                                    && let Err(e) =
                                                        crate::widgets::docking::operations::collapse_empty_container(
                                                            tree, tabs_node,
                                                        )
                                                {
                                                    tracing::warn!(
                                                        "Failed to collapse empty container: {}",
                                                        e
                                                    );
                                                }

                                            // Invalidate references to removed nodes
                                            drag_needs_invalidate = true;
                                            dp.docking_context.invalidate();
                                        } else {
                                            // Fallback: cancel drag
                                            if let Some(widget) = tree.get_widget_mut(tabs_node)
                                                && let Some(tabs) =
                                                    widget.as_any_mut().downcast_mut::<DockTabs>()
                                            {
                                                tabs.cancel_tab_drag();
                                            }
                                            tree.mark_dirty_flags(
                                                tabs_node,
                                                crate::dirty::DirtyFlags::LAYOUT,
                                            );
                                        }
                                    } else {
                                        // No cross-container target: complete within-container reordering
                                        let all_children = if let Some(widget) =
                                            tree.get_widget_mut(tabs_node)
                                            && let Some(tabs) =
                                                widget.as_any_mut().downcast_mut::<DockTabs>()
                                        {
                                            let children = tabs.children.clone();
                                            tabs.finish_tab_drag();
                                            children
                                        } else {
                                            Vec::new()
                                        };

                                        let mut batch: Vec<(
                                            crate::tree::NodeId,
                                            crate::dirty::DirtyFlags,
                                        )> = Vec::with_capacity(1 + all_children.len());
                                        batch.push((tabs_node, crate::dirty::DirtyFlags::LAYOUT));
                                        for child in all_children {
                                            batch.push((child, crate::dirty::DirtyFlags::LAYOUT));
                                        }
                                        tree.mark_dirty_batch(&batch);
                                    }

                                    // Clear cross-container preview
                                    if let Some(old_preview) = dp.cross_container_preview.take() {
                                        tree.mark_dirty_flags(
                                            old_preview.target_node,
                                            crate::dirty::DirtyFlags::GEOMETRY,
                                        );
                                    }
                                } else {
                                    // Treat as click (switch active tab)
                                    let (old_active_child, new_active_child) = if let Some(widget) =
                                        tree.get_widget_mut(tabs_node)
                                        && let Some(tabs) =
                                            widget.as_any_mut().downcast_mut::<DockTabs>()
                                    {
                                        let old_child = tabs.children.get(tabs.active_tab).copied();
                                        tabs.set_active_tab(tab_index);
                                        let new_child = tabs.children.get(tabs.active_tab).copied();
                                        (old_child, new_child)
                                    } else {
                                        (None, None)
                                    };

                                    tree.mark_dirty_flags(
                                        tabs_node,
                                        crate::dirty::DirtyFlags::LAYOUT,
                                    );

                                    if let Some(old_child) = old_active_child {
                                        tree.mark_dirty_flags(
                                            old_child,
                                            crate::dirty::DirtyFlags::LAYOUT,
                                        );
                                    }

                                    if let Some(new_child) = new_active_child {
                                        tree.mark_dirty_flags(
                                            new_child,
                                            crate::dirty::DirtyFlags::LAYOUT,
                                        );
                                    }
                                }
                            }
                            _ => {}
                        }
                    }

                    // Fade out docking animations on drag end
                    if let Some(ref mut ghost) = dp.dock_animations.ghost_tab {
                        ghost.fade_out();
                    }
                    if let Some(ref mut ghost) = dp.dock_animations.ghost_group {
                        ghost.fade_out();
                    }
                    if let Some(ref mut preview) = dp.dock_animations.drop_preview {
                        preview.fade_out();
                    }
                }
                if drag_needs_invalidate {
                    self.invalidate_removed_nodes(tree, plugins);
                }
                if drag_handled {
                    return;
                }
            }

            // Handle release - check if it's a click
            if let Some(hovered_id) = self.hovered
                && self.pressed_nodes.contains(&hovered_id)
            {
                // This is a click!
                self.dispatch_click(hovered_id, tree, plugins);
            }

            // Clear pressed state on ALL previously pressed nodes
            // (handles release outside button, drag-away scenarios)
            let mut dirty_batch: Vec<(crate::tree::NodeId, crate::dirty::DirtyFlags)> = Vec::new();
            for &pressed_node_id in &self.pressed_nodes {
                let on_press = tree
                    .get_widget(pressed_node_id)
                    .map(|w| w.as_any().type_id())
                    .and_then(|tid| plugins.widget_registry().get(tid))
                    .and_then(|desc| desc.on_press);
                if let Some(on_press) = on_press
                    && let Some(widget) = tree.get_widget_mut(pressed_node_id)
                {
                    on_press(widget.as_any_mut(), false);
                    dirty_batch.push((pressed_node_id, crate::dirty::DirtyFlags::COLOR));
                }
            }
            tree.mark_dirty_batch(&dirty_batch);

            self.pressed_nodes.clear();
        }
    }

    /// Get absolute layout for a node.
    fn get_absolute_layout(
        &self,
        tree: &UiTree,
        node_id: NodeId,
        layout: crate::tree::LayoutRect,
    ) -> crate::tree::LayoutRect {
        // Calculate absolute position by traversing parents
        let mut abs_x = layout.x;
        let mut abs_y = layout.y;

        if let Some(node) = tree.get_node(node_id) {
            let mut parent = node.parent;
            while let Some(parent_id) = parent {
                if let Some(parent_layout) = tree.get_layout(parent_id) {
                    abs_x += parent_layout.x;
                    abs_y += parent_layout.y;
                }
                // Subtract scroll offset if parent is a ScrollContainer
                if let Some(parent_widget) = tree.get_widget(parent_id)
                    && let Some(sc) = parent_widget.as_any().downcast_ref::<ScrollContainer>()
                {
                    abs_x -= sc.scroll_offset.x;
                    abs_y -= sc.scroll_offset.y;
                }
                if let Some(parent_node) = tree.get_node(parent_id) {
                    parent = parent_node.parent;
                } else {
                    break;
                }
            }
        }

        crate::tree::LayoutRect {
            x: abs_x,
            y: abs_y,
            width: layout.width,
            height: layout.height,
        }
    }

    /// Update hover state based on current mouse position.
    fn update_hover(&mut self, tree: &mut UiTree, plugins: &mut crate::plugin::PluginManager) {
        // If we're dragging the scrollbar thumb, update scroll offset
        #[cfg(feature = "docking")]
        {
            if let Some(dp) = plugins.get_mut::<crate::widgets::docking::plugin::DockingPlugin>()
                && let Some(node) = dp.scrollbar_drag_node
            {
                if let Some(layout) = tree.get_layout(node) {
                    let abs_layout = self.get_absolute_layout(tree, node, layout);
                    if let Some(widget) = tree.get_widget_mut(node)
                        && let Some(tabs) = widget.as_any_mut().downcast_mut::<DockTabs>()
                    {
                        tabs.update_scrollbar_drag(self.mouse_pos.x, &abs_layout);
                        tree.mark_dirty_flags(node, crate::dirty::DirtyFlags::GEOMETRY);
                    }
                }
                return;
            }
        }

        // If we're dragging a ScrollContainer scrollbar thumb, update scroll offset
        let sc_drag = plugins
            .get::<crate::scroll_plugin::ScrollPlugin>()
            .and_then(|sp| sp.scroll_container_drag);
        if let Some((node, is_vertical)) = sc_drag {
            if let Some(layout) = tree.get_layout(node) {
                let abs_layout = self.get_absolute_layout(tree, node, layout);
                if let Some(widget) = tree.get_widget_mut(node)
                    && let Some(sc) = widget.as_any_mut().downcast_mut::<ScrollContainer>()
                {
                    if is_vertical {
                        sc.update_v_drag(self.mouse_pos.y, &abs_layout);
                    } else {
                        sc.update_h_drag(self.mouse_pos.x, &abs_layout);
                    }
                    tree.mark_dirty_flags(node, crate::dirty::DirtyFlags::GEOMETRY);
                }
            }
            return;
        }

        // If we're dragging, update the drag state instead of hover
        #[cfg(feature = "docking")]
        {
            if let Some(dp) = plugins.get_mut::<crate::widgets::docking::plugin::DockingPlugin>()
                && (dp.drag_manager.is_dragging() || dp.drag_manager.has_pending_drag())
            {
                dp.drag_manager.update(self.mouse_pos);

                // Apply drag if active
                if let Some(drag_state) = dp.drag_manager.drag_state() {
                    if drag_state.is_active {
                        match drag_state.drag_type {
                            DragType::SplitterResize {
                                splitter_node,
                                direction: _,
                            } => {
                                let delta = drag_state.delta();
                                let original_ratio = drag_state.original_value;

                                if let Some(layout) = tree.get_layout(splitter_node) {
                                    let abs_layout =
                                        self.get_absolute_layout(tree, splitter_node, layout);
                                    if let Some(widget) = tree.get_widget_mut(splitter_node)
                                        && let Some(splitter) =
                                            widget.as_any_mut().downcast_mut::<DockSplitter>()
                                    {
                                        // Apply delta to the original ratio, not the current one
                                        splitter.apply_drag_delta_from_original(
                                            delta,
                                            &abs_layout,
                                            original_ratio,
                                        );
                                        tree.mark_dirty_flags(
                                            splitter_node,
                                            crate::dirty::DirtyFlags::LAYOUT,
                                        );
                                    }
                                }
                            }
                            DragType::TabGroupDrag { tabs_node } => {
                                // Tab group drag: always look for cross-container targets
                                // (no within-container reorder path for groups)

                                // Create ghost group animation if not already active
                                if dp.dock_animations.ghost_group.is_none() {
                                    let labels: Vec<String> = tree
                                        .get_widget(tabs_node)
                                        .and_then(|w| w.as_any().downcast_ref::<DockTabs>())
                                        .map(|t| t.tab_labels.clone())
                                        .unwrap_or_default();
                                    let total_width: f32 =
                                        labels.iter().map(|l| l.len() as f32 * 8.0 + 20.0).sum();
                                    let group_size = Vec2::new(total_width.min(300.0), 28.0);
                                    let mut ghost = GhostGroupAnimation::new(
                                        self.mouse_pos,
                                        group_size,
                                        labels,
                                    );
                                    ghost.set_target(self.mouse_pos);
                                    dp.dock_animations.ghost_group = Some(ghost);
                                } else if let Some(ref mut ghost) = dp.dock_animations.ghost_group {
                                    ghost.set_target(self.mouse_pos);
                                }

                                // Find cross-container drop targets (same logic as TabDrag but skipping source)
                                if dp.docking_context.is_dirty() {
                                    dp.docking_context.rebuild_cache(tree);
                                }
                                let all_tabs: Vec<_> = dp
                                    .docking_context
                                    .find_tab_containers(tree)
                                    .iter()
                                    .map(|(&id, info)| (id, info.layout, info.tab_count))
                                    .collect();

                                let mut found_target = false;
                                for (candidate_id, candidate_layout, _) in &all_tabs {
                                    // Skip source container entirely
                                    if *candidate_id == tabs_node {
                                        continue;
                                    }

                                    if let Some(mut zone) = dp
                                        .drop_zone_detector
                                        .detect_zone(self.mouse_pos, *candidate_layout)
                                    {
                                        // Remap to Center if cursor is in the tab bar area
                                        // (dropping on tab bar = merge, not edge split)
                                        if let Some(widget) = tree.get_widget(*candidate_id)
                                            && let Some(tabs) =
                                                widget.as_any().downcast_ref::<DockTabs>()
                                        {
                                            let tab_bar_bottom =
                                                candidate_layout.y + tabs.theme.tab_bar_height;
                                            if self.mouse_pos.y < tab_bar_bottom {
                                                zone = crate::widgets::docking::DockZone::Center;
                                            }
                                        }

                                        let preview_bounds = dp
                                            .drop_zone_detector
                                            .preview_bounds(zone, *candidate_layout);

                                        let insert_index = if matches!(
                                            zone,
                                            crate::widgets::docking::DockZone::Center
                                        ) {
                                            if let Some(widget) = tree.get_widget(*candidate_id)
                                                && let Some(target_tabs) =
                                                    widget.as_any().downcast_ref::<DockTabs>()
                                            {
                                                Some(target_tabs.tab_count())
                                            } else {
                                                None
                                            }
                                        } else {
                                            None
                                        };

                                        let preview = CrossContainerPreview {
                                            target_node: *candidate_id,
                                            target_layout: *candidate_layout,
                                            zone,
                                            preview_bounds,
                                            insert_index,
                                        };

                                        if let Some(old_preview) = &dp.cross_container_preview
                                            && old_preview.target_node != *candidate_id
                                        {
                                            tree.mark_dirty_flags(
                                                old_preview.target_node,
                                                crate::dirty::DirtyFlags::GEOMETRY,
                                            );
                                        }

                                        dp.cross_container_preview = Some(preview);
                                        dp.drag_manager.set_drop_target(
                                            DropTarget::new(*candidate_id, zone)
                                                .with_insert_index(insert_index.unwrap_or(0)),
                                        );
                                        tree.mark_dirty_flags(
                                            *candidate_id,
                                            crate::dirty::DirtyFlags::GEOMETRY,
                                        );

                                        if let Some(ref mut anim) = dp.dock_animations.drop_preview
                                        {
                                            anim.set_target(preview_bounds);
                                        } else {
                                            dp.dock_animations.drop_preview =
                                                Some(DropPreviewAnimation::new(preview_bounds));
                                        }
                                        found_target = true;
                                        break;
                                    }
                                }

                                if !found_target {
                                    if let Some(old_preview) = dp.cross_container_preview.take() {
                                        tree.mark_dirty_flags(
                                            old_preview.target_node,
                                            crate::dirty::DirtyFlags::GEOMETRY,
                                        );
                                    }
                                    dp.drag_manager.clear_drop_target();

                                    if let Some(ref mut anim) = dp.dock_animations.drop_preview {
                                        anim.fade_out();
                                    }
                                }
                            }
                            DragType::TabDrag {
                                tabs_node,
                                tab_index,
                            } => {
                                // Check if cursor is still within the source DockTabs
                                let source_contains_cursor =
                                    if let Some(layout) = tree.get_layout(tabs_node) {
                                        let abs_layout =
                                            self.get_absolute_layout(tree, tabs_node, layout);
                                        // Only check tab bar bounds for within-container reordering
                                        let tab_bar = crate::tree::LayoutRect {
                                            x: abs_layout.x,
                                            y: abs_layout.y,
                                            width: abs_layout.width,
                                            height: if let Some(widget) = tree.get_widget(tabs_node)
                                                && let Some(tabs) =
                                                    widget.as_any().downcast_ref::<DockTabs>()
                                            {
                                                tabs.theme.tab_bar_height
                                            } else {
                                                28.0
                                            },
                                        };
                                        tab_bar.contains(self.mouse_pos)
                                    } else {
                                        false
                                    };

                                if source_contains_cursor {
                                    // Within source container: do within-container reordering
                                    dp.cross_container_preview = None;
                                    dp.drag_manager.clear_drop_target();

                                    // Fade out ghost and drop preview when back in source container
                                    if let Some(ref mut ghost) = dp.dock_animations.ghost_tab {
                                        ghost.fade_out();
                                    }
                                    if let Some(ref mut preview) = dp.dock_animations.drop_preview {
                                        preview.fade_out();
                                    }

                                    if let Some(layout) = tree.get_layout(tabs_node) {
                                        let abs_layout =
                                            self.get_absolute_layout(tree, tabs_node, layout);
                                        if let Some(widget) = tree.get_widget_mut(tabs_node)
                                            && let Some(tabs) =
                                                widget.as_any_mut().downcast_mut::<DockTabs>()
                                        {
                                            tabs.update_drop_target(self.mouse_pos, &abs_layout);
                                            tree.mark_dirty_flags(
                                                tabs_node,
                                                crate::dirty::DirtyFlags::GEOMETRY,
                                            );
                                        }
                                    }
                                } else {
                                    // Outside source container: look for cross-container drop targets
                                    // Clear within-container drop target
                                    if let Some(widget) = tree.get_widget_mut(tabs_node)
                                        && let Some(tabs) =
                                            widget.as_any_mut().downcast_mut::<DockTabs>()
                                    {
                                        tabs.drag.drag_drop_target = None;
                                        tabs.drag.drag_cursor_pos = None;
                                        tree.mark_dirty_flags(
                                            tabs_node,
                                            crate::dirty::DirtyFlags::GEOMETRY,
                                        );
                                    }

                                    // Create ghost tab animation if not already active
                                    if dp.dock_animations.ghost_tab.is_none() {
                                        let label = tree
                                            .get_widget(tabs_node)
                                            .and_then(|w| w.as_any().downcast_ref::<DockTabs>())
                                            .and_then(|t| t.tab_labels.get(tab_index))
                                            .cloned()
                                            .unwrap_or_default();
                                        let tab_size = Vec2::new(
                                            label.len() as f32 * 8.0 + 20.0, // approximate tab width
                                            28.0,
                                        );
                                        let mut ghost =
                                            GhostTabAnimation::new(self.mouse_pos, tab_size, label);
                                        ghost.set_target(self.mouse_pos);
                                        dp.dock_animations.ghost_tab = Some(ghost);
                                    } else if let Some(ref mut ghost) = dp.dock_animations.ghost_tab
                                    {
                                        ghost.set_target(self.mouse_pos);
                                    }

                                    // Find all DockTabs containers using cached registry
                                    // Rebuild cache if needed, then collect to avoid borrow conflicts
                                    if dp.docking_context.is_dirty() {
                                        dp.docking_context.rebuild_cache(tree);
                                    }
                                    let all_tabs: Vec<_> = dp
                                        .docking_context
                                        .find_tab_containers(tree)
                                        .iter()
                                        .map(|(&id, info)| (id, info.layout, info.tab_count))
                                        .collect();

                                    let source_tab_count = all_tabs
                                        .iter()
                                        .find(|(id, _, _)| *id == tabs_node)
                                        .map(|(_, _, count)| *count)
                                        .unwrap_or(0);

                                    let mut found_target = false;
                                    for (candidate_id, candidate_layout, _) in &all_tabs {
                                        // Check if cursor is over this container
                                        if let Some(mut zone) = dp
                                            .drop_zone_detector
                                            .detect_zone(self.mouse_pos, *candidate_layout)
                                        {
                                            // Remap to Center if cursor is in the tab bar area
                                            // (dropping on tab bar = merge, not edge split)
                                            if let Some(widget) = tree.get_widget(*candidate_id)
                                                && let Some(tabs) =
                                                    widget.as_any().downcast_ref::<DockTabs>()
                                            {
                                                let tab_bar_bottom =
                                                    candidate_layout.y + tabs.theme.tab_bar_height;
                                                if self.mouse_pos.y < tab_bar_bottom {
                                                    zone =
                                                        crate::widgets::docking::DockZone::Center;
                                                }
                                            }

                                            // For the source container, only allow edge zones
                                            // (center = reorder, handled by within-container path)
                                            // and only when the source has 2+ tabs (can't split a single tab)
                                            if *candidate_id == tabs_node
                                                && (matches!(
                                                    zone,
                                                    crate::widgets::docking::DockZone::Center
                                                ) || source_tab_count < 2)
                                            {
                                                continue;
                                            }
                                            let preview_bounds = dp
                                                .drop_zone_detector
                                                .preview_bounds(zone, *candidate_layout);

                                            // For center zone, compute insertion index
                                            let insert_index = if matches!(
                                                zone,
                                                crate::widgets::docking::DockZone::Center
                                            ) {
                                                if let Some(widget) = tree.get_widget(*candidate_id)
                                                    && let Some(target_tabs) =
                                                        widget.as_any().downcast_ref::<DockTabs>()
                                                {
                                                    let idx = target_tabs
                                                        .hit_test_tab(
                                                            self.mouse_pos,
                                                            candidate_layout,
                                                        )
                                                        .map(|i| i + 1)
                                                        .unwrap_or(target_tabs.tab_count());
                                                    Some(idx)
                                                } else {
                                                    None
                                                }
                                            } else {
                                                None
                                            };

                                            let preview = CrossContainerPreview {
                                                target_node: *candidate_id,
                                                target_layout: *candidate_layout,
                                                zone,
                                                preview_bounds,
                                                insert_index,
                                            };

                                            // Mark old target dirty if it changed
                                            if let Some(old_preview) = &dp.cross_container_preview
                                                && old_preview.target_node != *candidate_id
                                            {
                                                tree.mark_dirty_flags(
                                                    old_preview.target_node,
                                                    crate::dirty::DirtyFlags::GEOMETRY,
                                                );
                                            }

                                            dp.cross_container_preview = Some(preview);
                                            dp.drag_manager.set_drop_target(
                                                DropTarget::new(*candidate_id, zone)
                                                    .with_insert_index(insert_index.unwrap_or(0)),
                                            );
                                            tree.mark_dirty_flags(
                                                *candidate_id,
                                                crate::dirty::DirtyFlags::GEOMETRY,
                                            );

                                            // Update drop preview animation
                                            if let Some(ref mut anim) =
                                                dp.dock_animations.drop_preview
                                            {
                                                anim.set_target(preview_bounds);
                                            } else {
                                                dp.dock_animations.drop_preview =
                                                    Some(DropPreviewAnimation::new(preview_bounds));
                                            }
                                            found_target = true;
                                            break;
                                        }
                                    }

                                    if !found_target {
                                        // Clear preview if cursor isn't over any target
                                        if let Some(old_preview) = dp.cross_container_preview.take()
                                        {
                                            tree.mark_dirty_flags(
                                                old_preview.target_node,
                                                crate::dirty::DirtyFlags::GEOMETRY,
                                            );
                                        }
                                        dp.drag_manager.clear_drop_target();

                                        // Fade out drop preview animation
                                        if let Some(ref mut anim) = dp.dock_animations.drop_preview
                                        {
                                            anim.fade_out();
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    } else {
                        // Check if threshold exceeded to activate drag
                        if let DragType::TabDrag {
                            tabs_node,
                            tab_index,
                        } = drag_state.drag_type
                            && let Some(widget) = tree.get_widget_mut(tabs_node)
                            && let Some(tabs) = widget.as_any_mut().downcast_mut::<DockTabs>()
                        {
                            tabs.start_tab_drag(tab_index);
                            // Use GEOMETRY to ensure ghost rendering starts
                            tree.mark_dirty_flags(tabs_node, crate::dirty::DirtyFlags::GEOMETRY);
                        }
                    }
                }
                return;
            }
        }

        let new_hovered = self.hit_test(tree, self.mouse_pos);

        // Check for splitter separator hover
        #[cfg(feature = "docking")]
        {
            if let Some(dp) = plugins.get_mut::<crate::widgets::docking::plugin::DockingPlugin>() {
                let new_splitter_hover = self.find_hovered_splitter(tree, self.mouse_pos, dp);

                // Update splitter hover state
                if new_splitter_hover != dp.hovered_splitter {
                    // Clear old splitter hover
                    if let Some(old_id) = dp.hovered_splitter
                        && let Some(widget) = tree.get_widget_mut(old_id)
                        && let Some(splitter) = widget.as_any_mut().downcast_mut::<DockSplitter>()
                    {
                        splitter.set_separator_hovered(false);
                        tree.mark_dirty_flags(old_id, crate::dirty::DirtyFlags::COLOR);
                    }

                    // Set new splitter hover
                    if let Some(new_id) = new_splitter_hover
                        && let Some(widget) = tree.get_widget_mut(new_id)
                        && let Some(splitter) = widget.as_any_mut().downcast_mut::<DockSplitter>()
                    {
                        splitter.set_separator_hovered(true);
                        tree.mark_dirty_flags(new_id, crate::dirty::DirtyFlags::COLOR);
                    }

                    dp.hovered_splitter = new_splitter_hover;
                }

                // Update tab hover state
                if let Some(hovered_id) = new_hovered
                    && let Some(widget) = tree.get_widget(hovered_id)
                    && let Some(tabs) = widget.as_any().downcast_ref::<DockTabs>()
                {
                    let layout = tree.get_layout(hovered_id).unwrap();
                    let abs_layout = self.get_absolute_layout(tree, hovered_id, layout);
                    let new_tab_hover = tabs.hit_test_tab(self.mouse_pos, &abs_layout);

                    // Read current hovered tab
                    let current_hover = tabs.hovered_tab;

                    // Detect scrollbar thumb hover
                    let new_scrollbar_hover =
                        tabs.hit_test_scrollbar_thumb(self.mouse_pos, &abs_layout);
                    let current_scrollbar_hover = tabs.scrollbar_thumb_hovered;

                    let tab_hover_changed = new_tab_hover != current_hover;
                    let scrollbar_hover_changed = new_scrollbar_hover != current_scrollbar_hover;

                    if (tab_hover_changed || scrollbar_hover_changed)
                        && let Some(widget) = tree.get_widget_mut(hovered_id)
                        && let Some(tabs) = widget.as_any_mut().downcast_mut::<DockTabs>()
                    {
                        if tab_hover_changed {
                            tabs.set_hovered_tab(new_tab_hover);
                        }
                        if scrollbar_hover_changed {
                            tabs.scrollbar_thumb_hovered = new_scrollbar_hover;
                        }
                        tree.mark_dirty_flags(hovered_id, crate::dirty::DirtyFlags::COLOR);
                    }
                }
            }
        }

        // Update ScrollContainer scrollbar thumb hover state
        if let Some(hovered_id) = new_hovered
            && let Some(widget) = tree.get_widget(hovered_id)
            && let Some(sc) = widget.as_any().downcast_ref::<ScrollContainer>()
        {
            let layout = tree.get_layout(hovered_id).unwrap();
            let abs_layout = self.get_absolute_layout(tree, hovered_id, layout);
            let new_v_hover = sc.hit_test_v_thumb(self.mouse_pos, &abs_layout);
            let new_h_hover = sc.hit_test_h_thumb(self.mouse_pos, &abs_layout);
            let old_v_hover = sc.v_thumb_hovered;
            let old_h_hover = sc.h_thumb_hovered;

            if (new_v_hover != old_v_hover || new_h_hover != old_h_hover)
                && let Some(widget) = tree.get_widget_mut(hovered_id)
                && let Some(sc) = widget.as_any_mut().downcast_mut::<ScrollContainer>()
            {
                sc.v_thumb_hovered = new_v_hover;
                sc.h_thumb_hovered = new_h_hover;
                tree.mark_dirty_flags(hovered_id, crate::dirty::DirtyFlags::COLOR);
            }
        }

        if new_hovered != self.hovered {
            // Clear old hover state
            if let Some(old_id) = self.hovered {
                let on_hover = tree
                    .get_widget(old_id)
                    .map(|w| w.as_any().type_id())
                    .and_then(|tid| plugins.widget_registry().get(tid))
                    .and_then(|desc| desc.on_hover);
                if let Some(on_hover) = on_hover
                    && let Some(widget) = tree.get_widget_mut(old_id)
                {
                    on_hover(widget.as_any_mut(), false);
                    tree.mark_dirty_flags(old_id, crate::dirty::DirtyFlags::COLOR);
                }
                // Clear ScrollContainer thumb hover when leaving
                if let Some(widget) = tree.get_widget_mut(old_id)
                    && let Some(sc) = widget.as_any_mut().downcast_mut::<ScrollContainer>()
                    && (sc.v_thumb_hovered || sc.h_thumb_hovered)
                {
                    sc.v_thumb_hovered = false;
                    sc.h_thumb_hovered = false;
                    tree.mark_dirty_flags(old_id, crate::dirty::DirtyFlags::COLOR);
                }
            }

            // Set new hover state
            if let Some(new_id) = new_hovered {
                let on_hover = tree
                    .get_widget(new_id)
                    .map(|w| w.as_any().type_id())
                    .and_then(|tid| plugins.widget_registry().get(tid))
                    .and_then(|desc| desc.on_hover);
                if let Some(on_hover) = on_hover
                    && let Some(widget) = tree.get_widget_mut(new_id)
                {
                    on_hover(widget.as_any_mut(), true);
                    tree.mark_dirty_flags(new_id, crate::dirty::DirtyFlags::COLOR);
                }
            }

            self.hovered = new_hovered;
        }
    }

    /// Find a splitter node whose separator is under the mouse.
    #[cfg(feature = "docking")]
    fn find_hovered_splitter(
        &self,
        tree: &UiTree,
        point: Vec2,
        dp: &crate::widgets::docking::plugin::DockingPlugin,
    ) -> Option<NodeId> {
        let default_tolerance = dp.docking_context.style().separator_tolerance;
        let root = tree.root()?;
        self.find_splitter_at_point(tree, root, point, Vec2::ZERO, default_tolerance)
    }

    /// Recursively find a splitter with separator at the given point.
    #[cfg(feature = "docking")]
    #[allow(clippy::only_used_in_recursion)]
    fn find_splitter_at_point(
        &self,
        tree: &UiTree,
        node_id: NodeId,
        point: Vec2,
        parent_offset: Vec2,
        default_tolerance: f32,
    ) -> Option<NodeId> {
        let layout = tree.get_layout(node_id)?;
        let abs_x = parent_offset.x + layout.x;
        let abs_y = parent_offset.y + layout.y;
        let mut abs_offset = Vec2::new(abs_x, abs_y);

        let abs_layout = crate::tree::LayoutRect {
            x: abs_x,
            y: abs_y,
            width: layout.width,
            height: layout.height,
        };

        // Check if this is a splitter with separator at point
        if let Some(widget) = tree.get_widget(node_id)
            && let Some(splitter) = widget.as_any().downcast_ref::<DockSplitter>()
        {
            let tolerance = splitter.separator_tolerance.unwrap_or(default_tolerance);
            if splitter.is_point_in_separator(&abs_layout, point, tolerance) {
                return Some(node_id);
            }
        }

        // If this node is a ScrollContainer, subtract scroll offset for children
        if let Some(widget) = tree.get_widget(node_id)
            && let Some(sc) = widget.as_any().downcast_ref::<ScrollContainer>()
        {
            abs_offset -= sc.scroll_offset;
        }

        // Check children
        if let Some(widget) = tree.get_widget(node_id) {
            for &child_id in widget.children() {
                if let Some(found) = self.find_splitter_at_point(
                    tree,
                    child_id,
                    point,
                    abs_offset,
                    default_tolerance,
                ) {
                    return Some(found);
                }
            }
        }

        None
    }

    /// Perform hit testing to find which node is under the mouse.
    fn hit_test(&self, tree: &UiTree, point: Vec2) -> Option<NodeId> {
        profile_function!();
        // Start from root and traverse depth-first
        let root = tree.root()?;
        self.hit_test_node(tree, root, point, Vec2::ZERO)
    }

    /// Recursively hit test a node and its children with position offset.
    #[allow(clippy::only_used_in_recursion)]
    fn hit_test_node(
        &self,
        tree: &UiTree,
        node_id: NodeId,
        point: Vec2,
        parent_offset: Vec2,
    ) -> Option<NodeId> {
        let layout = tree.get_layout(node_id)?;

        // Calculate absolute position
        let abs_x = parent_offset.x + layout.x;
        let abs_y = parent_offset.y + layout.y;

        // Create absolute layout rect
        let abs_layout = crate::tree::LayoutRect {
            x: abs_x,
            y: abs_y,
            width: layout.width,
            height: layout.height,
        };

        // Check if point is within this node
        if !abs_layout.contains(point) {
            return None;
        }

        let mut abs_offset = Vec2::new(abs_x, abs_y);

        // If this node is a ScrollContainer, subtract scroll offset for children hit testing.
        // Also check if the point hits a scrollbar (scrollbars are not scrolled).
        if let Some(widget) = tree.get_widget(node_id)
            && let Some(sc) = widget.as_any().downcast_ref::<ScrollContainer>()
        {
            // Scrollbar hit test first — scrollbars are above content
            if sc.hit_test_v_thumb(point, &abs_layout)
                || sc.hit_test_v_track(point, &abs_layout)
                || sc.hit_test_h_thumb(point, &abs_layout)
                || sc.hit_test_h_track(point, &abs_layout)
            {
                return Some(node_id);
            }
            abs_offset -= sc.scroll_offset;
        }

        // Check children front-to-back, sorted by z-index (highest first)
        if let Some(widget) = tree.get_widget(node_id) {
            let children = widget.children();

            if children.len() <= 1 {
                // Fast path: 0-1 children, no sorting needed
                for &child_id in children.iter().rev() {
                    if let Some(hit) = self.hit_test_node(tree, child_id, point, abs_offset) {
                        return Some(hit);
                    }
                }
            } else {
                // Collect (child_id, render_layer, computed_z_index) and sort by
                // render_layer descending then z_index descending.
                // Overlay nodes are tested before base nodes; within each layer,
                // higher z-index nodes are tested first.
                // Stable sort preserves tree order for equal values (last child = frontmost).
                let mut sorted: Vec<(NodeId, crate::draw_list::RenderLayer, u16)> = children
                    .iter()
                    .filter_map(|&cid| {
                        let node = tree.get_node(cid)?;
                        let rl = tree
                            .get_widget(cid)
                            .map(|w| w.style().render_layer)
                            .unwrap_or_default();
                        Some((cid, rl, node.computed_z_index))
                    })
                    .collect();

                sorted.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.cmp(&a.2)));

                for (child_id, _, _) in sorted {
                    if let Some(hit) = self.hit_test_node(tree, child_id, point, abs_offset) {
                        return Some(hit);
                    }
                }
            }
        }

        // If no children hit, this node is the hit target
        Some(node_id)
    }

    /// Dispatch a click event to a node.
    fn dispatch_click(
        &mut self,
        node_id: NodeId,
        tree: &mut UiTree,
        plugins: &mut crate::plugin::PluginManager,
    ) {
        let on_click = tree
            .get_widget(node_id)
            .map(|w| w.as_any().type_id())
            .and_then(|tid| plugins.widget_registry().get(tid))
            .and_then(|desc| desc.on_click);
        if let Some(on_click) = on_click
            && let Some(widget) = tree.get_widget_mut(node_id)
        {
            let response = on_click(widget.as_any_mut());
            match response {
                crate::plugin::registry::EventResponse::RequestFocus => {
                    self.focused = Some(node_id);
                }
                crate::plugin::registry::EventResponse::ReleaseFocus => {
                    self.focused = None;
                }
                crate::plugin::registry::EventResponse::None => {}
            }
        }
    }

    /// Handle keyboard input for focused widgets.
    fn handle_key_input(
        &mut self,
        key: &PhysicalKey,
        tree: &mut UiTree,
        plugins: &mut crate::plugin::PluginManager,
    ) {
        let Some(focused_id) = self.focused else {
            return;
        };
        let on_key_input = tree
            .get_widget(focused_id)
            .map(|w| w.as_any().type_id())
            .and_then(|tid| plugins.widget_registry().get(tid))
            .and_then(|desc| desc.on_key_input);
        if let Some(on_key_input) = on_key_input
            && let Some(widget) = tree.get_widget_mut(focused_id)
        {
            let response = on_key_input(widget.as_any_mut(), key);
            match response {
                crate::plugin::registry::EventResponse::RequestFocus => {
                    self.focused = Some(focused_id);
                }
                crate::plugin::registry::EventResponse::ReleaseFocus => {
                    self.focused = None;
                }
                crate::plugin::registry::EventResponse::None => {}
            }
        }
    }

    /// Handle mouse scroll events.
    fn handle_scroll_event(
        &mut self,
        delta: &astrelis_winit::event::MouseScrollDelta,
        tree: &mut UiTree,
        plugins: &mut crate::plugin::PluginManager,
    ) {
        let _ = plugins; // used only with docking feature
        let (dx, dy) = match delta {
            astrelis_winit::event::MouseScrollDelta::LineDelta(x, y) => (*x * 30.0, *y * 30.0),
            astrelis_winit::event::MouseScrollDelta::PixelDelta(pos) => {
                (pos.x as f32, pos.y as f32)
            }
        };

        // Try docking scroll first
        #[cfg(feature = "docking")]
        self.handle_dock_scroll(dx - dy, tree);

        // Try ScrollContainer scroll: walk from hovered node up to find a ScrollContainer
        self.handle_scroll_container_scroll(dx, dy, tree);
    }

    /// Handle pan gesture events.
    fn handle_pan_gesture(
        &mut self,
        gesture: &astrelis_winit::event::PanGesture,
        tree: &mut UiTree,
        plugins: &mut crate::plugin::PluginManager,
    ) {
        let _ = plugins; // used only with docking feature
        let dx = gesture.delta.x as f32;
        let dy = gesture.delta.y as f32;

        #[cfg(feature = "docking")]
        self.handle_dock_scroll(-dx - dy, tree);

        self.handle_scroll_container_scroll(dx, dy, tree);
    }

    /// Walk from the hovered node up through ancestors to find the nearest
    /// ScrollContainer that can accept the given scroll delta.
    fn handle_scroll_container_scroll(&mut self, dx: f32, dy: f32, tree: &mut UiTree) {
        let Some(hovered_id) = self.hovered else {
            return;
        };

        // Walk from hovered up to find the nearest ScrollContainer
        let mut candidate = Some(hovered_id);
        while let Some(node_id) = candidate {
            if let Some(widget) = tree.get_widget(node_id)
                && widget.as_any().downcast_ref::<ScrollContainer>().is_some()
            {
                // Found a ScrollContainer — apply delta
                if let Some(widget) = tree.get_widget_mut(node_id)
                    && let Some(sc) = widget.as_any_mut().downcast_mut::<ScrollContainer>()
                {
                    let old_offset = sc.scroll_offset;
                    // Vertical scroll: negative dy = scroll down (content moves up)
                    sc.scroll_by(Vec2::new(-dx, -dy));
                    if sc.scroll_offset != old_offset {
                        tree.mark_dirty_flags(node_id, crate::dirty::DirtyFlags::SCROLL);
                    }
                }
                return;
            }
            // Move to parent
            candidate = tree.get_node(node_id).and_then(|n| n.parent);
        }
    }

    /// Handle scroll for DockTabs tab bar scrolling.
    #[cfg(feature = "docking")]
    fn handle_dock_scroll(&mut self, dx: f32, tree: &mut UiTree) {
        // Check if the hovered widget is a DockTabs with scrollable tab bar
        if let Some(hovered_id) = self.hovered
            && let Some(widget) = tree.get_widget(hovered_id)
            && let Some(tabs) = widget.as_any().downcast_ref::<DockTabs>()
            && tabs.tab_bar_scrollable
        {
            let layout = tree.get_layout(hovered_id).unwrap();
            let abs_layout = self.get_absolute_layout(tree, hovered_id, layout);
            let bar = tabs.tab_bar_bounds(&abs_layout);
            let available_width = abs_layout.width;

            // Only scroll if cursor is in the tab bar area
            if self.mouse_pos.y >= bar.y
                && self.mouse_pos.y <= bar.y + bar.height
                && let Some(widget) = tree.get_widget_mut(hovered_id)
                && let Some(tabs) = widget.as_any_mut().downcast_mut::<DockTabs>()
            {
                tabs.scroll_tab_bar_by(-dx, available_width);
                tree.mark_dirty_flags(hovered_id, crate::dirty::DirtyFlags::GEOMETRY);
            }
        }
    }

    /// Handle character input for focused widgets.
    fn handle_char_input(
        &mut self,
        c: char,
        tree: &mut UiTree,
        plugins: &mut crate::plugin::PluginManager,
    ) {
        let Some(focused_id) = self.focused else {
            return;
        };
        let on_char_input = tree
            .get_widget(focused_id)
            .map(|w| w.as_any().type_id())
            .and_then(|tid| plugins.widget_registry().get(tid))
            .and_then(|desc| desc.on_char_input);
        if let Some(on_char_input) = on_char_input
            && let Some(widget) = tree.get_widget_mut(focused_id)
        {
            on_char_input(widget.as_any_mut(), c);
        }
    }
}

impl Default for UiEventSystem {
    fn default() -> Self {
        Self::new()
    }
}
