//! The scene arena: hierarchy, transforms, and component access.

use std::any::TypeId;
use std::collections::HashMap;

use slotmap::SlotMap;

use astrelis_core::math::{Quat, Vec3};

use crate::component::ComponentColumn;
use crate::node::{Node, NodeId};
use crate::transform::Transform;

/// Errors returned by structural scene mutations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneError {
    /// Reparenting would make a node its own ancestor.
    WouldCycle,
    /// A [`NodeId`] referred to a node that no longer exists.
    InvalidNode,
}

impl std::fmt::Display for SceneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WouldCycle => write!(f, "reparenting would create a cycle"),
            Self::InvalidNode => write!(f, "node id is stale or invalid"),
        }
    }
}

impl std::error::Error for SceneError {}

/// An arena-backed scene tree (a forest — multiple roots are allowed).
///
/// All node access goes through `Scene` methods using copyable
/// [`NodeId`] handles. Component data attaches via
/// [`insert`](Scene::insert) and is queried with [`iter`](Scene::iter).
#[derive(Default)]
pub struct Scene {
    pub(crate) nodes: SlotMap<NodeId, Node>,
    pub(crate) roots: Vec<NodeId>,
    pub(crate) columns: HashMap<TypeId, Box<dyn ComponentColumn>>,
}

impl Scene {
    /// Creates an empty scene.
    pub fn new() -> Self {
        Self::default()
    }

    /// Spawns a root-level node, returning a builder for it.
    pub fn spawn(&mut self) -> NodeBuilder<'_> {
        let id = self.nodes.insert(Node::new());
        self.roots.push(id);
        NodeBuilder { scene: self, id }
    }

    /// Spawns a child of `parent`, returning a builder for it.
    ///
    /// # Panics
    ///
    /// Panics if `parent` is stale — spawning under a despawned node is
    /// a programming error at the call site.
    pub fn spawn_child(&mut self, parent: NodeId) -> NodeBuilder<'_> {
        assert!(
            self.nodes.contains_key(parent),
            "spawn_child: parent node is stale"
        );
        let id = self.nodes.insert(Node::new());
        self.nodes[id].parent = Some(parent);
        self.nodes[parent].children.push(id);
        NodeBuilder { scene: self, id }
    }

    /// Despawns a node and its entire subtree, removing all components.
    ///
    /// A stale `id` is a no-op.
    pub fn despawn(&mut self, id: NodeId) {
        if !self.nodes.contains_key(id) {
            return;
        }
        self.detach(id);
        let mut stack = vec![id];
        while let Some(n) = stack.pop() {
            let node = self.nodes.remove(n).expect("subtree node exists");
            for column in self.columns.values_mut() {
                column.remove(n);
            }
            stack.extend(node.children);
        }
    }

    /// Moves `id` under `new_parent`, or to root level if `None`.
    ///
    /// The node keeps its *local* transform; its world transform changes
    /// accordingly on the next propagation pass.
    pub fn set_parent(&mut self, id: NodeId, new_parent: Option<NodeId>) -> Result<(), SceneError> {
        if !self.nodes.contains_key(id) {
            return Err(SceneError::InvalidNode);
        }
        if let Some(p) = new_parent {
            if !self.nodes.contains_key(p) {
                return Err(SceneError::InvalidNode);
            }
            // Walk up from the new parent; hitting `id` means a cycle.
            let mut cur = Some(p);
            while let Some(c) = cur {
                if c == id {
                    return Err(SceneError::WouldCycle);
                }
                cur = self.nodes[c].parent;
            }
        }
        self.detach(id);
        match new_parent {
            Some(p) => {
                self.nodes[p].children.push(id);
                self.nodes[id].parent = Some(p);
            }
            None => {
                self.roots.push(id);
                self.nodes[id].parent = None;
            }
        }
        self.nodes[id].dirty = true;
        Ok(())
    }

    /// Unlinks `id` from its parent's child list (or from `roots`).
    /// Does not touch `id`'s own `parent` field.
    fn detach(&mut self, id: NodeId) {
        match self.nodes[id].parent {
            Some(p) => self.nodes[p].children.retain(|&c| c != id),
            None => self.roots.retain(|&r| r != id),
        }
    }

    /// Returns whether `id` refers to a live node.
    pub fn contains(&self, id: NodeId) -> bool {
        self.nodes.contains_key(id)
    }

    /// The number of live nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// The root-level nodes, in spawn order.
    pub fn roots(&self) -> &[NodeId] {
        &self.roots
    }

    /// A node's parent, or `None` for roots and stale ids.
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.nodes.get(id)?.parent
    }

    /// A node's children, or an empty slice for stale ids.
    pub fn children(&self, id: NodeId) -> &[NodeId] {
        self.nodes.get(id).map_or(&[], |n| &n.children)
    }

    /// Iterates all descendants of `id` (depth-first, excluding `id`).
    pub fn descendants(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        let mut stack: Vec<NodeId> = self.children(id).to_vec();
        std::iter::from_fn(move || {
            let next = stack.pop()?;
            stack.extend_from_slice(self.children(next));
            Some(next)
        })
    }

    /// A node's name.
    pub fn name(&self, id: NodeId) -> Option<&str> {
        self.nodes.get(id)?.name.as_deref()
    }

    /// Sets a node's name. A stale `id` is a no-op.
    pub fn set_name(&mut self, id: NodeId, name: impl Into<String>) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.name = Some(name.into());
        }
    }

    /// A node's local transform.
    pub fn local_transform(&self, id: NodeId) -> Option<&Transform> {
        self.nodes.get(id).map(|n| &n.transform)
    }

    /// Replaces a node's local transform. A stale `id` is a no-op.
    pub fn set_transform(&mut self, id: NodeId, transform: Transform) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.transform = transform;
            node.dirty = true;
        }
    }

    /// Sets a node's local position. A stale `id` is a no-op.
    pub fn set_position(&mut self, id: NodeId, position: Vec3) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.transform.position = position;
            node.dirty = true;
        }
    }

    /// Sets a node's local rotation. A stale `id` is a no-op.
    pub fn set_rotation(&mut self, id: NodeId, rotation: Quat) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.transform.rotation = rotation;
            node.dirty = true;
        }
    }

    /// Sets a node's local scale. A stale `id` is a no-op.
    pub fn set_scale(&mut self, id: NodeId, scale: Vec3) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.transform.scale = scale;
            node.dirty = true;
        }
    }

    /// A node's own visibility flag (not inherited).
    pub fn visible(&self, id: NodeId) -> Option<bool> {
        self.nodes.get(id).map(|n| n.visible)
    }

    /// Sets a node's visibility flag. Descendants inherit invisibility
    /// via the propagation pass. A stale `id` is a no-op.
    pub fn set_visible(&mut self, id: NodeId, visible: bool) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.visible = visible;
            node.dirty = true;
        }
    }
}

/// Builder returned by [`Scene::spawn`]/[`Scene::spawn_child`].
///
/// The node already exists; the builder just configures it. Call
/// [`id`](Self::id) to finish and get the [`NodeId`].
pub struct NodeBuilder<'a> {
    scene: &'a mut Scene,
    id: NodeId,
}

impl NodeBuilder<'_> {
    /// Sets the node's name.
    pub fn name(self, name: impl Into<String>) -> Self {
        self.scene.nodes[self.id].name = Some(name.into());
        self
    }

    /// Sets the node's full local transform.
    pub fn transform(self, transform: Transform) -> Self {
        self.scene.nodes[self.id].transform = transform;
        self
    }

    /// Sets the node's local position.
    pub fn position(self, position: Vec3) -> Self {
        self.scene.nodes[self.id].transform.position = position;
        self
    }

    /// Sets the node's visibility flag.
    pub fn visible(self, visible: bool) -> Self {
        self.scene.nodes[self.id].visible = visible;
        self
    }

    /// Finishes building and returns the node's id.
    pub fn id(self) -> NodeId {
        self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use astrelis_core::math::Vec3;
    use crate::transform::Transform;

    #[test]
    fn spawn_creates_root_nodes() {
        let mut scene = Scene::new();
        let a = scene.spawn().name("a").id();
        let b = scene.spawn().id();
        assert_eq!(scene.roots(), &[a, b]);
        assert_eq!(scene.name(a), Some("a"));
        assert_eq!(scene.name(b), None);
        assert_eq!(scene.parent(a), None);
        assert!(scene.contains(a));
    }

    #[test]
    fn spawn_child_links_both_directions() {
        let mut scene = Scene::new();
        let parent = scene.spawn().id();
        let child = scene.spawn_child(parent).id();
        assert_eq!(scene.parent(child), Some(parent));
        assert_eq!(scene.children(parent), &[child]);
        assert_eq!(scene.roots(), &[parent]);
    }

    #[test]
    fn builder_sets_transform_fields() {
        let mut scene = Scene::new();
        let id = scene
            .spawn()
            .position(Vec3::new(1.0, 2.0, 3.0))
            .visible(false)
            .id();
        let t = scene.local_transform(id).unwrap();
        assert_eq!(t.position, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(scene.visible(id), Some(false));
    }

    #[test]
    fn stale_id_accessors_return_none_or_empty() {
        let mut scene = Scene::new();
        let id = scene.spawn().id();
        scene.despawn(id);
        assert!(!scene.contains(id));
        assert_eq!(scene.name(id), None);
        assert_eq!(scene.parent(id), None);
        assert_eq!(scene.local_transform(id), None);
        assert_eq!(scene.visible(id), None);
        assert!(scene.children(id).is_empty());
    }

    #[test]
    fn despawn_is_recursive_and_fixes_roots() {
        let mut scene = Scene::new();
        let a = scene.spawn().id();
        let b = scene.spawn_child(a).id();
        let c = scene.spawn_child(b).id();
        let other = scene.spawn().id();
        scene.despawn(a);
        assert!(!scene.contains(a));
        assert!(!scene.contains(b));
        assert!(!scene.contains(c));
        assert_eq!(scene.roots(), &[other]);
    }

    #[test]
    fn despawn_child_detaches_from_parent() {
        let mut scene = Scene::new();
        let a = scene.spawn().id();
        let b = scene.spawn_child(a).id();
        scene.despawn(b);
        assert!(scene.children(a).is_empty());
        assert!(scene.contains(a));
    }

    #[test]
    fn set_parent_reparents() {
        let mut scene = Scene::new();
        let a = scene.spawn().id();
        let b = scene.spawn().id();
        let c = scene.spawn_child(a).id();
        scene.set_parent(c, Some(b)).unwrap();
        assert!(scene.children(a).is_empty());
        assert_eq!(scene.children(b), &[c]);
        assert_eq!(scene.parent(c), Some(b));
        // Detach to root level.
        scene.set_parent(c, None).unwrap();
        assert_eq!(scene.parent(c), None);
        assert_eq!(scene.roots(), &[a, b, c]);
    }

    #[test]
    fn set_parent_rejects_cycles() {
        let mut scene = Scene::new();
        let a = scene.spawn().id();
        let b = scene.spawn_child(a).id();
        let c = scene.spawn_child(b).id();
        assert_eq!(scene.set_parent(a, Some(c)), Err(SceneError::WouldCycle));
        assert_eq!(scene.set_parent(a, Some(a)), Err(SceneError::WouldCycle));
        // Tree unchanged.
        assert_eq!(scene.parent(a), None);
        assert_eq!(scene.parent(c), Some(b));
    }

    #[test]
    fn set_parent_rejects_stale_ids() {
        let mut scene = Scene::new();
        let a = scene.spawn().id();
        let dead = scene.spawn().id();
        scene.despawn(dead);
        assert_eq!(scene.set_parent(dead, Some(a)), Err(SceneError::InvalidNode));
        assert_eq!(scene.set_parent(a, Some(dead)), Err(SceneError::InvalidNode));
    }

    #[test]
    fn descendants_walks_subtree() {
        let mut scene = Scene::new();
        let a = scene.spawn().id();
        let b = scene.spawn_child(a).id();
        let c = scene.spawn_child(b).id();
        let d = scene.spawn_child(a).id();
        let _other = scene.spawn().id();
        let mut desc: Vec<NodeId> = scene.descendants(a).collect();
        desc.sort();
        let mut expected = vec![b, c, d];
        expected.sort();
        assert_eq!(desc, expected);
    }
}
