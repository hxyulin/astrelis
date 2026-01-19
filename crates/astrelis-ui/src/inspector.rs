//! Developer tools - UI inspector and layout debugger.
//!
//! The UI inspector provides visual debugging tools for the UI system:
//! - Widget bounds visualization (colored by type)
//! - Dirty flag display (color-coded)
//! - Layout tree hierarchy view
//! - Selected widget properties
//! - Performance metrics
//!
//! # Example
//!
//! ```ignore
//! use astrelis_ui::*;
//!
//! let mut inspector = UiInspector::new();
//!
//! // Toggle inspector with F12
//! if keyboard.just_pressed(KeyCode::F12) {
//!     inspector.toggle();
//! }
//!
//! // Render overlay and inspector panel
//! if inspector.is_enabled() {
//!     inspector.render_overlay(pass, tree);
//!     inspector.render_panel(ui);
//! }
//! ```

use crate::{
    DirtyFlags, NodeId, UiTree, WidgetId, WidgetIdRegistry,
};
use astrelis_core::math::Vec2;
use astrelis_render::Color;

/// Widget type for visualization colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WidgetType {
    Container,
    Text,
    Button,
    Image,
    Input,
    Checkbox,
    Slider,
    Other,
}

impl WidgetType {
    /// Get the color for this widget type.
    pub fn color(&self) -> Color {
        match self {
            Self::Container => Color::rgba(0.2, 0.5, 0.8, 0.3),  // Blue
            Self::Text => Color::rgba(0.5, 0.8, 0.2, 0.3),       // Green
            Self::Button => Color::rgba(0.8, 0.5, 0.2, 0.3),     // Orange
            Self::Image => Color::rgba(0.8, 0.2, 0.5, 0.3),      // Magenta
            Self::Input => Color::rgba(0.2, 0.8, 0.8, 0.3),      // Cyan
            Self::Checkbox => Color::rgba(0.8, 0.8, 0.2, 0.3),   // Yellow
            Self::Slider => Color::rgba(0.5, 0.2, 0.8, 0.3),     // Purple
            Self::Other => Color::rgba(0.5, 0.5, 0.5, 0.3),      // Gray
        }
    }
}

/// Inspector display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorMode {
    /// Show widget bounds
    Bounds,
    /// Show dirty flags
    DirtyFlags,
    /// Show layout metrics
    Layout,
    /// Show all information
    All,
}

impl Default for InspectorMode {
    fn default() -> Self {
        Self::Bounds
    }
}

/// Widget information for inspector display.
#[derive(Debug, Clone)]
pub struct InspectedWidget {
    /// Node ID
    pub node_id: NodeId,
    /// Widget ID (if any)
    pub widget_id: Option<WidgetId>,
    /// Widget type
    pub widget_type: WidgetType,
    /// Bounding rectangle (x, y, width, height)
    pub bounds: (f32, f32, f32, f32),
    /// Dirty flags
    pub dirty_flags: DirtyFlags,
    /// Number of children
    pub child_count: usize,
    /// Depth in tree
    pub depth: usize,
}

/// UI inspector for visual debugging.
pub struct UiInspector {
    /// Inspector enabled state
    enabled: bool,
    /// Current display mode
    mode: InspectorMode,
    /// Selected widget node ID
    selected: Option<NodeId>,
    /// Show bounds overlay
    show_bounds: bool,
    /// Show dirty flags
    show_dirty_flags: bool,
    /// Show layout tree
    show_layout_tree: bool,
    /// Cached inspected widgets
    widgets: Vec<InspectedWidget>,
    /// Performance metrics
    metrics: InspectorMetrics,
}

/// Performance metrics for the inspector.
#[derive(Debug, Clone, Default)]
pub struct InspectorMetrics {
    /// Total widget count
    pub widget_count: usize,
    /// Dirty widget count
    pub dirty_count: usize,
    /// Layout computation time (ms)
    pub layout_time_ms: f32,
    /// Render time (ms)
    pub render_time_ms: f32,
    /// Frame time (ms)
    pub frame_time_ms: f32,
}

impl UiInspector {
    /// Create a new UI inspector (disabled by default).
    pub fn new() -> Self {
        Self {
            enabled: false,
            mode: InspectorMode::default(),
            selected: None,
            show_bounds: true,
            show_dirty_flags: true,
            show_layout_tree: true,
            widgets: Vec::new(),
            metrics: InspectorMetrics::default(),
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

    /// Set the display mode.
    pub fn set_mode(&mut self, mode: InspectorMode) {
        self.mode = mode;
    }

    /// Select a widget by node ID.
    pub fn select_widget(&mut self, node_id: NodeId) {
        self.selected = Some(node_id);
    }

    /// Clear widget selection.
    pub fn clear_selection(&mut self) {
        self.selected = None;
    }

    /// Get the selected widget node ID.
    pub fn selected(&self) -> Option<NodeId> {
        self.selected
    }

    /// Toggle bounds overlay.
    pub fn toggle_bounds(&mut self) {
        self.show_bounds = !self.show_bounds;
    }

    /// Toggle dirty flags display.
    pub fn toggle_dirty_flags(&mut self) {
        self.show_dirty_flags = !self.show_dirty_flags;
    }

    /// Toggle layout tree display.
    pub fn toggle_layout_tree(&mut self) {
        self.show_layout_tree = !self.show_layout_tree;
    }

    /// Update inspector data from UI tree.
    pub fn update(&mut self, tree: &UiTree, registry: &WidgetIdRegistry) {
        self.widgets.clear();

        // Traverse tree and collect widget info
        if let Some(root_id) = tree.root() {
            self.collect_widgets(tree, registry, root_id, 0);
        }

        // Update metrics
        self.metrics.widget_count = self.widgets.len();
        self.metrics.dirty_count = self.widgets.iter().filter(|w| !w.dirty_flags.is_empty()).count();
    }

    /// Update performance metrics.
    pub fn update_metrics(&mut self, layout_time_ms: f32, render_time_ms: f32, frame_time_ms: f32) {
        self.metrics.layout_time_ms = layout_time_ms;
        self.metrics.render_time_ms = render_time_ms;
        self.metrics.frame_time_ms = frame_time_ms;
    }

    /// Get widget information by node ID.
    pub fn get_widget_info(&self, node_id: NodeId) -> Option<&InspectedWidget> {
        self.widgets.iter().find(|w| w.node_id == node_id)
    }

    /// Get all inspected widgets.
    pub fn widgets(&self) -> &[InspectedWidget] {
        &self.widgets
    }

    /// Get performance metrics.
    pub fn metrics(&self) -> &InspectorMetrics {
        &self.metrics
    }

    /// Hit test to find widget at screen position.
    pub fn hit_test(&self, pos: Vec2) -> Option<NodeId> {
        // Find the deepest (last in list) widget that contains the point
        self.widgets
            .iter()
            .rev()
            .find(|w| {
                let (x, y, width, height) = w.bounds;
                pos.x >= x && pos.x <= x + width && pos.y >= y && pos.y <= y + height
            })
            .map(|w| w.node_id)
    }

    /// Generate overlay visualization data.
    ///
    /// Returns rectangles as (x, y, width, height, color).
    pub fn generate_overlay_rects(&self) -> Vec<(f32, f32, f32, f32, Color)> {
        let mut rects = Vec::new();

        for widget in &self.widgets {
            let (x, y, width, height) = widget.bounds;

            // Skip zero-sized widgets
            if width <= 0.0 || height <= 0.0 {
                continue;
            }

            // Bounds overlay
            if self.show_bounds && matches!(self.mode, InspectorMode::Bounds | InspectorMode::All) {
                let mut color = widget.widget_type.color();

                // Highlight selected widget
                if Some(widget.node_id) == self.selected {
                    color = Color::rgba(1.0, 1.0, 0.0, 0.5); // Yellow
                }

                rects.push((x, y, width, height, color));
            }

            // Dirty flags overlay
            if self.show_dirty_flags && !widget.dirty_flags.is_empty()
                && matches!(self.mode, InspectorMode::DirtyFlags | InspectorMode::All) {
                let color = dirty_flags_color(widget.dirty_flags);
                rects.push((x, y, width, height, color));
            }
        }

        rects
    }

    /// Generate layout tree text for display.
    pub fn generate_layout_tree_text(&self) -> String {
        let mut result = String::new();
        result.push_str("UI Tree:\n");

        for widget in &self.widgets {
            let indent = "  ".repeat(widget.depth);
            let selected = if self.selected == Some(widget.node_id) { " [SELECTED]" } else { "" };
            let dirty = if !widget.dirty_flags.is_empty() { " (DIRTY)" } else { "" };

            result.push_str(&format!(
                "{}Node {:?} - {:?}{}{}\n",
                indent, widget.node_id, widget.widget_type, selected, dirty
            ));
        }

        result
    }

    /// Generate metrics text for display.
    pub fn generate_metrics_text(&self) -> String {
        format!(
            "Performance:\n\
             Widget Count: {}\n\
             Dirty Widgets: {}\n\
             Layout Time: {:.2}ms\n\
             Render Time: {:.2}ms\n\
             Frame Time: {:.2}ms\n\
             FPS: {:.1}",
            self.metrics.widget_count,
            self.metrics.dirty_count,
            self.metrics.layout_time_ms,
            self.metrics.render_time_ms,
            self.metrics.frame_time_ms,
            if self.metrics.frame_time_ms > 0.0 {
                1000.0 / self.metrics.frame_time_ms
            } else {
                0.0
            }
        )
    }

    /// Generate selected widget details text.
    pub fn generate_selected_widget_text(&self) -> String {
        if let Some(node_id) = self.selected {
            if let Some(widget) = self.get_widget_info(node_id) {
                let (x, y, w, h) = widget.bounds;
                return format!(
                    "Selected Widget:\n\
                     Node: {:?}\n\
                     Type: {:?}\n\
                     Bounds: ({:.1}, {:.1}, {:.1}, {:.1})\n\
                     Children: {}\n\
                     Depth: {}\n\
                     Dirty: {}",
                    widget.node_id,
                    widget.widget_type,
                    x, y, w, h,
                    widget.child_count,
                    widget.depth,
                    if !widget.dirty_flags.is_empty() {
                        format!("{:?}", widget.dirty_flags)
                    } else {
                        "None".to_string()
                    }
                );
            }
        }
        "No widget selected".to_string()
    }

    // Private helper methods

    fn collect_widgets(
        &mut self,
        tree: &UiTree,
        registry: &WidgetIdRegistry,
        node_id: NodeId,
        depth: usize,
    ) {
        if let Some(node) = tree.get_node(node_id) {
            // Get widget ID from registry (reverse lookup)
            let widget_id = registry.get_widget_by_node(node_id);

            // Determine widget type from widget name or type
            let widget_type = classify_widget(node_id);

            // Get bounds from layout
            let bounds = if let Some(layout) = tree.get_layout(node_id) {
                (layout.x, layout.y, layout.width, layout.height)
            } else {
                (0.0, 0.0, 0.0, 0.0)
            };

            let inspected = InspectedWidget {
                node_id,
                widget_id,
                widget_type,
                bounds,
                dirty_flags: node.dirty_flags,
                child_count: node.children.len(),
                depth,
            };

            self.widgets.push(inspected);

            // Recurse into children
            for &child_id in &node.children {
                self.collect_widgets(tree, registry, child_id, depth + 1);
            }
        }
    }
}

impl Default for UiInspector {
    fn default() -> Self {
        Self::new()
    }
}

/// Classify a widget by its type for visualization.
fn classify_widget(_node_id: NodeId) -> WidgetType {
    // Try to determine widget type from the widget trait object
    // This is a simplified version - in real usage you'd need additional traits
    // to inspect widget types at runtime. For now, return Other.
    // TODO: Add type inspection via a new trait method
    WidgetType::Other
}

/// Get color for dirty flags visualization.
fn dirty_flags_color(flags: DirtyFlags) -> Color {
    if flags.contains(DirtyFlags::LAYOUT) {
        Color::rgba(1.0, 0.0, 0.0, 0.4) // Red - layout dirty
    } else if flags.contains(DirtyFlags::TEXT_SHAPING) {
        Color::rgba(1.0, 0.5, 0.0, 0.4) // Orange - text shaping dirty
    } else if flags.contains(DirtyFlags::GEOMETRY) {
        Color::rgba(1.0, 1.0, 0.0, 0.4) // Yellow - geometry dirty
    } else if flags.contains(DirtyFlags::COLOR_ONLY) {
        Color::rgba(0.0, 1.0, 0.0, 0.4) // Green - color only dirty
    } else {
        Color::rgba(0.5, 0.5, 0.5, 0.4) // Gray - other
    }
}

/// Extension trait for WidgetIdRegistry to support reverse lookups.
pub trait WidgetIdRegistryExt {
    /// Get widget ID by node ID (reverse lookup).
    fn get_widget_by_node(&self, node_id: NodeId) -> Option<WidgetId>;
}

impl WidgetIdRegistryExt for WidgetIdRegistry {
    fn get_widget_by_node(&self, _node_id: NodeId) -> Option<WidgetId> {
        // This is a linear search - in production you'd want a reverse mapping
        // For now, we'll return None as the registry doesn't expose iteration
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inspector_new() {
        let inspector = UiInspector::new();
        assert!(!inspector.is_enabled());
        assert_eq!(inspector.widgets().len(), 0);
    }

    #[test]
    fn test_inspector_toggle() {
        let mut inspector = UiInspector::new();
        assert!(!inspector.is_enabled());

        inspector.toggle();
        assert!(inspector.is_enabled());

        inspector.toggle();
        assert!(!inspector.is_enabled());
    }

    #[test]
    fn test_inspector_enable_disable() {
        let mut inspector = UiInspector::new();

        inspector.enable();
        assert!(inspector.is_enabled());

        inspector.disable();
        assert!(!inspector.is_enabled());
    }

    #[test]
    fn test_inspector_mode() {
        let mut inspector = UiInspector::new();
        assert_eq!(inspector.mode, InspectorMode::Bounds);

        inspector.set_mode(InspectorMode::DirtyFlags);
        assert_eq!(inspector.mode, InspectorMode::DirtyFlags);
    }

    #[test]
    fn test_inspector_selection() {
        let mut inspector = UiInspector::new();
        let node_id = NodeId(1);

        assert_eq!(inspector.selected(), None);

        inspector.select_widget(node_id);
        assert_eq!(inspector.selected(), Some(node_id));

        inspector.clear_selection();
        assert_eq!(inspector.selected(), None);
    }

    #[test]
    fn test_inspector_metrics() {
        let mut inspector = UiInspector::new();
        inspector.update_metrics(1.5, 2.5, 16.6);

        let metrics = inspector.metrics();
        assert_eq!(metrics.layout_time_ms, 1.5);
        assert_eq!(metrics.render_time_ms, 2.5);
        assert_eq!(metrics.frame_time_ms, 16.6);
    }

    #[test]
    fn test_widget_type_colors() {
        assert_ne!(WidgetType::Container.color(), WidgetType::Text.color());
        assert_ne!(WidgetType::Button.color(), WidgetType::Image.color());
    }

    #[test]
    fn test_dirty_flags_colors() {
        let layout_color = dirty_flags_color(DirtyFlags::LAYOUT);
        let text_color = dirty_flags_color(DirtyFlags::TEXT_SHAPING);
        let geometry_color = dirty_flags_color(DirtyFlags::GEOMETRY);
        let color_only = dirty_flags_color(DirtyFlags::COLOR_ONLY);

        // All should be different
        assert_ne!(layout_color, text_color);
        assert_ne!(text_color, geometry_color);
        assert_ne!(geometry_color, color_only);
    }

    #[test]
    fn test_inspector_text_generation() {
        let mut inspector = UiInspector::new();
        inspector.enable();
        inspector.update_metrics(1.0, 2.0, 16.0);

        let metrics_text = inspector.generate_metrics_text();
        assert!(metrics_text.contains("Performance"));
        assert!(metrics_text.contains("1.00ms"));
        assert!(metrics_text.contains("2.00ms"));

        let selected_text = inspector.generate_selected_widget_text();
        assert!(selected_text.contains("No widget selected"));
    }

    #[test]
    fn test_inspector_overlay_generation() {
        let inspector = UiInspector::new();
        let rects = inspector.generate_overlay_rects();
        assert_eq!(rects.len(), 0); // No widgets yet
    }

    #[test]
    fn test_inspector_hit_test_empty() {
        let inspector = UiInspector::new();
        let result = inspector.hit_test(Vec2::new(100.0, 100.0));
        assert_eq!(result, None);
    }
}
