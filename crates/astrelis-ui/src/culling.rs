//! Hierarchical AABB culling for efficient rendering.
//!
//! The culling system provides:
//! - Axis-Aligned Bounding Box (AABB) computation for nodes
//! - Hierarchical bounds (union of self + descendants)
//! - Fast viewport culling queries
//! - Hit testing for event dispatch
//!
//! # Architecture
//!
//! Each node has two bounding boxes:
//! - `bounds`: The node's own layout rectangle
//! - `hierarchical_bounds`: Union of node bounds with all descendant bounds
//!
//! Hierarchical culling allows skipping entire subtrees when their
//! hierarchical bounds don't intersect the viewport.
//!
//! # Example
//!
//! ```ignore
//! use astrelis_ui::culling::CullingTree;
//!
//! let mut culling = CullingTree::new();
//!
//! // Update after layout changes
//! culling.update(&tree, &dirty_nodes);
//!
//! // Query visible nodes
//! let viewport = AABB::new(0.0, 0.0, 800.0, 600.0);
//! let visible = culling.query_visible(&tree, viewport);
//!
//! // Hit testing
//! if let Some(node) = culling.hit_test(&tree, mouse_position) {
//!     // Handle click
//! }
//! ```

use astrelis_core::alloc::{HashMap, HashSet};
use astrelis_core::math::Vec2;

use crate::tree::{NodeId, UiTree};

/// Axis-Aligned Bounding Box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AABB {
    /// Minimum point (top-left).
    pub min: Vec2,
    /// Maximum point (bottom-right).
    pub max: Vec2,
}

impl AABB {
    /// Create a new AABB.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            min: Vec2::new(x, y),
            max: Vec2::new(x + width, y + height),
        }
    }

    /// Create from min/max points.
    pub fn from_min_max(min: Vec2, max: Vec2) -> Self {
        Self { min, max }
    }

    /// Create an empty (invalid) AABB.
    pub fn empty() -> Self {
        Self {
            min: Vec2::new(f32::MAX, f32::MAX),
            max: Vec2::new(f32::MIN, f32::MIN),
        }
    }

    /// Check if the AABB is empty/invalid.
    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x || self.min.y > self.max.y
    }

    /// Get the width.
    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    /// Get the height.
    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    /// Get the size.
    pub fn size(&self) -> Vec2 {
        Vec2::new(self.width(), self.height())
    }

    /// Get the center point.
    pub fn center(&self) -> Vec2 {
        Vec2::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
        )
    }

    /// Get the area.
    pub fn area(&self) -> f32 {
        self.width() * self.height()
    }

    /// Check if this AABB contains a point.
    pub fn contains_point(&self, point: Vec2) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }

    /// Check if this AABB intersects another.
    pub fn intersects(&self, other: &AABB) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
    }

    /// Check if this AABB fully contains another.
    pub fn contains(&self, other: &AABB) -> bool {
        self.min.x <= other.min.x
            && self.max.x >= other.max.x
            && self.min.y <= other.min.y
            && self.max.y >= other.max.y
    }

    /// Create the union of this AABB with another.
    pub fn union(&self, other: &AABB) -> AABB {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }

        AABB {
            min: Vec2::new(self.min.x.min(other.min.x), self.min.y.min(other.min.y)),
            max: Vec2::new(self.max.x.max(other.max.x), self.max.y.max(other.max.y)),
        }
    }

    /// Create the intersection of this AABB with another.
    pub fn intersection(&self, other: &AABB) -> Option<AABB> {
        let min = Vec2::new(self.min.x.max(other.min.x), self.min.y.max(other.min.y));
        let max = Vec2::new(self.max.x.min(other.max.x), self.max.y.min(other.max.y));

        if min.x <= max.x && min.y <= max.y {
            Some(AABB { min, max })
        } else {
            None
        }
    }

    /// Expand the AABB by a margin.
    pub fn expand(&self, margin: f32) -> AABB {
        AABB {
            min: Vec2::new(self.min.x - margin, self.min.y - margin),
            max: Vec2::new(self.max.x + margin, self.max.y + margin),
        }
    }

    /// Translate the AABB by an offset.
    pub fn translate(&self, offset: Vec2) -> AABB {
        AABB {
            min: self.min + offset,
            max: self.max + offset,
        }
    }
}

impl Default for AABB {
    fn default() -> Self {
        Self::empty()
    }
}

/// Node bounds data.
#[derive(Debug, Clone, Copy, Default)]
pub struct NodeBounds {
    /// Node's own bounds (absolute position).
    pub bounds: AABB,
    /// Hierarchical bounds (union with all descendants).
    pub hierarchical_bounds: AABB,
    /// Whether this node is visible (bounds computed).
    pub is_visible: bool,
}

/// Culling tree for hierarchical visibility testing.
pub struct CullingTree {
    /// Bounds data per node.
    bounds: HashMap<NodeId, NodeBounds>,
    /// Nodes that need bounds recomputation.
    dirty_nodes: HashSet<NodeId>,
    /// Statistics from last update.
    stats: CullingStats,
}

/// Statistics about culling operations.
#[derive(Debug, Clone, Default)]
pub struct CullingStats {
    /// Total nodes in the tree.
    pub total_nodes: usize,
    /// Nodes visible after culling.
    pub visible_nodes: usize,
    /// Subtrees culled (not traversed).
    pub subtrees_culled: usize,
    /// Individual nodes culled.
    pub nodes_culled: usize,
    /// Bounds updates performed.
    pub bounds_updated: usize,
}

impl CullingTree {
    /// Create a new culling tree.
    pub fn new() -> Self {
        Self {
            bounds: HashMap::new(),
            dirty_nodes: HashSet::new(),
            stats: CullingStats::default(),
        }
    }

    /// Create with initial capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            bounds: HashMap::with_capacity(capacity),
            dirty_nodes: HashSet::with_capacity(capacity),
            stats: CullingStats::default(),
        }
    }

    /// Mark a node as needing bounds update.
    pub fn mark_dirty(&mut self, node_id: NodeId) {
        self.dirty_nodes.insert(node_id);
    }

    /// Mark multiple nodes as dirty.
    pub fn mark_dirty_many(&mut self, nodes: impl IntoIterator<Item = NodeId>) {
        self.dirty_nodes.extend(nodes);
    }

    /// Update bounds for dirty nodes.
    ///
    /// This should be called after layout computation.
    pub fn update(&mut self, tree: &UiTree) {
        self.stats = CullingStats::default();
        self.stats.total_nodes = tree.iter().count();

        // If tree root changed or first update, update everything
        if self.bounds.is_empty() {
            if let Some(root_id) = tree.root() {
                self.update_subtree(tree, root_id, Vec2::ZERO);
            }
        } else {
            // Update dirty nodes and their ancestors
            let dirty: Vec<NodeId> = self.dirty_nodes.drain().collect();
            for node_id in dirty {
                // Find root of dirty subtree and update from there
                let subtree_root = self.find_dirty_subtree_root(tree, node_id);
                let parent_offset = self.get_parent_offset(tree, subtree_root);
                self.update_subtree(tree, subtree_root, parent_offset);
            }
        }
    }

    /// Update bounds for a subtree rooted at the given node.
    fn update_subtree(&mut self, tree: &UiTree, node_id: NodeId, parent_offset: Vec2) -> AABB {
        let Some(node) = tree.get_node(node_id) else {
            return AABB::empty();
        };

        self.stats.bounds_updated += 1;

        // Calculate absolute bounds
        let layout = node.layout;
        let abs_x = parent_offset.x + layout.x;
        let abs_y = parent_offset.y + layout.y;
        let bounds = AABB::new(abs_x, abs_y, layout.width, layout.height);

        // Recursively compute children bounds
        let offset = Vec2::new(abs_x, abs_y);
        let mut hierarchical_bounds = bounds;

        for &child_id in &node.children {
            let child_bounds = self.update_subtree(tree, child_id, offset);
            hierarchical_bounds = hierarchical_bounds.union(&child_bounds);
        }

        // Store bounds
        self.bounds.insert(
            node_id,
            NodeBounds {
                bounds,
                hierarchical_bounds,
                is_visible: !bounds.is_empty(),
            },
        );

        hierarchical_bounds
    }

    /// Find the root of the dirty subtree (highest dirty ancestor).
    fn find_dirty_subtree_root(&self, tree: &UiTree, node_id: NodeId) -> NodeId {
        let mut current = node_id;
        let mut root = node_id;

        while let Some(node) = tree.get_node(current) {
            if let Some(parent_id) = node.parent {
                if self.dirty_nodes.contains(&parent_id) {
                    root = parent_id;
                }
                current = parent_id;
            } else {
                break;
            }
        }

        root
    }

    /// Get the absolute offset of a node's parent.
    fn get_parent_offset(&self, tree: &UiTree, node_id: NodeId) -> Vec2 {
        let Some(node) = tree.get_node(node_id) else {
            return Vec2::ZERO;
        };

        let Some(parent_id) = node.parent else {
            return Vec2::ZERO;
        };

        if let Some(parent_bounds) = self.bounds.get(&parent_id) {
            parent_bounds.bounds.min
        } else {
            // Parent bounds not computed yet, walk up
            let mut offset = Vec2::ZERO;
            let mut current = Some(parent_id);

            while let Some(id) = current {
                if let Some(parent_node) = tree.get_node(id) {
                    offset.x += parent_node.layout.x;
                    offset.y += parent_node.layout.y;
                    current = parent_node.parent;
                } else {
                    break;
                }
            }

            offset
        }
    }

    /// Get bounds for a node.
    pub fn get_bounds(&self, node_id: NodeId) -> Option<&NodeBounds> {
        self.bounds.get(&node_id)
    }

    /// Query all nodes visible within a viewport.
    ///
    /// Uses hierarchical culling to skip entire subtrees.
    pub fn query_visible(&mut self, tree: &UiTree, viewport: AABB) -> Vec<NodeId> {
        let mut visible = Vec::new();
        self.stats.subtrees_culled = 0;
        self.stats.nodes_culled = 0;

        if let Some(root_id) = tree.root() {
            self.query_visible_recursive(tree, root_id, viewport, &mut visible);
        }

        self.stats.visible_nodes = visible.len();
        visible
    }

    /// Recursive visibility query with hierarchical culling.
    fn query_visible_recursive(
        &mut self,
        tree: &UiTree,
        node_id: NodeId,
        viewport: AABB,
        result: &mut Vec<NodeId>,
    ) {
        let Some(bounds) = self.bounds.get(&node_id) else {
            return;
        };

        // First, check hierarchical bounds
        // If the entire subtree is outside viewport, skip it
        if !viewport.intersects(&bounds.hierarchical_bounds) {
            self.stats.subtrees_culled += 1;
            return;
        }

        // Check this node's bounds
        if viewport.intersects(&bounds.bounds) {
            result.push(node_id);
        } else {
            self.stats.nodes_culled += 1;
        }

        // Recurse to children
        if let Some(node) = tree.get_node(node_id) {
            for &child_id in &node.children {
                self.query_visible_recursive(tree, child_id, viewport, result);
            }
        }
    }

    /// Query nodes within a rectangular region.
    ///
    /// Similar to `query_visible` but returns all nodes intersecting the region.
    pub fn query_region(&self, tree: &UiTree, region: AABB) -> Vec<NodeId> {
        let mut result = Vec::new();

        if let Some(root_id) = tree.root() {
            self.query_region_recursive(tree, root_id, region, &mut result);
        }

        result
    }

    fn query_region_recursive(
        &self,
        tree: &UiTree,
        node_id: NodeId,
        region: AABB,
        result: &mut Vec<NodeId>,
    ) {
        let Some(bounds) = self.bounds.get(&node_id) else {
            return;
        };

        if !region.intersects(&bounds.hierarchical_bounds) {
            return;
        }

        if region.intersects(&bounds.bounds) {
            result.push(node_id);
        }

        if let Some(node) = tree.get_node(node_id) {
            for &child_id in &node.children {
                self.query_region_recursive(tree, child_id, region, result);
            }
        }
    }

    /// Hit test to find the deepest node at a point.
    ///
    /// Returns the frontmost (deepest) node containing the point.
    pub fn hit_test(&self, tree: &UiTree, point: Vec2) -> Option<NodeId> {
        tree.root()
            .and_then(|root_id| self.hit_test_recursive(tree, root_id, point))
    }

    fn hit_test_recursive(&self, tree: &UiTree, node_id: NodeId, point: Vec2) -> Option<NodeId> {
        let bounds = self.bounds.get(&node_id)?;

        // Quick reject using hierarchical bounds
        if !bounds.hierarchical_bounds.contains_point(point) {
            return None;
        }

        // Check children first (front to back in reverse order)
        if let Some(node) = tree.get_node(node_id) {
            for &child_id in node.children.iter().rev() {
                if let Some(hit) = self.hit_test_recursive(tree, child_id, point) {
                    return Some(hit);
                }
            }
        }

        // Check this node
        if bounds.bounds.contains_point(point) {
            Some(node_id)
        } else {
            None
        }
    }

    /// Find all nodes at a point (for debugging/inspection).
    pub fn hit_test_all(&self, tree: &UiTree, point: Vec2) -> Vec<NodeId> {
        let mut result = Vec::new();

        if let Some(root_id) = tree.root() {
            self.hit_test_all_recursive(tree, root_id, point, &mut result);
        }

        result
    }

    fn hit_test_all_recursive(
        &self,
        tree: &UiTree,
        node_id: NodeId,
        point: Vec2,
        result: &mut Vec<NodeId>,
    ) {
        let Some(bounds) = self.bounds.get(&node_id) else {
            return;
        };

        if !bounds.hierarchical_bounds.contains_point(point) {
            return;
        }

        if bounds.bounds.contains_point(point) {
            result.push(node_id);
        }

        if let Some(node) = tree.get_node(node_id) {
            for &child_id in &node.children {
                self.hit_test_all_recursive(tree, child_id, point, result);
            }
        }
    }

    /// Get culling statistics.
    pub fn stats(&self) -> &CullingStats {
        &self.stats
    }

    /// Clear all bounds data.
    pub fn clear(&mut self) {
        self.bounds.clear();
        self.dirty_nodes.clear();
        self.stats = CullingStats::default();
    }

    /// Get the number of nodes with bounds.
    pub fn node_count(&self) -> usize {
        self.bounds.len()
    }
}

impl Default for CullingTree {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aabb_construction() {
        let aabb = AABB::new(10.0, 20.0, 100.0, 50.0);
        assert_eq!(aabb.min, Vec2::new(10.0, 20.0));
        assert_eq!(aabb.max, Vec2::new(110.0, 70.0));
        assert_eq!(aabb.width(), 100.0);
        assert_eq!(aabb.height(), 50.0);
    }

    #[test]
    fn test_aabb_empty() {
        let empty = AABB::empty();
        assert!(empty.is_empty());

        let non_empty = AABB::new(0.0, 0.0, 10.0, 10.0);
        assert!(!non_empty.is_empty());
    }

    #[test]
    fn test_aabb_contains_point() {
        let aabb = AABB::new(0.0, 0.0, 100.0, 100.0);

        assert!(aabb.contains_point(Vec2::new(50.0, 50.0)));
        assert!(aabb.contains_point(Vec2::new(0.0, 0.0)));
        assert!(aabb.contains_point(Vec2::new(100.0, 100.0)));
        assert!(!aabb.contains_point(Vec2::new(-1.0, 50.0)));
        assert!(!aabb.contains_point(Vec2::new(101.0, 50.0)));
    }

    #[test]
    fn test_aabb_intersects() {
        let a = AABB::new(0.0, 0.0, 100.0, 100.0);
        let b = AABB::new(50.0, 50.0, 100.0, 100.0);
        let c = AABB::new(200.0, 200.0, 50.0, 50.0);

        assert!(a.intersects(&b));
        assert!(b.intersects(&a));
        assert!(!a.intersects(&c));
        assert!(!c.intersects(&a));
    }

    #[test]
    fn test_aabb_union() {
        let a = AABB::new(0.0, 0.0, 50.0, 50.0);
        let b = AABB::new(100.0, 100.0, 50.0, 50.0);
        let union = a.union(&b);

        assert_eq!(union.min, Vec2::new(0.0, 0.0));
        assert_eq!(union.max, Vec2::new(150.0, 150.0));
    }

    #[test]
    fn test_aabb_intersection() {
        let a = AABB::new(0.0, 0.0, 100.0, 100.0);
        let b = AABB::new(50.0, 50.0, 100.0, 100.0);

        let intersection = a.intersection(&b);
        assert!(intersection.is_some());

        let inter = intersection.unwrap();
        assert_eq!(inter.min, Vec2::new(50.0, 50.0));
        assert_eq!(inter.max, Vec2::new(100.0, 100.0));

        let c = AABB::new(200.0, 200.0, 50.0, 50.0);
        assert!(a.intersection(&c).is_none());
    }

    #[test]
    fn test_culling_tree_basic() {
        let mut tree = UiTree::new();
        let root = tree.add_widget(Box::new(crate::widgets::Container::new()));
        tree.set_root(root);

        // Compute layout
        let registry = crate::plugin::registry::WidgetTypeRegistry::new();
        tree.compute_layout(astrelis_core::geometry::Size::new(800.0, 600.0), None, &registry);

        let mut culling = CullingTree::new();
        culling.update(&tree);

        assert_eq!(culling.node_count(), 1);
    }

    #[test]
    fn test_culling_tree_query() {
        let mut tree = UiTree::new();

        // Create a simple tree
        let mut root_container = crate::widgets::Container::new();
        root_container.style.layout.size.width = taffy::Dimension::Length(800.0);
        root_container.style.layout.size.height = taffy::Dimension::Length(600.0);
        let root = tree.add_widget(Box::new(root_container));

        let mut child1 = crate::widgets::Container::new();
        child1.style.layout.size.width = taffy::Dimension::Length(100.0);
        child1.style.layout.size.height = taffy::Dimension::Length(100.0);
        let child1_id = tree.add_widget(Box::new(child1));

        tree.add_child(root, child1_id);
        tree.set_root(root);

        // Compute layout
        let registry = crate::plugin::registry::WidgetTypeRegistry::new();
        tree.compute_layout(astrelis_core::geometry::Size::new(800.0, 600.0), None, &registry);

        let mut culling = CullingTree::new();
        culling.update(&tree);

        // Query visible in full viewport
        let viewport = AABB::new(0.0, 0.0, 800.0, 600.0);
        let visible = culling.query_visible(&tree, viewport);
        assert_eq!(visible.len(), 2);

        // Query with smaller viewport
        let small_viewport = AABB::new(0.0, 0.0, 50.0, 50.0);
        let visible_small = culling.query_visible(&tree, small_viewport);
        // Should still include nodes that intersect
        assert!(!visible_small.is_empty());
    }

    #[test]
    fn test_hit_test() {
        let mut tree = UiTree::new();

        let mut root_container = crate::widgets::Container::new();
        root_container.style.layout.size.width = taffy::Dimension::Length(800.0);
        root_container.style.layout.size.height = taffy::Dimension::Length(600.0);
        let root = tree.add_widget(Box::new(root_container));
        tree.set_root(root);

        let registry = crate::plugin::registry::WidgetTypeRegistry::new();
        tree.compute_layout(astrelis_core::geometry::Size::new(800.0, 600.0), None, &registry);

        let mut culling = CullingTree::new();
        culling.update(&tree);

        // Hit test inside
        let hit = culling.hit_test(&tree, Vec2::new(400.0, 300.0));
        assert_eq!(hit, Some(root));

        // Hit test outside
        let miss = culling.hit_test(&tree, Vec2::new(1000.0, 1000.0));
        assert!(miss.is_none());
    }

    #[test]
    fn test_culling_stats() {
        let culling = CullingTree::new();
        let stats = culling.stats();
        assert_eq!(stats.total_nodes, 0);
        assert_eq!(stats.visible_nodes, 0);
    }

    #[test]
    fn test_aabb_center() {
        let aabb = AABB::new(0.0, 0.0, 100.0, 50.0);
        let center = aabb.center();
        assert_eq!(center, Vec2::new(50.0, 25.0));
    }

    #[test]
    fn test_aabb_expand() {
        let aabb = AABB::new(10.0, 10.0, 50.0, 50.0);
        let expanded = aabb.expand(5.0);

        assert_eq!(expanded.min, Vec2::new(5.0, 5.0));
        assert_eq!(expanded.max, Vec2::new(65.0, 65.0));
    }

    #[test]
    fn test_aabb_from_min_max() {
        let aabb = AABB::from_min_max(Vec2::new(10.0, 20.0), Vec2::new(100.0, 80.0));
        assert_eq!(aabb.min, Vec2::new(10.0, 20.0));
        assert_eq!(aabb.max, Vec2::new(100.0, 80.0));
        assert_eq!(aabb.width(), 90.0);
        assert_eq!(aabb.height(), 60.0);
    }

    #[test]
    fn test_aabb_contains() {
        let outer = AABB::new(0.0, 0.0, 100.0, 100.0);
        let inner = AABB::new(25.0, 25.0, 50.0, 50.0);
        let partial = AABB::new(50.0, 50.0, 100.0, 100.0);
        let outside = AABB::new(200.0, 200.0, 50.0, 50.0);

        assert!(outer.contains(&inner));
        assert!(!outer.contains(&partial));
        assert!(!outer.contains(&outside));
    }

    #[test]
    fn test_aabb_area() {
        let aabb = AABB::new(0.0, 0.0, 100.0, 50.0);
        assert_eq!(aabb.area(), 5000.0);

        let small = AABB::new(0.0, 0.0, 10.0, 5.0);
        assert_eq!(small.area(), 50.0);

        let zero = AABB::new(0.0, 0.0, 0.0, 0.0);
        assert_eq!(zero.area(), 0.0);
    }

    #[test]
    fn test_culling_tree_clear() {
        let mut tree = UiTree::new();
        let root = tree.add_widget(Box::new(crate::widgets::Container::new()));
        tree.set_root(root);
        let registry = crate::plugin::registry::WidgetTypeRegistry::new();
        tree.compute_layout(astrelis_core::geometry::Size::new(800.0, 600.0), None, &registry);

        let mut culling = CullingTree::new();
        culling.update(&tree);
        assert_eq!(culling.node_count(), 1);

        culling.clear();
        assert_eq!(culling.node_count(), 0);
    }

    #[test]
    fn test_aabb_translate() {
        let aabb = AABB::new(10.0, 20.0, 50.0, 30.0);
        let translated = aabb.translate(Vec2::new(5.0, -10.0));

        assert_eq!(translated.min, Vec2::new(15.0, 10.0));
        assert_eq!(translated.max, Vec2::new(65.0, 40.0));
    }

    #[test]
    fn test_culling_tree_get_bounds() {
        let mut tree = UiTree::new();
        let mut container = crate::widgets::Container::new();
        container.style.layout.size.width = taffy::Dimension::Length(200.0);
        container.style.layout.size.height = taffy::Dimension::Length(100.0);
        let root = tree.add_widget(Box::new(container));
        tree.set_root(root);
        let registry = crate::plugin::registry::WidgetTypeRegistry::new();
        tree.compute_layout(astrelis_core::geometry::Size::new(800.0, 600.0), None, &registry);

        let mut culling = CullingTree::new();
        culling.update(&tree);

        let node_bounds = culling.get_bounds(root);
        assert!(node_bounds.is_some());
        let bounds = &node_bounds.unwrap().bounds;
        assert_eq!(bounds.width(), 200.0);
        assert_eq!(bounds.height(), 100.0);
    }
}
