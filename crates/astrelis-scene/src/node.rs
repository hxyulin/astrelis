//! Node identity and per-node data.

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
    /// Set on any transform/visibility/parent change; cleared by the propagation pass.
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
            dirty: true,
        }
    }
}
