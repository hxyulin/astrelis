//! Layout cache for storing computed layout information.

use crate::tree::{LayoutRect, NodeId};
use astrelis_core::alloc::HashMap;

/// Cache for layout computations.
#[derive(Debug, Clone, Default)]
pub struct LayoutCache {
    layouts: HashMap<NodeId, LayoutRect>,
}

impl LayoutCache {
    /// Create a new layout cache.
    pub fn new() -> Self {
        Self {
            layouts: HashMap::new(),
        }
    }

    /// Store layout for a node.
    pub fn set(&mut self, node_id: NodeId, layout: LayoutRect) {
        self.layouts.insert(node_id, layout);
    }

    /// Get layout for a node.
    pub fn get(&self, node_id: NodeId) -> Option<&LayoutRect> {
        self.layouts.get(&node_id)
    }

    /// Check if a node has cached layout.
    pub fn contains(&self, node_id: NodeId) -> bool {
        self.layouts.contains_key(&node_id)
    }

    /// Clear all cached layouts.
    pub fn clear(&mut self) {
        self.layouts.clear();
    }

    /// Get the number of cached layouts.
    pub fn len(&self) -> usize {
        self.layouts.len()
    }

    /// Check if cache is empty.
    pub fn is_empty(&self) -> bool {
        self.layouts.is_empty()
    }
}
