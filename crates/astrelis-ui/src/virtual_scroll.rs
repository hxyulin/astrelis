//! Virtual scrolling for efficient rendering of large lists.
//!
//! This module provides virtualization support for lists with thousands of items,
//! only rendering visible items plus a configurable overscan buffer.

use std::ops::Range;

use astrelis_core::alloc::HashMap;

use crate::tree::{NodeId, UiTree};

/// Configuration for virtual scrolling behavior.
#[derive(Debug, Clone)]
pub struct VirtualScrollConfig {
    /// Number of items to render above/below visible area.
    pub overscan: usize,
    /// Minimum scroll delta before updating visible range.
    pub scroll_threshold: f32,
    /// Whether to enable smooth scrolling animations.
    pub smooth_scrolling: bool,
    /// Animation duration for smooth scrolling in seconds.
    pub scroll_animation_duration: f32,
}

impl Default for VirtualScrollConfig {
    fn default() -> Self {
        Self {
            overscan: 3,
            scroll_threshold: 1.0,
            smooth_scrolling: true,
            scroll_animation_duration: 0.15,
        }
    }
}

/// Specifies how item heights are determined.
#[derive(Debug, Clone)]
pub enum ItemHeight {
    /// All items have the same fixed height.
    Fixed(f32),
    /// Items have variable heights with an estimated default.
    Variable {
        /// Estimated height for unmeasured items.
        estimated: f32,
        /// Measured heights for items that have been rendered.
        measured: HashMap<usize, f32>,
    },
}

impl ItemHeight {
    /// Creates a fixed item height.
    pub fn fixed(height: f32) -> Self {
        Self::Fixed(height)
    }

    /// Creates a variable item height with an estimated default.
    pub fn variable(estimated: f32) -> Self {
        Self::Variable {
            estimated,
            measured: HashMap::default(),
        }
    }

    /// Gets the height for a specific item index.
    pub fn get(&self, index: usize) -> f32 {
        match self {
            Self::Fixed(h) => *h,
            Self::Variable { estimated, measured } => {
                measured.get(&index).copied().unwrap_or(*estimated)
            }
        }
    }

    /// Sets the measured height for a specific item.
    pub fn set_measured(&mut self, index: usize, height: f32) {
        if let Self::Variable { measured, .. } = self {
            measured.insert(index, height);
        }
    }

    /// Returns true if this is a fixed height.
    pub fn is_fixed(&self) -> bool {
        matches!(self, Self::Fixed(_))
    }
}

/// Tracks the state of a mounted (rendered) item.
#[derive(Debug, Clone)]
pub struct MountedItem {
    /// The node ID of the rendered widget.
    pub node_id: NodeId,
    /// The computed Y offset of this item.
    pub y_offset: f32,
    /// The measured height of this item.
    pub height: f32,
}

/// Statistics about virtual scroll performance.
#[derive(Debug, Clone, Default)]
pub struct VirtualScrollStats {
    /// Total number of items in the list.
    pub total_items: usize,
    /// Number of currently mounted (rendered) items.
    pub mounted_count: usize,
    /// Current visible range.
    pub visible_range: Range<usize>,
    /// Total scroll height.
    pub total_height: f32,
    /// Current scroll offset.
    pub scroll_offset: f32,
    /// Number of items recycled this frame.
    pub recycled_count: usize,
    /// Number of items created this frame.
    pub created_count: usize,
}

/// State for virtual scrolling of a list.
#[derive(Debug)]
pub struct VirtualScrollState {
    /// Configuration.
    config: VirtualScrollConfig,
    /// Total number of items.
    total_items: usize,
    /// Item height specification.
    item_height: ItemHeight,
    /// Current scroll offset (pixels from top).
    scroll_offset: f32,
    /// Target scroll offset for smooth scrolling.
    target_scroll_offset: f32,
    /// Viewport height.
    viewport_height: f32,
    /// Currently visible range (including overscan).
    visible_range: Range<usize>,
    /// Mounted items by index.
    mounted: HashMap<usize, MountedItem>,
    /// Container node ID for the scroll content.
    container_node: Option<NodeId>,
    /// Cached total height.
    cached_total_height: Option<f32>,
    /// Statistics.
    stats: VirtualScrollStats,
}

impl VirtualScrollState {
    /// Creates a new virtual scroll state.
    pub fn new(total_items: usize, item_height: ItemHeight) -> Self {
        Self {
            config: VirtualScrollConfig::default(),
            total_items,
            item_height,
            scroll_offset: 0.0,
            target_scroll_offset: 0.0,
            viewport_height: 0.0,
            visible_range: 0..0,
            mounted: HashMap::default(),
            container_node: None,
            cached_total_height: None,
            stats: VirtualScrollStats::default(),
        }
    }

    /// Creates a new virtual scroll state with configuration.
    pub fn with_config(
        total_items: usize,
        item_height: ItemHeight,
        config: VirtualScrollConfig,
    ) -> Self {
        Self {
            config,
            total_items,
            item_height,
            scroll_offset: 0.0,
            target_scroll_offset: 0.0,
            viewport_height: 0.0,
            visible_range: 0..0,
            mounted: HashMap::default(),
            container_node: None,
            cached_total_height: None,
            stats: VirtualScrollStats::default(),
        }
    }

    /// Sets the container node for the scroll content.
    pub fn set_container(&mut self, node: NodeId) {
        self.container_node = Some(node);
    }

    /// Gets the container node.
    pub fn container(&self) -> Option<NodeId> {
        self.container_node
    }

    /// Updates the total number of items.
    pub fn set_total_items(&mut self, count: usize) {
        if self.total_items != count {
            self.total_items = count;
            self.cached_total_height = None;
            // Clamp scroll offset if needed
            let max_offset = self.max_scroll_offset();
            if self.scroll_offset > max_offset {
                self.scroll_offset = max_offset;
                self.target_scroll_offset = max_offset;
            }
        }
    }

    /// Gets the total number of items.
    pub fn total_items(&self) -> usize {
        self.total_items
    }

    /// Updates the viewport height.
    pub fn set_viewport_height(&mut self, height: f32) {
        if (self.viewport_height - height).abs() > 0.1 {
            self.viewport_height = height;
        }
    }

    /// Gets the viewport height.
    pub fn viewport_height(&self) -> f32 {
        self.viewport_height
    }

    /// Calculates the total scroll height.
    pub fn total_height(&self) -> f32 {
        if let Some(cached) = self.cached_total_height {
            return cached;
        }

        match &self.item_height {
            ItemHeight::Fixed(h) => *h * self.total_items as f32,
            ItemHeight::Variable { estimated, measured } => {
                let mut height = 0.0;
                for i in 0..self.total_items {
                    height += measured.get(&i).copied().unwrap_or(*estimated);
                }
                height
            }
        }
    }

    /// Gets the maximum scroll offset.
    pub fn max_scroll_offset(&self) -> f32 {
        (self.total_height() - self.viewport_height).max(0.0)
    }

    /// Gets the current scroll offset.
    pub fn scroll_offset(&self) -> f32 {
        self.scroll_offset
    }

    /// Sets the scroll offset directly.
    pub fn set_scroll_offset(&mut self, offset: f32) {
        let clamped = offset.clamp(0.0, self.max_scroll_offset());
        self.scroll_offset = clamped;
        self.target_scroll_offset = clamped;
    }

    /// Scrolls by a delta amount.
    pub fn scroll_by(&mut self, delta: f32) {
        if self.config.smooth_scrolling {
            self.target_scroll_offset =
                (self.target_scroll_offset + delta).clamp(0.0, self.max_scroll_offset());
        } else {
            self.set_scroll_offset(self.scroll_offset + delta);
        }
    }

    /// Scrolls to show a specific item.
    pub fn scroll_to_item(&mut self, index: usize) {
        if index >= self.total_items {
            return;
        }

        let item_offset = self.get_item_offset(index);
        let item_height = self.item_height.get(index);

        // Check if item is already fully visible
        if item_offset >= self.scroll_offset
            && item_offset + item_height <= self.scroll_offset + self.viewport_height
        {
            return;
        }

        // Scroll to show the item
        let target = if item_offset < self.scroll_offset {
            // Item is above viewport
            item_offset
        } else {
            // Item is below viewport
            (item_offset + item_height - self.viewport_height).max(0.0)
        };

        if self.config.smooth_scrolling {
            self.target_scroll_offset = target;
        } else {
            self.set_scroll_offset(target);
        }
    }

    /// Scrolls to center a specific item in the viewport.
    pub fn scroll_to_item_centered(&mut self, index: usize) {
        if index >= self.total_items {
            return;
        }

        let item_offset = self.get_item_offset(index);
        let item_height = self.item_height.get(index);
        let target = (item_offset + item_height / 2.0 - self.viewport_height / 2.0).max(0.0);

        if self.config.smooth_scrolling {
            self.target_scroll_offset = target.min(self.max_scroll_offset());
        } else {
            self.set_scroll_offset(target);
        }
    }

    /// Gets the Y offset for an item at the given index.
    pub fn get_item_offset(&self, index: usize) -> f32 {
        match &self.item_height {
            ItemHeight::Fixed(h) => *h * index as f32,
            ItemHeight::Variable { estimated, measured } => {
                let mut offset = 0.0;
                for i in 0..index {
                    offset += measured.get(&i).copied().unwrap_or(*estimated);
                }
                offset
            }
        }
    }

    /// Gets the item index at a given Y position.
    pub fn get_item_at_position(&self, y: f32) -> Option<usize> {
        if y < 0.0 || self.total_items == 0 {
            return None;
        }

        match &self.item_height {
            ItemHeight::Fixed(h) => {
                let index = (y / h) as usize;
                if index < self.total_items {
                    Some(index)
                } else {
                    None
                }
            }
            ItemHeight::Variable { estimated, measured } => {
                let mut offset = 0.0;
                for i in 0..self.total_items {
                    let height = measured.get(&i).copied().unwrap_or(*estimated);
                    if y >= offset && y < offset + height {
                        return Some(i);
                    }
                    offset += height;
                }
                None
            }
        }
    }

    /// Updates smooth scrolling animation.
    /// Returns true if the scroll position changed.
    pub fn update_animation(&mut self, dt: f32) -> bool {
        if !self.config.smooth_scrolling {
            return false;
        }

        let diff = self.target_scroll_offset - self.scroll_offset;
        if diff.abs() < 0.5 {
            if diff.abs() > 0.0 {
                self.scroll_offset = self.target_scroll_offset;
                return true;
            }
            return false;
        }

        // Exponential easing
        let t = (dt / self.config.scroll_animation_duration).min(1.0);
        let eased = 1.0 - (1.0 - t).powi(3); // Ease-out cubic
        self.scroll_offset += diff * eased;
        true
    }

    /// Calculates the visible range based on current scroll position.
    pub fn calculate_visible_range(&self) -> Range<usize> {
        if self.total_items == 0 || self.viewport_height <= 0.0 {
            return 0..0;
        }

        let start_index = self
            .get_item_at_position(self.scroll_offset)
            .unwrap_or(0)
            .saturating_sub(self.config.overscan);

        let end_y = self.scroll_offset + self.viewport_height;
        let end_index = self
            .get_item_at_position(end_y)
            .map(|i| i + 1)
            .unwrap_or(self.total_items)
            .saturating_add(self.config.overscan)
            .min(self.total_items);

        start_index..end_index
    }

    /// Updates the visible range and returns items that need to be mounted/unmounted.
    /// Returns (items_to_mount, items_to_unmount).
    pub fn update_visible(&mut self) -> (Vec<usize>, Vec<usize>) {
        let new_range = self.calculate_visible_range();

        if new_range == self.visible_range {
            return (vec![], vec![]);
        }

        let old_range = self.visible_range.clone();
        self.visible_range = new_range.clone();

        // Find items to unmount (in old range but not in new)
        let to_unmount: Vec<usize> = old_range
            .filter(|i| !new_range.contains(i))
            .filter(|i| self.mounted.contains_key(i))
            .collect();

        // Find items to mount (in new range but not mounted)
        let to_mount: Vec<usize> = new_range.filter(|i| !self.mounted.contains_key(i)).collect();

        (to_mount, to_unmount)
    }

    /// Records a mounted item.
    pub fn mount_item(&mut self, index: usize, node_id: NodeId, height: f32) {
        let y_offset = self.get_item_offset(index);
        self.mounted.insert(
            index,
            MountedItem {
                node_id,
                y_offset,
                height,
            },
        );

        // Update measured height for variable height items
        self.item_height.set_measured(index, height);
        self.cached_total_height = None;
    }

    /// Unmounts an item and returns its node ID.
    pub fn unmount_item(&mut self, index: usize) -> Option<NodeId> {
        self.mounted.remove(&index).map(|item| item.node_id)
    }

    /// Gets the mounted item for an index.
    pub fn get_mounted(&self, index: usize) -> Option<&MountedItem> {
        self.mounted.get(&index)
    }

    /// Gets all mounted items.
    pub fn mounted_items(&self) -> impl Iterator<Item = (usize, &MountedItem)> {
        self.mounted.iter().map(|(k, v)| (*k, v))
    }

    /// Gets the current visible range.
    pub fn visible_range(&self) -> Range<usize> {
        self.visible_range.clone()
    }

    /// Checks if an index is in the visible range.
    pub fn is_visible(&self, index: usize) -> bool {
        self.visible_range.contains(&index)
    }

    /// Gets statistics about the virtual scroll.
    pub fn stats(&self) -> &VirtualScrollStats {
        &self.stats
    }

    /// Updates statistics.
    pub fn update_stats(&mut self) {
        self.stats = VirtualScrollStats {
            total_items: self.total_items,
            mounted_count: self.mounted.len(),
            visible_range: self.visible_range.clone(),
            total_height: self.total_height(),
            scroll_offset: self.scroll_offset,
            recycled_count: 0,
            created_count: 0,
        };
    }

    /// Gets the configuration.
    pub fn config(&self) -> &VirtualScrollConfig {
        &self.config
    }

    /// Gets mutable configuration.
    pub fn config_mut(&mut self) -> &mut VirtualScrollConfig {
        &mut self.config
    }
}

/// A virtual scroll view that manages item rendering.
pub struct VirtualScrollView<T> {
    /// The items being virtualized.
    items: Vec<T>,
    /// Scroll state.
    state: VirtualScrollState,
    /// Item builder function.
    builder: Box<dyn Fn(usize, &T, &mut UiTree) -> NodeId>,
}

impl<T> VirtualScrollView<T> {
    /// Creates a new virtual scroll view with fixed height items.
    pub fn new<F>(items: Vec<T>, item_height: f32, builder: F) -> Self
    where
        F: Fn(usize, &T, &mut UiTree) -> NodeId + 'static,
    {
        Self {
            state: VirtualScrollState::new(items.len(), ItemHeight::fixed(item_height)),
            items,
            builder: Box::new(builder),
        }
    }

    /// Creates a new virtual scroll view with variable height items.
    pub fn with_variable_height<F>(items: Vec<T>, estimated_height: f32, builder: F) -> Self
    where
        F: Fn(usize, &T, &mut UiTree) -> NodeId + 'static,
    {
        Self {
            state: VirtualScrollState::new(items.len(), ItemHeight::variable(estimated_height)),
            items,
            builder: Box::new(builder),
        }
    }

    /// Sets the configuration.
    pub fn with_config(mut self, config: VirtualScrollConfig) -> Self {
        self.state.config = config;
        self
    }

    /// Gets the scroll state.
    pub fn state(&self) -> &VirtualScrollState {
        &self.state
    }

    /// Gets mutable scroll state.
    pub fn state_mut(&mut self) -> &mut VirtualScrollState {
        &mut self.state
    }

    /// Gets the items.
    pub fn items(&self) -> &[T] {
        &self.items
    }

    /// Updates the items list.
    pub fn set_items(&mut self, items: Vec<T>) {
        self.items = items;
        self.state.set_total_items(self.items.len());
    }

    /// Sets the viewport height.
    pub fn set_viewport_height(&mut self, height: f32) {
        self.state.set_viewport_height(height);
    }

    /// Scrolls by a delta amount.
    pub fn scroll_by(&mut self, delta: f32) {
        self.state.scroll_by(delta);
    }

    /// Scrolls to show a specific item.
    pub fn scroll_to_item(&mut self, index: usize) {
        self.state.scroll_to_item(index);
    }

    /// Updates the scroll animation and visible items.
    /// Returns the nodes that were added or removed.
    pub fn update(&mut self, tree: &mut UiTree, dt: f32) -> VirtualScrollUpdate {
        let mut update = VirtualScrollUpdate::default();

        // Update animation
        if self.state.update_animation(dt) {
            update.scroll_changed = true;
        }

        // Calculate visible range changes
        let (to_mount, to_unmount) = self.state.update_visible();

        // Unmount items that are no longer visible
        for index in to_unmount {
            if let Some(node_id) = self.state.unmount_item(index) {
                // TODO: Implement proper node removal when UiTree supports it
                // For now, we just track the removal - the node remains in the tree
                // but won't be rendered if culling is enabled
                let _ = node_id;
                update.removed.push((index, node_id));
            }
        }

        // Mount newly visible items
        for index in to_mount {
            if let Some(item) = self.items.get(index) {
                let node_id = (self.builder)(index, item, tree);

                // Add to container
                if let Some(container) = self.state.container() {
                    tree.add_child(container, node_id);
                }

                // Get the measured height from layout
                let height = tree
                    .get_layout(node_id)
                    .map(|l| l.height)
                    .unwrap_or(self.state.item_height.get(index));

                self.state.mount_item(index, node_id, height);
                update.added.push((index, node_id));
            }
        }

        // Update item positions based on scroll offset
        self.update_item_positions(tree);

        self.state.update_stats();
        update
    }

    /// Updates the Y positions of all mounted items.
    fn update_item_positions(&self, _tree: &mut UiTree) {
        // TODO: Implement position updates when UiTree supports transforms
        // For now, positions are managed through layout constraints
        let _scroll_offset = self.state.scroll_offset();

        for (_index, _item) in self.state.mounted_items() {
            // let y = item.y_offset - scroll_offset;
            // tree.set_transform(item.node_id, 0.0, y);
        }
    }
}

/// Information about what changed during a virtual scroll update.
#[derive(Debug, Default)]
pub struct VirtualScrollUpdate {
    /// Whether the scroll position changed.
    pub scroll_changed: bool,
    /// Items that were added (index, node_id).
    pub added: Vec<(usize, NodeId)>,
    /// Items that were removed (index, node_id).
    pub removed: Vec<(usize, NodeId)>,
}

impl VirtualScrollUpdate {
    /// Returns true if any changes occurred.
    pub fn has_changes(&self) -> bool {
        self.scroll_changed || !self.added.is_empty() || !self.removed.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_height_offset() {
        let state = VirtualScrollState::new(100, ItemHeight::fixed(50.0));
        assert_eq!(state.get_item_offset(0), 0.0);
        assert_eq!(state.get_item_offset(1), 50.0);
        assert_eq!(state.get_item_offset(10), 500.0);
    }

    #[test]
    fn test_total_height_fixed() {
        let state = VirtualScrollState::new(100, ItemHeight::fixed(50.0));
        assert_eq!(state.total_height(), 5000.0);
    }

    #[test]
    fn test_variable_height() {
        let mut item_height = ItemHeight::variable(50.0);
        item_height.set_measured(0, 30.0);
        item_height.set_measured(1, 70.0);

        assert_eq!(item_height.get(0), 30.0);
        assert_eq!(item_height.get(1), 70.0);
        assert_eq!(item_height.get(2), 50.0); // Uses estimated
    }

    #[test]
    fn test_get_item_at_position() {
        let state = VirtualScrollState::new(100, ItemHeight::fixed(50.0));
        assert_eq!(state.get_item_at_position(0.0), Some(0));
        assert_eq!(state.get_item_at_position(49.0), Some(0));
        assert_eq!(state.get_item_at_position(50.0), Some(1));
        assert_eq!(state.get_item_at_position(125.0), Some(2));
        assert_eq!(state.get_item_at_position(5000.0), None);
    }

    #[test]
    fn test_visible_range_calculation() {
        let mut state = VirtualScrollState::new(100, ItemHeight::fixed(50.0));
        state.set_viewport_height(200.0);
        state.config_mut().overscan = 2;

        // At scroll offset 0, visible items are 0-3 (4 items fit in 200px)
        // With overscan of 2, range should include items 0 through 5 (end exclusive = 6)
        let range = state.calculate_visible_range();
        assert_eq!(range.start, 0);
        // The visible range calculation: start_index=0-2=0, end_index=(3+1)+2=6
        assert!(range.end >= 4, "end should be at least 4, got {}", range.end);
    }

    #[test]
    fn test_scroll_clamping() {
        let mut state = VirtualScrollState::new(100, ItemHeight::fixed(50.0));
        state.set_viewport_height(200.0);

        // Max offset = 5000 - 200 = 4800
        state.set_scroll_offset(10000.0);
        assert_eq!(state.scroll_offset(), 4800.0);

        state.set_scroll_offset(-100.0);
        assert_eq!(state.scroll_offset(), 0.0);
    }

    #[test]
    fn test_scroll_to_item() {
        let mut state = VirtualScrollState::new(100, ItemHeight::fixed(50.0));
        state.config_mut().smooth_scrolling = false;
        state.set_viewport_height(200.0);

        // Scroll to item 20 (at y=1000, height 50)
        // The implementation scrolls to make item visible at bottom of viewport
        // So offset = item_offset + item_height - viewport_height = 1000 + 50 - 200 = 850
        state.scroll_to_item(20);
        assert_eq!(state.scroll_offset(), 850.0);
    }

    #[test]
    fn test_empty_list() {
        let state = VirtualScrollState::new(0, ItemHeight::fixed(50.0));
        assert_eq!(state.total_height(), 0.0);
        assert_eq!(state.max_scroll_offset(), 0.0);
        assert_eq!(state.calculate_visible_range(), 0..0);
    }

    #[test]
    fn test_config_default() {
        let config = VirtualScrollConfig::default();
        assert!(config.overscan > 0);
        assert!(config.smooth_scrolling);
    }

    #[test]
    fn test_item_height_fixed() {
        let item_height = ItemHeight::fixed(30.0);
        assert_eq!(item_height.get(0), 30.0);
        assert_eq!(item_height.get(100), 30.0);
        assert_eq!(item_height.get(9999), 30.0);
    }

    #[test]
    fn test_item_height_variable_update() {
        let mut item_height = ItemHeight::variable(50.0);

        // Initially uses estimate
        assert_eq!(item_height.get(5), 50.0);

        // After measuring, uses actual
        item_height.set_measured(5, 75.0);
        assert_eq!(item_height.get(5), 75.0);

        // Other items still use estimate
        assert_eq!(item_height.get(6), 50.0);
    }

    #[test]
    fn test_scroll_delta() {
        let mut state = VirtualScrollState::new(100, ItemHeight::fixed(50.0));
        state.set_viewport_height(200.0);
        state.config_mut().smooth_scrolling = false; // Disable smooth scrolling for direct updates

        state.scroll_by(100.0);
        assert_eq!(state.scroll_offset(), 100.0);

        state.scroll_by(50.0);
        assert_eq!(state.scroll_offset(), 150.0);

        state.scroll_by(-200.0);
        assert_eq!(state.scroll_offset(), 0.0); // Clamped to 0
    }

    #[test]
    fn test_item_count_change() {
        let mut state = VirtualScrollState::new(100, ItemHeight::fixed(50.0));
        assert_eq!(state.total_items(), 100);

        state.set_total_items(200);
        assert_eq!(state.total_items(), 200);
        assert_eq!(state.total_height(), 10000.0);

        state.set_total_items(50);
        assert_eq!(state.total_items(), 50);
        assert_eq!(state.total_height(), 2500.0);
    }

    #[test]
    fn test_visible_range_scrolled() {
        let mut state = VirtualScrollState::new(100, ItemHeight::fixed(50.0));
        state.set_viewport_height(200.0);
        state.config_mut().overscan = 0; // Disable overscan for clearer test

        // At scroll 0, items 0-3 visible (4 items * 50px = 200px)
        let range = state.calculate_visible_range();
        assert_eq!(range.start, 0);

        // Scroll to show items 10-13
        state.set_scroll_offset(500.0);
        let range = state.calculate_visible_range();
        assert_eq!(range.start, 10);
    }

    #[test]
    fn test_is_visible() {
        let mut state = VirtualScrollState::new(100, ItemHeight::fixed(50.0));
        state.set_viewport_height(200.0);
        state.update_visible(); // Need to update visible range first

        // At scroll 0, items near top should be visible
        assert!(state.is_visible(0));
        assert!(state.is_visible(3));
        // Items far down should not be visible
        assert!(!state.is_visible(50));
        assert!(!state.is_visible(99));
    }

    #[test]
    fn test_stats() {
        let mut state = VirtualScrollState::new(1000, ItemHeight::fixed(50.0));
        state.set_viewport_height(400.0);
        state.update_visible(); // Update visible range
        state.update_stats();   // Update stats

        let stats = state.stats();
        assert_eq!(stats.total_items, 1000);
        assert_eq!(stats.total_height, 50000.0);
        // Visible range should be a subset of total items
        assert!(stats.visible_range.end <= stats.total_items);
    }

    #[test]
    fn test_scroll_to_first_and_last() {
        let mut state = VirtualScrollState::new(100, ItemHeight::fixed(50.0));
        state.config_mut().smooth_scrolling = false;
        state.set_viewport_height(200.0);

        // Scroll to last item
        state.scroll_to_item(99);
        assert!(state.scroll_offset() > 0.0);

        // Scroll back to first
        state.scroll_to_item(0);
        assert_eq!(state.scroll_offset(), 0.0);
    }

    #[test]
    fn test_variable_height_total() {
        let mut item_height = ItemHeight::variable(50.0);
        item_height.set_measured(0, 100.0);
        item_height.set_measured(1, 25.0);

        let state = VirtualScrollState::new(3, item_height);
        // Total = 100 + 25 + 50 = 175
        assert_eq!(state.total_height(), 175.0);
    }

    #[test]
    fn test_scroll_preserves_position_on_resize() {
        let mut state = VirtualScrollState::new(100, ItemHeight::fixed(50.0));
        state.set_viewport_height(200.0);
        state.set_scroll_offset(500.0);

        // Resize viewport
        state.set_viewport_height(400.0);

        // Scroll position should be preserved
        assert_eq!(state.scroll_offset(), 500.0);
    }

    #[test]
    fn test_overscan_increases_visible_range() {
        let mut state = VirtualScrollState::new(100, ItemHeight::fixed(50.0));
        state.set_viewport_height(200.0);

        state.config_mut().overscan = 0;
        let range_no_overscan = state.calculate_visible_range();

        state.config_mut().overscan = 5;
        let range_with_overscan = state.calculate_visible_range();

        // With overscan, more items should be in range
        let no_overscan_count = range_no_overscan.end - range_no_overscan.start;
        let with_overscan_count = range_with_overscan.end - range_with_overscan.start;
        assert!(with_overscan_count > no_overscan_count);
    }

    #[test]
    fn test_single_item_list() {
        let state = VirtualScrollState::new(1, ItemHeight::fixed(50.0));
        assert_eq!(state.total_height(), 50.0);
        assert_eq!(state.get_item_at_position(25.0), Some(0));
        assert_eq!(state.get_item_at_position(60.0), None);
    }
}
