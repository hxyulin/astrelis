//! UI tree structure with Taffy layout integration.

use crate::constraint_resolver::{ConstraintResolver, ResolveContext};
use crate::dirty::{DirtyCounters, DirtyFlags, StyleGuard};
use crate::metrics::{DirtyStats, MetricsTimer, UiMetrics};
use crate::plugin::registry::WidgetTypeRegistry;
use crate::style::Style;
use crate::widgets::Widget;
#[cfg(feature = "docking")]
use crate::widgets::docking::{DockSplitter, DockTabs};
use astrelis_core::alloc::HashSet;
use astrelis_core::math::Vec2;
use astrelis_core::profiling::{profile_function, profile_scope};
use astrelis_text::FontRenderer;
use astrelis_text::ShapedTextData;
use indexmap::IndexMap;
use std::sync::Arc;
use taffy::{TaffyTree, prelude::*};

/// Node identifier in the UI tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

/// Layout information computed by Taffy.
#[derive(Debug, Clone, Copy)]
pub struct LayoutRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl LayoutRect {
    pub fn contains(&self, point: Vec2) -> bool {
        point.x >= self.x
            && point.x <= self.x + self.width
            && point.y >= self.y
            && point.y <= self.y + self.height
    }

    pub fn position(&self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }

    pub fn size(&self) -> Vec2 {
        Vec2::new(self.width, self.height)
    }
}

#[cfg(feature = "docking")]
/// Internal enum for collecting docking layout info before processing.
enum DockingLayoutInfo {
    Splitter {
        children: Vec<NodeId>,
        direction: crate::widgets::docking::SplitDirection,
        split_ratio: f32,
        separator_size: f32,
        parent_layout: LayoutRect,
    },
    Tabs {
        children: Vec<NodeId>,
        active_tab: usize,
        tab_bar_height: f32,
        content_padding: f32,
        parent_layout: LayoutRect,
    },
}

/// A node in the UI tree.
pub struct UiNode {
    pub widget: Box<dyn Widget>,
    pub taffy_node: taffy::NodeId,
    pub layout: LayoutRect,
    pub dirty_flags: DirtyFlags,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    /// Cached text measurement (width, height)
    pub text_measurement: Option<(f32, f32)>,
    /// Version counters for cache invalidation
    pub layout_version: u32,
    pub text_version: u32,
    pub paint_version: u32,
    /// Cached shaped text data (Phase 3)
    pub text_cache: Option<Arc<ShapedTextData>>,
    /// Accumulated z_index from all parent containers.
    /// Computed during layout traversal.
    pub computed_z_index: u16,
}

impl UiNode {
    /// Bump version counters based on dirty flags and invalidate caches.
    ///
    /// This is called automatically when dirty flags are set to ensure
    /// cached data is invalidated when it becomes stale.
    pub fn bump_version(&mut self, flags: DirtyFlags) {
        if flags.intersects(DirtyFlags::LAYOUT | DirtyFlags::CHILDREN_ORDER) {
            self.layout_version = self.layout_version.wrapping_add(1);
        }
        if flags.contains(DirtyFlags::TEXT_SHAPING) {
            self.text_version = self.text_version.wrapping_add(1);
            // Invalidate text cache when text changes
            self.text_cache = None;
        }
        if flags.intersects(DirtyFlags::COLOR | DirtyFlags::OPACITY) {
            self.paint_version = self.paint_version.wrapping_add(1);
        }
    }
}

/// UI tree managing widgets and layout.
pub struct UiTree {
    taffy: TaffyTree<()>,
    nodes: IndexMap<NodeId, UiNode>,
    root: Option<NodeId>,
    next_id: usize,
    /// Set of dirty nodes that need layout recomputation
    dirty_nodes: HashSet<NodeId>,
    /// Roots of dirty subtrees (for selective layout)
    dirty_roots: HashSet<NodeId>,
    /// Performance metrics from last update
    last_metrics: Option<UiMetrics>,
    /// Nodes with viewport-dependent constraints (vw, vh, vmin, vmax, calc, min, max, clamp)
    viewport_constraint_nodes: HashSet<NodeId>,
    /// Nodes removed since the last drain (for renderer cleanup).
    removed_nodes: Vec<NodeId>,
    /// O(1) dirty state counters.
    dirty_counters: DirtyCounters,
    /// Global content padding for docking tab panels (set from DockingStyle before layout).
    #[cfg(feature = "docking")]
    docking_content_padding: f32,
}

impl UiTree {
    /// Create a new UI tree.
    pub fn new() -> Self {
        Self {
            taffy: TaffyTree::new(),
            nodes: IndexMap::new(),
            root: None,
            next_id: 0,
            dirty_nodes: HashSet::new(),
            dirty_roots: HashSet::new(),
            last_metrics: None,
            viewport_constraint_nodes: HashSet::new(),
            removed_nodes: Vec::new(),
            dirty_counters: DirtyCounters::new(),
            #[cfg(feature = "docking")]
            docking_content_padding: 4.0,
        }
    }

    /// Set the global docking content padding (called before layout from DockingStyle).
    #[cfg(feature = "docking")]
    pub fn set_docking_content_padding(&mut self, padding: f32) {
        self.docking_content_padding = padding;
    }

    /// Add a widget to the tree and return its NodeId.
    pub fn add_widget(&mut self, widget: Box<dyn Widget>) -> NodeId {
        let node_id = NodeId(self.next_id);
        self.next_id += 1;

        // Track nodes with viewport-dependent constraints
        if widget.style().has_unresolved_constraints() {
            self.viewport_constraint_nodes.insert(node_id);
        }

        // Create Taffy node with widget's style
        let style = widget.style().layout.clone();
        let taffy_node = self
            .taffy
            .new_leaf(style)
            .expect("Failed to create taffy node");

        let ui_node = UiNode {
            widget,
            taffy_node,
            layout: LayoutRect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
            dirty_flags: DirtyFlags::NONE,
            parent: None,
            children: Vec::new(),
            text_measurement: None,
            layout_version: 0,
            text_version: 0,
            paint_version: 0,
            text_cache: None,
            computed_z_index: 0,
        };

        self.nodes.insert(node_id, ui_node);
        self.mark_dirty_flags(node_id, DirtyFlags::LAYOUT);

        node_id
    }

    /// Set a node as a child of another node.
    pub fn add_child(&mut self, parent: NodeId, child: NodeId) {
        if let (Some(parent_node), Some(child_node)) =
            (self.nodes.get(&parent), self.nodes.get(&child))
        {
            self.taffy
                .add_child(parent_node.taffy_node, child_node.taffy_node)
                .ok();

            // Update parent/child relationships
            if let Some(child_node) = self.nodes.get_mut(&child) {
                child_node.parent = Some(parent);
            }
            if let Some(parent_node) = self.nodes.get_mut(&parent) {
                parent_node.children.push(child);
            }

            self.mark_dirty_flags(parent, DirtyFlags::CHILDREN_ORDER);
        }
    }

    /// Set multiple children for a node.
    pub fn set_children(&mut self, parent: NodeId, children: &[NodeId]) {
        if let Some(parent_node) = self.nodes.get(&parent) {
            let taffy_children: Vec<taffy::NodeId> = children
                .iter()
                .filter_map(|id| self.nodes.get(id).map(|n| n.taffy_node))
                .collect();

            self.taffy
                .set_children(parent_node.taffy_node, &taffy_children)
                .ok();

            // Update parent/child relationships
            for &child_id in children {
                if let Some(child_node) = self.nodes.get_mut(&child_id) {
                    child_node.parent = Some(parent);
                }
            }
            if let Some(parent_node) = self.nodes.get_mut(&parent) {
                parent_node.children = children.to_vec();
            }

            self.mark_dirty_flags(parent, DirtyFlags::CHILDREN_ORDER);
        }
    }

    /// Set the root node.
    pub fn set_root(&mut self, node_id: NodeId) {
        self.root = Some(node_id);
        self.mark_dirty_flags(node_id, DirtyFlags::LAYOUT);
    }

    /// Check if a node is a layout boundary (fixed size).
    fn is_layout_boundary(node: &UiNode) -> bool {
        let style = &node.widget.style().layout;
        matches!(style.size.width, Dimension::Length(_))
            && matches!(style.size.height, Dimension::Length(_))
    }

    /// Mark a node with specific dirty flags and propagate to ancestors if needed.
    pub fn mark_dirty_flags(&mut self, node_id: NodeId, flags: DirtyFlags) {
        profile_function!();

        if flags.is_empty() {
            return;
        }

        let needs_propagation = self.mark_node_dirty_inner(node_id, flags);

        // Propagate to ancestors if needed
        if needs_propagation {
            self.propagate_dirty_to_ancestors(node_id, flags);
        }
    }

    /// Inner dirty marking: sets flags, bumps versions, notifies Taffy.
    /// Returns `true` if ancestor propagation is needed.
    fn mark_node_dirty_inner(&mut self, node_id: NodeId, flags: DirtyFlags) -> bool {
        self.dirty_nodes.insert(node_id);

        let Some(node) = self.nodes.get_mut(&node_id) else {
            return false;
        };

        let old_flags = node.dirty_flags;
        node.dirty_flags |= flags;
        self.dirty_counters.on_mark(old_flags, flags);

        // Notify Taffy of changes
        if flags
            .intersects(DirtyFlags::LAYOUT | DirtyFlags::CHILDREN_ORDER | DirtyFlags::TEXT_SHAPING)
        {
            self.taffy.mark_dirty(node.taffy_node).ok();
        }

        // Bump version counters
        if flags.intersects(DirtyFlags::LAYOUT | DirtyFlags::CHILDREN_ORDER) {
            node.layout_version = node.layout_version.wrapping_add(1);
            // Layout changes can affect text wrapping width, invalidate measurement
            node.text_measurement = None;
        }
        if flags.contains(DirtyFlags::TEXT_SHAPING) {
            node.text_version = node.text_version.wrapping_add(1);
            node.text_measurement = None; // Invalidate measurement cache
            node.text_cache = None; // Invalidate shaped text cache
        }
        if flags.intersects(
            DirtyFlags::COLOR
                | DirtyFlags::OPACITY
                | DirtyFlags::GEOMETRY
                | DirtyFlags::IMAGE
                | DirtyFlags::FOCUS
                | DirtyFlags::SCROLL
                | DirtyFlags::Z_INDEX,
        ) {
            node.paint_version = node.paint_version.wrapping_add(1);
        }
        if flags.contains(DirtyFlags::VISIBILITY) {
            node.layout_version = node.layout_version.wrapping_add(1);
        }

        flags.should_propagate_to_parent()
    }

    /// Propagate dirty flags from a node up to its ancestors.
    fn propagate_dirty_to_ancestors(&mut self, node_id: NodeId, flags: DirtyFlags) {
        let propagation_flags = flags.propagation_flags();

        let Some(node) = self.nodes.get(&node_id) else {
            return;
        };

        // Check if this node is a layout boundary
        if Self::is_layout_boundary(node) {
            self.dirty_roots.insert(node_id);
            return;
        }

        let mut current_parent = node.parent;

        while let Some(parent_id) = current_parent {
            if !self.dirty_nodes.insert(parent_id) {
                // Already marked, check if we need to add more flags
                if let Some(parent_node) = self.nodes.get(&parent_id)
                    && parent_node.dirty_flags.contains(propagation_flags)
                {
                    // Already has these flags, stop propagation
                    break;
                }
            }

            if let Some(parent_node) = self.nodes.get_mut(&parent_id) {
                parent_node.dirty_flags |= propagation_flags;
                if propagation_flags.contains(DirtyFlags::LAYOUT) {
                    parent_node.layout_version = parent_node.layout_version.wrapping_add(1);
                }

                if Self::is_layout_boundary(parent_node) {
                    self.dirty_roots.insert(parent_id);
                    return;
                }

                current_parent = parent_node.parent;
            } else {
                break;
            }
        }

        // If we reached here, we hit the top without a boundary.
        if let Some(root) = self.root
            && self.dirty_nodes.contains(&root)
        {
            self.dirty_roots.insert(root);
        }
    }

    /// Mark multiple nodes dirty in a batch with deduplicated ancestor propagation.
    ///
    /// This is more efficient than calling `mark_dirty_flags` in a loop when
    /// multiple sibling nodes need marking, since ancestor walks are deduplicated.
    pub fn mark_dirty_batch(&mut self, updates: &[(NodeId, DirtyFlags)]) {
        profile_function!();

        if updates.is_empty() {
            return;
        }

        // Phase 1: Mark all flags, counters, versions (no propagation)
        // Collect nodes that need ancestor propagation
        let mut needs_propagation: Vec<(NodeId, DirtyFlags)> = Vec::new();
        for &(node_id, flags) in updates {
            if flags.is_empty() {
                continue;
            }
            let needs_prop = self.mark_node_dirty_inner(node_id, flags);
            if needs_prop {
                needs_propagation.push((node_id, flags));
            }
        }

        // Phase 2: Deduplicated ancestor propagation
        // Nodes already in dirty_nodes with the right propagation flags will short-circuit
        // the walk, so siblings sharing ancestors naturally deduplicate.
        for (node_id, flags) in needs_propagation {
            self.propagate_dirty_to_ancestors(node_id, flags);
        }
    }

    /// Mark all nodes with the same dirty flags efficiently.
    ///
    /// This is a fast path for operations like theme changes where every node
    /// gets the same flags. For non-propagating flags (COLOR, OPACITY, etc.),
    /// this skips all ancestor logic and does a simple O(N) pass.
    pub fn mark_all_dirty_uniform(&mut self, flags: DirtyFlags) {
        profile_function!();

        if flags.is_empty() {
            return;
        }

        let needs_propagation = flags.should_propagate_to_parent();

        // Fast path: iterate all nodes directly, mark flags and bump versions
        for (&node_id, node) in self.nodes.iter_mut() {
            let old_flags = node.dirty_flags;
            node.dirty_flags |= flags;
            self.dirty_counters.on_mark(old_flags, flags);
            self.dirty_nodes.insert(node_id);

            // Notify Taffy if needed
            if flags.intersects(
                DirtyFlags::LAYOUT | DirtyFlags::CHILDREN_ORDER | DirtyFlags::TEXT_SHAPING,
            ) {
                self.taffy.mark_dirty(node.taffy_node).ok();
            }

            // Bump version counters
            if flags.intersects(DirtyFlags::LAYOUT | DirtyFlags::CHILDREN_ORDER) {
                node.layout_version = node.layout_version.wrapping_add(1);
                node.text_measurement = None;
            }
            if flags.contains(DirtyFlags::TEXT_SHAPING) {
                node.text_version = node.text_version.wrapping_add(1);
                node.text_measurement = None;
                node.text_cache = None;
            }
            if flags.intersects(
                DirtyFlags::COLOR
                    | DirtyFlags::OPACITY
                    | DirtyFlags::GEOMETRY
                    | DirtyFlags::IMAGE
                    | DirtyFlags::FOCUS
                    | DirtyFlags::SCROLL
                    | DirtyFlags::Z_INDEX,
            ) {
                node.paint_version = node.paint_version.wrapping_add(1);
            }
            if flags.contains(DirtyFlags::VISIBILITY) {
                node.layout_version = node.layout_version.wrapping_add(1);
            }
        }

        // For propagating flags, mark the root as dirty root since all nodes are dirty
        if needs_propagation && let Some(root) = self.root {
            self.dirty_roots.insert(root);
        }
    }

    /// Clear all dirty flags after rendering (called by renderer).
    ///
    /// Only iterates the dirty nodes set rather than all nodes for O(dirty) complexity.
    pub fn clear_dirty_flags(&mut self) {
        for &node_id in &self.dirty_nodes {
            if let Some(node) = self.nodes.get_mut(&node_id) {
                self.dirty_counters.on_clear(node.dirty_flags);
                node.dirty_flags = DirtyFlags::NONE;
            }
        }
        self.dirty_nodes.clear();
        self.dirty_roots.clear();
    }

    /// Get the root node.
    pub fn root(&self) -> Option<NodeId> {
        self.root
    }

    /// Get a widget by node ID.
    pub fn get_widget(&self, node_id: NodeId) -> Option<&dyn Widget> {
        self.nodes.get(&node_id).map(|n| &*n.widget)
    }

    /// Get a mutable widget by node ID.
    pub fn get_widget_mut(&mut self, node_id: NodeId) -> Option<&mut dyn Widget> {
        self.nodes.get_mut(&node_id).map(|n| &mut *n.widget)
    }

    /// Get layout for a node.
    pub fn get_layout(&self, node_id: NodeId) -> Option<LayoutRect> {
        self.nodes.get(&node_id).map(|n| n.layout)
    }

    /// Check if tree needs layout.
    pub fn is_dirty(&self) -> bool {
        !self.dirty_nodes.is_empty()
    }

    /// O(1) check: any node needs layout recomputation?
    pub fn has_layout_dirty(&self) -> bool {
        self.dirty_counters.has_layout_dirty()
    }

    /// O(1) check: any node needs text shaping?
    pub fn has_text_dirty(&self) -> bool {
        self.dirty_counters.has_text_dirty()
    }

    /// Get a snapshot of current dirty counter state (for metrics).
    pub fn dirty_summary(&self) -> crate::dirty::DirtySummary {
        self.dirty_counters.summary()
    }

    /// Get the dirty roots for selective tree traversal.
    ///
    /// Dirty roots are the topmost nodes in dirty subtrees. Starting traversal
    /// from these nodes allows skipping clean subtrees entirely.
    pub fn dirty_roots(&self) -> &HashSet<NodeId> {
        &self.dirty_roots
    }

    /// Get the set of all dirty nodes.
    pub fn dirty_nodes(&self) -> &HashSet<NodeId> {
        &self.dirty_nodes
    }

    /// Get the last computed metrics.
    pub fn last_metrics(&self) -> Option<&UiMetrics> {
        self.last_metrics.as_ref()
    }

    /// Get immutable reference to a node.
    pub(crate) fn get_node(&self, node_id: NodeId) -> Option<&UiNode> {
        self.nodes.get(&node_id)
    }

    /// Get mutable reference to a node.
    pub(crate) fn get_node_mut(&mut self, node_id: NodeId) -> Option<&mut UiNode> {
        self.nodes.get_mut(&node_id)
    }

    /// Register a widget ID to node ID mapping (for builder API).
    pub fn register_widget(&mut self, widget_id: crate::widget_id::WidgetId, node_id: NodeId) {
        // Store mapping in widget registry if tree has one
        // For now, this is a no-op as the mapping is managed by UiCore/UiSystem
        // This method exists for builder API compatibility
        let _ = (widget_id, node_id);
    }

    /// Create a style guard for automatic dirty marking on style changes.
    ///
    /// The guard automatically marks appropriate dirty flags when dropped
    /// if the style's layout-affecting properties changed.
    ///
    /// # Example
    /// ```ignore
    /// let mut guard = tree.style_guard_mut(node_id);
    /// if let Some(style) = guard.layout_mut() {
    ///     style.padding = Rect::all(length(10.0));
    /// }
    /// // Automatically marks LAYOUT flag on drop if padding changed
    /// ```
    pub fn style_guard_mut(&mut self, node_id: NodeId) -> StyleGuard<'_> {
        StyleGuard::new(self, node_id)
    }

    /// Update text content with automatic dirty marking.
    ///
    /// Marks TEXT_SHAPING flag if content changed.
    pub fn update_text_content(&mut self, node_id: NodeId, new_content: impl Into<String>) -> bool {
        if let Some(node) = self.nodes.get_mut(&node_id) {
            // Try to downcast to Text widget
            if let Some(text) = node
                .widget
                .as_any_mut()
                .downcast_mut::<crate::widgets::Text>()
            {
                let changed = text.set_content(new_content);
                if changed {
                    self.mark_dirty_flags(node_id, DirtyFlags::TEXT_SHAPING);
                }
                return changed;
            }
        }
        false
    }

    /// Update color with automatic dirty marking.
    ///
    /// Marks COLOR flag (doesn't require layout recomputation).
    pub fn update_color(&mut self, node_id: NodeId, new_color: astrelis_render::Color) -> bool {
        if let Some(node) = self.nodes.get_mut(&node_id) {
            let old_color = node.widget.style().background_color;
            node.widget.style_mut().background_color = Some(new_color);

            if old_color != Some(new_color) {
                self.mark_dirty_flags(node_id, DirtyFlags::COLOR);
                return true;
            }
        }
        false
    }

    /// Update opacity with automatic dirty marking.
    ///
    /// Marks OPACITY flag (doesn't require layout recomputation).
    pub fn update_opacity(&mut self, node_id: NodeId, _opacity: f32) -> bool {
        // Store opacity in a future opacity field or as part of color alpha
        // For now, mark the flag to demonstrate the pattern
        self.mark_dirty_flags(node_id, DirtyFlags::OPACITY);
        true
    }

    /// Compute layout for all nodes.
    /// Compute layout with performance metrics collection.
    pub fn compute_layout_instrumented(
        &mut self,
        viewport_size: astrelis_core::geometry::Size<f32>,
        font_renderer: Option<&FontRenderer>,
        widget_registry: &WidgetTypeRegistry,
    ) -> UiMetrics {
        profile_function!();

        let total_timer = MetricsTimer::start();
        let mut metrics = UiMetrics::new();
        metrics.total_nodes = self.nodes.len();

        // Collect dirty stats
        let mut dirty_stats = DirtyStats::new();
        for node in self.nodes.values() {
            dirty_stats.add_node(node.dirty_flags);
        }
        metrics.nodes_layout_dirty = dirty_stats.layout_count;
        metrics.nodes_text_dirty = dirty_stats.text_count;
        metrics.nodes_paint_dirty = dirty_stats.paint_count;
        metrics.nodes_geometry_dirty = dirty_stats.geometry_count;

        // Early exit if nothing to do
        if self.dirty_nodes.is_empty() {
            metrics.total_time = total_timer.stop();
            self.last_metrics = Some(metrics.clone());
            return metrics;
        }

        // Skip layout if no layout-affecting changes
        if !self.has_layout_dirty() {
            metrics.layout_skips = self.nodes.len();
            metrics.total_time = total_timer.stop();
            self.last_metrics = Some(metrics.clone());
            return metrics;
        }

        let layout_timer = MetricsTimer::start();
        self.compute_layout_internal(viewport_size, font_renderer, widget_registry);
        metrics.layout_time = layout_timer.stop();

        metrics.total_time = total_timer.stop();
        self.last_metrics = Some(metrics.clone());
        metrics
    }

    /// Compute layout (standard API without metrics).
    pub fn compute_layout(
        &mut self,
        size: astrelis_core::geometry::Size<f32>,
        font_renderer: Option<&FontRenderer>,
        widget_registry: &WidgetTypeRegistry,
    ) {
        profile_function!();

        // Skip if nothing to do
        if self.dirty_nodes.is_empty() {
            return;
        }

        // Skip layout if no layout-affecting changes
        // Don't clear dirty flags - renderer needs them for visual updates
        if !self.has_layout_dirty() {
            return;
        }

        self.compute_layout_internal(size, font_renderer, widget_registry);
        // Don't clear flags here - renderer will clear them after processing
    }

    /// Internal layout computation implementation.
    ///
    /// Always computes layout from the tree root to ensure correct absolute positioning.
    /// The subtree optimization was removed because it caused positioning bugs when
    /// layout boundaries (fixed-size nodes) stopped dirty propagation but Taffy computed
    /// positions relative to subtree roots instead of the tree root.
    fn compute_layout_internal(
        &mut self,
        viewport_size: astrelis_core::geometry::Size<f32>,
        font_renderer: Option<&FontRenderer>,
        widget_registry: &WidgetTypeRegistry,
    ) {
        profile_scope!("compute_layout_internal");

        // Resolve viewport-relative units before layout computation
        self.resolve_viewport_units(viewport_size);

        // Always compute layout from tree root for correct positioning
        let Some(root_id) = self.root else { return };
        let Some(root_node) = self.nodes.get(&root_id) else {
            return;
        };
        let root_taffy_node = root_node.taffy_node;

        let available_space = Size {
            width: AvailableSpace::Definite(viewport_size.width),
            height: AvailableSpace::Definite(viewport_size.height),
        };

        let nodes_ptr = &mut self.nodes as *mut IndexMap<NodeId, UiNode>;

        let measure_func = |known_dimensions: Size<Option<f32>>,
                            available_space: Size<AvailableSpace>,
                            node_id: taffy::NodeId,
                            _node_context: Option<&mut ()>,
                            _style: &taffy::Style|
         -> Size<f32> {
            // SAFETY: nodes_ptr is valid during layout computation
            let nodes = unsafe { &mut *nodes_ptr };

            let (widget, cached_measurement) = nodes
                .values_mut()
                .find(|node| node.taffy_node == node_id)
                .map(|node| (&node.widget, &mut node.text_measurement))
                .unzip();

            if let (Some(widget), Some(cached_measurement)) = (widget, cached_measurement) {
                if let Some((cached_w, cached_h)) = *cached_measurement {
                    return Size {
                        width: known_dimensions.width.unwrap_or(cached_w),
                        height: known_dimensions.height.unwrap_or(cached_h),
                    };
                }

                let available = Vec2::new(
                    match available_space.width {
                        AvailableSpace::Definite(w) => w,
                        AvailableSpace::MinContent => 0.0,
                        AvailableSpace::MaxContent => f32::MAX,
                    },
                    match available_space.height {
                        AvailableSpace::Definite(h) => h,
                        AvailableSpace::MinContent => 0.0,
                        AvailableSpace::MaxContent => f32::MAX,
                    },
                );

                let measured = widget.measure(available, font_renderer);

                // Cache measurement for widget types that opt in (e.g. Text)
                if widget_registry.caches_measurement(widget.as_any().type_id()) {
                    *cached_measurement = Some((measured.x, measured.y));
                }

                Size {
                    width: known_dimensions.width.unwrap_or(measured.x),
                    height: known_dimensions.height.unwrap_or(measured.y),
                }
            } else {
                Size {
                    width: known_dimensions.width.unwrap_or(0.0),
                    height: known_dimensions.height.unwrap_or(0.0),
                }
            }
        };

        self.taffy
            .compute_layout_with_measure(root_taffy_node, available_space, measure_func)
            .ok();

        // Update ALL nodes from tree root
        self.update_subtree_layout(root_id);

        // Post-process docking widgets to override child layouts
        #[cfg(feature = "docking")]
        self.post_process_docking_layouts(root_id);
    }

    /// Post-process layouts for DockSplitter and DockTabs widgets.
    ///
    /// These widgets have custom layout logic that can't be expressed in Taffy:
    /// - DockSplitter: positions children based on split ratio
    /// - DockTabs: children fill the content area below the tab bar
    ///
    /// This function recursively processes the tree from root to ensure
    /// parent layouts are computed before children (important for nested docking).
    #[cfg(feature = "docking")]
    fn post_process_docking_layouts(&mut self, node_id: NodeId) {
        profile_scope!("post_process_docking_layouts");

        // Get info for this node first
        let info = {
            let Some(node) = self.nodes.get(&node_id) else {
                return;
            };

            node.widget
                .as_any()
                .downcast_ref::<DockSplitter>()
                .map(|splitter| DockingLayoutInfo::Splitter {
                    children: splitter.children.clone(),
                    direction: splitter.direction,
                    split_ratio: splitter.split_ratio,
                    separator_size: splitter.separator_size,
                    parent_layout: node.layout,
                })
                .or_else(|| {
                    node.widget.as_any().downcast_ref::<DockTabs>().map(|tabs| {
                        let content_padding =
                            tabs.content_padding.unwrap_or(self.docking_content_padding);
                        DockingLayoutInfo::Tabs {
                            children: tabs.children.clone(),
                            active_tab: tabs.active_tab,
                            tab_bar_height: tabs.theme.tab_bar_height,
                            content_padding,
                            parent_layout: node.layout,
                        }
                    })
                })
        };

        // Apply layout if this is a docking widget
        let children_to_recurse = match info {
            Some(DockingLayoutInfo::Splitter {
                children,
                direction,
                split_ratio,
                separator_size,
                parent_layout,
            }) => {
                self.apply_splitter_layout(
                    children.clone(),
                    direction,
                    split_ratio,
                    separator_size,
                    parent_layout,
                );
                children
            }
            Some(DockingLayoutInfo::Tabs {
                children,
                active_tab,
                tab_bar_height,
                content_padding,
                parent_layout,
            }) => {
                self.apply_tabs_layout(
                    children.clone(),
                    active_tab,
                    tab_bar_height,
                    content_padding,
                    parent_layout,
                );
                children
            }
            None => {
                // Not a docking widget, get regular children
                let Some(node) = self.nodes.get(&node_id) else {
                    return;
                };
                node.children.clone()
            }
        };

        // Recursively process children
        for child_id in children_to_recurse {
            self.post_process_docking_layouts(child_id);
        }
    }

    /// Apply layout to DockSplitter children.
    #[cfg(feature = "docking")]
    fn apply_splitter_layout(
        &mut self,
        children: Vec<NodeId>,
        direction: crate::widgets::docking::SplitDirection,
        split_ratio: f32,
        separator_size: f32,
        parent_layout: LayoutRect,
    ) {
        if children.len() < 2 {
            return;
        }

        let half_sep = separator_size / 2.0;

        match direction {
            crate::widgets::docking::SplitDirection::Horizontal => {
                // Left/Right split
                let split_x = parent_layout.width * split_ratio;

                // First child (left)
                if let Some(node) = self.nodes.get_mut(&children[0]) {
                    node.layout = LayoutRect {
                        x: 0.0,
                        y: 0.0,
                        width: (split_x - half_sep).max(0.0),
                        height: parent_layout.height,
                    };
                }

                // Second child (right)
                if let Some(node) = self.nodes.get_mut(&children[1]) {
                    node.layout = LayoutRect {
                        x: split_x + half_sep,
                        y: 0.0,
                        width: (parent_layout.width - split_x - half_sep).max(0.0),
                        height: parent_layout.height,
                    };
                }
            }
            crate::widgets::docking::SplitDirection::Vertical => {
                // Top/Bottom split
                let split_y = parent_layout.height * split_ratio;

                // First child (top)
                if let Some(node) = self.nodes.get_mut(&children[0]) {
                    node.layout = LayoutRect {
                        x: 0.0,
                        y: 0.0,
                        width: parent_layout.width,
                        height: (split_y - half_sep).max(0.0),
                    };
                }

                // Second child (bottom)
                if let Some(node) = self.nodes.get_mut(&children[1]) {
                    node.layout = LayoutRect {
                        x: 0.0,
                        y: split_y + half_sep,
                        width: parent_layout.width,
                        height: (parent_layout.height - split_y - half_sep).max(0.0),
                    };
                }
            }
        }
    }

    /// Apply layout to DockTabs children.
    #[cfg(feature = "docking")]
    fn apply_tabs_layout(
        &mut self,
        children: Vec<NodeId>,
        _active_tab: usize,
        tab_bar_height: f32,
        content_padding: f32,
        parent_layout: LayoutRect,
    ) {
        // Content area is below the tab bar, inset by content_padding on all sides
        let content_layout = LayoutRect {
            x: content_padding,
            y: tab_bar_height + content_padding,
            width: (parent_layout.width - content_padding * 2.0).max(0.0),
            height: (parent_layout.height - tab_bar_height - content_padding * 2.0).max(0.0),
        };

        // All tab content children get the same layout (content area)
        // The renderer will only show the active one
        for child_id in &children {
            if let Some(node) = self.nodes.get_mut(child_id) {
                node.layout = content_layout;
            }
        }
    }

    /// Resolve viewport-relative units (vw, vh, vmin, vmax) and complex constraints to absolute pixels.
    ///
    /// This is called before Taffy layout computation to convert viewport units
    /// and complex constraints into pixel values that Taffy can understand.
    fn resolve_viewport_units(&mut self, viewport_size: astrelis_core::geometry::Size<f32>) {
        profile_scope!("resolve_viewport_units");

        if self.viewport_constraint_nodes.is_empty() {
            return;
        }

        let viewport = Vec2::new(viewport_size.width, viewport_size.height);
        let ctx = ResolveContext::viewport_only(viewport);

        // Collect nodes to avoid borrowing issues
        let constraint_nodes: Vec<NodeId> =
            self.viewport_constraint_nodes.iter().copied().collect();

        for node_id in constraint_nodes {
            if let Some(node) = self.nodes.get_mut(&node_id) {
                let style = node.widget.style_mut();
                let mut changed = false;

                // Get the constraints box if present
                if let Some(ref constraints) = style.constraints {
                    // Resolve width
                    if let Some(ref constraint) = constraints.width
                        && constraint.needs_resolution()
                        && let Some(px) = ConstraintResolver::resolve(constraint, &ctx)
                    {
                        style.layout.size.width = taffy::Dimension::Length(px);
                        changed = true;
                    }

                    // Resolve height
                    if let Some(ref constraint) = constraints.height
                        && constraint.needs_resolution()
                        && let Some(px) = ConstraintResolver::resolve(constraint, &ctx)
                    {
                        style.layout.size.height = taffy::Dimension::Length(px);
                        changed = true;
                    }

                    // Resolve min_width
                    if let Some(ref constraint) = constraints.min_width
                        && constraint.needs_resolution()
                        && let Some(px) = ConstraintResolver::resolve(constraint, &ctx)
                    {
                        style.layout.min_size.width = taffy::Dimension::Length(px);
                        changed = true;
                    }

                    // Resolve min_height
                    if let Some(ref constraint) = constraints.min_height
                        && constraint.needs_resolution()
                        && let Some(px) = ConstraintResolver::resolve(constraint, &ctx)
                    {
                        style.layout.min_size.height = taffy::Dimension::Length(px);
                        changed = true;
                    }

                    // Resolve max_width
                    if let Some(ref constraint) = constraints.max_width
                        && constraint.needs_resolution()
                        && let Some(px) = ConstraintResolver::resolve(constraint, &ctx)
                    {
                        style.layout.max_size.width = taffy::Dimension::Length(px);
                        changed = true;
                    }

                    // Resolve max_height
                    if let Some(ref constraint) = constraints.max_height
                        && constraint.needs_resolution()
                        && let Some(px) = ConstraintResolver::resolve(constraint, &ctx)
                    {
                        style.layout.max_size.height = taffy::Dimension::Length(px);
                        changed = true;
                    }

                    // Resolve padding
                    if let Some(ref padding) = constraints.padding
                        && padding.iter().any(|c| c.needs_resolution())
                    {
                        if let Some(px) = ConstraintResolver::resolve(&padding[0], &ctx) {
                            style.layout.padding.left = taffy::LengthPercentage::Length(px);
                            changed = true;
                        }
                        if let Some(px) = ConstraintResolver::resolve(&padding[1], &ctx) {
                            style.layout.padding.top = taffy::LengthPercentage::Length(px);
                            changed = true;
                        }
                        if let Some(px) = ConstraintResolver::resolve(&padding[2], &ctx) {
                            style.layout.padding.right = taffy::LengthPercentage::Length(px);
                            changed = true;
                        }
                        if let Some(px) = ConstraintResolver::resolve(&padding[3], &ctx) {
                            style.layout.padding.bottom = taffy::LengthPercentage::Length(px);
                            changed = true;
                        }
                    }

                    // Resolve margin
                    if let Some(ref margin) = constraints.margin
                        && margin.iter().any(|c| c.needs_resolution())
                    {
                        if let Some(px) = ConstraintResolver::resolve(&margin[0], &ctx) {
                            style.layout.margin.left = taffy::LengthPercentageAuto::Length(px);
                            changed = true;
                        }
                        if let Some(px) = ConstraintResolver::resolve(&margin[1], &ctx) {
                            style.layout.margin.top = taffy::LengthPercentageAuto::Length(px);
                            changed = true;
                        }
                        if let Some(px) = ConstraintResolver::resolve(&margin[2], &ctx) {
                            style.layout.margin.right = taffy::LengthPercentageAuto::Length(px);
                            changed = true;
                        }
                        if let Some(px) = ConstraintResolver::resolve(&margin[3], &ctx) {
                            style.layout.margin.bottom = taffy::LengthPercentageAuto::Length(px);
                            changed = true;
                        }
                    }

                    // Resolve gap
                    if let Some(ref constraint) = constraints.gap
                        && constraint.needs_resolution()
                        && let Some(px) = ConstraintResolver::resolve(constraint, &ctx)
                    {
                        style.layout.gap.width = taffy::LengthPercentage::Length(px);
                        style.layout.gap.height = taffy::LengthPercentage::Length(px);
                        changed = true;
                    }

                    // Resolve flex_basis
                    if let Some(ref constraint) = constraints.flex_basis
                        && constraint.needs_resolution()
                        && let Some(px) = ConstraintResolver::resolve(constraint, &ctx)
                    {
                        style.layout.flex_basis = taffy::Dimension::Length(px);
                        changed = true;
                    }
                }

                // Update Taffy with the resolved style if anything changed
                if changed {
                    let taffy_node = node.taffy_node;
                    let layout_style = style.layout.clone();
                    self.taffy.set_style(taffy_node, layout_style).ok();
                    self.taffy.mark_dirty(taffy_node).ok();
                }
            }
        }
    }

    /// Mark all viewport-constraint nodes as needing layout.
    ///
    /// Called when viewport size changes to trigger re-resolution of viewport units.
    pub fn mark_viewport_dirty(&mut self) {
        let updates: Vec<(NodeId, DirtyFlags)> = self
            .viewport_constraint_nodes
            .iter()
            .map(|&id| (id, DirtyFlags::LAYOUT))
            .collect();
        self.mark_dirty_batch(&updates);
    }

    /// Mark all nodes with the given dirty flags.
    pub fn mark_all_dirty(&mut self, flags: DirtyFlags) {
        self.mark_all_dirty_uniform(flags);
    }

    /// Cache layout results from Taffy into our nodes.
    #[allow(dead_code)]
    fn cache_layouts(&mut self) {
        let node_ids: Vec<NodeId> = self.nodes.keys().copied().collect();

        for node_id in node_ids {
            if let Some(node) = self.nodes.get(&node_id)
                && let Ok(layout) = self.taffy.layout(node.taffy_node)
            {
                let layout_rect = LayoutRect {
                    x: layout.location.x,
                    y: layout.location.y,
                    width: layout.size.width,
                    height: layout.size.height,
                };

                if let Some(node) = self.nodes.get_mut(&node_id) {
                    node.layout = layout_rect;
                }
            }
        }
    }

    /// Update layout for a specific subtree from Taffy results.
    fn update_subtree_layout(&mut self, root_id: NodeId) {
        // Use a stack with (node_id, parent_z_index) for depth-first traversal
        let mut stack = vec![(root_id, 0u16)];
        while let Some((node_id, parent_z_index)) = stack.pop() {
            // Get node's z_index offset and children before any mutable borrows
            let (z_offset, children) = if let Some(node) = self.nodes.get(&node_id) {
                let z_offset = node.widget.style().z_index;
                (z_offset, node.children.clone())
            } else {
                continue;
            };

            // Compute accumulated z-index (saturating to prevent overflow)
            let computed_z = parent_z_index.saturating_add(z_offset);

            // Update this node's layout and computed z-index
            if let Some(node) = self.nodes.get_mut(&node_id)
                && let Ok(layout) = self.taffy.layout(node.taffy_node)
            {
                node.layout = LayoutRect {
                    x: layout.location.x,
                    y: layout.location.y,
                    width: layout.size.width,
                    height: layout.size.height,
                };
                node.computed_z_index = computed_z;
            }

            // Push children with accumulated z-index
            for child_id in children {
                stack.push((child_id, computed_z));
            }
        }
    }

    /// Mark a node and all its descendants with the Z_INDEX dirty flag.
    ///
    /// Z_INDEX propagates DOWN to children (unlike LAYOUT flags which propagate up).
    /// This is called when a node's z_index style property changes, since the
    /// computed_z_index of all descendants depends on ancestor z_index values.
    pub fn mark_z_index_dirty(&mut self, node_id: NodeId) {
        let mut stack = vec![node_id];
        while let Some(id) = stack.pop() {
            self.dirty_nodes.insert(id);
            if let Some(node) = self.nodes.get_mut(&id) {
                let old_flags = node.dirty_flags;
                node.dirty_flags |= DirtyFlags::Z_INDEX;
                self.dirty_counters.on_mark(old_flags, DirtyFlags::Z_INDEX);
                node.paint_version = node.paint_version.wrapping_add(1);
                stack.extend(node.children.iter().copied());
            }
        }
    }

    /// Clear the entire tree.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.taffy.clear();
        self.root = None;
        self.next_id = 0;
        self.dirty_nodes.clear();
        self.dirty_roots.clear();
        self.dirty_counters.reset();
        self.viewport_constraint_nodes.clear();
        self.removed_nodes.clear();
    }

    /// Drain the list of removed node IDs (returns and clears the list).
    ///
    /// The renderer calls this to learn which nodes were removed since
    /// the last drain, allowing it to clean up stale draw commands.
    pub fn drain_removed_nodes(&mut self) -> Vec<NodeId> {
        std::mem::take(&mut self.removed_nodes)
    }

    /// Check whether a node still exists in the tree.
    pub fn node_exists(&self, node_id: NodeId) -> bool {
        self.nodes.contains_key(&node_id)
    }

    /// Sync a node's widget style to its Taffy layout node.
    ///
    /// Call after externally modifying a widget's style to ensure Taffy
    /// picks up the changes on next layout computation.
    pub(crate) fn sync_taffy_style(&mut self, node_id: NodeId) {
        if let Some(node) = self.nodes.get(&node_id) {
            let taffy_node = node.taffy_node;
            let layout_style = node.widget.style().layout.clone();
            self.taffy.set_style(taffy_node, layout_style).ok();
            self.taffy.mark_dirty(taffy_node).ok();
        }
    }

    /// Find all nodes whose widget downcasts to the given type.
    ///
    /// Returns a vector of (NodeId, absolute layout rect) pairs for each matching widget.
    /// Useful for finding all DockTabs containers during cross-container drag operations.
    pub fn find_widgets_with_layout<T: 'static>(&self) -> Vec<(NodeId, LayoutRect)> {
        let mut results = Vec::new();
        if let Some(root) = self.root {
            self.find_widgets_recursive::<T>(root, Vec2::ZERO, &mut results);
        }
        results
    }

    /// Recursively search for widgets of a given type.
    fn find_widgets_recursive<T: 'static>(
        &self,
        node_id: NodeId,
        parent_offset: Vec2,
        results: &mut Vec<(NodeId, LayoutRect)>,
    ) {
        let Some(node) = self.nodes.get(&node_id) else {
            return;
        };

        let abs_x = parent_offset.x + node.layout.x;
        let abs_y = parent_offset.y + node.layout.y;

        // Check if this widget matches the type
        if node.widget.as_any().downcast_ref::<T>().is_some() {
            results.push((
                node_id,
                LayoutRect {
                    x: abs_x,
                    y: abs_y,
                    width: node.layout.width,
                    height: node.layout.height,
                },
            ));
        }

        // Recurse into children
        let children = node.children.clone();
        let offset = Vec2::new(abs_x, abs_y);
        for child_id in children {
            self.find_widgets_recursive::<T>(child_id, offset, results);
        }
    }

    /// Iterate over all nodes.
    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &UiNode)> {
        self.nodes.iter().map(|(id, node)| (*id, node))
    }

    /// Iterate over all nodes mutably.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (NodeId, &mut UiNode)> {
        self.nodes.iter_mut().map(|(id, node)| (*id, node))
    }

    /// Get all node IDs.
    pub fn node_ids(&self) -> Vec<NodeId> {
        self.nodes.keys().copied().collect()
    }

    /// Update a widget's style and mark tree dirty.
    pub fn update_style(&mut self, node_id: NodeId, style: Style) {
        if let Some(node) = self.nodes.get_mut(&node_id) {
            *node.widget.style_mut() = style.clone();
            self.taffy.set_style(node.taffy_node, style.layout).ok();
            self.mark_dirty_flags(node_id, DirtyFlags::LAYOUT);
        }
    }

    /// Remove a node and all its descendants from the tree.
    ///
    /// This properly cleans up both the UI tree and the underlying Taffy layout tree.
    /// If the node has a parent, it will be removed from the parent's children list.
    ///
    /// # Arguments
    ///
    /// * `node_id` - The node to remove
    ///
    /// # Returns
    ///
    /// `true` if the node was removed, `false` if it didn't exist
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Remove a node from virtual scrolling when it scrolls out of view
    /// if tree.remove_node(old_node_id) {
    ///     // Node was successfully removed
    /// }
    /// ```
    pub fn remove_node(&mut self, node_id: NodeId) -> bool {
        // Check if node exists
        if !self.nodes.contains_key(&node_id) {
            return false;
        }

        // Collect all descendant nodes to remove (depth-first traversal)
        let mut to_remove = Vec::new();
        let mut stack = vec![node_id];

        while let Some(id) = stack.pop() {
            to_remove.push(id);
            if let Some(node) = self.nodes.get(&id) {
                stack.extend(node.children.iter().copied());
            }
        }

        // Remove from parent's children list
        if let Some(node) = self.nodes.get(&node_id)
            && let Some(parent_id) = node.parent
            && let Some(parent_node) = self.nodes.get_mut(&parent_id)
        {
            parent_node.children.retain(|&child| child != node_id);
            // Mark parent dirty since children changed
            self.mark_dirty_flags(parent_id, DirtyFlags::CHILDREN_ORDER);
        }

        // Remove all nodes (children first, then parent)
        for id in to_remove.iter().rev() {
            if let Some(node) = self.nodes.shift_remove(id) {
                // Remove from Taffy
                self.taffy.remove(node.taffy_node).ok();
                // Update dirty counters before removing from dirty tracking
                if !node.dirty_flags.is_empty() {
                    self.dirty_counters.on_clear(node.dirty_flags);
                }
                // Remove from dirty tracking
                self.dirty_nodes.remove(id);
                self.dirty_roots.remove(id);
                // Remove from viewport constraint tracking
                self.viewport_constraint_nodes.remove(id);
                // Track removal for renderer cleanup
                self.removed_nodes.push(*id);
            }
        }

        // If we removed the root, clear it
        if self.root == Some(node_id) {
            self.root = None;
        }

        true
    }

    /// Remove a child from a parent node without removing it from the tree.
    ///
    /// This is useful for reorganizing the tree structure without destroying nodes.
    ///
    /// # Arguments
    ///
    /// * `parent` - The parent node
    /// * `child` - The child to remove from the parent
    ///
    /// # Returns
    ///
    /// `true` if the child was removed from the parent, `false` otherwise
    pub fn remove_child(&mut self, parent: NodeId, child: NodeId) -> bool {
        // Get taffy nodes before any mutable borrows
        let (parent_taffy, child_taffy) = match (self.nodes.get(&parent), self.nodes.get(&child)) {
            (Some(p), Some(c)) => (p.taffy_node, c.taffy_node),
            _ => return false,
        };

        // Check if parent has this child
        let had_child = self
            .nodes
            .get(&parent)
            .map(|p| p.children.contains(&child))
            .unwrap_or(false);

        if !had_child {
            return false;
        }

        // Remove child from parent's children list
        if let Some(parent_node) = self.nodes.get_mut(&parent) {
            parent_node.children.retain(|&c| c != child);
        }

        // Update Taffy
        self.taffy.remove_child(parent_taffy, child_taffy).ok();

        // Clear child's parent
        if let Some(child_node) = self.nodes.get_mut(&child) {
            child_node.parent = None;
        }

        self.mark_dirty_flags(parent, DirtyFlags::CHILDREN_ORDER);
        true
    }

    /// Apply a position offset to a node for effects like scrolling.
    ///
    /// This modifies the layout position without affecting the Taffy layout,
    /// useful for virtual scrolling or other transform effects.
    ///
    /// # Arguments
    ///
    /// * `node_id` - The node to offset
    /// * `x_offset` - X position offset in pixels
    /// * `y_offset` - Y position offset in pixels
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Apply scroll offset to virtually scrolled items
    /// let scroll_offset = 100.0;
    /// tree.set_position_offset(item_node, 0.0, item_y - scroll_offset);
    /// ```
    pub fn set_position_offset(&mut self, node_id: NodeId, x_offset: f32, y_offset: f32) {
        if let Some(node) = self.nodes.get_mut(&node_id) {
            // Store the Taffy-computed position if not already stored
            // This assumes layout has been computed
            if let Ok(layout) = self.taffy.layout(node.taffy_node) {
                // Apply offset to the Taffy position
                node.layout.x = layout.location.x + x_offset;
                node.layout.y = layout.location.y + y_offset;
            }
        }
    }

    /// Get the computed layout position without any applied offsets.
    ///
    /// This returns the position as computed by Taffy, ignoring any manual offsets.
    pub fn get_base_position(&self, node_id: NodeId) -> Option<(f32, f32)> {
        self.nodes.get(&node_id).and_then(|node| {
            self.taffy
                .layout(node.taffy_node)
                .ok()
                .map(|layout| (layout.location.x, layout.location.y))
        })
    }
}

impl Default for UiTree {
    fn default() -> Self {
        Self::new()
    }
}
