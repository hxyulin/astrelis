//! Event handling system for UI interactions.

use crate::tree::{NodeId, UiTree};
use crate::widgets::{Button, TextInput};
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
    /// Node with active tooltip.
    tooltip_node: Option<NodeId>,
    /// Current mouse position.
    mouse_pos: Vec2,
    /// Pressed mouse buttons.
    mouse_buttons: HashSet<MouseButton>,
    /// Nodes that were pressed this frame.
    pressed_nodes: HashSet<NodeId>,
}

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

    /// Handle events from the event batch.
    pub fn handle_events(&mut self, events: &mut EventBatch, tree: &mut UiTree) {
        profile_function!();
        events.dispatch(|event| match event {
            Event::MouseMoved(pos) => {
                self.mouse_pos = Vec2::new(pos.x as f32, pos.y as f32);
                self.update_hover(tree);
                HandleStatus::consumed()
            }
            Event::MouseButtonDown(button) => {
                self.handle_mouse_input(*button, true, tree);
                HandleStatus::consumed()
            }
            Event::MouseButtonUp(button) => {
                self.handle_mouse_input(*button, false, tree);
                HandleStatus::consumed()
            }
            Event::KeyInput(key_event) => {
                if key_event.state == ElementState::Pressed {
                    // Handle text input from key event
                    if let Some(ref text) = key_event.text {
                        for c in text.chars() {
                            self.handle_char_input(c, tree);
                        }
                    }
                    // Handle special keys
                    self.handle_key_input(&key_event.physical_key, tree);
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
    ) {
        let mouse_button = match button {
            astrelis_winit::event::MouseButton::Left => MouseButton::Left,
            astrelis_winit::event::MouseButton::Right => MouseButton::Right,
            astrelis_winit::event::MouseButton::Middle => MouseButton::Middle,
            _ => return,
        };

        if pressed {
            self.mouse_buttons.insert(mouse_button);

            // Handle press on hovered widget
            if let Some(hovered_id) = self.hovered {
                self.pressed_nodes.insert(hovered_id);

                // Update button state
                if let Some(widget) = tree.get_widget_mut(hovered_id) {
                    if let Some(button) = widget.as_any_mut().downcast_mut::<Button>() {
                        button.is_pressed = true;
                        // Mark dirty for retained renderer
                        tree.mark_dirty_flags(hovered_id, crate::dirty::DirtyFlags::COLOR_ONLY);
                    }
                }
            }
        } else {
            self.mouse_buttons.remove(&mouse_button);

            // Handle release - check if it's a click
            if let Some(hovered_id) = self.hovered {
                if self.pressed_nodes.contains(&hovered_id) {
                    // This is a click!
                    self.dispatch_click(hovered_id, tree);
                }
            }

            // Clear pressed state on ALL previously pressed nodes
            // (handles release outside button, drag-away scenarios)
            for &pressed_node_id in &self.pressed_nodes {
                if let Some(widget) = tree.get_widget_mut(pressed_node_id) {
                    if let Some(button) = widget.as_any_mut().downcast_mut::<Button>() {
                        button.is_pressed = false;
                        // Mark dirty for retained renderer
                        tree.mark_dirty_flags(
                            pressed_node_id,
                            crate::dirty::DirtyFlags::COLOR_ONLY,
                        );
                    }
                }
            }

            self.pressed_nodes.clear();
        }
    }

    /// Update hover state based on current mouse position.
    fn update_hover(&mut self, tree: &mut UiTree) {
        let new_hovered = self.hit_test(tree, self.mouse_pos);

        if new_hovered != self.hovered {
            // Clear old hover state
            if let Some(old_id) = self.hovered {
                if let Some(widget) = tree.get_widget_mut(old_id) {
                    if let Some(button) = widget.as_any_mut().downcast_mut::<Button>() {
                        button.is_hovered = false;
                        // Mark dirty for retained renderer
                        tree.mark_dirty_flags(old_id, crate::dirty::DirtyFlags::COLOR_ONLY);
                    }
                }
            }

            // Set new hover state
            if let Some(new_id) = new_hovered {
                if let Some(widget) = tree.get_widget_mut(new_id) {
                    if let Some(button) = widget.as_any_mut().downcast_mut::<Button>() {
                        button.is_hovered = true;
                        // Mark dirty for retained renderer
                        tree.mark_dirty_flags(new_id, crate::dirty::DirtyFlags::COLOR_ONLY);
                    }
                }
            }

            self.hovered = new_hovered;
        }
    }

    /// Perform hit testing to find which node is under the mouse.
    fn hit_test(&self, tree: &UiTree, point: Vec2) -> Option<NodeId> {
        profile_function!();
        // Start from root and traverse depth-first
        let root = tree.root()?;
        self.hit_test_node(tree, root, point, Vec2::ZERO)
    }

    /// Recursively hit test a node and its children with position offset.
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

        let abs_offset = Vec2::new(abs_x, abs_y);

        // Check children first (front to back)
        if let Some(widget) = tree.get_widget(node_id) {
            let children = widget.children();
            // Reverse iteration to check front-most children first
            for &child_id in children.iter().rev() {
                if let Some(hit) = self.hit_test_node(tree, child_id, point, abs_offset) {
                    return Some(hit);
                }
            }
        }

        // If no children hit, this node is the hit target
        Some(node_id)
    }

    /// Dispatch a click event to a node.
    fn dispatch_click(&mut self, node_id: NodeId, tree: &mut UiTree) {
        if let Some(widget) = tree.get_widget_mut(node_id) {
            // Handle button clicks
            if let Some(button) = widget.as_any_mut().downcast_mut::<Button>() {
                if let Some(callback) = button.on_click.clone() {
                    callback();
                    tracing::debug!("Button clicked: {}", button.label);
                }
            }
            // Handle text input focus
            else if let Some(text_input) = widget.as_any_mut().downcast_mut::<TextInput>() {
                text_input.is_focused = true;
                self.focused = Some(node_id);
                tracing::debug!("Text input focused");
            }
        }
    }

    /// Handle keyboard input for focused widgets.
    fn handle_key_input(&mut self, key: &PhysicalKey, tree: &mut UiTree) {
        if let Some(focused_id) = self.focused {
            if let Some(widget) = tree.get_widget_mut(focused_id) {
                if let Some(text_input) = widget.as_any_mut().downcast_mut::<TextInput>() {
                    if let PhysicalKey::Code(code) = key {
                        use astrelis_winit::event::KeyCode;
                        match code {
                            KeyCode::Backspace => {
                                text_input.delete_char();
                            }
                            KeyCode::Escape => {
                                text_input.is_focused = false;
                                self.focused = None;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    /// Handle character input for focused widgets.
    fn handle_char_input(&mut self, c: char, tree: &mut UiTree) {
        if let Some(focused_id) = self.focused {
            if let Some(widget) = tree.get_widget_mut(focused_id) {
                if let Some(text_input) = widget.as_any_mut().downcast_mut::<TextInput>() {
                    if !c.is_control() {
                        text_input.insert_char(c);
                    }
                }
            }
        }
    }
}

impl Default for UiEventSystem {
    fn default() -> Self {
        Self::new()
    }
}
