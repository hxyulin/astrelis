//! Visual debug overlay for UI system.

use astrelis_render::Color;

use crate::dirty::DirtyFlags;
use crate::metrics::UiMetrics;
use crate::tree::{LayoutRect, NodeId, UiNode, UiTree};

/// Configuration for debug overlay visualization.
#[derive(Debug, Clone)]
pub struct DebugOverlay {
    /// Show rectangles around dirty nodes
    pub show_dirty_rects: bool,

    /// Show layout bounds for all nodes
    pub show_layout_bounds: bool,

    /// Show performance metrics as text overlay
    pub show_metrics: bool,

    /// Show node IDs
    pub show_node_ids: bool,

    /// Show dirty flags as text labels
    pub show_dirty_flags: bool,

    /// Highlight nodes with specific dirty flags
    pub highlight_layout_dirty: bool,
    pub highlight_text_dirty: bool,
    pub highlight_paint_only: bool,

    /// Opacity of debug overlays (0.0 to 1.0)
    pub overlay_opacity: f32,
}

impl Default for DebugOverlay {
    fn default() -> Self {
        Self {
            show_dirty_rects: false,
            show_layout_bounds: false,
            show_metrics: false,
            show_node_ids: false,
            show_dirty_flags: false,
            highlight_layout_dirty: false,
            highlight_text_dirty: false,
            highlight_paint_only: false,
            overlay_opacity: 0.5,
        }
    }
}

impl DebugOverlay {
    /// Create a new debug overlay with all features disabled.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable all debug features.
    pub fn all() -> Self {
        Self {
            show_dirty_rects: true,
            show_layout_bounds: true,
            show_metrics: true,
            show_node_ids: true,
            show_dirty_flags: true,
            highlight_layout_dirty: true,
            highlight_text_dirty: true,
            highlight_paint_only: true,
            overlay_opacity: 0.7,
        }
    }

    /// Enable only dirty rect visualization.
    pub fn dirty_rects_only() -> Self {
        Self {
            show_dirty_rects: true,
            ..Default::default()
        }
    }

    /// Enable only layout bounds.
    pub fn layout_bounds_only() -> Self {
        Self {
            show_layout_bounds: true,
            ..Default::default()
        }
    }

    /// Enable only metrics display.
    pub fn metrics_only() -> Self {
        Self {
            show_metrics: true,
            ..Default::default()
        }
    }

    /// Check if any debug features are enabled.
    pub fn is_enabled(&self) -> bool {
        self.show_dirty_rects
            || self.show_layout_bounds
            || self.show_metrics
            || self.show_node_ids
            || self.show_dirty_flags
            || self.highlight_layout_dirty
            || self.highlight_text_dirty
            || self.highlight_paint_only
    }

    /// Toggle dirty rect visualization.
    pub fn toggle_dirty_rects(&mut self) {
        self.show_dirty_rects = !self.show_dirty_rects;
    }

    /// Toggle layout bounds visualization.
    pub fn toggle_layout_bounds(&mut self) {
        self.show_layout_bounds = !self.show_layout_bounds;
    }

    /// Toggle metrics display.
    pub fn toggle_metrics(&mut self) {
        self.show_metrics = !self.show_metrics;
    }
}

/// Information about a node to be rendered in the debug overlay.
#[derive(Debug, Clone)]
pub struct DebugNodeInfo {
    pub node_id: NodeId,
    pub layout: LayoutRect,
    pub dirty_flags: DirtyFlags,
    pub color: Color,
    pub label: Option<String>,
}

impl DebugNodeInfo {
    /// Create debug info for a node.
    pub fn from_node(node_id: NodeId, node: &UiNode, overlay: &DebugOverlay) -> Option<Self> {
        let dirty_flags = node.dirty_flags;

        // Determine if this node should be shown
        if !overlay.show_dirty_rects
            && !overlay.show_layout_bounds
            && !should_highlight(dirty_flags, overlay)
        {
            return None;
        }

        let color = get_debug_color(dirty_flags, overlay);
        let label = if overlay.show_node_ids || overlay.show_dirty_flags {
            Some(format_label(node_id, dirty_flags, overlay))
        } else {
            None
        };

        Some(DebugNodeInfo {
            node_id,
            layout: node.layout,
            dirty_flags,
            color,
            label,
        })
    }
}

/// Get the debug color for a node based on its dirty flags.
fn get_debug_color(flags: DirtyFlags, overlay: &DebugOverlay) -> Color {
    if flags.is_empty() && overlay.show_layout_bounds {
        // Clean node - gray
        return Color::from_rgba_u8(128, 128, 128, (overlay.overlay_opacity * 255.0) as u8);
    }

    // Priority-based coloring
    if flags.contains(DirtyFlags::LAYOUT) && overlay.highlight_layout_dirty {
        // Layout dirty - red
        Color::from_rgba_u8(255, 0, 0, (overlay.overlay_opacity * 255.0) as u8)
    } else if flags.contains(DirtyFlags::TEXT_SHAPING) && overlay.highlight_text_dirty {
        // Text dirty - yellow
        Color::from_rgba_u8(255, 255, 0, (overlay.overlay_opacity * 255.0) as u8)
    } else if flags.is_paint_only() && overlay.highlight_paint_only {
        // Paint only - green
        Color::from_rgba_u8(0, 255, 0, (overlay.overlay_opacity * 255.0) as u8)
    } else if flags.contains(DirtyFlags::CHILDREN_ORDER) {
        // Children order - purple
        Color::from_rgba_u8(255, 0, 255, (overlay.overlay_opacity * 255.0) as u8)
    } else if flags.contains(DirtyFlags::GEOMETRY) {
        // Geometry - cyan
        Color::from_rgba_u8(0, 255, 255, (overlay.overlay_opacity * 255.0) as u8)
    } else if flags.contains(DirtyFlags::TRANSFORM) {
        // Transform - orange
        Color::from_rgba_u8(255, 165, 0, (overlay.overlay_opacity * 255.0) as u8)
    } else if overlay.show_dirty_rects && !flags.is_empty() {
        // Generic dirty - white
        Color::from_rgba_u8(255, 255, 255, (overlay.overlay_opacity * 255.0) as u8)
    } else {
        // Fallback - gray
        Color::from_rgba_u8(128, 128, 128, (overlay.overlay_opacity * 255.0) as u8)
    }
}

/// Check if a node should be highlighted based on its flags.
fn should_highlight(flags: DirtyFlags, overlay: &DebugOverlay) -> bool {
    (overlay.highlight_layout_dirty && flags.contains(DirtyFlags::LAYOUT))
        || (overlay.highlight_text_dirty && flags.contains(DirtyFlags::TEXT_SHAPING))
        || (overlay.highlight_paint_only && flags.is_paint_only())
}

/// Format a label for a node.
fn format_label(node_id: NodeId, flags: DirtyFlags, overlay: &DebugOverlay) -> String {
    let mut parts = Vec::new();

    if overlay.show_node_ids {
        parts.push(format!("#{}", node_id.0));
    }

    if overlay.show_dirty_flags && !flags.is_empty() {
        parts.push(format_flags(flags));
    }

    parts.join(" ")
}

/// Format dirty flags as a compact string.
fn format_flags(flags: DirtyFlags) -> String {
    if flags.is_empty() {
        return String::from("CLEAN");
    }

    let mut parts = Vec::new();

    if flags.contains(DirtyFlags::LAYOUT) {
        parts.push("L");
    }
    if flags.contains(DirtyFlags::TEXT_SHAPING) {
        parts.push("T");
    }
    if flags.contains(DirtyFlags::COLOR) {
        parts.push("C");
    }
    if flags.contains(DirtyFlags::OPACITY) {
        parts.push("O");
    }
    if flags.contains(DirtyFlags::GEOMETRY) {
        parts.push("G");
    }
    if flags.contains(DirtyFlags::IMAGE) {
        parts.push("I");
    }
    if flags.contains(DirtyFlags::FOCUS) {
        parts.push("F");
    }
    if flags.contains(DirtyFlags::TRANSFORM) {
        parts.push("X");
    }
    if flags.contains(DirtyFlags::CLIP) {
        parts.push("CL");
    }
    if flags.contains(DirtyFlags::VISIBILITY) {
        parts.push("V");
    }
    if flags.contains(DirtyFlags::SCROLL) {
        parts.push("S");
    }
    if flags.contains(DirtyFlags::CHILDREN_ORDER) {
        parts.push("CH");
    }

    parts.join("|")
}

/// Collect debug info for all nodes in a tree.
pub fn collect_debug_info(tree: &UiTree, overlay: &DebugOverlay) -> Vec<DebugNodeInfo> {
    if !overlay.is_enabled() {
        return Vec::new();
    }

    tree.iter()
        .filter_map(|(node_id, node)| DebugNodeInfo::from_node(node_id, node, overlay))
        .collect()
}

/// Format metrics for overlay display.
pub fn format_metrics_overlay(metrics: &UiMetrics) -> String {
    format!(
        "UI Metrics:\n\
         Total: {:.2}ms\n\
         Layout: {:.2}ms ({} nodes)\n\
         Text: {:.2}ms ({} dirty)\n\
         Paint: {} nodes\n\
         Cache: {:.0}% hits",
        metrics.total_time.as_secs_f64() * 1000.0,
        metrics.layout_time.as_secs_f64() * 1000.0,
        metrics.nodes_layout_dirty,
        metrics.text_shape_time.as_secs_f64() * 1000.0,
        metrics.nodes_text_dirty,
        metrics.nodes_paint_dirty,
        metrics.text_cache_hit_rate() * 100.0,
    )
}

/// Color legend for debug overlay.
pub fn color_legend() -> Vec<(Color, &'static str)> {
    vec![
        (Color::from_rgb_u8(255, 0, 0), "Layout Dirty"),
        (Color::from_rgb_u8(255, 255, 0), "Text Dirty"),
        (Color::from_rgb_u8(0, 255, 0), "Paint Only"),
        (Color::from_rgb_u8(255, 0, 255), "Children Order"),
        (Color::from_rgb_u8(0, 255, 255), "Geometry"),
        (Color::from_rgb_u8(255, 165, 0), "Transform"),
        (Color::from_rgb_u8(128, 128, 128), "Clean"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_overlay_default() {
        let overlay = DebugOverlay::default();
        assert!(!overlay.is_enabled());
    }

    #[test]
    fn test_debug_overlay_all() {
        let overlay = DebugOverlay::all();
        assert!(overlay.is_enabled());
        assert!(overlay.show_dirty_rects);
        assert!(overlay.show_metrics);
    }

    #[test]
    fn test_format_flags() {
        assert_eq!(format_flags(DirtyFlags::NONE), "CLEAN");
        assert_eq!(format_flags(DirtyFlags::LAYOUT), "L");
        assert_eq!(
            format_flags(DirtyFlags::LAYOUT | DirtyFlags::TEXT_SHAPING),
            "L|T"
        );
    }

    #[test]
    fn test_should_highlight() {
        let overlay = DebugOverlay {
            highlight_layout_dirty: true,
            ..Default::default()
        };
        assert!(should_highlight(DirtyFlags::LAYOUT, &overlay));
        assert!(!should_highlight(DirtyFlags::COLOR, &overlay));
    }

    #[test]
    fn test_toggle() {
        let mut overlay = DebugOverlay::default();
        assert!(!overlay.show_dirty_rects);
        overlay.toggle_dirty_rects();
        assert!(overlay.show_dirty_rects);
    }
}
