//! Widget ID system for tracking UI nodes across frames.
//!
//! This module provides a type-safe way to reference widgets in the UI tree,
//! enabling incremental updates without full tree rebuilds.

use crate::tree::NodeId;
use astrelis_core::alloc::HashMap;
use std::fmt;

/// A stable identifier for a widget that persists across frame rebuilds.
///
/// Widget IDs allow you to update specific widgets without rebuilding the entire UI tree.
///
/// # Example
/// ```no_run
/// # use astrelis_ui::{UiSystem, WidgetId};
/// # use astrelis_render::GraphicsContext;
/// # let context = GraphicsContext::new_owned_sync();
/// # let mut ui = UiSystem::new(context);
/// let counter_text_id = WidgetId::new("counter_text");
///
/// // Build UI
/// ui.build(|root| {
///     root.text("Count: 0").build();
/// });
///
/// // Later, update just that widget
/// ui.update_text(counter_text_id, "Count: 1");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WidgetId(u64);

impl WidgetId {
    /// Create a new widget ID from a string key.
    ///
    /// Uses FNV-1a hash for fast, consistent hashing.
    pub fn new(key: &str) -> Self {
        Self(Self::hash_str(key))
    }

    /// Create a widget ID from raw u64 (for generated IDs).
    pub const fn from_raw(id: u64) -> Self {
        Self(id)
    }

    /// Get the raw u64 value.
    pub const fn as_u64(&self) -> u64 {
        self.0
    }

    /// FNV-1a hash implementation for consistent string hashing.
    fn hash_str(s: &str) -> u64 {
        const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x100000001b3;

        let mut hash = FNV_OFFSET_BASIS;
        for byte in s.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash
    }
}

impl fmt::Display for WidgetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WidgetId(0x{:016x})", self.0)
    }
}

impl From<&str> for WidgetId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for WidgetId {
    fn from(s: String) -> Self {
        Self::new(&s)
    }
}

/// Registry mapping widget IDs to node IDs in the tree.
///
/// This is internal to the UI system and manages the bidirectional mapping
/// between stable WidgetIds and frame-specific NodeIds.
pub struct WidgetIdRegistry {
    id_to_node: HashMap<WidgetId, NodeId>,
    node_to_id: HashMap<NodeId, WidgetId>,
}

impl WidgetIdRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            id_to_node: HashMap::new(),
            node_to_id: HashMap::new(),
        }
    }

    /// Register a widget ID to node ID mapping.
    pub fn register(&mut self, widget_id: WidgetId, node_id: NodeId) {
        self.id_to_node.insert(widget_id, node_id);
        self.node_to_id.insert(node_id, widget_id);
    }

    /// Get the node ID for a widget ID.
    pub fn get_node(&self, widget_id: WidgetId) -> Option<NodeId> {
        self.id_to_node.get(&widget_id).copied()
    }

    /// Get the widget ID for a node ID.
    pub fn get_widget_id(&self, node_id: NodeId) -> Option<WidgetId> {
        self.node_to_id.get(&node_id).copied()
    }

    /// Check if a widget ID is registered.
    pub fn contains(&self, widget_id: WidgetId) -> bool {
        self.id_to_node.contains_key(&widget_id)
    }

    /// Remove a widget ID mapping.
    pub fn remove(&mut self, widget_id: WidgetId) {
        if let Some(node_id) = self.id_to_node.remove(&widget_id) {
            self.node_to_id.remove(&node_id);
        }
    }

    /// Clear all mappings (e.g., on full rebuild).
    pub fn clear(&mut self) {
        self.id_to_node.clear();
        self.node_to_id.clear();
    }

    /// Get the number of registered widgets.
    pub fn len(&self) -> usize {
        self.id_to_node.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.id_to_node.is_empty()
    }

    /// Iterate over all widget ID to node ID mappings.
    pub fn iter(&self) -> impl Iterator<Item = (WidgetId, NodeId)> + '_ {
        self.id_to_node.iter().map(|(&id, &node)| (id, node))
    }
}

impl Default for WidgetIdRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_widget_id_creation() {
        let id1 = WidgetId::new("button1");
        let id2 = WidgetId::new("button1");
        let id3 = WidgetId::new("button2");

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_widget_id_from_str() {
        let id1: WidgetId = "test".into();
        let id2 = WidgetId::new("test");
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_registry() {
        let mut registry = WidgetIdRegistry::new();
        let widget_id = WidgetId::new("test");
        let node_id = NodeId(42);

        registry.register(widget_id, node_id);

        assert_eq!(registry.get_node(widget_id), Some(node_id));
        assert_eq!(registry.get_widget_id(node_id), Some(widget_id));
        assert!(registry.contains(widget_id));

        registry.remove(widget_id);
        assert_eq!(registry.get_node(widget_id), None);
    }

    #[test]
    fn test_registry_clear() {
        let mut registry = WidgetIdRegistry::new();
        registry.register(WidgetId::new("a"), NodeId(1));
        registry.register(WidgetId::new("b"), NodeId(2));

        assert_eq!(registry.len(), 2);

        registry.clear();
        assert_eq!(registry.len(), 0);
        assert!(registry.is_empty());
    }
}
