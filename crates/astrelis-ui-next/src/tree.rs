//! Retained arena, incremental passes, fragments, and hit testing.

use std::{any::Any, collections::HashSet, marker::PhantomData, sync::Arc};

use astrelis_core::{
    geometry::{LogicalPoint, LogicalRect, LogicalSize},
    math::{Affine2, Vec2},
};
use astrelis_paint::{DisplayList, DisplayListInstance, Painter};
use astrelis_text::{FontDatabase, TextLayout, TextLayoutContext, TextLayoutRequest};

use crate::{
    AccessibilityUpdate, Constraints, Element, Invalidation, LayoutContext, SemanticNode, UiError,
    UiInput,
};

/// Stable generational retained identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId {
    index: u32,
    generation: u32,
}

/// Typed retained identity.
pub struct NodeHandle<E> {
    id: NodeId,
    marker: PhantomData<fn() -> E>,
}

impl<E> NodeHandle<E> {
    /// Returns the kind-erased retained identity.
    pub const fn id(self) -> NodeId {
        self.id
    }
}

impl<E> Clone for NodeHandle<E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<E> Copy for NodeHandle<E> {}

impl<E> std::fmt::Debug for NodeHandle<E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_tuple("NodeHandle").field(&self.id).finish()
    }
}

/// Per-update diagnostic counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PassStats {
    /// Elements whose layout method ran.
    pub layout_elements: usize,
    /// Nodes whose composition metadata changed.
    pub composed_nodes: usize,
    /// Paint fragments rebuilt.
    pub rebuilt_fragments: usize,
    /// Paint fragments reused.
    pub reused_fragments: usize,
    /// Nodes visited by the most recent hit test.
    pub hit_test_nodes: usize,
    /// Accessibility nodes emitted in the delta.
    pub accessibility_nodes: usize,
    /// Text layouts shaped during the update.
    pub shaped_text: usize,
}

/// Cached fragment scene for one window.
#[derive(Clone, Debug, Default)]
pub struct Scene {
    fragments: Vec<DisplayListInstance>,
    rebuilt: HashSet<NodeId>,
}

impl Scene {
    /// Fragment instances in paint order.
    pub fn fragments(&self) -> &[DisplayListInstance] {
        &self.fragments
    }

    /// Returns whether a node's local fragment was rebuilt this update.
    pub fn rebuilt(&self, id: NodeId) -> bool {
        self.rebuilt.contains(&id)
    }

    /// Flattens the cached fragments for today's display-list renderer.
    pub fn flatten(&self) -> Result<DisplayList, astrelis_paint::PaintError> {
        DisplayList::compose(self.fragments.clone())
    }
}

struct Node {
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    element: Option<Box<dyn Element>>,
    offset: LogicalPoint,
    size: LogicalSize,
    last_constraints: Option<Constraints>,
    world_transform: Affine2,
    world_bounds: LogicalRect,
    subtree_bounds: LogicalRect,
    world_clip: Option<LogicalRect>,
    visible: bool,
    enabled: bool,
    own_dirty: Invalidation,
    subtree_dirty: Invalidation,
    fragment: Option<Arc<DisplayList>>,
    semantic: Option<SemanticNode>,
}

struct Slot {
    generation: u32,
    node: Option<Node>,
}

/// Incremental retained tree for one viewport.
pub struct UiRoot {
    slots: Vec<Slot>,
    free: Vec<u32>,
    root: NodeId,
    viewport: LogicalSize,
    dirty: Invalidation,
    removed_semantics: Vec<NodeId>,
    focus: Option<NodeId>,
    scene: Scene,
    accessibility: AccessibilityUpdate,
    stats: PassStats,
    fonts: FontDatabase,
    text_context: TextLayoutContext,
}

impl UiRoot {
    /// Creates a retained tree with `root` as its root element.
    pub fn new(root: impl Element, viewport: LogicalSize) -> Self {
        Self::with_fonts(root, viewport, FontDatabase::default())
    }

    /// Creates a retained tree using an application-provided font database.
    pub fn with_fonts(root: impl Element, viewport: LogicalSize, fonts: FontDatabase) -> Self {
        let root_id = NodeId {
            index: 0,
            generation: 1,
        };
        let node = Node {
            parent: None,
            children: Vec::new(),
            element: Some(Box::new(root)),
            offset: LogicalPoint::ZERO,
            size: LogicalSize::ZERO,
            last_constraints: None,
            world_transform: Affine2::IDENTITY,
            world_bounds: LogicalRect::default(),
            subtree_bounds: LogicalRect::default(),
            world_clip: None,
            visible: true,
            enabled: true,
            own_dirty: Invalidation::ALL,
            subtree_dirty: Invalidation::ALL,
            fragment: None,
            semantic: None,
        };
        Self {
            slots: vec![Slot {
                generation: 1,
                node: Some(node),
            }],
            free: Vec::new(),
            root: root_id,
            viewport,
            dirty: Invalidation::ALL,
            removed_semantics: Vec::new(),
            focus: None,
            scene: Scene::default(),
            accessibility: AccessibilityUpdate::default(),
            stats: PassStats::default(),
            fonts,
            text_context: TextLayoutContext::new(),
        }
    }

    /// Returns the root identity.
    pub const fn root(&self) -> NodeId {
        self.root
    }

    /// Returns the logical viewport.
    pub const fn viewport(&self) -> LogicalSize {
        self.viewport
    }

    /// Changes the viewport.
    pub fn set_viewport(&mut self, viewport: LogicalSize) {
        if self.viewport != viewport {
            self.viewport = viewport;
            self.invalidate(self.root, Invalidation::LAYOUT_ALL);
        }
    }

    /// Appends one retained child.
    pub fn append<E: Element>(
        &mut self,
        parent: NodeId,
        element: E,
    ) -> Result<NodeHandle<E>, UiError> {
        self.node(parent)?;
        let id = self.allocate(Node {
            parent: Some(parent),
            children: Vec::new(),
            element: Some(Box::new(element)),
            offset: LogicalPoint::ZERO,
            size: LogicalSize::ZERO,
            last_constraints: None,
            world_transform: Affine2::IDENTITY,
            world_bounds: LogicalRect::default(),
            subtree_bounds: LogicalRect::default(),
            world_clip: None,
            visible: true,
            enabled: true,
            own_dirty: Invalidation::ALL,
            subtree_dirty: Invalidation::ALL,
            fragment: None,
            semantic: None,
        });
        self.node_mut(parent)?.children.push(id);
        self.invalidate(parent, Invalidation::ALL);
        Ok(NodeHandle {
            id,
            marker: PhantomData,
        })
    }

    /// Removes one subtree.
    pub fn remove(&mut self, id: NodeId) -> Result<(), UiError> {
        if id == self.root {
            return Err(UiError::new("the retained root cannot be removed"));
        }
        let parent = self.node(id)?.parent;
        if let Some(parent) = parent {
            self.node_mut(parent)?.children.retain(|child| *child != id);
        }
        self.remove_subtree(id);
        if let Some(parent) = parent {
            self.invalidate(parent, Invalidation::ALL);
        }
        Ok(())
    }

    /// Reorders a parent's children without recreating them.
    pub fn set_children(&mut self, parent: NodeId, children: &[NodeId]) -> Result<(), UiError> {
        let old = self.node(parent)?.children.clone();
        if old == children {
            return Ok(());
        }
        let unique = children.iter().copied().collect::<HashSet<_>>();
        if unique.len() != children.len() {
            return Err(UiError::new("duplicate retained child"));
        }
        for child in children {
            let node = self.node(*child)?;
            if node.parent != Some(parent) {
                return Err(UiError::new("set_children received a non-child identity"));
            }
        }
        self.node_mut(parent)?.children = children.to_vec();
        self.invalidate(parent, Invalidation::ALL);
        Ok(())
    }

    /// Mutates a typed element and requests the declared passes.
    pub fn update<E: Element>(
        &mut self,
        handle: NodeHandle<E>,
        invalidation: Invalidation,
        update: impl FnOnce(&mut E),
    ) -> Result<(), UiError> {
        let element = self
            .node_mut(handle.id)?
            .element
            .as_deref_mut()
            .and_then(|element| element.as_any_mut().downcast_mut::<E>())
            .ok_or_else(|| UiError::new("retained handle has the wrong element type"))?;
        update(element);
        self.invalidate(handle.id, invalidation);
        Ok(())
    }

    /// Reads a typed retained element.
    pub fn element<E: Element>(&self, handle: NodeHandle<E>) -> Result<&E, UiError> {
        self.node(handle.id)?
            .element
            .as_deref()
            .and_then(|element| element.as_any().downcast_ref::<E>())
            .ok_or_else(|| UiError::new("retained handle has the wrong element type"))
    }

    /// Returns whether an identity is still live.
    pub fn contains(&self, id: NodeId) -> bool {
        self.node(id).is_ok()
    }

    /// Sets retained visibility.
    pub fn set_visible(&mut self, id: NodeId, visible: bool) -> Result<(), UiError> {
        if self.node(id)?.visible != visible {
            self.node_mut(id)?.visible = visible;
            self.invalidate(id, Invalidation::ALL);
        }
        Ok(())
    }

    /// Sets effective interaction enablement.
    pub fn set_enabled(&mut self, id: NodeId, enabled: bool) -> Result<(), UiError> {
        if self.node(id)?.enabled != enabled {
            self.node_mut(id)?.enabled = enabled;
            self.invalidate(
                id,
                Invalidation::PAINT | Invalidation::ACCESSIBILITY | Invalidation::HIT_TEST,
            );
        }
        Ok(())
    }

    /// Runs invalidated passes and returns cached output.
    pub fn update_passes(&mut self) -> Result<FrameUpdate<'_>, UiError> {
        self.stats = PassStats::default();
        self.accessibility = AccessibilityUpdate {
            changed: Vec::new(),
            removed: std::mem::take(&mut self.removed_semantics),
        };
        if self.dirty.contains(Invalidation::LAYOUT) {
            self.layout_node(self.root, Constraints::tight(self.viewport))?;
            self.dirty.insert(
                Invalidation::COMPOSE
                    | Invalidation::PAINT
                    | Invalidation::ACCESSIBILITY
                    | Invalidation::HIT_TEST,
            );
        }
        if self
            .dirty
            .intersects(Invalidation::COMPOSE | Invalidation::HIT_TEST)
        {
            self.compose_node(self.root, Affine2::IDENTITY, None)?;
        }
        if self.dirty.contains(Invalidation::PAINT) {
            self.rebuild_scene()?;
        } else {
            self.stats.reused_fragments = self.scene.fragments.len();
        }
        if self.dirty.contains(Invalidation::ACCESSIBILITY) {
            self.update_accessibility()?;
        }
        self.dirty = Invalidation::empty();
        Ok(FrameUpdate {
            scene: &self.scene,
            accessibility: &self.accessibility,
            stats: self.stats,
        })
    }

    /// Returns the most recently produced scene.
    pub const fn scene(&self) -> &Scene {
        &self.scene
    }

    /// Returns a full deterministic semantic snapshot.
    pub fn semantic_snapshot(&self) -> Vec<SemanticNode> {
        self.live_ids()
            .filter_map(|id| self.node(id).ok()?.semantic.clone())
            .collect()
    }

    /// Hit-tests a window-space point and records traversal work.
    pub fn hit_test(&mut self, point: LogicalPoint) -> Option<NodeId> {
        let mut visited = 0;
        let hit = self.hit_test_node(self.root, point, true, &mut visited);
        self.stats.hit_test_nodes = visited;
        hit
    }

    /// Delivers pointer input and returns a typed erased payload.
    pub fn dispatch(&mut self, input: UiInput) -> Result<Option<Box<dyn Any>>, UiError> {
        let point = match &input {
            UiInput::PointerMoved(point)
            | UiInput::PointerPressed(point)
            | UiInput::PointerReleased(point) => *point,
            UiInput::FocusChanged(_) | UiInput::Keyboard { .. } | UiInput::Ime(_) => {
                let Some(focus) = self.focus else {
                    return Ok(None);
                };
                return self.dispatch_to(focus, input);
            }
        };
        let Some(target) = self.hit_test(point) else {
            return Ok(None);
        };
        if matches!(input, UiInput::PointerPressed(_))
            && self
                .node(target)?
                .element
                .as_deref()
                .is_some_and(Element::focusable)
            && self.focus != Some(target)
        {
            if let Some(old) = self.focus {
                let _ = self.dispatch_to(old, UiInput::FocusChanged(false))?;
            }
            self.focus = Some(target);
            let _ = self.dispatch_to(target, UiInput::FocusChanged(true))?;
            self.invalidate(target, Invalidation::ACCESSIBILITY);
        }
        self.dispatch_to(target, input)
    }

    fn dispatch_to(
        &mut self,
        target: NodeId,
        input: UiInput,
    ) -> Result<Option<Box<dyn Any>>, UiError> {
        let input = self.localize_input(target, input)?;
        let element = self
            .node_mut(target)?
            .element
            .as_deref_mut()
            .ok_or_else(|| UiError::new("element is temporarily unavailable"))?;
        let result = element.event(input);
        if !result.invalidation.is_empty() {
            self.invalidate(target, result.invalidation);
        }
        Ok(result.action)
    }

    fn localize_input(&self, target: NodeId, input: UiInput) -> Result<UiInput, UiError> {
        let inverse = self.node(target)?.world_transform.inverse();
        let local = |point: LogicalPoint| {
            let point = inverse.transform_point2(Vec2::new(point.x, point.y));
            LogicalPoint::new(point.x, point.y)
        };
        Ok(match input {
            UiInput::PointerMoved(point) => UiInput::PointerMoved(local(point)),
            UiInput::PointerPressed(point) => UiInput::PointerPressed(local(point)),
            UiInput::PointerReleased(point) => UiInput::PointerReleased(local(point)),
            other => other,
        })
    }

    pub(crate) fn shape_text(&mut self, request: TextLayoutRequest) -> Result<TextLayout, UiError> {
        let layout = self
            .text_context
            .layout(&mut self.fonts, request)
            .map_err(|error| UiError::new(error.to_string()))?;
        self.stats.shaped_text += 1;
        Ok(layout)
    }

    pub(crate) fn children_ids(&self, id: NodeId) -> Vec<NodeId> {
        self.node(id)
            .map(|node| node.children.clone())
            .unwrap_or_default()
    }

    pub(crate) fn layout_child(
        &mut self,
        parent: NodeId,
        child: NodeId,
        constraints: Constraints,
    ) -> Result<LogicalSize, UiError> {
        if self.node(child)?.parent != Some(parent) {
            return Err(UiError::new("layout_child requires a direct child"));
        }
        self.layout_node(child, constraints)
    }

    pub(crate) fn place_child(
        &mut self,
        parent: NodeId,
        child: NodeId,
        origin: LogicalPoint,
    ) -> Result<(), UiError> {
        if self.node(child)?.parent != Some(parent) {
            return Err(UiError::new("place_child requires a direct child"));
        }
        if self.node(child)?.offset != origin {
            self.node_mut(child)?.offset = origin;
            self.invalidate(child, Invalidation::COMPOSE | Invalidation::ACCESSIBILITY);
        }
        Ok(())
    }

    pub(crate) fn child_size(&self, parent: NodeId, child: NodeId) -> Result<LogicalSize, UiError> {
        if self.node(child)?.parent != Some(parent) {
            return Err(UiError::new("child_size requires a direct child"));
        }
        Ok(self.node(child)?.size)
    }

    fn layout_node(
        &mut self,
        id: NodeId,
        constraints: Constraints,
    ) -> Result<LogicalSize, UiError> {
        let needs_layout = {
            let node = self.node(id)?;
            node.own_dirty.contains(Invalidation::LAYOUT)
                || node.subtree_dirty.contains(Invalidation::LAYOUT)
                || node.last_constraints != Some(constraints)
        };
        if !needs_layout {
            return Ok(self.node(id)?.size);
        }
        let mut element = self
            .node_mut(id)?
            .element
            .take()
            .ok_or_else(|| UiError::new("recursive layout of the same element"))?;
        let size = element.layout(
            &mut LayoutContext {
                ui: self,
                current: id,
            },
            constraints,
        )?;
        let size = constraints.constrain(size);
        self.stats.layout_elements += 1;
        let node = self.node_mut(id)?;
        let size_changed = node.size != size;
        node.size = size;
        node.last_constraints = Some(constraints);
        node.element = Some(element);
        node.own_dirty.remove(Invalidation::LAYOUT);
        node.subtree_dirty.remove(Invalidation::LAYOUT);
        if size_changed {
            node.own_dirty.insert(
                Invalidation::COMPOSE
                    | Invalidation::PAINT
                    | Invalidation::ACCESSIBILITY
                    | Invalidation::HIT_TEST,
            );
            self.dirty.insert(
                Invalidation::COMPOSE
                    | Invalidation::PAINT
                    | Invalidation::ACCESSIBILITY
                    | Invalidation::HIT_TEST,
            );
        }
        Ok(size)
    }

    fn compose_node(
        &mut self,
        id: NodeId,
        parent_transform: Affine2,
        parent_clip: Option<LogicalRect>,
    ) -> Result<LogicalRect, UiError> {
        let (offset, size, transform, clips, visible, children) = {
            let node = self.node(id)?;
            let element = node
                .element
                .as_deref()
                .ok_or_else(|| UiError::new("element is temporarily unavailable"))?;
            (
                node.offset,
                node.size,
                element.transform(),
                element.clips_children(),
                node.visible,
                node.children.clone(),
            )
        };
        if !visible {
            let node = self.node_mut(id)?;
            node.world_bounds = LogicalRect::default();
            node.subtree_bounds = LogicalRect::default();
            return Ok(LogicalRect::default());
        }
        let world =
            parent_transform * Affine2::from_translation(Vec2::new(offset.x, offset.y)) * transform;
        let local_bounds = LogicalRect::from_xywh(0.0, 0.0, size.width, size.height);
        let bounds = transform_rect(world, local_bounds);
        let clip = if clips {
            Some(intersect_rect(parent_clip, Some(bounds)).unwrap_or_default())
        } else {
            parent_clip
        };
        let mut subtree = bounds;
        for child in children {
            let child_bounds = self.compose_node(child, world, clip)?;
            subtree = union_rect(subtree, child_bounds);
        }
        let node = self.node_mut(id)?;
        let changed = node.world_transform != world
            || node.world_bounds != bounds
            || node.subtree_bounds != subtree
            || node.world_clip != clip;
        node.world_transform = world;
        node.world_bounds = bounds;
        node.subtree_bounds = subtree;
        node.world_clip = clip;
        node.own_dirty
            .remove(Invalidation::COMPOSE | Invalidation::HIT_TEST);
        node.subtree_dirty
            .remove(Invalidation::COMPOSE | Invalidation::HIT_TEST);
        if changed {
            node.own_dirty.insert(Invalidation::ACCESSIBILITY);
            self.dirty.insert(Invalidation::ACCESSIBILITY);
            self.stats.composed_nodes += 1;
        }
        Ok(subtree)
    }

    fn rebuild_scene(&mut self) -> Result<(), UiError> {
        let mut scene = Scene::default();
        self.collect_fragments(self.root, &mut scene)?;
        self.scene = scene;
        Ok(())
    }

    fn collect_fragments(&mut self, id: NodeId, scene: &mut Scene) -> Result<(), UiError> {
        let (visible, dirty, size, world, clip, children) = {
            let node = self.node(id)?;
            (
                node.visible,
                node.own_dirty.contains(Invalidation::PAINT),
                node.size,
                node.world_transform,
                node.world_clip,
                node.children.clone(),
            )
        };
        if !visible {
            return Ok(());
        }
        if dirty || self.node(id)?.fragment.is_none() {
            let mut painter = Painter::new();
            self.node(id)?
                .element
                .as_deref()
                .ok_or_else(|| UiError::new("element is temporarily unavailable"))?
                .paint(&mut painter, size)?;
            let fragment = Arc::new(painter.finish()?);
            let node = self.node_mut(id)?;
            node.fragment = Some(fragment);
            node.own_dirty.remove(Invalidation::PAINT);
            scene.rebuilt.insert(id);
            self.stats.rebuilt_fragments += 1;
        } else {
            self.stats.reused_fragments += 1;
        }
        let fragment = self
            .node(id)?
            .fragment
            .as_deref()
            .expect("fragment was ensured")
            .clone();
        scene.fragments.push(DisplayListInstance {
            list: fragment,
            transform: world,
            clip,
            opacity: 1.0,
        });
        for child in children {
            self.collect_fragments(child, scene)?;
        }
        self.node_mut(id)?.subtree_dirty.remove(Invalidation::PAINT);
        Ok(())
    }

    fn update_accessibility(&mut self) -> Result<(), UiError> {
        let ids = self.live_ids().collect::<Vec<_>>();
        for id in ids {
            let dirty = {
                let node = self.node(id)?;
                node.own_dirty.contains(Invalidation::ACCESSIBILITY) || node.semantic.is_none()
            };
            if !dirty {
                continue;
            }
            let (parent, bounds, data, focusable, visible) = {
                let node = self.node(id)?;
                let element = node
                    .element
                    .as_deref()
                    .ok_or_else(|| UiError::new("element is temporarily unavailable"))?;
                (
                    node.parent,
                    node.world_bounds,
                    element.accessibility(),
                    element.focusable(),
                    node.visible,
                )
            };
            let semantic = data.filter(|_| visible).map(|data| SemanticNode {
                id,
                parent,
                bounds,
                data,
                focusable,
                focused: self.focus == Some(id),
            });
            let old = self.node(id)?.semantic.clone();
            if old != semantic {
                if let Some(semantic) = semantic.clone() {
                    self.accessibility.changed.push(semantic);
                    self.stats.accessibility_nodes += 1;
                } else if old.is_some() {
                    self.accessibility.removed.push(id);
                }
            }
            let node = self.node_mut(id)?;
            node.semantic = semantic;
            node.own_dirty.remove(Invalidation::ACCESSIBILITY);
        }
        for id in self.live_ids().collect::<Vec<_>>() {
            self.node_mut(id)?
                .subtree_dirty
                .remove(Invalidation::ACCESSIBILITY);
        }
        Ok(())
    }

    fn hit_test_node(
        &self,
        id: NodeId,
        point: LogicalPoint,
        ancestors_enabled: bool,
        visited: &mut usize,
    ) -> Option<NodeId> {
        *visited += 1;
        let node = self.node(id).ok()?;
        if !node.visible
            || !ancestors_enabled
            || !node.subtree_bounds.contains(point)
            || node.world_clip.is_some_and(|clip| !clip.contains(point))
        {
            return None;
        }
        for child in node.children.iter().rev() {
            if let Some(hit) = self.hit_test_node(*child, point, node.enabled, visited) {
                return Some(hit);
            }
        }
        let inverse = node.world_transform.inverse();
        let local = inverse.transform_point2(Vec2::new(point.x, point.y));
        let point = LogicalPoint::new(local.x, local.y);
        let element = node.element.as_deref()?;
        (node.enabled && element.hit_testable() && element.hit_test(point, node.size)).then_some(id)
    }

    fn invalidate(&mut self, id: NodeId, invalidation: Invalidation) {
        if self.node(id).is_err() || invalidation.is_empty() {
            return;
        }
        let expanded = if invalidation.contains(Invalidation::TREE) {
            invalidation | Invalidation::LAYOUT_ALL
        } else if invalidation.contains(Invalidation::LAYOUT) {
            invalidation
                | Invalidation::COMPOSE
                | Invalidation::PAINT
                | Invalidation::ACCESSIBILITY
                | Invalidation::HIT_TEST
        } else {
            invalidation
        };
        if let Ok(node) = self.node_mut(id) {
            node.own_dirty.insert(expanded);
            node.subtree_dirty.insert(expanded);
        }
        let mut current = self.node(id).ok().and_then(|node| node.parent);
        while let Some(parent) = current {
            if let Ok(node) = self.node_mut(parent) {
                node.subtree_dirty.insert(expanded);
                if expanded.contains(Invalidation::LAYOUT) {
                    node.own_dirty.insert(Invalidation::LAYOUT);
                }
                current = node.parent;
            } else {
                break;
            }
        }
        self.dirty.insert(expanded);
    }

    fn allocate(&mut self, node: Node) -> NodeId {
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            slot.generation = slot.generation.wrapping_add(1).max(1);
            slot.node = Some(node);
            NodeId {
                index,
                generation: slot.generation,
            }
        } else {
            let id = NodeId {
                index: self.slots.len() as u32,
                generation: 1,
            };
            self.slots.push(Slot {
                generation: 1,
                node: Some(node),
            });
            id
        }
    }

    fn remove_subtree(&mut self, id: NodeId) {
        let children = self
            .node(id)
            .map(|node| node.children.clone())
            .unwrap_or_default();
        for child in children {
            self.remove_subtree(child);
        }
        if self
            .node(id)
            .ok()
            .is_some_and(|node| node.semantic.is_some())
        {
            self.removed_semantics.push(id);
        }
        if self.focus == Some(id) {
            self.focus = None;
        }
        if let Some(slot) = self.slots.get_mut(id.index as usize)
            && slot.generation == id.generation
        {
            slot.node = None;
            self.free.push(id.index);
        }
    }

    fn node(&self, id: NodeId) -> Result<&Node, UiError> {
        let slot = self
            .slots
            .get(id.index as usize)
            .filter(|slot| slot.generation == id.generation)
            .ok_or_else(|| UiError::new("stale retained identity"))?;
        slot.node
            .as_ref()
            .ok_or_else(|| UiError::new("stale retained identity"))
    }

    fn node_mut(&mut self, id: NodeId) -> Result<&mut Node, UiError> {
        let slot = self
            .slots
            .get_mut(id.index as usize)
            .filter(|slot| slot.generation == id.generation)
            .ok_or_else(|| UiError::new("stale retained identity"))?;
        slot.node
            .as_mut()
            .ok_or_else(|| UiError::new("stale retained identity"))
    }

    fn live_ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.slots.iter().enumerate().filter_map(|(index, slot)| {
            slot.node.as_ref().map(|_| NodeId {
                index: index as u32,
                generation: slot.generation,
            })
        })
    }
}

/// Borrowed outputs from one completed retained update.
pub struct FrameUpdate<'a> {
    /// Cached paint-fragment scene.
    pub scene: &'a Scene,
    /// Accessibility delta.
    pub accessibility: &'a AccessibilityUpdate,
    /// Diagnostic pass counters.
    pub stats: PassStats,
}

fn transform_rect(transform: Affine2, rect: LogicalRect) -> LogicalRect {
    let points = [
        transform.transform_point2(Vec2::new(rect.origin.x, rect.origin.y)),
        transform.transform_point2(Vec2::new(rect.max_x(), rect.origin.y)),
        transform.transform_point2(Vec2::new(rect.origin.x, rect.max_y())),
        transform.transform_point2(Vec2::new(rect.max_x(), rect.max_y())),
    ];
    let min_x = points
        .iter()
        .map(|point| point.x)
        .fold(f32::INFINITY, f32::min);
    let min_y = points
        .iter()
        .map(|point| point.y)
        .fold(f32::INFINITY, f32::min);
    let max_x = points
        .iter()
        .map(|point| point.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let max_y = points
        .iter()
        .map(|point| point.y)
        .fold(f32::NEG_INFINITY, f32::max);
    LogicalRect::from_xywh(min_x, min_y, max_x - min_x, max_y - min_y)
}

fn union_rect(a: LogicalRect, b: LogicalRect) -> LogicalRect {
    if a.size.width == 0.0 && a.size.height == 0.0 {
        return b;
    }
    if b.size.width == 0.0 && b.size.height == 0.0 {
        return a;
    }
    let min_x = a.origin.x.min(b.origin.x);
    let min_y = a.origin.y.min(b.origin.y);
    let max_x = a.max_x().max(b.max_x());
    let max_y = a.max_y().max(b.max_y());
    LogicalRect::from_xywh(min_x, min_y, max_x - min_x, max_y - min_y)
}

fn intersect_rect(a: Option<LogicalRect>, b: Option<LogicalRect>) -> Option<LogicalRect> {
    match (a, b) {
        (None, value) | (value, None) => value,
        (Some(a), Some(b)) => {
            let min_x = a.origin.x.max(b.origin.x);
            let min_y = a.origin.y.max(b.origin.y);
            let max_x = a.max_x().min(b.max_x());
            let max_y = a.max_y().min(b.max_y());
            Some(LogicalRect::from_xywh(
                min_x,
                min_y,
                (max_x - min_x).max(0.0),
                (max_y - min_y).max(0.0),
            ))
        }
    }
}
