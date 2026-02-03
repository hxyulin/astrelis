//! Drag system for docking operations.

use astrelis_core::math::Vec2;

use crate::tree::NodeId;

use super::types::{DragState, DragType, SplitDirection, DRAG_THRESHOLD};

/// Manages drag operations for the docking system.
#[derive(Debug, Default)]
pub struct DragManager {
    /// Current active drag state.
    drag_state: Option<DragState>,
}

impl DragManager {
    /// Create a new drag manager.
    pub fn new() -> Self {
        Self { drag_state: None }
    }

    /// Start a new splitter resize drag.
    pub fn start_splitter_drag(
        &mut self,
        splitter_node: NodeId,
        direction: SplitDirection,
        start_pos: Vec2,
        original_ratio: f32,
    ) {
        self.drag_state = Some(DragState::new(
            DragType::SplitterResize {
                splitter_node,
                direction,
            },
            start_pos,
            original_ratio,
        ));
    }

    /// Start a new panel move drag.
    pub fn start_panel_drag(&mut self, panel_node: NodeId, start_pos: Vec2) {
        self.drag_state = Some(DragState::new(
            DragType::PanelMove { panel_node },
            start_pos,
            0.0, // No original value for panel moves
        ));
    }

    /// Start a new tab drag.
    pub fn start_tab_drag(&mut self, tabs_node: NodeId, tab_index: usize, start_pos: Vec2) {
        self.drag_state = Some(DragState::new(
            DragType::TabDrag {
                tabs_node,
                tab_index,
            },
            start_pos,
            tab_index as f32,
        ));
    }

    /// Update the current drag position.
    ///
    /// Returns true if there's an active drag operation.
    pub fn update(&mut self, pos: Vec2) -> bool {
        if let Some(ref mut state) = self.drag_state {
            state.update(pos);
            true
        } else {
            false
        }
    }

    /// Check if there's an active drag operation.
    pub fn is_dragging(&self) -> bool {
        self.drag_state
            .as_ref()
            .is_some_and(|s| s.is_active)
    }

    /// Check if there's a pending drag (mouse down but threshold not exceeded).
    pub fn has_pending_drag(&self) -> bool {
        self.drag_state
            .as_ref()
            .is_some_and(|s| !s.is_active)
    }

    /// Get the current drag state.
    pub fn drag_state(&self) -> Option<&DragState> {
        self.drag_state.as_ref()
    }

    /// Get the current drag state mutably.
    pub fn drag_state_mut(&mut self) -> Option<&mut DragState> {
        self.drag_state.as_mut()
    }

    /// Cancel the current drag operation.
    pub fn cancel_drag(&mut self) {
        self.drag_state = None;
    }

    /// End the current drag operation and return the final state.
    pub fn end_drag(&mut self) -> Option<DragState> {
        self.drag_state.take()
    }

    /// Check if the current drag is a splitter resize for a specific node.
    pub fn is_splitter_drag(&self, node: NodeId) -> bool {
        self.drag_state.as_ref().is_some_and(|s| {
            matches!(s.drag_type, DragType::SplitterResize { splitter_node, .. } if splitter_node == node)
        })
    }

    /// Check if the current drag is a tab drag for a specific node.
    pub fn is_tab_drag(&self, node: NodeId) -> bool {
        self.drag_state.as_ref().is_some_and(|s| {
            matches!(s.drag_type, DragType::TabDrag { tabs_node, .. } if tabs_node == node)
        })
    }

    /// Get the drag delta if there's an active drag.
    pub fn drag_delta(&self) -> Option<Vec2> {
        self.drag_state
            .as_ref()
            .filter(|s| s.is_active)
            .map(|s| s.delta())
    }

    /// Get the splitter node being dragged, if any.
    pub fn dragged_splitter(&self) -> Option<(NodeId, SplitDirection, f32)> {
        self.drag_state.as_ref().and_then(|s| {
            if let DragType::SplitterResize {
                splitter_node,
                direction,
            } = s.drag_type
            {
                Some((splitter_node, direction, s.original_value))
            } else {
                None
            }
        })
    }

    /// Check if a position exceeds the drag threshold from the start position.
    pub fn exceeds_threshold(start: Vec2, current: Vec2) -> bool {
        (current - start).length() >= DRAG_THRESHOLD
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_splitter_drag() {
        let mut manager = DragManager::new();
        let node = NodeId(1);

        manager.start_splitter_drag(node, SplitDirection::Horizontal, Vec2::new(100.0, 100.0), 0.5);

        assert!(manager.has_pending_drag());
        assert!(!manager.is_dragging());

        // Move past threshold
        manager.update(Vec2::new(110.0, 100.0));
        assert!(manager.is_dragging());
        assert!(manager.is_splitter_drag(node));

        // Check delta
        let delta = manager.drag_delta().unwrap();
        assert!((delta.x - 10.0).abs() < 0.001);
        assert!((delta.y - 0.0).abs() < 0.001);

        // End drag
        let final_state = manager.end_drag().unwrap();
        assert!(final_state.is_active);
        assert!(!manager.is_dragging());
    }

    #[test]
    fn test_tab_drag() {
        let mut manager = DragManager::new();
        let node = NodeId(2);

        manager.start_tab_drag(node, 1, Vec2::new(50.0, 20.0));

        assert!(manager.is_tab_drag(node));
        assert!(!manager.is_splitter_drag(NodeId(1)));
    }

    #[test]
    fn test_cancel_drag() {
        let mut manager = DragManager::new();
        manager.start_panel_drag(NodeId(1), Vec2::ZERO);

        assert!(manager.has_pending_drag());

        manager.cancel_drag();

        assert!(!manager.has_pending_drag());
        assert!(!manager.is_dragging());
        assert!(manager.drag_state().is_none());
    }
}
