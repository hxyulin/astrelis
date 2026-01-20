//! Enhanced UI Inspector with property editing, graphs, and metrics integration.
//!
//! UiInspector provides:
//! - Tree hierarchy visualization with collapsible nodes
//! - Property editing for selected widgets
//! - Performance graphs (frame time, dirty counts)
//! - Search functionality
//! - Tree snapshot export
//! - Integration with MetricsCollector
//!
//! # Example
//!
//! ```ignore
//! use astrelis_ui::inspector::{UiInspector, InspectorConfig};
//!
//! let mut inspector = UiInspector::new(InspectorConfig::default());
//!
//! // Toggle with F12
//! if keyboard.just_pressed(KeyCode::F12) {
//!     inspector.toggle();
//! }
//!
//! // Update inspector state
//! inspector.update(&tree, &registry, &metrics_collector);
//!
//! // Handle inspector input
//! inspector.handle_input(&events);
//!
//! // Generate overlay data for rendering
//! let overlay_quads = inspector.generate_overlay_quads();
//! ```

use std::collections::VecDeque;

use astrelis_core::alloc::{HashMap, HashSet};
use astrelis_core::math::Vec2;
use astrelis_render::Color;

use crate::dirty::DirtyFlags;
use crate::metrics_collector::{FrameTimingMetrics, MetricsCollector};
use crate::tree::{NodeId, UiTree};
use crate::widget_id::{WidgetId, WidgetIdRegistry};

/// Configuration for the inspector.
#[derive(Debug, Clone)]
pub struct InspectorConfig {
    /// Enable bounds overlay visualization.
    pub show_bounds: bool,
    /// Enable dirty flag overlay.
    pub show_dirty_flags: bool,
    /// Enable performance graphs.
    pub show_graphs: bool,
    /// Enable tree view panel.
    pub show_tree_view: bool,
    /// Enable property panel.
    pub show_properties: bool,
    /// Maximum depth to display in tree view (0 = unlimited).
    pub max_tree_depth: usize,
    /// Graph history size in frames.
    pub graph_history_size: usize,
    /// Highlight hovered widget.
    pub highlight_hover: bool,
    /// Show layout bounds with padding/margin.
    pub show_layout_details: bool,
}

impl Default for InspectorConfig {
    fn default() -> Self {
        Self {
            show_bounds: true,
            show_dirty_flags: true,
            show_graphs: true,
            show_tree_view: true,
            show_properties: true,
            max_tree_depth: 0,
            graph_history_size: 120,
            highlight_hover: true,
            show_layout_details: false,
        }
    }
}

/// Widget classification for visualization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WidgetKind {
    Container,
    Text,
    Button,
    Image,
    TextInput,
    Checkbox,
    Slider,
    ScrollView,
    Custom,
    Unknown,
}

impl WidgetKind {
    /// Get visualization color for this widget kind.
    pub fn color(&self) -> Color {
        match self {
            Self::Container => Color::rgba(0.2, 0.5, 0.9, 0.25),
            Self::Text => Color::rgba(0.2, 0.8, 0.3, 0.25),
            Self::Button => Color::rgba(0.9, 0.5, 0.2, 0.25),
            Self::Image => Color::rgba(0.8, 0.2, 0.6, 0.25),
            Self::TextInput => Color::rgba(0.2, 0.8, 0.8, 0.25),
            Self::Checkbox => Color::rgba(0.8, 0.8, 0.2, 0.25),
            Self::Slider => Color::rgba(0.6, 0.2, 0.8, 0.25),
            Self::ScrollView => Color::rgba(0.4, 0.6, 0.8, 0.25),
            Self::Custom => Color::rgba(0.6, 0.6, 0.6, 0.25),
            Self::Unknown => Color::rgba(0.5, 0.5, 0.5, 0.25),
        }
    }

    /// Get border color for this widget kind.
    pub fn border_color(&self) -> Color {
        let mut c = self.color();
        c.a = 0.8;
        c
    }
}

/// Editable property types.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    Float(f32),
    Int(i32),
    Bool(bool),
    Color(Color),
    String(String),
    Vec2(Vec2),
}

/// Property that can be edited in the inspector.
#[derive(Debug, Clone)]
pub struct EditableProperty {
    /// Property name for display.
    pub name: String,
    /// Property category.
    pub category: PropertyCategory,
    /// Current value.
    pub value: PropertyValue,
    /// Whether this property affects layout.
    pub affects_layout: bool,
}

/// Property categories for organization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyCategory {
    Layout,
    Style,
    Text,
    Transform,
    Behavior,
}

/// Tree node state for display.
#[derive(Debug, Clone)]
pub struct TreeNodeInfo {
    pub node_id: NodeId,
    pub widget_id: Option<WidgetId>,
    pub kind: WidgetKind,
    pub label: String,
    pub depth: usize,
    pub child_count: usize,
    pub bounds: (f32, f32, f32, f32),
    pub dirty_flags: DirtyFlags,
    pub is_expanded: bool,
    pub is_visible: bool,
}

/// State for the tree view.
#[derive(Debug, Default)]
pub struct TreeViewState {
    /// All nodes in display order.
    nodes: Vec<TreeNodeInfo>,
    /// Expanded node IDs.
    expanded: HashSet<NodeId>,
    /// Scroll offset.
    scroll_offset: f32,
    /// Filter string.
    filter: String,
}

impl TreeViewState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggle expansion state of a node.
    pub fn toggle_expand(&mut self, node_id: NodeId) {
        if self.expanded.contains(&node_id) {
            self.expanded.remove(&node_id);
        } else {
            self.expanded.insert(node_id);
        }
    }

    /// Expand all nodes.
    pub fn expand_all(&mut self) {
        for node in &self.nodes {
            self.expanded.insert(node.node_id);
        }
    }

    /// Collapse all nodes.
    pub fn collapse_all(&mut self) {
        self.expanded.clear();
    }

    /// Set filter string.
    pub fn set_filter(&mut self, filter: String) {
        self.filter = filter;
    }

    /// Get visible nodes after filtering and expansion.
    pub fn visible_nodes(&self) -> impl Iterator<Item = &TreeNodeInfo> {
        self.nodes.iter().filter(|n| n.is_visible)
    }
}

/// Property editor state.
#[derive(Debug, Clone)]
pub struct PropertyEditor {
    pub target_node: NodeId,
    pub properties: Vec<EditableProperty>,
    pub pending_changes: Vec<(String, PropertyValue)>,
}

impl PropertyEditor {
    pub fn new(node_id: NodeId) -> Self {
        Self {
            target_node: node_id,
            properties: Vec::new(),
            pending_changes: Vec::new(),
        }
    }

    /// Queue a property change.
    pub fn set_property(&mut self, name: String, value: PropertyValue) {
        self.pending_changes.push((name, value));
    }

    /// Check if there are pending changes.
    pub fn has_pending_changes(&self) -> bool {
        !self.pending_changes.is_empty()
    }

    /// Clear pending changes.
    pub fn clear_pending(&mut self) {
        self.pending_changes.clear();
    }
}

/// Graph data point.
#[derive(Debug, Clone, Copy)]
pub struct GraphPoint {
    pub frame: u64,
    pub value: f32,
}

/// Performance graph state.
#[derive(Debug)]
pub struct InspectorGraphs {
    /// Frame time history.
    pub frame_times: VecDeque<GraphPoint>,
    /// Layout time history.
    pub layout_times: VecDeque<GraphPoint>,
    /// Text shaping time history.
    pub text_times: VecDeque<GraphPoint>,
    /// Dirty node count history.
    pub dirty_counts: VecDeque<GraphPoint>,
    /// Maximum history size.
    pub max_size: usize,
}

impl InspectorGraphs {
    pub fn new(max_size: usize) -> Self {
        Self {
            frame_times: VecDeque::with_capacity(max_size),
            layout_times: VecDeque::with_capacity(max_size),
            text_times: VecDeque::with_capacity(max_size),
            dirty_counts: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    /// Update graphs from frame metrics.
    pub fn update(&mut self, metrics: &FrameTimingMetrics) {
        let frame_id = metrics.frame_id;

        // Add frame time
        self.frame_times.push_back(GraphPoint {
            frame: frame_id,
            value: metrics.total_frame_time().as_secs_f32() * 1000.0,
        });

        // Add layout time
        self.layout_times.push_back(GraphPoint {
            frame: frame_id,
            value: metrics.total_layout_time.as_secs_f32() * 1000.0,
        });

        // Add text time
        self.text_times.push_back(GraphPoint {
            frame: frame_id,
            value: metrics.text_shaping_time.as_secs_f32() * 1000.0,
        });

        // Add dirty count
        let total_dirty =
            (metrics.nodes_layout_dirty + metrics.nodes_text_dirty + metrics.nodes_paint_dirty) as f32;
        self.dirty_counts.push_back(GraphPoint {
            frame: frame_id,
            value: total_dirty,
        });

        // Trim to max size
        while self.frame_times.len() > self.max_size {
            self.frame_times.pop_front();
        }
        while self.layout_times.len() > self.max_size {
            self.layout_times.pop_front();
        }
        while self.text_times.len() > self.max_size {
            self.text_times.pop_front();
        }
        while self.dirty_counts.len() > self.max_size {
            self.dirty_counts.pop_front();
        }
    }

    /// Get max value from a graph for scaling.
    pub fn max_frame_time(&self) -> f32 {
        self.frame_times
            .iter()
            .map(|p| p.value)
            .fold(16.67, f32::max)
    }

    /// Get average frame time.
    pub fn avg_frame_time(&self) -> f32 {
        if self.frame_times.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.frame_times.iter().map(|p| p.value).sum();
        sum / self.frame_times.len() as f32
    }

    /// Clear all graph data.
    pub fn clear(&mut self) {
        self.frame_times.clear();
        self.layout_times.clear();
        self.text_times.clear();
        self.dirty_counts.clear();
    }
}

/// Search state for the inspector.
#[derive(Debug, Default)]
pub struct SearchState {
    pub query: String,
    pub results: Vec<NodeId>,
    pub current_result_index: usize,
}

impl SearchState {
    /// Perform search on tree nodes.
    pub fn search(&mut self, nodes: &[TreeNodeInfo]) {
        self.results.clear();
        self.current_result_index = 0;

        if self.query.is_empty() {
            return;
        }

        let query_lower = self.query.to_lowercase();
        for node in nodes {
            if node.label.to_lowercase().contains(&query_lower) {
                self.results.push(node.node_id);
            }
        }
    }

    /// Go to next search result.
    pub fn next_result(&mut self) -> Option<NodeId> {
        if self.results.is_empty() {
            return None;
        }
        self.current_result_index = (self.current_result_index + 1) % self.results.len();
        Some(self.results[self.current_result_index])
    }

    /// Go to previous search result.
    pub fn prev_result(&mut self) -> Option<NodeId> {
        if self.results.is_empty() {
            return None;
        }
        if self.current_result_index == 0 {
            self.current_result_index = self.results.len() - 1;
        } else {
            self.current_result_index -= 1;
        }
        Some(self.results[self.current_result_index])
    }
}

/// Overlay quad for rendering.
#[derive(Debug, Clone)]
pub struct OverlayQuad {
    pub position: Vec2,
    pub size: Vec2,
    pub fill_color: Color,
    pub border_color: Option<Color>,
    pub border_width: f32,
}

/// Enhanced UI inspector with property editing and metrics integration.
pub struct UiInspector {
    /// Configuration.
    config: InspectorConfig,
    /// Whether inspector is enabled.
    enabled: bool,
    /// Tree view state.
    tree_view: TreeViewState,
    /// Currently selected node.
    selected: Option<NodeId>,
    /// Hovered node (from mouse).
    hovered: Option<NodeId>,
    /// Property editor for selected widget.
    property_editor: Option<PropertyEditor>,
    /// Performance graphs.
    graphs: InspectorGraphs,
    /// Search state.
    search: SearchState,
    /// Node ID to widget kind mapping (cached).
    widget_kinds: HashMap<NodeId, WidgetKind>,
    /// Reverse mapping from NodeId to WidgetId.
    node_to_widget_id: HashMap<NodeId, WidgetId>,
}

impl UiInspector {
    /// Create a new inspector with the given configuration.
    pub fn new(config: InspectorConfig) -> Self {
        Self {
            graphs: InspectorGraphs::new(config.graph_history_size),
            config,
            enabled: false,
            tree_view: TreeViewState::new(),
            selected: None,
            hovered: None,
            property_editor: None,
            search: SearchState::default(),
            widget_kinds: HashMap::new(),
            node_to_widget_id: HashMap::new(),
        }
    }

    /// Toggle inspector enabled state.
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    /// Check if inspector is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable the inspector.
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable the inspector.
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Get configuration.
    pub fn config(&self) -> &InspectorConfig {
        &self.config
    }

    /// Modify configuration.
    pub fn config_mut(&mut self) -> &mut InspectorConfig {
        &mut self.config
    }

    /// Get currently selected node.
    pub fn selected(&self) -> Option<NodeId> {
        self.selected
    }

    /// Select a node.
    pub fn select(&mut self, node_id: Option<NodeId>) {
        self.selected = node_id;
        if let Some(id) = node_id {
            self.property_editor = Some(PropertyEditor::new(id));
        } else {
            self.property_editor = None;
        }
    }

    /// Set hovered node (typically from mouse position).
    pub fn set_hovered(&mut self, node_id: Option<NodeId>) {
        self.hovered = node_id;
    }

    /// Get tree view state.
    pub fn tree_view(&self) -> &TreeViewState {
        &self.tree_view
    }

    /// Get mutable tree view state.
    pub fn tree_view_mut(&mut self) -> &mut TreeViewState {
        &mut self.tree_view
    }

    /// Get property editor.
    pub fn property_editor(&self) -> Option<&PropertyEditor> {
        self.property_editor.as_ref()
    }

    /// Get mutable property editor.
    pub fn property_editor_mut(&mut self) -> Option<&mut PropertyEditor> {
        self.property_editor.as_mut()
    }

    /// Get graphs.
    pub fn graphs(&self) -> &InspectorGraphs {
        &self.graphs
    }

    /// Get search state.
    pub fn search(&self) -> &SearchState {
        &self.search
    }

    /// Get mutable search state.
    pub fn search_mut(&mut self) -> &mut SearchState {
        &mut self.search
    }

    /// Update inspector state from UI tree and metrics.
    pub fn update(
        &mut self,
        tree: &UiTree,
        registry: &WidgetIdRegistry,
        metrics: Option<&MetricsCollector>,
    ) {
        if !self.enabled {
            return;
        }

        // Update tree view
        self.update_tree_view(tree, registry);

        // Update graphs from metrics
        if let Some(collector) = metrics
            && let Some(frame_metrics) = collector.current_metrics()
        {
            self.graphs.update(frame_metrics);
        }

        // Update property editor if selected
        let update_node = self
            .property_editor
            .as_ref()
            .filter(|editor| tree.get_node(editor.target_node).is_some())
            .map(|editor| editor.target_node);

        if let Some(node_id) = update_node {
            self.update_properties(tree, node_id);
        }

        // Update search if active
        if !self.search.query.is_empty() {
            self.search.search(&self.tree_view.nodes);
        }
    }

    /// Update tree view from UI tree.
    fn update_tree_view(&mut self, tree: &UiTree, registry: &WidgetIdRegistry) {
        self.tree_view.nodes.clear();
        self.widget_kinds.clear();
        self.node_to_widget_id.clear();

        // Build reverse mapping from registry
        // Note: This is O(n) - in production we'd want the registry to maintain this
        // For now, we rebuild each frame when inspector is open

        if let Some(root_id) = tree.root() {
            self.collect_tree_nodes(tree, registry, root_id, 0);
        }

        // Update visibility based on expansion state and filter
        self.update_node_visibility();
    }

    /// Recursively collect tree nodes.
    fn collect_tree_nodes(
        &mut self,
        tree: &UiTree,
        registry: &WidgetIdRegistry,
        node_id: NodeId,
        depth: usize,
    ) {
        let Some(node) = tree.get_node(node_id) else {
            return;
        };

        // Determine widget kind from the widget
        let kind = self.classify_widget(tree, node_id);
        self.widget_kinds.insert(node_id, kind);

        // Try to find widget ID (linear search - could be optimized)
        let widget_id = registry.find_by_node(node_id);
        if let Some(wid) = widget_id {
            self.node_to_widget_id.insert(node_id, wid);
        }

        // Get layout bounds
        let bounds = if let Some(layout) = tree.get_layout(node_id) {
            (layout.x, layout.y, layout.width, layout.height)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };

        // Generate label
        let label = self.generate_node_label(kind, widget_id, node_id);

        let info = TreeNodeInfo {
            node_id,
            widget_id,
            kind,
            label,
            depth,
            child_count: node.children.len(),
            bounds,
            dirty_flags: node.dirty_flags,
            is_expanded: self.tree_view.expanded.contains(&node_id),
            is_visible: true, // Will be updated in update_node_visibility
        };

        self.tree_view.nodes.push(info);

        // Recurse to children if expanded or max depth not reached
        if self.config.max_tree_depth == 0 || depth < self.config.max_tree_depth {
            for &child_id in &node.children {
                self.collect_tree_nodes(tree, registry, child_id, depth + 1);
            }
        }
    }

    /// Classify a widget by examining its type.
    fn classify_widget(&self, tree: &UiTree, node_id: NodeId) -> WidgetKind {
        let Some(widget) = tree.get_widget(node_id) else {
            return WidgetKind::Unknown;
        };

        // Check widget type by downcasting
        let any = widget.as_any();
        if any.is::<crate::widgets::Container>() {
            WidgetKind::Container
        } else if any.is::<crate::widgets::Text>() {
            WidgetKind::Text
        } else if any.is::<crate::widgets::Button>() {
            WidgetKind::Button
        } else if any.is::<crate::widgets::Image>() {
            WidgetKind::Image
        } else if any.is::<crate::widgets::TextInput>() {
            WidgetKind::TextInput
        } else {
            WidgetKind::Custom
        }
    }

    /// Generate a label for a tree node.
    fn generate_node_label(
        &self,
        kind: WidgetKind,
        widget_id: Option<WidgetId>,
        node_id: NodeId,
    ) -> String {
        let kind_str = match kind {
            WidgetKind::Container => "Container",
            WidgetKind::Text => "Text",
            WidgetKind::Button => "Button",
            WidgetKind::Image => "Image",
            WidgetKind::TextInput => "TextInput",
            WidgetKind::Checkbox => "Checkbox",
            WidgetKind::Slider => "Slider",
            WidgetKind::ScrollView => "ScrollView",
            WidgetKind::Custom => "Custom",
            WidgetKind::Unknown => "Unknown",
        };

        if let Some(wid) = widget_id {
            format!("{} {}", kind_str, wid)
        } else {
            format!("{} #{}", kind_str, node_id.0)
        }
    }

    /// Update node visibility based on expansion and filter.
    fn update_node_visibility(&mut self) {
        // Track parent expansion state
        let mut visible_depths: HashMap<usize, bool> = HashMap::new();
        visible_depths.insert(0, true); // Root is always potentially visible

        let filter_lower = self.tree_view.filter.to_lowercase();
        let has_filter = !filter_lower.is_empty();

        for node in &mut self.tree_view.nodes {
            // Check if this node should be visible based on parent expansion
            let parent_visible = visible_depths.get(&node.depth).copied().unwrap_or(false);

            // Check filter match
            let matches_filter = !has_filter || node.label.to_lowercase().contains(&filter_lower);

            node.is_visible = parent_visible && matches_filter;

            // Update visibility for children
            let children_visible = parent_visible && node.is_expanded;
            visible_depths.insert(node.depth + 1, children_visible);
        }
    }

    /// Update properties for selected node.
    fn update_properties(&mut self, tree: &UiTree, node_id: NodeId) {
        let Some(widget) = tree.get_widget(node_id) else {
            return;
        };

        let Some(editor) = &mut self.property_editor else {
            return;
        };

        editor.properties.clear();

        let style = widget.style();

        // Layout properties
        editor.properties.push(EditableProperty {
            name: "background_color".to_string(),
            category: PropertyCategory::Style,
            value: PropertyValue::Color(style.background_color.unwrap_or(Color::TRANSPARENT)),
            affects_layout: false,
        });

        editor.properties.push(EditableProperty {
            name: "border_radius".to_string(),
            category: PropertyCategory::Style,
            value: PropertyValue::Float(style.border_radius),
            affects_layout: false,
        });

        editor.properties.push(EditableProperty {
            name: "border_width".to_string(),
            category: PropertyCategory::Style,
            value: PropertyValue::Float(style.border_width),
            affects_layout: true,
        });

        // Add layout bounds if available
        if let Some(layout) = tree.get_layout(node_id) {
            editor.properties.push(EditableProperty {
                name: "position".to_string(),
                category: PropertyCategory::Layout,
                value: PropertyValue::Vec2(Vec2::new(layout.x, layout.y)),
                affects_layout: false, // Read-only computed value
            });

            editor.properties.push(EditableProperty {
                name: "size".to_string(),
                category: PropertyCategory::Layout,
                value: PropertyValue::Vec2(Vec2::new(layout.width, layout.height)),
                affects_layout: false, // Read-only computed value
            });
        }
    }

    /// Hit test to find node at screen position.
    pub fn hit_test(&self, tree: &UiTree, pos: Vec2) -> Option<NodeId> {
        // Find deepest node containing the point
        let mut result = None;

        for node_info in &self.tree_view.nodes {
            let (_x, _y, w, h) = node_info.bounds;
            if w <= 0.0 || h <= 0.0 {
                continue;
            }

            // Calculate absolute position by walking up tree
            let abs_bounds = self.calculate_absolute_bounds(tree, node_info.node_id)?;
            let (ax, ay, aw, ah) = abs_bounds;

            if pos.x >= ax && pos.x <= ax + aw && pos.y >= ay && pos.y <= ay + ah {
                result = Some(node_info.node_id);
            }
        }

        result
    }

    /// Calculate absolute bounds for a node.
    fn calculate_absolute_bounds(&self, tree: &UiTree, node_id: NodeId) -> Option<(f32, f32, f32, f32)> {
        let layout = tree.get_layout(node_id)?;
        let mut abs_x = layout.x;
        let mut abs_y = layout.y;

        // Walk up to root accumulating positions
        let mut current = tree.get_node(node_id)?.parent;
        while let Some(parent_id) = current {
            if let Some(parent_layout) = tree.get_layout(parent_id) {
                abs_x += parent_layout.x;
                abs_y += parent_layout.y;
            }
            current = tree.get_node(parent_id)?.parent;
        }

        Some((abs_x, abs_y, layout.width, layout.height))
    }

    /// Generate overlay quads for rendering.
    pub fn generate_overlay_quads(&self, tree: &UiTree) -> Vec<OverlayQuad> {
        if !self.enabled {
            return Vec::new();
        }

        let mut quads = Vec::new();

        for node_info in &self.tree_view.nodes {
            let Some((abs_x, abs_y, width, height)) =
                self.calculate_absolute_bounds(tree, node_info.node_id)
            else {
                continue;
            };

            if width <= 0.0 || height <= 0.0 {
                continue;
            }

            let is_selected = self.selected == Some(node_info.node_id);
            let is_hovered = self.hovered == Some(node_info.node_id);

            // Bounds overlay
            if self.config.show_bounds {
                let mut fill_color = node_info.kind.color();
                let mut border_color = node_info.kind.border_color();

                if is_selected {
                    fill_color = Color::rgba(1.0, 0.8, 0.0, 0.3);
                    border_color = Color::rgba(1.0, 0.8, 0.0, 1.0);
                } else if is_hovered && self.config.highlight_hover {
                    fill_color = Color::rgba(0.3, 0.7, 1.0, 0.3);
                    border_color = Color::rgba(0.3, 0.7, 1.0, 1.0);
                }

                quads.push(OverlayQuad {
                    position: Vec2::new(abs_x, abs_y),
                    size: Vec2::new(width, height),
                    fill_color,
                    border_color: Some(border_color),
                    border_width: 1.0,
                });
            }

            // Dirty flag overlay
            if self.config.show_dirty_flags && !node_info.dirty_flags.is_empty() {
                let color = dirty_flags_to_color(node_info.dirty_flags);
                quads.push(OverlayQuad {
                    position: Vec2::new(abs_x + 2.0, abs_y + 2.0),
                    size: Vec2::new(8.0, 8.0),
                    fill_color: color,
                    border_color: None,
                    border_width: 0.0,
                });
            }
        }

        quads
    }

    /// Export tree structure as formatted string.
    pub fn export_tree_snapshot(&self) -> String {
        let mut result = String::new();
        result.push_str("=== UI Tree Snapshot ===\n\n");

        for node in &self.tree_view.nodes {
            let indent = "  ".repeat(node.depth);
            let dirty = if !node.dirty_flags.is_empty() {
                format!(" [DIRTY: {:?}]", node.dirty_flags)
            } else {
                String::new()
            };

            let (x, y, w, h) = node.bounds;
            result.push_str(&format!(
                "{}{}  @ ({:.1}, {:.1}) {}x{}{}\n",
                indent, node.label, x, y, w as i32, h as i32, dirty
            ));
        }

        result
    }

    /// Generate summary text for display.
    pub fn generate_summary_text(&self) -> String {
        let total_nodes = self.tree_view.nodes.len();
        let dirty_nodes = self.tree_view.nodes.iter().filter(|n| !n.dirty_flags.is_empty()).count();
        let avg_frame_time = self.graphs.avg_frame_time();

        format!(
            "Nodes: {} | Dirty: {} | Avg Frame: {:.2}ms | FPS: {:.1}",
            total_nodes,
            dirty_nodes,
            avg_frame_time,
            if avg_frame_time > 0.0 {
                1000.0 / avg_frame_time
            } else {
                0.0
            }
        )
    }

    /// Generate tree view text for display.
    pub fn generate_tree_text(&self) -> String {
        let mut result = String::new();

        for node in self.tree_view.visible_nodes() {
            let indent = "  ".repeat(node.depth);
            let expand_marker = if node.child_count > 0 {
                if node.is_expanded {
                    "▼ "
                } else {
                    "▶ "
                }
            } else {
                "  "
            };
            let selected_marker = if self.selected == Some(node.node_id) {
                " ◄"
            } else {
                ""
            };

            result.push_str(&format!(
                "{}{}{}{}\n",
                indent, expand_marker, node.label, selected_marker
            ));
        }

        result
    }

    /// Generate properties text for selected widget.
    pub fn generate_properties_text(&self) -> String {
        let Some(editor) = &self.property_editor else {
            return "No widget selected".to_string();
        };

        let mut result = format!("=== Node {} Properties ===\n\n", editor.target_node.0);

        let mut current_category: Option<PropertyCategory> = None;

        for prop in &editor.properties {
            if current_category != Some(prop.category) {
                current_category = Some(prop.category);
                result.push_str(&format!("\n[{:?}]\n", prop.category));
            }

            let value_str = match &prop.value {
                PropertyValue::Float(f) => format!("{:.2}", f),
                PropertyValue::Int(i) => format!("{}", i),
                PropertyValue::Bool(b) => format!("{}", b),
                PropertyValue::Color(c) => format!("#{:02X}{:02X}{:02X}{:02X}",
                    (c.r * 255.0) as u8,
                    (c.g * 255.0) as u8,
                    (c.b * 255.0) as u8,
                    (c.a * 255.0) as u8),
                PropertyValue::String(s) => format!("\"{}\"", s),
                PropertyValue::Vec2(v) => format!("({:.1}, {:.1})", v.x, v.y),
            };

            result.push_str(&format!("  {}: {}\n", prop.name, value_str));
        }

        result
    }
}

impl Default for UiInspector {
    fn default() -> Self {
        Self::new(InspectorConfig::default())
    }
}

/// Convert dirty flags to overlay color.
fn dirty_flags_to_color(flags: DirtyFlags) -> Color {
    if flags.contains(DirtyFlags::LAYOUT) {
        Color::rgba(1.0, 0.0, 0.0, 0.8) // Red for layout
    } else if flags.contains(DirtyFlags::TEXT_SHAPING) {
        Color::rgba(1.0, 0.5, 0.0, 0.8) // Orange for text
    } else if flags.contains(DirtyFlags::GEOMETRY) {
        Color::rgba(1.0, 1.0, 0.0, 0.8) // Yellow for geometry
    } else if flags.contains(DirtyFlags::COLOR_ONLY) {
        Color::rgba(0.0, 1.0, 0.0, 0.8) // Green for color
    } else if flags.contains(DirtyFlags::OPACITY_ONLY) {
        Color::rgba(0.0, 0.8, 0.8, 0.8) // Cyan for opacity
    } else {
        Color::rgba(0.5, 0.5, 0.5, 0.8) // Gray for other
    }
}

/// Extension trait for WidgetIdRegistry to find by node.
pub trait WidgetIdRegistryExt {
    fn find_by_node(&self, node_id: NodeId) -> Option<WidgetId>;
}

impl WidgetIdRegistryExt for WidgetIdRegistry {
    fn find_by_node(&self, node_id: NodeId) -> Option<WidgetId> {
        // Linear search through registry - could be optimized with reverse mapping
        self.iter()
            .find(|(_, nid)| *nid == node_id)
            .map(|(wid, _)| wid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inspector_toggle() {
        let mut inspector = UiInspector::new(InspectorConfig::default());
        assert!(!inspector.is_enabled());

        inspector.toggle();
        assert!(inspector.is_enabled());

        inspector.toggle();
        assert!(!inspector.is_enabled());
    }

    #[test]
    fn test_inspector_selection() {
        let mut inspector = UiInspector::new(InspectorConfig::default());
        let node_id = NodeId(42);

        inspector.select(Some(node_id));
        assert_eq!(inspector.selected(), Some(node_id));
        assert!(inspector.property_editor().is_some());

        inspector.select(None);
        assert_eq!(inspector.selected(), None);
        assert!(inspector.property_editor().is_none());
    }

    #[test]
    fn test_tree_view_expansion() {
        let mut tree_view = TreeViewState::new();
        let node_id = NodeId(1);

        assert!(!tree_view.expanded.contains(&node_id));

        tree_view.toggle_expand(node_id);
        assert!(tree_view.expanded.contains(&node_id));

        tree_view.toggle_expand(node_id);
        assert!(!tree_view.expanded.contains(&node_id));
    }

    #[test]
    fn test_search_state() {
        let mut search = SearchState::default();
        let nodes = vec![
            TreeNodeInfo {
                node_id: NodeId(1),
                widget_id: None,
                kind: WidgetKind::Button,
                label: "Button \"submit\"".to_string(),
                depth: 0,
                child_count: 0,
                bounds: (0.0, 0.0, 100.0, 50.0),
                dirty_flags: DirtyFlags::NONE,
                is_expanded: false,
                is_visible: true,
            },
            TreeNodeInfo {
                node_id: NodeId(2),
                widget_id: None,
                kind: WidgetKind::Text,
                label: "Text \"hello\"".to_string(),
                depth: 0,
                child_count: 0,
                bounds: (0.0, 0.0, 100.0, 20.0),
                dirty_flags: DirtyFlags::NONE,
                is_expanded: false,
                is_visible: true,
            },
        ];

        search.query = "button".to_string();
        search.search(&nodes);

        assert_eq!(search.results.len(), 1);
        assert_eq!(search.results[0], NodeId(1));
    }

    #[test]
    fn test_graph_update() {
        let mut graphs = InspectorGraphs::new(10);

        for i in 0..15 {
            let metrics = FrameTimingMetrics {
                frame_id: i,
                nodes_layout_dirty: (i % 5) as usize,
                nodes_text_dirty: 0,
                nodes_paint_dirty: 0,
                ..Default::default()
            };
            graphs.update(&metrics);
        }

        // Should be trimmed to max_size
        assert_eq!(graphs.frame_times.len(), 10);
        assert_eq!(graphs.dirty_counts.len(), 10);
    }

    #[test]
    fn test_widget_kind_colors() {
        // Each widget kind should have a unique color
        let kinds = [
            WidgetKind::Container,
            WidgetKind::Text,
            WidgetKind::Button,
            WidgetKind::Image,
        ];

        for (i, kind1) in kinds.iter().enumerate() {
            for (j, kind2) in kinds.iter().enumerate() {
                if i != j {
                    assert_ne!(kind1.color(), kind2.color());
                }
            }
        }
    }

    #[test]
    fn test_dirty_flags_color() {
        let layout_color = dirty_flags_to_color(DirtyFlags::LAYOUT);
        let text_color = dirty_flags_to_color(DirtyFlags::TEXT_SHAPING);
        let color_only = dirty_flags_to_color(DirtyFlags::COLOR_ONLY);

        assert_ne!(layout_color, text_color);
        assert_ne!(text_color, color_only);
    }

    #[test]
    fn test_property_editor() {
        let node_id = NodeId(1);
        let mut editor = PropertyEditor::new(node_id);

        assert_eq!(editor.target_node, node_id);
        assert!(!editor.has_pending_changes());

        editor.set_property("color".to_string(), PropertyValue::Color(Color::RED));
        assert!(editor.has_pending_changes());

        editor.clear_pending();
        assert!(!editor.has_pending_changes());
    }

    #[test]
    fn test_search_navigation() {
        let mut search = SearchState::default();
        let nodes = vec![
            TreeNodeInfo {
                node_id: NodeId(1),
                widget_id: None,
                kind: WidgetKind::Button,
                label: "Button 1".to_string(),
                depth: 0,
                child_count: 0,
                bounds: (0.0, 0.0, 100.0, 50.0),
                dirty_flags: DirtyFlags::NONE,
                is_expanded: false,
                is_visible: true,
            },
            TreeNodeInfo {
                node_id: NodeId(2),
                widget_id: None,
                kind: WidgetKind::Button,
                label: "Button 2".to_string(),
                depth: 0,
                child_count: 0,
                bounds: (0.0, 0.0, 100.0, 50.0),
                dirty_flags: DirtyFlags::NONE,
                is_expanded: false,
                is_visible: true,
            },
            TreeNodeInfo {
                node_id: NodeId(3),
                widget_id: None,
                kind: WidgetKind::Button,
                label: "Button 3".to_string(),
                depth: 0,
                child_count: 0,
                bounds: (0.0, 0.0, 100.0, 50.0),
                dirty_flags: DirtyFlags::NONE,
                is_expanded: false,
                is_visible: true,
            },
        ];

        search.query = "Button".to_string();
        search.search(&nodes);

        assert_eq!(search.results.len(), 3);

        // Navigate forward
        assert_eq!(search.next_result(), Some(NodeId(2)));
        assert_eq!(search.next_result(), Some(NodeId(3)));
        assert_eq!(search.next_result(), Some(NodeId(1))); // Wraps around

        // Navigate backward
        assert_eq!(search.prev_result(), Some(NodeId(3)));
        assert_eq!(search.prev_result(), Some(NodeId(2)));
    }

    #[test]
    fn test_search_empty_query() {
        let mut search = SearchState::default();
        let nodes = vec![TreeNodeInfo {
            node_id: NodeId(1),
            widget_id: None,
            kind: WidgetKind::Button,
            label: "Button".to_string(),
            depth: 0,
            child_count: 0,
            bounds: (0.0, 0.0, 100.0, 50.0),
            dirty_flags: DirtyFlags::NONE,
            is_expanded: false,
            is_visible: true,
        }];

        search.query = "".to_string();
        search.search(&nodes);

        assert!(search.results.is_empty());
        assert_eq!(search.next_result(), None);
        assert_eq!(search.prev_result(), None);
    }

    #[test]
    fn test_inspector_config() {
        let config = InspectorConfig {
            show_bounds: false,
            show_dirty_flags: true,
            show_graphs: false,
            show_tree_view: true,
            show_properties: false,
            max_tree_depth: 5,
            graph_history_size: 60,
            highlight_hover: false,
            show_layout_details: true,
        };

        let inspector = UiInspector::new(config.clone());
        assert_eq!(inspector.config().max_tree_depth, 5);
        assert_eq!(inspector.config().graph_history_size, 60);
        assert!(!inspector.config().show_bounds);
    }

    #[test]
    fn test_inspector_enable_disable() {
        let mut inspector = UiInspector::new(InspectorConfig::default());

        inspector.enable();
        assert!(inspector.is_enabled());

        inspector.disable();
        assert!(!inspector.is_enabled());
    }

    #[test]
    fn test_inspector_hover() {
        let mut inspector = UiInspector::new(InspectorConfig::default());

        inspector.set_hovered(Some(NodeId(42)));
        // Hovered is internal state, just verify it doesn't panic

        inspector.set_hovered(None);
    }

    #[test]
    fn test_tree_view_expand_collapse_all() {
        let mut tree_view = TreeViewState::new();
        tree_view.nodes = vec![
            TreeNodeInfo {
                node_id: NodeId(1),
                widget_id: None,
                kind: WidgetKind::Container,
                label: "Root".to_string(),
                depth: 0,
                child_count: 2,
                bounds: (0.0, 0.0, 100.0, 100.0),
                dirty_flags: DirtyFlags::NONE,
                is_expanded: false,
                is_visible: true,
            },
            TreeNodeInfo {
                node_id: NodeId(2),
                widget_id: None,
                kind: WidgetKind::Container,
                label: "Child 1".to_string(),
                depth: 1,
                child_count: 1,
                bounds: (0.0, 0.0, 50.0, 50.0),
                dirty_flags: DirtyFlags::NONE,
                is_expanded: false,
                is_visible: true,
            },
        ];

        tree_view.expand_all();
        assert!(tree_view.expanded.contains(&NodeId(1)));
        assert!(tree_view.expanded.contains(&NodeId(2)));

        tree_view.collapse_all();
        assert!(tree_view.expanded.is_empty());
    }

    #[test]
    fn test_tree_view_filter() {
        let mut tree_view = TreeViewState::new();
        tree_view.set_filter("test".to_string());
        assert_eq!(tree_view.filter, "test");
    }

    #[test]
    fn test_property_value_types() {
        let float_val = PropertyValue::Float(3.14);
        let int_val = PropertyValue::Int(42);
        let bool_val = PropertyValue::Bool(true);
        let color_val = PropertyValue::Color(Color::RED);
        let string_val = PropertyValue::String("hello".to_string());
        let vec2_val = PropertyValue::Vec2(Vec2::new(1.0, 2.0));

        assert!(matches!(float_val, PropertyValue::Float(_)));
        assert!(matches!(int_val, PropertyValue::Int(_)));
        assert!(matches!(bool_val, PropertyValue::Bool(_)));
        assert!(matches!(color_val, PropertyValue::Color(_)));
        assert!(matches!(string_val, PropertyValue::String(_)));
        assert!(matches!(vec2_val, PropertyValue::Vec2(_)));
    }

    #[test]
    fn test_editable_property() {
        let prop = EditableProperty {
            name: "width".to_string(),
            category: PropertyCategory::Layout,
            value: PropertyValue::Float(100.0),
            affects_layout: true,
        };

        assert_eq!(prop.name, "width");
        assert_eq!(prop.category, PropertyCategory::Layout);
        assert!(prop.affects_layout);
    }

    #[test]
    fn test_widget_kind_all_variants() {
        let kinds = [
            WidgetKind::Container,
            WidgetKind::Text,
            WidgetKind::Button,
            WidgetKind::Image,
            WidgetKind::TextInput,
            WidgetKind::Checkbox,
            WidgetKind::Slider,
            WidgetKind::ScrollView,
            WidgetKind::Custom,
            WidgetKind::Unknown,
        ];

        // All kinds should have a color
        for kind in &kinds {
            let color = kind.color();
            assert!(color.a > 0.0);
        }

        // All kinds should have a border color
        for kind in &kinds {
            let border = kind.border_color();
            assert!(border.a > kind.color().a); // Border should be more opaque
        }
    }

    #[test]
    fn test_inspector_graphs_clear() {
        let mut graphs = InspectorGraphs::new(10);

        // Add some data
        let metrics = FrameTimingMetrics {
            frame_id: 1,
            nodes_layout_dirty: 5,
            ..Default::default()
        };
        graphs.update(&metrics);

        assert!(!graphs.frame_times.is_empty());

        graphs.clear();
        assert!(graphs.frame_times.is_empty());
        assert!(graphs.layout_times.is_empty());
        assert!(graphs.text_times.is_empty());
        assert!(graphs.dirty_counts.is_empty());
    }

    #[test]
    fn test_inspector_graphs_averages() {
        let mut graphs = InspectorGraphs::new(10);

        // Add data with known frame times
        for i in 0..5 {
            let mut metrics = FrameTimingMetrics::new(i);
            metrics.total_layout_time = std::time::Duration::from_millis(10);
            graphs.update(&metrics);
        }

        let avg = graphs.avg_frame_time();
        assert!(avg >= 10.0); // At least 10ms from layout time
    }

    #[test]
    fn test_inspector_default() {
        let inspector = UiInspector::default();
        assert!(!inspector.is_enabled());
        assert!(inspector.selected().is_none());
    }

    #[test]
    fn test_overlay_quad() {
        let quad = OverlayQuad {
            position: Vec2::new(10.0, 20.0),
            size: Vec2::new(100.0, 50.0),
            fill_color: Color::RED,
            border_color: Some(Color::WHITE),
            border_width: 2.0,
        };

        assert_eq!(quad.position.x, 10.0);
        assert_eq!(quad.size.y, 50.0);
        assert!(quad.border_color.is_some());
    }

    #[test]
    fn test_dirty_flags_to_color_opacity() {
        let layout = dirty_flags_to_color(DirtyFlags::LAYOUT);
        let opacity = dirty_flags_to_color(DirtyFlags::OPACITY_ONLY);

        // Both should have high alpha for visibility
        assert!(layout.a >= 0.8);
        assert!(opacity.a >= 0.8);
    }
}
