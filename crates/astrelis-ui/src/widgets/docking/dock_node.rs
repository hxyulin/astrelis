//! Logical dock tree node (design scaffold for floating windows).
//!
//! Currently the widget tree IS the dock tree. When floating windows are added,
//! this enum will track dock topology independently, allowing a DockNode to
//! exist as a floating overlay without being part of the main widget tree.

use super::SplitDirection;
use crate::tree::NodeId;

/// Logical dock tree node.
///
/// Currently unused — the widget tree serves as the dock tree.
/// When floating windows are implemented, this will track dock topology
/// independently from the widget tree.
#[derive(Debug, Clone)]
pub enum DockNode {
    /// A tabbed container backed by a DockTabs widget.
    Tabs {
        /// The widget tree node for the DockTabs widget.
        widget_node: NodeId,
    },
    /// A split container backed by a DockSplitter widget.
    Split {
        /// The widget tree node for the DockSplitter widget.
        widget_node: NodeId,
        /// Split direction.
        direction: SplitDirection,
        /// Split ratio (0.0–1.0).
        ratio: f32,
        /// Two child dock nodes.
        children: [Box<DockNode>; 2],
    },
    // Future: Floating window variant
    // Floating {
    //     inner: Box<DockNode>,
    //     position: Vec2,
    //     size: Vec2,
    //     z_order: u32,
    // },
}

/// Objects that participate in hit testing (for floating windows).
///
/// When floating windows are added, each floating window will implement this
/// trait to participate in the hit-test ordering. The main docked tree acts
/// as the base layer (`z_order = 0`), and floating windows stack above it.
pub trait HitTestLayer {
    /// Test if a point hits a node in this layer.
    fn hit_test(&self, point: astrelis_core::math::Vec2) -> Option<NodeId>;

    /// Z-order of this layer (higher = closer to viewer).
    fn z_order(&self) -> u32;
}
