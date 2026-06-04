//! Component trait and columnar storage.

use crate::node::NodeId;

/// Object-safe interface over one per-type component column.
pub(crate) trait ComponentColumn: Send + Sync {
    /// Removes `id`'s component from this column, if present.
    fn remove(&mut self, id: NodeId);
}
