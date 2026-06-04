# Astrelis Scene Foundation (`astrelis-scene`) — Design

**Date:** 2026-06-04
**Status:** Approved design, pending implementation plan

## Context & Roadmap

Astrelis `main` (v0.3.0) has solid layer-0–2 foundations (core, profiling,
window, gpu, render-2d, text, assets, input) and a plugin-based app
framework, but nothing above "draw things this frame": no spatial
hierarchy, no entity organization, no 3D, no engine-native UI. The
`legacy` branch had UI/scene/audio attempts that accumulated tech debt
(god crates, bidirectional render↔UI coupling, trait soup); the rewrite
deliberately dropped them.

Agreed roadmap toward "can build a game", each step its own
spec → plan → implementation cycle:

1. **Scene foundation** (`astrelis-scene`) — this document.
2. **Unlit 3D** (`astrelis-render-3d`) — meshes, 3D camera, depth,
   unlit materials, model loading; first real consumer of the scene tree.
3. **UI framework** (`astrelis-ui`) — retained, scene-tree-native
   Control-style nodes; draws via render-2d + text-wgpu.
4. **Audio** (`astrelis-audio`), **Networking** (`astrelis-net`) — later,
   designed on the scene/plugin foundation.

Anti-debt rules carried through all steps:

- One concern per crate; renderers (layer 2) never know about the scene
  (layer 3). Scene→renderer glue is one-directional and lives downstream.
- No trait-object node hierarchies; concrete types, open component set.
- Third-party layout/physics/etc. stay leaf dependencies, never
  architectural boundaries.

## Decisions (with rationale)

| Decision | Choice | Why |
|---|---|---|
| Entity model | Scene tree + components (not ECS) | Simpler to build/reason about; fits UI and transforms naturally; columnar storage keeps an ECS upgrade path open |
| Tree storage | Arena (`slotmap`) + generational `NodeId` handles | Idiomatic Rust, no `Rc<RefCell>` borrow panics, serialization-friendly, cache-friendly |
| Node typing | One generic `Node` + component bag (Unity-style) | Uniform arena storage, no `Box<dyn Node>` downcasting (the legacy trait-soup pattern) |
| Component storage | Columnar: one `SecondaryMap<NodeId, T>` per type | Queries are O(#components-of-T) not O(#nodes); one downcast per column access, not per node |
| Transforms | Unified 3D TRS (`Vec3`/`Quat`/`Vec3`) on every node | One propagation pass, no 2D/3D mixed-tree edge cases; 2D uses x/y + z-order; UI layout rects are a separate concern |
| Game logic | App systems query the scene; nodes are pure data | Rides the existing phase/plugin model; avoids `&mut Scene` reentrancy inside per-node scripts and the command-buffer machinery it forces |
| 3D ambition (near-term) | Unlit/debug first | Quickest path to validating scene transforms + camera; lighting design deferred |

## Crate

`astrelis-scene`, layer 3.

Dependencies: `astrelis-core` (math), `astrelis-app` (plugin
integration), `slotmap` (new `[workspace.dependencies]` entry).
**No renderer or GPU dependencies.** Renderer glue (e.g. a `Sprite`
component plus a Render-phase system calling `Renderer2D`) lives in
downstream crates or game code; its exact home is decided in roadmap
steps 2 and 3.

## Data model

```rust
new_key_type! { pub struct NodeId; }          // generational, Copy

pub struct Transform {                        // local, relative to parent
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}
// + 2D conveniences: Transform::from_xy(x, y), set_rotation_2d(angle), …

struct Node {                                 // PRIVATE — all access via Scene
    name: Option<String>,
    transform: Transform,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    visible: bool,
    // engine-maintained caches:
    world: Mat4,
    world_visible: bool,
    transform_dirty: bool,
}

pub struct Scene {
    nodes: SlotMap<NodeId, Node>,
    roots: Vec<NodeId>,                       // forest — no hidden root node
    columns: HashMap<TypeId, Box<dyn ComponentColumn>>,
}

pub trait Component: 'static + Send + Sync {} // blanket impl
```

`Node` is private so every mutation flows through `Scene` methods —
that is what keeps dirty flags and column cleanup correct. Legacy's UI
bugs largely came from state mutable from two places.

## Public API

Tree construction and mutation:

```rust
let player = scene.spawn()                    // root-level
    .name("player")
    .position(Vec3::new(0.0, 1.0, 0.0))
    .with(Sprite { /* … */ })                 // any T: Component
    .id();

let gun = scene.spawn_child(player).name("gun").id();

scene.set_parent(gun, other)?;                // Err(SceneError::WouldCycle)
scene.despawn(player);                        // recursive; clears every column
```

Components and queries:

```rust
scene.insert(id, Velocity(v));                // column auto-created on first insert
scene.get::<Sprite>(id)      -> Option<&Sprite>;   // None on stale id / missing
scene.get_mut::<Sprite>(id)  -> Option<&mut Sprite>;
scene.remove::<Sprite>(id)   -> Option<Sprite>;

scene.iter::<Sprite>()       -> impl Iterator<Item = (NodeId, &Sprite)>;
scene.iter_mut::<Sprite>()   -> …;
```

Traversal: `children(id)`, `parent(id)`, `roots()`, `descendants(id)`.

Transform/visibility reads: `world_transform(id) -> Option<Mat4>`,
`is_world_visible(id) -> Option<bool>`, `local_transform(id)` /
`set_transform(id, t)` and field-level setters.

API conventions: `Option`-returning everywhere (stale generational IDs
are normal, not a bug); no panicking shortcut variants — `.unwrap()` at
the call site is explicit enough.

## Transform & visibility propagation

- Setting a local transform (or `visible`) marks only that node dirty —
  O(1) writes; reparenting marks the moved node dirty.
- One propagation pass per frame (registered by `ScenePlugin` in
  **PostUpdate**): depth-first over the forest, recomputes `world` and
  `world_visible` for any node whose ancestor chain contains a dirty
  node; clean subtrees are skipped. `world_visible =
  parent.world_visible && self.visible`, computed in the same pass.
- Read contract: `world_transform`/`is_world_visible` return the cache
  **as of the last pass**. Mutate in Update, read world-state in
  Render — the existing phase order enforces this. For rare mid-frame
  needs, `scene.flush_transforms()` forces a pass.
- Rationale: recompute-on-read gives always-fresh values but
  unpredictable per-read cost and subtle invalidation bugs. One
  deterministic pass per frame profiles as a single span and is easier
  to get right.

## App integration

`ScenePlugin`:

- Inserts `Scene` into `Resources`.
- Registers the propagation pass in PostUpdate.

Game logic is ordinary phase systems:

```rust
app.add_system(Phase::Update, |res| {
    let mut scene = res.get_mut::<Scene>();
    for (id, vel) in scene.iter_mut::<Velocity>() { /* … */ }
});
```

`Scene` is a plain value — tests construct one directly with no app,
window, or GPU.

## Error handling

- `SceneError` with a single v1 variant: `WouldCycle` (from
  `set_parent`).
- Everything else is `Option` (stale IDs, missing components).
- Despawn-during-iteration is prevented structurally: `iter()` borrows
  `&Scene`; structural mutation needs `&mut Scene`. No command queue in
  v1 — it only becomes necessary with behavior components, which are
  deferred.

## Testing

All headless `cargo test` (no GPU/window deps):

- **Hierarchy:** reparent cycle rejection; recursive despawn clears all
  columns; stale-ID access returns `None`; forest root bookkeeping
  across spawn/despawn/reparent.
- **Transforms:** nested TRS correctness against hand-computed
  matrices; **dirty-pass equivalence vs brute-force full recompute under
  randomized mutation sequences** (the load-bearing test); visibility
  inheritance.
- **Components:** column auto-creation; `iter` contents; insert
  overwrites existing.

## Success criteria

A `scene_demo` example: a moving parent node with orbiting children,
rendered via `astrelis-render-2d` through a **user-written** Render
system — demonstrating scene→renderer glue works without `astrelis-scene`
referencing any renderer.

## Deferred (designed-for, not built)

- **Scene serialization / scene files** — columns are per-type and
  serializable later; `NodeId`s never persist.
- **Lifecycle events** (`NodeSpawned`/`NodeDespawned` via `Events<T>`) —
  added when UI needs enter/exit-tree notification.
- **Behavior components + command queue** — only if systems-style logic
  proves insufficient.
- **Parallel queries** — columnar storage permits it later; unnecessary
  at current scale.
