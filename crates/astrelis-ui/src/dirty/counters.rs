//! O(1) dirty state counters for the UI tree.
//!
//! Replaces O(n) iteration over all nodes to check for layout/text dirty state.

use super::DirtyFlags;

/// Tracks counts of dirty nodes by category for O(1) queries.
///
/// Updated incrementally as flags are marked and cleared on individual nodes.
#[derive(Debug, Clone, Default)]
pub struct DirtyCounters {
    /// Number of nodes with any layout-affecting dirty flag.
    layout_dirty: u32,
    /// Number of nodes with TEXT_SHAPING dirty flag.
    text_dirty: u32,
    /// Number of nodes with any paint-only dirty flag.
    paint_dirty: u32,
    /// Number of nodes with any dirty flag at all.
    any_dirty: u32,
}

impl DirtyCounters {
    /// Create new zeroed counters.
    pub fn new() -> Self {
        Self::default()
    }

    /// Called when dirty flags are added to a node.
    ///
    /// `old_flags` is the node's flags before the `|=` operation.
    /// `added_flags` is the flags being added.
    pub fn on_mark(&mut self, old_flags: DirtyFlags, added_flags: DirtyFlags) {
        let new_flags = old_flags | added_flags;

        // Track transitions from clean to dirty in each category
        if old_flags.is_empty() && !new_flags.is_empty() {
            self.any_dirty += 1;
        }
        if !old_flags.needs_layout() && new_flags.needs_layout() {
            self.layout_dirty += 1;
        }
        if !old_flags.needs_text_shaping() && new_flags.needs_text_shaping() {
            self.text_dirty += 1;
        }
        if !old_flags.intersects(DirtyFlags::PAINT_GROUP) && new_flags.intersects(DirtyFlags::PAINT_GROUP) {
            self.paint_dirty += 1;
        }
    }

    /// Called when a node's dirty flags are cleared entirely.
    ///
    /// `old_flags` is the node's flags before clearing.
    pub fn on_clear(&mut self, old_flags: DirtyFlags) {
        if old_flags.is_empty() {
            return;
        }

        self.any_dirty = self.any_dirty.saturating_sub(1);
        if old_flags.needs_layout() {
            self.layout_dirty = self.layout_dirty.saturating_sub(1);
        }
        if old_flags.needs_text_shaping() {
            self.text_dirty = self.text_dirty.saturating_sub(1);
        }
        if old_flags.intersects(DirtyFlags::PAINT_GROUP) {
            self.paint_dirty = self.paint_dirty.saturating_sub(1);
        }
    }

    /// Reset all counters (called on full tree clear).
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// O(1) check: any node needs layout?
    #[inline]
    pub fn has_layout_dirty(&self) -> bool {
        self.layout_dirty > 0
    }

    /// O(1) check: any node needs text shaping?
    #[inline]
    pub fn has_text_dirty(&self) -> bool {
        self.text_dirty > 0
    }

    /// O(1) check: any node has paint-only changes?
    #[inline]
    pub fn has_paint_dirty(&self) -> bool {
        self.paint_dirty > 0
    }

    /// O(1) check: any node is dirty at all?
    #[inline]
    pub fn has_any_dirty(&self) -> bool {
        self.any_dirty > 0
    }

    /// Get a summary snapshot of current dirty state.
    pub fn summary(&self) -> DirtySummary {
        DirtySummary {
            layout_count: self.layout_dirty as usize,
            text_count: self.text_dirty as usize,
            paint_count: self.paint_dirty as usize,
            any_count: self.any_dirty as usize,
        }
    }
}

/// Snapshot of dirty counter state, used for metrics reporting.
#[derive(Debug, Clone, Default)]
pub struct DirtySummary {
    pub layout_count: usize,
    pub text_count: usize,
    pub paint_count: usize,
    pub any_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counters_mark_layout() {
        let mut counters = DirtyCounters::new();

        counters.on_mark(DirtyFlags::NONE, DirtyFlags::LAYOUT);
        assert!(counters.has_layout_dirty());
        assert!(counters.has_any_dirty());
        assert!(!counters.has_text_dirty());
    }

    #[test]
    fn test_counters_mark_text() {
        let mut counters = DirtyCounters::new();

        counters.on_mark(DirtyFlags::NONE, DirtyFlags::TEXT_SHAPING);
        assert!(counters.has_text_dirty());
        assert!(counters.has_layout_dirty()); // TEXT_SHAPING is layout-affecting
    }

    #[test]
    fn test_counters_mark_paint() {
        let mut counters = DirtyCounters::new();

        counters.on_mark(DirtyFlags::NONE, DirtyFlags::COLOR);
        assert!(counters.has_paint_dirty());
        assert!(!counters.has_layout_dirty());
    }

    #[test]
    fn test_counters_clear() {
        let mut counters = DirtyCounters::new();

        counters.on_mark(DirtyFlags::NONE, DirtyFlags::LAYOUT);
        assert!(counters.has_layout_dirty());

        counters.on_clear(DirtyFlags::LAYOUT);
        assert!(!counters.has_layout_dirty());
        assert!(!counters.has_any_dirty());
    }

    #[test]
    fn test_counters_no_double_count() {
        let mut counters = DirtyCounters::new();

        // Mark layout once
        counters.on_mark(DirtyFlags::NONE, DirtyFlags::LAYOUT);
        // Adding color to an already-layout-dirty node shouldn't increment layout_dirty
        counters.on_mark(DirtyFlags::LAYOUT, DirtyFlags::COLOR);

        assert_eq!(counters.summary().layout_count, 1);
    }

    #[test]
    fn test_counters_reset() {
        let mut counters = DirtyCounters::new();

        counters.on_mark(DirtyFlags::NONE, DirtyFlags::LAYOUT);
        counters.on_mark(DirtyFlags::NONE, DirtyFlags::COLOR);

        counters.reset();
        assert!(!counters.has_any_dirty());
        assert!(!counters.has_layout_dirty());
    }

    #[test]
    fn test_counters_multiple_nodes() {
        let mut counters = DirtyCounters::new();

        // Two nodes become layout dirty
        counters.on_mark(DirtyFlags::NONE, DirtyFlags::LAYOUT);
        counters.on_mark(DirtyFlags::NONE, DirtyFlags::LAYOUT);

        assert_eq!(counters.summary().layout_count, 2);

        // Clear one
        counters.on_clear(DirtyFlags::LAYOUT);
        assert_eq!(counters.summary().layout_count, 1);
        assert!(counters.has_layout_dirty());

        // Clear second
        counters.on_clear(DirtyFlags::LAYOUT);
        assert!(!counters.has_layout_dirty());
    }
}
