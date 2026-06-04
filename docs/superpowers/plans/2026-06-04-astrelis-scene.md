# `astrelis-scene` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `astrelis-scene` crate — an arena-backed scene tree with columnar component storage, cached transform/visibility propagation, and `astrelis-app` plugin integration — plus a `scene_demo` example.

**Architecture:** `Scene` owns all nodes in a `slotmap::SlotMap` (generational `NodeId` handles) and one `SecondaryMap<NodeId, T>` column per component type. All mutation goes through `Scene` methods (the `Node` struct is private), which keeps dirty flags and column cleanup correct. A `ScenePlugin` registers the scene as a resource and runs one transform/visibility propagation pass per frame in `Phase::PostUpdate`. The crate has **zero renderer dependencies**; the demo shows scene→renderer glue written in user code.

**Tech Stack:** Rust edition 2024, `slotmap` 1.x (new workspace dep), `astrelis-core` (glam math), `astrelis-app`, `astrelis-profiling`. Spec: `docs/superpowers/specs/2026-06-04-astrelis-scene-design.md`.

**Codebase conventions that apply to every task:**
- All public items need doc comments (`#![warn(missing_docs)]` via workspace lints).
- Crate `Cargo.toml` files use `version.workspace = true` etc. and `{ workspace = true }` deps (copy the pattern from `crates/astrelis-render-2d/Cargo.toml`).
- No `println!`/`eprintln!` in library code; use `tracing` macros if logging is ever needed.
- Conventional commits.
- A `PostToolUse` hook auto-formats Rust files on write — do not hand-format.

**Verification command used throughout** (run from repo root `/Users/hxyulin/dev/projects/astrelis`):

```bash
cargo test -p astrelis-scene 2>&1 | tail -20
```

---

### Task 1: Crate scaffold

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Create: `crates/astrelis-scene/Cargo.toml`
- Create: `crates/astrelis-scene/src/lib.rs`

- [ ] **Step 1: Register the crate in the workspace**

In root `Cargo.toml`:

1. Add to `[workspace] members`, after `"crates/astrelis-app"`:

```toml
    "crates/astrelis-scene",
```

2. Add to `[workspace.dependencies]`, in the "Internal crates" block after the `astrelis-app` line:

```toml
astrelis-scene = { path = "crates/astrelis-scene", version = "0.3.0" }
```

3. Add a new dependency section after the `# Math` block:

```toml
# Scene
slotmap = "1"
```

- [ ] **Step 2: Create the crate manifest**

Create `crates/astrelis-scene/Cargo.toml`:

```toml
[package]
name = "astrelis-scene"
description = "Scene tree with columnar component storage for the Astrelis engine"
version.workspace = true
authors.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
astrelis-core = { workspace = true }
astrelis-app = { workspace = true }
astrelis-profiling = { workspace = true }
slotmap = { workspace = true }

[lints]
workspace = true
```

- [ ] **Step 3: Create the lib.rs skeleton**

Create `crates/astrelis-scene/src/lib.rs`:

```rust
//! Scene tree for the Astrelis engine.
//!
//! A [`Scene`] owns a forest of nodes in an arena, addressed by
//! generational [`NodeId`] handles. Each node has a name, a local
//! [`Transform`], a visibility flag, and parent/children links.
//! Arbitrary data attaches to nodes as [`Component`]s, stored in
//! per-type columns so queries iterate only nodes that have the
//! component.
//!
//! Nodes are pure data: game logic lives in ordinary `astrelis-app`
//! systems that query the scene. [`ScenePlugin`] inserts a [`Scene`]
//! resource and runs one transform/visibility propagation pass per
//! frame in `Phase::PostUpdate` — mutate the scene in `Update`, read
//! world transforms in `Render`.
//!
//! This crate has no renderer or GPU dependencies. Rendering glue
//! (e.g. a sprite component plus a `Render`-phase system that calls a
//! renderer) lives downstream.

#![warn(missing_docs)]

pub mod component;
pub mod node;
pub mod plugin;
pub mod scene;
pub mod transform;

pub use component::Component;
pub use node::NodeId;
pub use plugin::ScenePlugin;
pub use scene::{NodeBuilder, Scene, SceneError};
pub use transform::Transform;
```

This will not compile until the modules exist — that is expected; the remaining steps of this task stub them.

- [ ] **Step 4: Create empty module stubs so the crate compiles**

Create `crates/astrelis-scene/src/transform.rs`:

```rust
//! Local node transform (translation, rotation, scale).
```

Create `crates/astrelis-scene/src/node.rs`:

```rust
//! Node identity and per-node data.
```

Create `crates/astrelis-scene/src/component.rs`:

```rust
//! Component trait and columnar storage.
```

Create `crates/astrelis-scene/src/scene.rs`:

```rust
//! The scene arena: hierarchy, transforms, and component access.
```

Create `crates/astrelis-scene/src/plugin.rs`:

```rust
//! App integration.
```

Then **temporarily comment out** the `pub use` lines in `lib.rs` (the types don't exist yet):

```rust
// Re-exports restored as the types land in Tasks 2-7.
// pub use component::Component;
// pub use node::NodeId;
// pub use plugin::ScenePlugin;
// pub use scene::{NodeBuilder, Scene, SceneError};
// pub use transform::Transform;
```

- [ ] **Step 5: Verify the workspace builds**

Run: `cargo build -p astrelis-scene 2>&1 | tail -5`
Expected: `Finished` with no errors.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/astrelis-scene
git commit -m "feat(scene): scaffold astrelis-scene crate"
```

---

### Task 2: `Transform`

**Files:**
- Modify: `crates/astrelis-scene/src/transform.rs`
- Modify: `crates/astrelis-scene/src/lib.rs` (restore one re-export)

- [ ] **Step 1: Write the failing tests**

Append to `crates/astrelis-scene/src/transform.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use astrelis_core::math::{Mat4, Quat, Vec3};

    #[test]
    fn default_is_identity() {
        assert_eq!(Transform::default(), Transform::IDENTITY);
        assert_eq!(Transform::IDENTITY.matrix(), Mat4::IDENTITY);
    }

    #[test]
    fn matrix_applies_scale_then_rotation_then_translation() {
        let t = Transform {
            position: Vec3::new(1.0, 0.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(2.0),
        };
        // (1,1,1) scaled by 2 -> (2,2,2), translated by (1,0,0) -> (3,2,2)
        let p = t.matrix().transform_point3(Vec3::ONE);
        assert!(p.abs_diff_eq(Vec3::new(3.0, 2.0, 2.0), 1e-6));
    }

    #[test]
    fn from_xy_sets_z_zero() {
        let t = Transform::from_xy(3.0, 4.0);
        assert_eq!(t.position, Vec3::new(3.0, 4.0, 0.0));
        assert_eq!(t.rotation, Quat::IDENTITY);
        assert_eq!(t.scale, Vec3::ONE);
    }

    #[test]
    fn rotation_2d_rotates_around_z() {
        let mut t = Transform::IDENTITY;
        t.set_rotation_2d(std::f32::consts::FRAC_PI_2);
        // +X rotated 90 degrees around +Z lands on +Y.
        let p = t.matrix().transform_point3(Vec3::X);
        assert!(p.abs_diff_eq(Vec3::Y, 1e-6));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p astrelis-scene transform 2>&1 | tail -10`
Expected: compile error — `Transform` not found.

- [ ] **Step 3: Implement `Transform`**

Insert between the module doc comment and the test module in `transform.rs`:

```rust
use astrelis_core::math::{Mat4, Quat, Vec3};

/// A node's local transform, relative to its parent.
///
/// Composed as scale, then rotation, then translation (standard TRS).
/// 2D content uses `position.x`/`position.y` plus
/// [`set_rotation_2d`](Self::set_rotation_2d); `position.z` is
/// available for draw-order layering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// Translation relative to the parent.
    pub position: Vec3,
    /// Rotation relative to the parent.
    pub rotation: Quat,
    /// Scale relative to the parent.
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    /// The identity transform: zero translation, no rotation, unit scale.
    pub const IDENTITY: Self = Self {
        position: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    /// Creates a transform with the given translation.
    pub fn from_position(position: Vec3) -> Self {
        Self {
            position,
            ..Self::IDENTITY
        }
    }

    /// Creates a 2D transform at `(x, y)` with `z = 0`.
    pub fn from_xy(x: f32, y: f32) -> Self {
        Self::from_position(Vec3::new(x, y, 0.0))
    }

    /// Sets the rotation to `angle` radians around +Z (the 2D rotation axis).
    pub fn set_rotation_2d(&mut self, angle: f32) {
        self.rotation = Quat::from_rotation_z(angle);
    }

    /// Computes the local transformation matrix.
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }
}
```

In `lib.rs`, restore the re-export (delete the commented line):

```rust
pub use transform::Transform;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p astrelis-scene transform 2>&1 | tail -10`
Expected: `test result: ok. 4 passed`

- [ ] **Step 5: Commit**

```bash
git add crates/astrelis-scene
git commit -m "feat(scene): add Transform with TRS matrix and 2D conveniences"
```

---

### Task 3: Scene arena, spawn, and hierarchy

**Files:**
- Modify: `crates/astrelis-scene/src/node.rs`
- Modify: `crates/astrelis-scene/src/scene.rs`
- Modify: `crates/astrelis-scene/src/lib.rs` (restore re-exports)

- [ ] **Step 1: Write the failing tests**

Append to `crates/astrelis-scene/src/scene.rs`:

```rust
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p astrelis-scene scene 2>&1 | tail -10`
Expected: compile error — `Scene` not found.

- [ ] **Step 3: Implement `Node` and `NodeId`**

Append to `crates/astrelis-scene/src/node.rs`:

```rust
use astrelis_core::math::Mat4;
use slotmap::new_key_type;

use crate::transform::Transform;

new_key_type! {
    /// A generational handle to a node in a [`Scene`](crate::Scene).
    ///
    /// `NodeId`s are cheap to copy and remain safe to hold after the
    /// node is despawned: access through a stale id returns `None`.
    pub struct NodeId;
}

/// Per-node data. Private — all access goes through `Scene` methods,
/// which is what keeps dirty flags and column cleanup correct.
pub(crate) struct Node {
    pub(crate) name: Option<String>,
    pub(crate) transform: Transform,
    pub(crate) parent: Option<NodeId>,
    pub(crate) children: Vec<NodeId>,
    pub(crate) visible: bool,
    /// Cached world matrix, valid as of the last propagation pass.
    pub(crate) world: Mat4,
    /// Cached hierarchical visibility, valid as of the last pass.
    pub(crate) world_visible: bool,
    /// Set on any transform/visibility/parent change; cleared by the pass.
    pub(crate) dirty: bool,
}

impl Node {
    pub(crate) fn new() -> Self {
        Self {
            name: None,
            transform: Transform::IDENTITY,
            parent: None,
            children: Vec::new(),
            visible: true,
            world: Mat4::IDENTITY,
            world_visible: true,
            dirty: true,
        }
    }
}
```

- [ ] **Step 4: Implement `Scene`, `NodeBuilder`, and `SceneError`**

Insert into `crates/astrelis-scene/src/scene.rs`, between the module doc and the tests:

```rust
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
```

- [ ] **Step 5: Add the `ComponentColumn` trait stub**

`Scene` references `ComponentColumn`, so add the minimal trait now (Task 4 fleshes out storage). Append to `crates/astrelis-scene/src/component.rs`:

```rust
use std::any::Any;

use crate::node::NodeId;

/// Object-safe interface over one per-type component column.
pub(crate) trait ComponentColumn: Send + Sync {
    /// Removes `id`'s component from this column, if present.
    fn remove(&mut self, id: NodeId);
    /// Upcast for typed downcasting.
    fn as_any(&self) -> &dyn Any;
    /// Upcast for typed downcasting.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
```

In `lib.rs`, restore the re-exports (Component stays commented until Task 4):

```rust
pub use node::NodeId;
pub use scene::{NodeBuilder, Scene, SceneError};
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p astrelis-scene scene 2>&1 | tail -10`
Expected: `test result: ok. 10 passed`

- [ ] **Step 7: Commit**

```bash
git add crates/astrelis-scene
git commit -m "feat(scene): add arena-backed node hierarchy with builder API"
```

---

### Task 4: Component storage and queries

**Files:**
- Modify: `crates/astrelis-scene/src/component.rs`
- Modify: `crates/astrelis-scene/src/lib.rs` (restore `Component` re-export)

- [ ] **Step 1: Write the failing tests**

Append to `crates/astrelis-scene/src/component.rs`:

```rust
#[cfg(test)]
mod tests {
    use crate::scene::Scene;

    #[derive(Debug, PartialEq)]
    struct Health(u32);

    #[derive(Debug, PartialEq)]
    struct Tag;

    #[test]
    fn insert_get_remove_roundtrip() {
        let mut scene = Scene::new();
        let id = scene.spawn().id();
        assert_eq!(scene.insert(id, Health(10)), None);
        assert_eq!(scene.get::<Health>(id), Some(&Health(10)));
        scene.get_mut::<Health>(id).unwrap().0 = 20;
        assert_eq!(scene.remove::<Health>(id), Some(Health(20)));
        assert_eq!(scene.get::<Health>(id), None);
    }

    #[test]
    fn insert_replaces_and_returns_old_value() {
        let mut scene = Scene::new();
        let id = scene.spawn().id();
        scene.insert(id, Health(1));
        assert_eq!(scene.insert(id, Health(2)), Some(Health(1)));
        assert_eq!(scene.get::<Health>(id), Some(&Health(2)));
    }

    #[test]
    fn stale_id_is_rejected() {
        let mut scene = Scene::new();
        let id = scene.spawn().id();
        scene.despawn(id);
        assert_eq!(scene.insert(id, Health(1)), None);
        assert_eq!(scene.get::<Health>(id), None);
        assert_eq!(scene.remove::<Health>(id), None);
    }

    #[test]
    fn missing_column_returns_none_and_empty_iter() {
        let mut scene = Scene::new();
        let id = scene.spawn().id();
        assert_eq!(scene.get::<Health>(id), None);
        assert_eq!(scene.iter::<Health>().count(), 0);
    }

    #[test]
    fn iter_yields_only_nodes_with_component() {
        let mut scene = Scene::new();
        let a = scene.spawn().with(Health(1)).id();
        let _no_health = scene.spawn().with(Tag).id();
        let b = scene.spawn().with(Health(2)).id();
        let mut found: Vec<_> = scene.iter::<Health>().map(|(id, h)| (id, h.0)).collect();
        found.sort();
        let mut expected = vec![(a, 1), (b, 2)];
        expected.sort();
        assert_eq!(found, expected);
    }

    #[test]
    fn iter_mut_mutates_in_place() {
        let mut scene = Scene::new();
        let id = scene.spawn().with(Health(1)).id();
        for (_, h) in scene.iter_mut::<Health>() {
            h.0 += 10;
        }
        assert_eq!(scene.get::<Health>(id), Some(&Health(11)));
    }

    #[test]
    fn despawn_clears_all_columns_for_subtree() {
        let mut scene = Scene::new();
        let parent = scene.spawn().with(Health(1)).id();
        let _child = scene.spawn_child(parent).with(Health(2)).with(Tag).id();
        scene.despawn(parent);
        assert_eq!(scene.iter::<Health>().count(), 0);
        assert_eq!(scene.iter::<Tag>().count(), 0);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p astrelis-scene component 2>&1 | tail -10`
Expected: compile error — `Component` trait / `insert` / `with` not found.

- [ ] **Step 3: Implement the `Component` trait and column storage**

In `crates/astrelis-scene/src/component.rs`, insert above the `ComponentColumn` trait:

```rust
use slotmap::SecondaryMap;

/// Marker for data attachable to scene nodes.
///
/// Blanket-implemented for every `Send + Sync + 'static` type — any
/// plain struct works as a component with no derive or registration.
pub trait Component: Send + Sync + 'static {}

impl<T: Send + Sync + 'static> Component for T {}
```

And below the `ComponentColumn` trait, add its implementation:

```rust
impl<T: Component> ComponentColumn for SecondaryMap<NodeId, T> {
    fn remove(&mut self, id: NodeId) {
        SecondaryMap::remove(self, id);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
```

- [ ] **Step 4: Implement the typed accessors on `Scene`**

Append to `crates/astrelis-scene/src/component.rs` (above the tests):

```rust
use std::any::TypeId;

use crate::scene::Scene;

impl Scene {
    fn column<T: Component>(&self) -> Option<&SecondaryMap<NodeId, T>> {
        self.columns
            .get(&TypeId::of::<T>())?
            .as_any()
            .downcast_ref()
    }

    fn column_mut<T: Component>(&mut self) -> Option<&mut SecondaryMap<NodeId, T>> {
        self.columns
            .get_mut(&TypeId::of::<T>())?
            .as_any_mut()
            .downcast_mut()
    }

    /// Attaches `value` to node `id`, replacing and returning any
    /// existing `T`. Returns `None` (without storing) if `id` is stale.
    ///
    /// The column for `T` is created on first insert.
    pub fn insert<T: Component>(&mut self, id: NodeId, value: T) -> Option<T> {
        if !self.nodes.contains_key(id) {
            return None;
        }
        let column = self
            .columns
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(SecondaryMap::<NodeId, T>::new()));
        column
            .as_any_mut()
            .downcast_mut::<SecondaryMap<NodeId, T>>()
            .expect("column type matches TypeId key")
            .insert(id, value)
    }

    /// Node `id`'s `T` component, if the node is live and has one.
    pub fn get<T: Component>(&self, id: NodeId) -> Option<&T> {
        self.column::<T>()?.get(id)
    }

    /// Mutable access to node `id`'s `T` component.
    pub fn get_mut<T: Component>(&mut self, id: NodeId) -> Option<&mut T> {
        self.column_mut::<T>()?.get_mut(id)
    }

    /// Detaches and returns node `id`'s `T` component.
    pub fn remove<T: Component>(&mut self, id: NodeId) -> Option<T> {
        self.column_mut::<T>()?.remove(id)
    }

    /// Iterates all `(NodeId, &T)` pairs — O(number of `T` components),
    /// independent of total node count.
    pub fn iter<T: Component>(&self) -> impl Iterator<Item = (NodeId, &T)> {
        self.column::<T>().into_iter().flatten()
    }

    /// Iterates all `(NodeId, &mut T)` pairs.
    pub fn iter_mut<T: Component>(&mut self) -> impl Iterator<Item = (NodeId, &mut T)> {
        self.column_mut::<T>().into_iter().flatten()
    }
}
```

- [ ] **Step 5: Add `NodeBuilder::with`**

In `crates/astrelis-scene/src/scene.rs`, add to the `impl NodeBuilder<'_>` block (before `id()`):

```rust
    /// Attaches a component to the node.
    pub fn with<T: crate::component::Component>(self, component: T) -> Self {
        self.scene.insert(self.id, component);
        self
    }
```

In `lib.rs`, restore:

```rust
pub use component::Component;
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p astrelis-scene 2>&1 | tail -10`
Expected: `test result: ok. 21 passed` (4 transform + 10 scene + 7 component)

- [ ] **Step 7: Commit**

```bash
git add crates/astrelis-scene
git commit -m "feat(scene): columnar component storage with typed queries"
```

---

### Task 5: Transform & visibility propagation

**Files:**
- Modify: `crates/astrelis-scene/src/scene.rs`
- Create: `crates/astrelis-scene/tests/propagation.rs`

- [ ] **Step 1: Write the failing unit tests**

Add to the `tests` module in `crates/astrelis-scene/src/scene.rs`:

```rust
    #[test]
    fn world_transform_composes_down_the_tree() {
        let mut scene = Scene::new();
        let parent = scene.spawn().position(Vec3::new(10.0, 0.0, 0.0)).id();
        let child = scene
            .spawn_child(parent)
            .position(Vec3::new(0.0, 5.0, 0.0))
            .id();
        scene.flush_transforms();
        let world = scene.world_transform(child).unwrap();
        let origin = world.transform_point3(Vec3::ZERO);
        assert!(origin.abs_diff_eq(Vec3::new(10.0, 5.0, 0.0), 1e-5));
    }

    #[test]
    fn parent_rotation_moves_children() {
        let mut scene = Scene::new();
        let parent = scene.spawn().id();
        let child = scene
            .spawn_child(parent)
            .position(Vec3::new(1.0, 0.0, 0.0))
            .id();
        let mut t = Transform::IDENTITY;
        t.set_rotation_2d(std::f32::consts::FRAC_PI_2);
        scene.set_transform(parent, t);
        scene.flush_transforms();
        let origin = scene
            .world_transform(child)
            .unwrap()
            .transform_point3(Vec3::ZERO);
        assert!(origin.abs_diff_eq(Vec3::new(0.0, 1.0, 0.0), 1e-5));
    }

    #[test]
    fn cache_is_stale_until_flush() {
        let mut scene = Scene::new();
        let id = scene.spawn().id();
        scene.flush_transforms();
        scene.set_position(id, Vec3::new(5.0, 0.0, 0.0));
        // Documented semantics: reads return the cache as of the last pass.
        let before = scene
            .world_transform(id)
            .unwrap()
            .transform_point3(Vec3::ZERO);
        assert!(before.abs_diff_eq(Vec3::ZERO, 1e-6));
        scene.flush_transforms();
        let after = scene
            .world_transform(id)
            .unwrap()
            .transform_point3(Vec3::ZERO);
        assert!(after.abs_diff_eq(Vec3::new(5.0, 0.0, 0.0), 1e-6));
    }

    #[test]
    fn reparenting_keeps_local_changes_world() {
        let mut scene = Scene::new();
        let a = scene.spawn().position(Vec3::new(100.0, 0.0, 0.0)).id();
        let b = scene.spawn().id();
        let child = scene
            .spawn_child(a)
            .position(Vec3::new(1.0, 0.0, 0.0))
            .id();
        scene.flush_transforms();
        scene.set_parent(child, Some(b)).unwrap();
        scene.flush_transforms();
        let origin = scene
            .world_transform(child)
            .unwrap()
            .transform_point3(Vec3::ZERO);
        assert!(origin.abs_diff_eq(Vec3::new(1.0, 0.0, 0.0), 1e-5));
    }

    #[test]
    fn visibility_inherits_down() {
        let mut scene = Scene::new();
        let a = scene.spawn().id();
        let b = scene.spawn_child(a).id();
        let c = scene.spawn_child(b).id();
        scene.flush_transforms();
        assert_eq!(scene.is_world_visible(c), Some(true));
        scene.set_visible(a, false);
        scene.flush_transforms();
        assert_eq!(scene.is_world_visible(b), Some(false));
        assert_eq!(scene.is_world_visible(c), Some(false));
        // Child's own flag is untouched.
        assert_eq!(scene.visible(c), Some(true));
        scene.set_visible(a, true);
        scene.flush_transforms();
        assert_eq!(scene.is_world_visible(c), Some(true));
    }

    #[test]
    fn world_transform_on_stale_id_is_none() {
        let mut scene = Scene::new();
        let id = scene.spawn().id();
        scene.despawn(id);
        assert_eq!(scene.world_transform(id), None);
        assert_eq!(scene.is_world_visible(id), None);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p astrelis-scene scene 2>&1 | tail -10`
Expected: compile error — `flush_transforms` / `world_transform` not found.

- [ ] **Step 3: Implement propagation and world reads**

Add to the `impl Scene` block in `crates/astrelis-scene/src/scene.rs` (after `set_visible`), and add `Mat4` to the existing `astrelis_core::math` import:

```rust
    /// Recomputes cached world transforms and hierarchical visibility.
    ///
    /// Runs once per frame in `Phase::PostUpdate` when using
    /// [`ScenePlugin`](crate::ScenePlugin); call directly for
    /// mid-frame freshness. Nodes whose ancestor chain contains no
    /// dirty node skip the matrix math.
    pub fn flush_transforms(&mut self) {
        astrelis_profiling::profile_function!();
        // (id, parent world, parent world-visible, ancestor dirty)
        let mut stack: Vec<(NodeId, Mat4, bool, bool)> = self
            .roots
            .iter()
            .map(|&r| (r, Mat4::IDENTITY, true, false))
            .collect();
        while let Some((id, parent_world, parent_visible, ancestor_dirty)) = stack.pop() {
            let Some(node) = self.nodes.get_mut(id) else {
                continue;
            };
            let dirty = ancestor_dirty || node.dirty;
            if dirty {
                node.world = parent_world * node.transform.matrix();
                node.world_visible = parent_visible && node.visible;
                node.dirty = false;
            }
            let world = node.world;
            let world_visible = node.world_visible;
            // Descend even when clean: descendants may be dirty.
            for &child in &node.children {
                stack.push((child, world, world_visible, dirty));
            }
        }
    }

    /// A node's cached world matrix, as of the last propagation pass.
    pub fn world_transform(&self, id: NodeId) -> Option<Mat4> {
        self.nodes.get(id).map(|n| n.world)
    }

    /// A node's cached hierarchical visibility (its own flag ANDed with
    /// all ancestors'), as of the last propagation pass.
    pub fn is_world_visible(&self, id: NodeId) -> Option<bool> {
        self.nodes.get(id).map(|n| n.world_visible)
    }
```

- [ ] **Step 4: Run unit tests to verify they pass**

Run: `cargo test -p astrelis-scene scene 2>&1 | tail -10`
Expected: `test result: ok. 16 passed`

- [ ] **Step 5: Write the randomized equivalence test (the load-bearing one)**

Create `crates/astrelis-scene/tests/propagation.rs`:

```rust
//! Dirty-pass propagation must produce results identical to a
//! brute-force recompute, under randomized mutation sequences.

use astrelis_core::math::{Mat4, Quat, Vec3};
use astrelis_scene::{NodeId, Scene, Transform};

/// Deterministic LCG so failures reproduce; no rand dependency.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn pick(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn f32(&mut self) -> f32 {
        (self.next() % 2000) as f32 / 100.0 - 10.0
    }
}

/// Ground truth: walk up the parent chain multiplying local matrices.
fn brute_world(scene: &Scene, id: NodeId) -> Mat4 {
    let local = scene.local_transform(id).unwrap().matrix();
    match scene.parent(id) {
        Some(p) => brute_world(scene, p) * local,
        None => local,
    }
}

/// Ground truth visibility: AND of own flag and all ancestors'.
fn brute_visible(scene: &Scene, id: NodeId) -> bool {
    let own = scene.visible(id).unwrap();
    match scene.parent(id) {
        Some(p) => own && brute_visible(scene, p),
        None => own,
    }
}

fn live_nodes(scene: &Scene) -> Vec<NodeId> {
    let mut out = Vec::new();
    for &root in scene.roots() {
        out.push(root);
        out.extend(scene.descendants(root));
    }
    out
}

#[test]
fn dirty_pass_matches_brute_force_under_random_mutations() {
    for seed in 0..10u64 {
        let mut rng = Lcg(seed.wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(1));
        let mut scene = Scene::new();
        // Seed with a few roots.
        for _ in 0..3 {
            scene.spawn();
        }

        for step in 0..200 {
            let nodes = live_nodes(&scene);
            match rng.pick(6) {
                // Spawn (child of a random node, or a new root).
                0 => {
                    if !nodes.is_empty() && rng.pick(4) != 0 {
                        let parent = nodes[rng.pick(nodes.len())];
                        scene.spawn_child(parent);
                    } else {
                        scene.spawn();
                    }
                }
                // Despawn a random subtree (keep at least one node).
                1 => {
                    if nodes.len() > 1 {
                        scene.despawn(nodes[rng.pick(nodes.len())]);
                    }
                }
                // Random local transform.
                2 => {
                    if !nodes.is_empty() {
                        let id = nodes[rng.pick(nodes.len())];
                        scene.set_transform(
                            id,
                            Transform {
                                position: Vec3::new(rng.f32(), rng.f32(), rng.f32()),
                                rotation: Quat::from_rotation_z(rng.f32()),
                                scale: Vec3::splat(rng.f32().abs().max(0.1)),
                            },
                        );
                    }
                }
                // Toggle visibility.
                3 => {
                    if !nodes.is_empty() {
                        let id = nodes[rng.pick(nodes.len())];
                        let v = scene.visible(id).unwrap();
                        scene.set_visible(id, !v);
                    }
                }
                // Reparent (cycles rejected by the API — ignore errors).
                4 => {
                    if nodes.len() >= 2 {
                        let id = nodes[rng.pick(nodes.len())];
                        let target = if rng.pick(5) == 0 {
                            None
                        } else {
                            Some(nodes[rng.pick(nodes.len())])
                        };
                        let _ = scene.set_parent(id, target);
                    }
                }
                // Flush mid-sequence so the dirty state is exercised
                // across multiple passes, not just one big one.
                _ => scene.flush_transforms(),
            }

            // Every 20 steps: flush and compare everything to ground truth.
            if step % 20 == 19 {
                scene.flush_transforms();
                for id in live_nodes(&scene) {
                    let cached = scene.world_transform(id).unwrap();
                    let truth = brute_world(&scene, id);
                    assert!(
                        cached.abs_diff_eq(truth, 1e-3),
                        "seed {seed} step {step}: world mismatch for {id:?}\ncached: {cached}\ntruth: {truth}"
                    );
                    assert_eq!(
                        scene.is_world_visible(id).unwrap(),
                        brute_visible(&scene, id),
                        "seed {seed} step {step}: visibility mismatch for {id:?}"
                    );
                }
            }
        }
    }
}
```

- [ ] **Step 6: Run the equivalence test**

Run: `cargo test -p astrelis-scene --test propagation 2>&1 | tail -5`
Expected: `test result: ok. 1 passed`

If it fails: the assertion message includes seed and step — reproduce by hard-coding that seed, then bisect the mutation kinds. Do not loosen the epsilon past `1e-3`; accumulated float error at depth stays well under it.

- [ ] **Step 7: Commit**

```bash
git add crates/astrelis-scene
git commit -m "feat(scene): cached transform/visibility propagation with dirty tracking"
```

---

### Task 6: `ScenePlugin`

**Files:**
- Modify: `crates/astrelis-scene/src/plugin.rs`
- Modify: `crates/astrelis-scene/src/lib.rs` (restore re-export)

- [ ] **Step 1: Write the failing test**

Append to `crates/astrelis-scene/src/plugin.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Scene;

    #[test]
    fn plugin_inserts_scene_resource() {
        let mut app = astrelis_app::App::new();
        ScenePlugin.build(&mut app);
        // The scene resource must exist and be usable immediately.
        let mut scene = app.resources().get_mut::<Scene>();
        let id = scene.spawn().id();
        assert!(scene.contains(id));
    }
}
```

**Note:** this test assumes `App` exposes its resources (e.g. `app.resources()`). Check `crates/astrelis-app/src/app.rs` first — if no such accessor exists, this is the one place the plan touches `astrelis-app`: add a doc-commented `pub fn resources(&self) -> &Resources` accessor to `App` (a read accessor is safe to add and useful for any plugin test). If it already exists under another name, use that name in the test.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p astrelis-scene plugin 2>&1 | tail -10`
Expected: compile error — `ScenePlugin` not found.

- [ ] **Step 3: Implement `ScenePlugin`**

Insert into `crates/astrelis-scene/src/plugin.rs` between the module doc and tests:

```rust
use astrelis_app::{App, Phase, Plugin};

use crate::scene::Scene;

/// Registers a [`Scene`] resource and the per-frame propagation pass.
///
/// The pass runs in [`Phase::PostUpdate`]: mutate the scene in
/// `Update` systems, read world transforms/visibility in `Render`
/// systems. For mid-frame freshness call
/// [`Scene::flush_transforms`] directly.
pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Scene::new());
        app.add_system(Phase::PostUpdate, |resources| {
            let mut scene = resources.get_mut::<Scene>();
            scene.flush_transforms();
        });
    }
}
```

In `lib.rs`, restore the final re-export and delete the leftover comment block:

```rust
pub use plugin::ScenePlugin;
```

- [ ] **Step 4: Run all crate tests**

Run: `cargo test -p astrelis-scene 2>&1 | tail -5`
Expected: all tests pass (28 unit + 1 integration).

- [ ] **Step 5: Commit**

```bash
git add crates/astrelis-scene crates/astrelis-app
git commit -m "feat(scene): add ScenePlugin with PostUpdate propagation pass"
```

(Drop `crates/astrelis-app` from the `git add` if Step 1 needed no accessor.)

---

### Task 7: Facade integration

**Files:**
- Modify: `crates/astrelis/Cargo.toml`
- Modify: `crates/astrelis/src/lib.rs`

- [ ] **Step 1: Add the dependency**

In `crates/astrelis/Cargo.toml` `[dependencies]`, alongside the other internal crates:

```toml
astrelis-scene = { workspace = true }
```

- [ ] **Step 2: Add the namespaced re-export**

In `crates/astrelis/src/lib.rs`, with the other `pub use astrelis_* as *;` lines (alphabetical order — after `render_2d`/`profiling`, matching the existing ordering):

```rust
/// Scene tree with columnar component storage.
pub use astrelis_scene as scene;
```

- [ ] **Step 3: Extend the prelude**

In the `prelude` module of `crates/astrelis/src/lib.rs`, add alongside the existing re-exports:

```rust
    pub use astrelis_scene::{Component, NodeId, Scene, SceneError, ScenePlugin, Transform};
```

If any of these names collide with an existing prelude item (check for an existing `Transform` or `Component`), re-export the colliding name only via the `scene::` namespace and leave it out of the prelude — note which in the commit body.

- [ ] **Step 4: Verify the facade builds**

Run: `cargo build -p astrelis 2>&1 | tail -5`
Expected: `Finished` with no errors.

- [ ] **Step 5: Commit**

```bash
git add crates/astrelis
git commit -m "feat: re-export astrelis-scene through the facade crate"
```

---

### Task 8: `scene_demo` example

**Files:**
- Create: `crates/astrelis/examples/scene_demo.rs`

This is the spec's success criterion: scene→renderer glue written entirely in user code, with `astrelis-scene` knowing nothing about rendering. Model the GPU/render boilerplate on `crates/astrelis/examples/game_demo.rs`.

- [ ] **Step 1: Write the example**

Create `crates/astrelis/examples/scene_demo.rs`:

```rust
//! Scene tree demo: a spinning hub with orbiting arms and a nested
//! grandchild, rendered through user-written glue — the scene crate
//! itself knows nothing about rendering.
//!
//! Controls: Space toggles visibility of one arm (its children follow).
//!
//! Run with:
//! ```sh
//! cargo run -p astrelis --example scene_demo
//! ```

use astrelis::prelude::*;

use astrelis::gpu::{Gpu, GpuError, Surface};

/// User-defined drawable component — note: defined HERE, not in the engine.
struct Shape {
    half_size: f32,
    color: Color,
}

/// Marks nodes that spin around their own origin.
struct Spin {
    speed: f32,
}

/// The arm whose visibility Space toggles.
struct ToggleTarget(NodeId);

struct ScenePopulatePlugin;

impl Plugin for ScenePopulatePlugin {
    fn build(&self, app: &mut App) {
        app.add_startup(|resources, _ctx| {
            let mut scene = resources.get_mut::<Scene>();

            // Hub at screen center; spins, dragging its subtree around.
            let hub = scene
                .spawn()
                .name("hub")
                .position(Vec3::new(640.0, 360.0, 0.0))
                .with(Shape { half_size: 30.0, color: Color::new(0.9, 0.3, 0.2, 1.0) })
                .with(Spin { speed: 0.8 })
                .id();

            // Four arms orbiting the hub via parent rotation.
            let mut toggle_arm = None;
            for i in 0..4 {
                let angle = std::f32::consts::FRAC_PI_2 * i as f32;
                let offset = Vec3::new(angle.cos() * 150.0, angle.sin() * 150.0, 0.0);
                let arm = scene
                    .spawn_child(hub)
                    .name(format!("arm{i}"))
                    .position(offset)
                    .with(Shape { half_size: 15.0, color: Color::new(0.2, 0.6, 0.9, 1.0) })
                    .with(Spin { speed: 3.0 })
                    .id();
                // One arm gets a grandchild to show two-level nesting.
                if i == 0 {
                    scene
                        .spawn_child(arm)
                        .name("tip")
                        .position(Vec3::new(50.0, 0.0, 0.0))
                        .with(Shape { half_size: 8.0, color: Color::new(0.3, 0.9, 0.4, 1.0) })
                        .id();
                    toggle_arm = Some(arm);
                }
            }
            let toggle = ToggleTarget(toggle_arm.expect("arm0 created"));
            drop(scene);
            resources.insert(toggle);
        });

        app.add_system(Phase::Update, update_scene);
        app.add_system(Phase::Render, render_scene);
    }
}

fn update_scene(resources: &Resources) {
    let mut scene = resources.get_mut::<Scene>();
    let time = resources.get::<Time>();
    let input = resources.get::<InputState>();

    // Mutating transforms while iterating a column would alias the
    // scene borrow, so collect the targets first (cheap: ids + f32s).
    let spinners: Vec<(NodeId, f32)> =
        scene.iter::<Spin>().map(|(id, s)| (id, s.speed)).collect();
    for (id, speed) in spinners {
        let mut t = *scene.local_transform(id).expect("spinner is live");
        t.set_rotation_2d(time.elapsed_secs() as f32 * speed);
        scene.set_transform(id, t);
    }

    if input.is_key_just_pressed(KeyCode::Space) {
        let target = resources.get::<ToggleTarget>().0;
        let visible = scene.visible(target).expect("toggle target is live");
        scene.set_visible(target, !visible);
    }
}

fn render_scene(resources: &Resources) {
    let gpu = resources.get::<Gpu>();
    let mut surface = resources.get_mut::<Surface>();
    let mut renderer = resources.get_mut::<Renderer2D>();
    let camera = resources.get::<Camera2D>();
    let scene = resources.get::<Scene>();

    let frame = match surface.acquire() {
        Ok(f) => f,
        Err(GpuError::SurfaceOutdated | GpuError::SurfaceLost | GpuError::Timeout) => return,
        Err(e) => panic!("failed to acquire surface: {e}"),
    };

    let mut encoder = gpu.device().create_command_encoder(Some("frame"));

    // Clear pass.
    {
        let _pass = encoder.begin_render_pass(&astrelis::gpu::command::RenderPassDescriptor {
            label: Some("clear"),
            color_attachments: &[astrelis::gpu::command::ColorAttachment {
                view: frame.view(),
                resolve_target: None,
                load_op: astrelis::gpu::types::LoadOp::Clear(Color::new(0.08, 0.08, 0.12, 1.0)),
                store_op: astrelis::gpu::types::StoreOp::Store,
            }],
            depth_stencil_attachment: None,
        });
    }

    renderer.begin(&camera);

    // The glue: world transform from the scene, draw call to the renderer.
    for (id, shape) in scene.iter::<Shape>() {
        if scene.is_world_visible(id) != Some(true) {
            continue;
        }
        let world = scene.world_transform(id).expect("shape node is live");
        let pos = world.transform_point3(Vec3::ZERO);
        renderer.draw_rect_filled(
            Vec2::new(pos.x - shape.half_size, pos.y - shape.half_size),
            Vec2::splat(shape.half_size * 2.0),
            shape.color,
        );
    }

    renderer.end(&gpu, &mut encoder, frame.view(), &camera);

    gpu.submit(std::iter::once(encoder));
    frame.present();
}

/// 2D renderer setup, mirroring game_demo.
struct Render2DSetupPlugin;

impl Plugin for Render2DSetupPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup(|resources, _ctx| {
            let gpu = resources.get::<Gpu>();
            let surface = resources.get::<Surface>();
            let format = surface.preferred_format();
            let renderer = Renderer2D::new(&gpu, format);
            let camera = Camera2D::new(1280, 720);

            drop(gpu);
            drop(surface);

            resources.insert(renderer);
            resources.insert(camera);
        });
    }
}

fn main() {
    astrelis::core::logging::init_default();
    App::new()
        .add_default_plugins()
        .add_plugin(ScenePlugin)
        .add_plugin(Render2DSetupPlugin)
        .add_plugin(ScenePopulatePlugin)
        .run();
}
```

**API checks while writing** (adjust to what actually exists rather than forcing this code):
- `InputState`: confirm the just-pressed query name with `grep -n "just" crates/astrelis-input/src/*.rs` — if it's `is_key_just_pressed`, use as-is; otherwise use the actual name (plain `is_key_pressed` + a debounce bool resource as fallback).
- `NodeBuilder::name` takes `impl Into<String>`, so `format!` works.
- Startup ordering: `ScenePlugin` is added before `ScenePopulatePlugin`, and `insert_resource` runs at build time, so the `Scene` resource exists when the populate startup runs.

- [ ] **Step 2: Build the example**

Run: `cargo build -p astrelis --example scene_demo 2>&1 | tail -5`
Expected: `Finished` with no errors.

- [ ] **Step 3: Run the example briefly (visual smoke test)**

Run: `cargo run -p astrelis --example scene_demo` (let the user observe, or run for a few seconds and close).
Expected: a red hub square at screen center with four blue squares orbiting it; one arm carries a small green square orbiting the arm (two-level nesting). Space hides/shows that arm *and* its green child.

- [ ] **Step 4: Commit**

```bash
git add crates/astrelis/examples/scene_demo.rs
git commit -m "feat(examples): add scene_demo with hierarchy, spin, and visibility toggle"
```

---

### Task 9: Workspace verification

**Files:** none (verification only)

- [ ] **Step 1: Full workspace build + tests**

Run: `cargo build --workspace 2>&1 | tail -5 && cargo test --workspace 2>&1 | tail -10`
Expected: everything compiles; all tests pass, including the 29 in `astrelis-scene`.

- [ ] **Step 2: Clippy**

Run: `cargo clippy -p astrelis-scene 2>&1 | tail -15`
Expected: no warnings. Fix any that appear (workspace lints treat clippy `all` as warn; keep the crate clean).

- [ ] **Step 3: Docs build**

Run: `cargo doc -p astrelis-scene --no-deps 2>&1 | tail -5`
Expected: no `missing_docs` warnings.

- [ ] **Step 4: Final commit if fixes were needed**

```bash
git status --short
# Only if Steps 1-3 required changes:
git add crates/astrelis-scene
git commit -m "fix(scene): address clippy and doc warnings"
```
