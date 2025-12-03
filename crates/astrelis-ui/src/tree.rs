//! UI tree structure with Taffy layout integration.

use crate::auto_dirty::StyleGuard;
use crate::dirty::DirtyFlags;
use crate::metrics::{DirtyStats, MetricsTimer, UiMetrics};
use crate::style::Style;
use astrelis_text::ShapedTextData;
use crate::widgets::Widget;
use astrelis_core::alloc::HashSet;
use astrelis_core::math::Vec2;
use astrelis_core::profiling::profile_function;
use astrelis_text::FontRenderer;
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
        if flags.intersects(DirtyFlags::COLOR_ONLY | DirtyFlags::OPACITY_ONLY) {
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
        }
    }

    /// Add a widget to the tree and return its NodeId.
    pub fn add_widget(&mut self, widget: Box<dyn Widget>) -> NodeId {
        let node_id = NodeId(self.next_id);
        self.next_id += 1;

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
            dirty_flags: DirtyFlags::LAYOUT | DirtyFlags::STYLE,
            parent: None,
            children: Vec::new(),
            text_measurement: None,
            layout_version: 0,
            text_version: 0,
            paint_version: 0,
            text_cache: None,
        };

        self.nodes.insert(node_id, ui_node);
        self.mark_dirty_flags(node_id, DirtyFlags::LAYOUT | DirtyFlags::STYLE);

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
        matches!(style.size.width, Dimension::Length(_)) && 
        matches!(style.size.height, Dimension::Length(_))
    }

    /// Mark a node with specific dirty flags and propagate to ancestors if needed.
    pub fn mark_dirty_flags(&mut self, node_id: NodeId, flags: DirtyFlags) {
        profile_function!();

        if flags.is_empty() {
            return;
        }

        self.dirty_nodes.insert(node_id);

        // Mark node with flags and bump versions
        if let Some(node) = self.nodes.get_mut(&node_id) {
            node.dirty_flags |= flags;

            // Notify Taffy of changes
            if flags.intersects(DirtyFlags::LAYOUT | DirtyFlags::CHILDREN_ORDER | DirtyFlags::TEXT_SHAPING) {
                 self.taffy.mark_dirty(node.taffy_node).ok();
            }

            // Bump version counters
            if flags.intersects(DirtyFlags::LAYOUT | DirtyFlags::CHILDREN_ORDER | DirtyFlags::STYLE)
            {
                node.layout_version = node.layout_version.wrapping_add(1);
            }
            if flags.contains(DirtyFlags::TEXT_SHAPING) {
                node.text_version = node.text_version.wrapping_add(1);
                node.text_measurement = None; // Invalidate measurement cache
                node.text_cache = None; // Invalidate shaped text cache
            }
            if flags.intersects(
                DirtyFlags::COLOR_ONLY | DirtyFlags::OPACITY_ONLY | DirtyFlags::GEOMETRY,
            ) {
                node.paint_version = node.paint_version.wrapping_add(1);
            }

            // Propagate to ancestors if needed
            if flags.should_propagate_to_parent() {
                let propagation_flags = flags.propagation_flags();
                
                // Check if this node is a layout boundary
                let is_boundary = Self::is_layout_boundary(node);

                if is_boundary {
                    self.dirty_roots.insert(node_id);
                    return;
                }

                let mut current_parent = node.parent;

                while let Some(parent_id) = current_parent {
                    if !self.dirty_nodes.insert(parent_id) {
                        // Already marked, check if we need to add more flags
                        if let Some(parent_node) = self.nodes.get(&parent_id) {
                            if parent_node.dirty_flags.contains(propagation_flags) {
                                // Already has these flags, stop propagation
                                break;
                            }
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
                if let Some(root) = self.root {
                    if self.dirty_nodes.contains(&root) {
                        self.dirty_roots.insert(root);
                    }
                }
            }
        }
    }

    /// Clear all dirty flags after rendering (called by renderer).
    pub fn clear_dirty_flags(&mut self) {
        for node in self.nodes.values_mut() {
            node.dirty_flags = DirtyFlags::NONE;
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

    /// Check if any node needs layout recomputation.
    pub fn has_layout_dirty(&self) -> bool {
        self.nodes.values().any(|n| n.dirty_flags.needs_layout())
    }

    /// Check if any node needs text shaping.
    pub fn has_text_dirty(&self) -> bool {
        self.nodes
            .values()
            .any(|n| n.dirty_flags.needs_text_shaping())
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
    /// Marks COLOR_ONLY flag (doesn't require layout recomputation).
    pub fn update_color(&mut self, node_id: NodeId, new_color: astrelis_render::Color) -> bool {
        if let Some(node) = self.nodes.get_mut(&node_id) {
            let old_color = node.widget.style().background_color;
            node.widget.style_mut().background_color = Some(new_color);

            if old_color != Some(new_color) {
                self.mark_dirty_flags(node_id, DirtyFlags::COLOR_ONLY);
                return true;
            }
        }
        false
    }

    /// Update opacity with automatic dirty marking.
    ///
    /// Marks OPACITY_ONLY flag (doesn't require layout recomputation).
    pub fn update_opacity(&mut self, node_id: NodeId, _opacity: f32) -> bool {
        // Store opacity in a future opacity field or as part of color alpha
        // For now, mark the flag to demonstrate the pattern
        self.mark_dirty_flags(node_id, DirtyFlags::OPACITY_ONLY);
        true
    }

    /// Compute layout for all nodes.
    /// Compute layout with performance metrics collection.
    pub fn compute_layout_instrumented(
        &mut self,
        viewport_size: astrelis_core::geometry::Size<f32>,
        font_renderer: Option<&FontRenderer>,
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
        self.compute_layout_internal(viewport_size, font_renderer);
        metrics.layout_time = layout_timer.stop();

        metrics.total_time = total_timer.stop();
        self.last_metrics = Some(metrics.clone());
        metrics
    }

    /// Compute layout (standard API without metrics).
    pub fn compute_layout(&mut self, size: astrelis_core::geometry::Size<f32>, font_renderer: Option<&FontRenderer>) {
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

        self.compute_layout_internal(size, font_renderer);
        // Don't clear flags here - renderer will clear them after processing
    }

    /// Internal layout computation implementation.
    fn compute_layout_internal(
        &mut self,
        viewport_size: astrelis_core::geometry::Size<f32>,
        font_renderer: Option<&FontRenderer>,
    ) {
        // If no dirty roots but dirty nodes exist, default to root
        if self.dirty_roots.is_empty() && !self.dirty_nodes.is_empty() {
             if let Some(root) = self.root {
                 self.dirty_roots.insert(root);
             }
        }

        // Filter redundant roots (keep only top-most)
        let mut roots_to_process: Vec<NodeId> = Vec::new();
        let dirty_roots_vec: Vec<NodeId> = self.dirty_roots.iter().copied().collect();
        
        for &root_id in &dirty_roots_vec {
            let mut is_redundant = false;
            if let Some(node) = self.nodes.get(&root_id) {
                let mut current = node.parent;
                while let Some(parent_id) = current {
                    if self.dirty_roots.contains(&parent_id) {
                        is_redundant = true;
                        break;
                    }
                    if let Some(parent) = self.nodes.get(&parent_id) {
                        current = parent.parent;
                    } else {
                        break;
                    }
                }
            }
            if !is_redundant {
                roots_to_process.push(root_id);
            }
        }

        // Store previous positions for subtree roots to prevent them from jumping to (0,0)
        // when Taffy computes layout relative to the subtree root.
        let mut restored_positions: Vec<(NodeId, f32, f32)> = Vec::new();
        for &root_id in &roots_to_process {
             if Some(root_id) != self.root {
                 if let Some(node) = self.nodes.get(&root_id) {
                     restored_positions.push((root_id, node.layout.x, node.layout.y));
                 }
             }
        }

        let nodes_ptr = &mut self.nodes as *mut IndexMap<NodeId, UiNode>;

        for root_id in roots_to_process {
            let Some(root_node) = self.nodes.get(&root_id) else { continue };
            let root_taffy_node = root_node.taffy_node;
            
            let available_space = if Some(root_id) == self.root {
                Size {
                    width: AvailableSpace::Definite(viewport_size.width),
                    height: AvailableSpace::Definite(viewport_size.height),
                }
            } else {
                let style = &root_node.widget.style().layout;
                let width = match style.size.width {
                    Dimension::Length(l) => l,
                    _ => 0.0,
                };
                let height = match style.size.height {
                    Dimension::Length(l) => l,
                    _ => 0.0,
                };
                
                Size {
                    width: AvailableSpace::Definite(width),
                    height: AvailableSpace::Definite(height),
                }
            };

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

                    if widget
                        .as_any()
                        .downcast_ref::<crate::widgets::Text>()
                        .is_some()
                    {
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
            
            // Update layout for this subtree immediately
            self.update_subtree_layout(root_id);
        }

        // Restore positions for subtree roots
        for (id, x, y) in restored_positions {
            if let Some(node) = self.nodes.get_mut(&id) {
                node.layout.x = x;
                node.layout.y = y;
            }
        }
    }

    /// Cache layout results from Taffy into our nodes.
    fn cache_layouts(&mut self) {
        let node_ids: Vec<NodeId> = self.nodes.keys().copied().collect();

        for node_id in node_ids {
            if let Some(node) = self.nodes.get(&node_id) {
                if let Ok(layout) = self.taffy.layout(node.taffy_node) {
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
    }

    /// Update layout for a specific subtree from Taffy results.
    fn update_subtree_layout(&mut self, root_id: NodeId) {
        let mut stack = vec![root_id];
        while let Some(node_id) = stack.pop() {
            // Get children first to avoid holding borrow
            let children = if let Some(node) = self.nodes.get(&node_id) {
                node.children.clone()
            } else {
                Vec::new()
            };
            
            // Update this node
            if let Some(node) = self.nodes.get_mut(&node_id) {
                 if let Ok(layout) = self.taffy.layout(node.taffy_node) {
                    node.layout = LayoutRect {
                        x: layout.location.x,
                        y: layout.location.y,
                        width: layout.size.width,
                        height: layout.size.height,
                    };
                }
            }

            stack.extend(children);
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
            self.mark_dirty_flags(node_id, DirtyFlags::STYLE | DirtyFlags::LAYOUT);
        }
    }
}

impl Default for UiTree {
    fn default() -> Self {
        Self::new()
    }
}
