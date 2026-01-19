//! Focus management and keyboard navigation for UI widgets.
//!
//! Provides a system for managing widget focus and enabling keyboard navigation
//! through the UI tree. This is essential for accessibility and power user workflows.
//!
//! # Features
//!
//! - Focus ring with Tab/Shift+Tab navigation
//! - Directional navigation (arrow keys)
//! - Focus trapping for modals
//! - Programmatic focus control
//! - Focus events
//!
//! # Example
//!
//! ```ignore
//! use astrelis_ui::*;
//!
//! let mut focus_manager = FocusManager::new();
//!
//! // Register focusable widgets in order
//! focus_manager.register(widget_id_1);
//! focus_manager.register(widget_id_2);
//! focus_manager.register(widget_id_3);
//!
//! // Navigate with keyboard
//! focus_manager.focus_next(); // Tab
//! focus_manager.focus_previous(); // Shift+Tab
//!
//! // Check current focus
//! if let Some(focused_id) = focus_manager.focused() {
//!     println!("Widget {:?} has focus", focused_id);
//! }
//! ```

use crate::widget_id::WidgetId;

/// Focus navigation direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDirection {
    /// Move focus forward (Tab)
    Next,
    /// Move focus backward (Shift+Tab)
    Previous,
    /// Move focus up (Arrow Up)
    Up,
    /// Move focus down (Arrow Down)
    Down,
    /// Move focus left (Arrow Left)
    Left,
    /// Move focus right (Arrow Right)
    Right,
}

/// Focus event indicating a change in focus state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusEvent {
    /// Widget gained focus
    Gained(WidgetId),
    /// Widget lost focus
    Lost(WidgetId),
    /// Focus moved from one widget to another
    Changed {
        from: Option<WidgetId>,
        to: Option<WidgetId>,
    },
}

/// Focus policy determining how a widget can receive focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPolicy {
    /// Widget can receive focus via Tab/Shift+Tab and mouse clicks
    Focusable,
    /// Widget can only receive focus via mouse clicks, not keyboard
    ClickFocusable,
    /// Widget cannot receive focus
    NotFocusable,
}

impl Default for FocusPolicy {
    fn default() -> Self {
        Self::NotFocusable
    }
}

/// Focus scope for focus trapping (e.g., in modals).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FocusScopeId(pub u64);

impl FocusScopeId {
    pub const ROOT: Self = Self(0);

    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// A focusable widget entry in the focus order.
#[derive(Debug, Clone)]
struct FocusEntry {
    widget_id: WidgetId,
    policy: FocusPolicy,
    scope: FocusScopeId,
    tab_index: i32, // Custom tab order (-1 = not in tab order, 0+ = explicit order)
}

/// Manages keyboard focus and navigation for UI widgets.
pub struct FocusManager {
    /// All registered focusable widgets
    entries: Vec<FocusEntry>,
    /// Currently focused widget
    focused: Option<usize>, // Index into entries
    /// Active focus scope (for modal trapping)
    active_scope: FocusScopeId,
    /// Focus event queue
    events: Vec<FocusEvent>,
}

impl FocusManager {
    /// Create a new focus manager.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            focused: None,
            active_scope: FocusScopeId::ROOT,
            events: Vec::new(),
        }
    }

    /// Register a widget as focusable.
    pub fn register(&mut self, widget_id: WidgetId) {
        self.register_with_policy(widget_id, FocusPolicy::Focusable);
    }

    /// Register a widget with a specific focus policy.
    pub fn register_with_policy(&mut self, widget_id: WidgetId, policy: FocusPolicy) {
        self.register_with_details(widget_id, policy, FocusScopeId::ROOT, 0);
    }

    /// Register a widget with full focus details.
    pub fn register_with_details(
        &mut self,
        widget_id: WidgetId,
        policy: FocusPolicy,
        scope: FocusScopeId,
        tab_index: i32,
    ) {
        // Check if already registered
        if let Some(entry) = self.entries.iter_mut().find(|e| e.widget_id == widget_id) {
            entry.policy = policy;
            entry.scope = scope;
            entry.tab_index = tab_index;
        } else {
            self.entries.push(FocusEntry {
                widget_id,
                policy,
                scope,
                tab_index,
            });
        }
    }

    /// Unregister a widget.
    pub fn unregister(&mut self, widget_id: WidgetId) {
        if let Some(index) = self.entries.iter().position(|e| e.widget_id == widget_id) {
            self.entries.remove(index);

            // Clear focus if this widget was focused
            if self.focused == Some(index) {
                self.focused = None;
                self.events.push(FocusEvent::Lost(widget_id));
            } else if let Some(focused_idx) = self.focused {
                // Adjust focus index if needed
                if focused_idx > index {
                    self.focused = Some(focused_idx - 1);
                }
            }
        }
    }

    /// Clear all registered widgets.
    pub fn clear(&mut self) {
        if let Some(old_focus) = self.focused.and_then(|idx| self.entries.get(idx)) {
            self.events.push(FocusEvent::Lost(old_focus.widget_id));
        }
        self.entries.clear();
        self.focused = None;
    }

    /// Get the currently focused widget.
    pub fn focused(&self) -> Option<WidgetId> {
        self.focused
            .and_then(|idx| self.entries.get(idx))
            .map(|e| e.widget_id)
    }

    /// Set focus to a specific widget.
    pub fn set_focus(&mut self, widget_id: WidgetId) -> bool {
        if let Some(index) = self.entries.iter().position(|e| e.widget_id == widget_id) {
            let old_focus = self.focused.and_then(|idx| self.entries.get(idx).map(|e| e.widget_id));

            if old_focus == Some(widget_id) {
                return true; // Already focused
            }

            self.focused = Some(index);

            // Emit events
            if let Some(old_id) = old_focus {
                self.events.push(FocusEvent::Lost(old_id));
            }
            self.events.push(FocusEvent::Gained(widget_id));
            self.events.push(FocusEvent::Changed {
                from: old_focus,
                to: Some(widget_id),
            });

            true
        } else {
            false
        }
    }

    /// Clear focus (no widget focused).
    pub fn clear_focus(&mut self) {
        if let Some(old_focus) = self.focused.and_then(|idx| self.entries.get(idx)) {
            let old_id = old_focus.widget_id;
            self.focused = None;

            self.events.push(FocusEvent::Lost(old_id));
            self.events.push(FocusEvent::Changed {
                from: Some(old_id),
                to: None,
            });
        }
    }

    /// Move focus to the next focusable widget (Tab).
    pub fn focus_next(&mut self) {
        self.navigate(FocusDirection::Next);
    }

    /// Move focus to the previous focusable widget (Shift+Tab).
    pub fn focus_previous(&mut self) {
        self.navigate(FocusDirection::Previous);
    }

    /// Navigate focus in a specific direction.
    pub fn navigate(&mut self, direction: FocusDirection) {
        if self.entries.is_empty() {
            return;
        }

        let current_idx = self.focused.unwrap_or(0);

        // Find next focusable widget based on direction
        let next_idx = match direction {
            FocusDirection::Next => self.find_next_focusable(current_idx),
            FocusDirection::Previous => self.find_previous_focusable(current_idx),
            // Directional navigation would need spatial information (widget positions)
            // For now, treat them like Next/Previous
            FocusDirection::Down | FocusDirection::Right => self.find_next_focusable(current_idx),
            FocusDirection::Up | FocusDirection::Left => self.find_previous_focusable(current_idx),
        };

        if let Some(next_idx) = next_idx {
            let next_widget = self.entries[next_idx].widget_id;
            self.set_focus(next_widget);
        }
    }

    /// Find the next focusable widget after the given index.
    fn find_next_focusable(&self, current_idx: usize) -> Option<usize> {
        let count = self.entries.len();
        for i in 1..=count {
            let idx = (current_idx + i) % count;
            if self.can_focus(idx) {
                return Some(idx);
            }
        }
        None
    }

    /// Find the previous focusable widget before the given index.
    fn find_previous_focusable(&self, current_idx: usize) -> Option<usize> {
        let count = self.entries.len();
        for i in 1..=count {
            let idx = (current_idx + count - i) % count;
            if self.can_focus(idx) {
                return Some(idx);
            }
        }
        None
    }

    /// Check if a widget at the given index can be focused.
    fn can_focus(&self, index: usize) -> bool {
        if let Some(entry) = self.entries.get(index) {
            // Must be in active scope
            if entry.scope != self.active_scope {
                return false;
            }

            // Check policy
            match entry.policy {
                FocusPolicy::Focusable => entry.tab_index >= 0,
                FocusPolicy::ClickFocusable => false, // Can't focus via keyboard
                FocusPolicy::NotFocusable => false,
            }
        } else {
            false
        }
    }

    /// Set the active focus scope (for modal trapping).
    pub fn set_scope(&mut self, scope: FocusScopeId) {
        self.active_scope = scope;

        // Clear focus if currently focused widget is not in this scope
        if let Some(idx) = self.focused {
            if let Some(entry) = self.entries.get(idx) {
                if entry.scope != scope {
                    self.clear_focus();
                }
            }
        }
    }

    /// Get the active focus scope.
    pub fn scope(&self) -> FocusScopeId {
        self.active_scope
    }

    /// Pop all pending focus events.
    pub fn pop_events(&mut self) -> Vec<FocusEvent> {
        std::mem::take(&mut self.events)
    }

    /// Check if a widget is currently focused.
    pub fn is_focused(&self, widget_id: WidgetId) -> bool {
        self.focused() == Some(widget_id)
    }

    /// Get the number of registered focusable widgets.
    pub fn widget_count(&self) -> usize {
        self.entries.len()
    }

    /// Get the number of focusable widgets in the active scope.
    pub fn focusable_count(&self) -> usize {
        self.entries
            .iter()
            .enumerate()
            .filter(|(idx, _)| self.can_focus(*idx))
            .count()
    }
}

impl Default for FocusManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_focus_manager_new() {
        let manager = FocusManager::new();
        assert_eq!(manager.widget_count(), 0);
        assert_eq!(manager.focused(), None);
    }

    #[test]
    fn test_register_and_focus() {
        let mut manager = FocusManager::new();
        let id = WidgetId::from_raw(1);

        manager.register(id);
        assert_eq!(manager.widget_count(), 1);

        let result = manager.set_focus(id);
        assert!(result);
        assert_eq!(manager.focused(), Some(id));
    }

    #[test]
    fn test_focus_next() {
        let mut manager = FocusManager::new();
        let id1 = WidgetId::from_raw(1);
        let id2 = WidgetId::from_raw(2);
        let id3 = WidgetId::from_raw(3);

        manager.register(id1);
        manager.register(id2);
        manager.register(id3);

        manager.set_focus(id1);
        assert_eq!(manager.focused(), Some(id1));

        manager.focus_next();
        assert_eq!(manager.focused(), Some(id2));

        manager.focus_next();
        assert_eq!(manager.focused(), Some(id3));

        // Wrap around
        manager.focus_next();
        assert_eq!(manager.focused(), Some(id1));
    }

    #[test]
    fn test_focus_previous() {
        let mut manager = FocusManager::new();
        let id1 = WidgetId::from_raw(1);
        let id2 = WidgetId::from_raw(2);
        let id3 = WidgetId::from_raw(3);

        manager.register(id1);
        manager.register(id2);
        manager.register(id3);

        manager.set_focus(id3);
        assert_eq!(manager.focused(), Some(id3));

        manager.focus_previous();
        assert_eq!(manager.focused(), Some(id2));

        manager.focus_previous();
        assert_eq!(manager.focused(), Some(id1));

        // Wrap around
        manager.focus_previous();
        assert_eq!(manager.focused(), Some(id3));
    }

    #[test]
    fn test_clear_focus() {
        let mut manager = FocusManager::new();
        let id = WidgetId::from_raw(1);

        manager.register(id);
        manager.set_focus(id);
        assert_eq!(manager.focused(), Some(id));

        manager.clear_focus();
        assert_eq!(manager.focused(), None);
    }

    #[test]
    fn test_unregister() {
        let mut manager = FocusManager::new();
        let id1 = WidgetId::from_raw(1);
        let id2 = WidgetId::from_raw(2);

        manager.register(id1);
        manager.register(id2);
        manager.set_focus(id1);

        manager.unregister(id1);
        assert_eq!(manager.widget_count(), 1);
        assert_eq!(manager.focused(), None);
    }

    #[test]
    fn test_focus_policy() {
        let mut manager = FocusManager::new();
        let id1 = WidgetId::from_raw(1);
        let id2 = WidgetId::from_raw(2);
        let id3 = WidgetId::from_raw(3);

        manager.register_with_policy(id1, FocusPolicy::Focusable);
        manager.register_with_policy(id2, FocusPolicy::NotFocusable);
        manager.register_with_policy(id3, FocusPolicy::Focusable);

        manager.set_focus(id1);
        manager.focus_next();

        // Should skip id2 (not focusable) and go to id3
        assert_eq!(manager.focused(), Some(id3));
    }

    #[test]
    fn test_focus_scope() {
        let mut manager = FocusManager::new();
        let id1 = WidgetId::from_raw(1);
        let id2 = WidgetId::from_raw(2);
        let modal_scope = FocusScopeId::new(1);

        manager.register_with_details(id1, FocusPolicy::Focusable, FocusScopeId::ROOT, 0);
        manager.register_with_details(id2, FocusPolicy::Focusable, modal_scope, 0);

        manager.set_focus(id1);
        assert_eq!(manager.focused(), Some(id1));

        // Enter modal scope
        manager.set_scope(modal_scope);
        assert_eq!(manager.focused(), None); // id1 is not in modal scope

        manager.set_focus(id2);
        assert_eq!(manager.focused(), Some(id2));
    }

    #[test]
    fn test_focus_events() {
        let mut manager = FocusManager::new();
        let id1 = WidgetId::from_raw(1);
        let id2 = WidgetId::from_raw(2);

        manager.register(id1);
        manager.register(id2);

        manager.set_focus(id1);
        let events = manager.pop_events();
        assert_eq!(events.len(), 2); // Gained + Changed

        manager.set_focus(id2);
        let events = manager.pop_events();
        assert_eq!(events.len(), 3); // Lost + Gained + Changed

        // No more events
        let events = manager.pop_events();
        assert_eq!(events.len(), 0);
    }
}
