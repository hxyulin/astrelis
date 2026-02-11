//! Dock operations for modifying the docking layout.

use super::splitter::DockSplitter;
use super::tabs::DockTabs;
use super::types::DockZone;
use crate::tree::{NodeId, UiTree};

/// Error types for dock operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DockError {
    /// Node not found in tree.
    NodeNotFound(NodeId),
    /// Widget is not the expected type.
    InvalidWidgetType,
    /// Tab index out of bounds.
    InvalidTabIndex,
    /// Cannot transfer a tab to the same container (use reorder instead).
    SameContainerTransfer,
    /// SplitContainerOperation requires an edge zone, not Center.
    EdgeZoneRequired,
    /// No rollback data available (operation not executed or already rolled back).
    NoRollbackData,
    /// Splitter has no remaining child after collapse.
    NoRemainingSibling,
}

impl std::fmt::Display for DockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DockError::NodeNotFound(id) => write!(f, "Node {:?} not found", id),
            DockError::InvalidWidgetType => write!(f, "Widget is not the expected type"),
            DockError::InvalidTabIndex => write!(f, "Tab index out of bounds"),
            DockError::SameContainerTransfer => write!(f, "Cannot transfer to same container"),
            DockError::EdgeZoneRequired => {
                write!(f, "SplitContainerOperation requires an edge zone")
            }
            DockError::NoRollbackData => write!(f, "No rollback data available"),
            DockError::NoRemainingSibling => write!(f, "Splitter has no remaining child"),
        }
    }
}

impl std::error::Error for DockError {}

/// Result type for dock operations.
pub type DockResult<T> = Result<T, DockError>;

/// Trait for atomic docking operations.
///
/// Operations can be executed and rolled back if needed.
pub trait DockOperation {
    /// Execute the operation.
    fn execute(&mut self, tree: &mut UiTree) -> DockResult<()>;

    /// Rollback the operation (if possible).
    ///
    /// Not all operations support rollback. Returns an error if rollback
    /// is not supported or fails.
    fn rollback(&mut self, tree: &mut UiTree) -> DockResult<()>;
}

/// Transfer a tab from one DockTabs container to another.
#[derive(Debug)]
pub struct TransferTabOperation {
    /// Source container node.
    pub source_container: NodeId,
    /// Target container node.
    pub target_container: NodeId,
    /// Index of tab in source container.
    pub source_tab_index: usize,
    /// Index where tab will be inserted in target container.
    pub target_insert_index: usize,
    /// Rollback data (stored after execution).
    rollback_data: Option<TransferRollback>,
}

#[derive(Debug)]
struct TransferRollback {
    tab_label: String,
    tab_content: NodeId,
    source_index: usize,
    target_index: usize,
}

impl TransferTabOperation {
    /// Create a new tab transfer operation.
    pub fn new(
        source_container: NodeId,
        target_container: NodeId,
        source_tab_index: usize,
        target_insert_index: usize,
    ) -> Self {
        Self {
            source_container,
            target_container,
            source_tab_index,
            target_insert_index,
            rollback_data: None,
        }
    }
}

impl DockOperation for TransferTabOperation {
    fn execute(&mut self, tree: &mut UiTree) -> DockResult<()> {
        // Validate source container
        let source_widget = tree
            .get_widget(self.source_container)
            .ok_or(DockError::NodeNotFound(self.source_container))?;

        let source_tabs = source_widget
            .as_any()
            .downcast_ref::<DockTabs>()
            .ok_or(DockError::InvalidWidgetType)?;

        // Validate target container
        let target_widget = tree
            .get_widget(self.target_container)
            .ok_or(DockError::NodeNotFound(self.target_container))?;

        let _ = target_widget
            .as_any()
            .downcast_ref::<DockTabs>()
            .ok_or(DockError::InvalidWidgetType)?;

        // Check if tab index is valid
        if self.source_tab_index >= source_tabs.tab_count() {
            return Err(DockError::InvalidTabIndex);
        }

        // Check if source and target are the same (should use reorder instead)
        if self.source_container == self.target_container {
            return Err(DockError::SameContainerTransfer);
        }

        // Remove tab from source
        let (tab_label, tab_content) = {
            let source_mut = tree
                .get_widget_mut(self.source_container)
                .ok_or(DockError::NodeNotFound(self.source_container))?;

            let source_tabs = source_mut
                .as_any_mut()
                .downcast_mut::<DockTabs>()
                .ok_or(DockError::InvalidWidgetType)?;

            source_tabs
                .remove_tab(self.source_tab_index)
                .ok_or(DockError::InvalidTabIndex)?
        };

        // Add tab to target at specified index
        {
            let target_mut = tree
                .get_widget_mut(self.target_container)
                .ok_or(DockError::NodeNotFound(self.target_container))?;

            let target_tabs = target_mut
                .as_any_mut()
                .downcast_mut::<DockTabs>()
                .ok_or(DockError::InvalidWidgetType)?;

            let insert_index = self.target_insert_index.min(target_tabs.tab_count());
            target_tabs.insert_tab_at(insert_index, &tab_label, tab_content);
        }

        let insert_index = self.target_insert_index;

        // Sync tree-level parent/child relationships
        tree.remove_child(self.source_container, tab_content);
        tree.add_child(self.target_container, tab_content);

        // Store rollback data
        self.rollback_data = Some(TransferRollback {
            tab_label,
            tab_content,
            source_index: self.source_tab_index,
            target_index: insert_index,
        });

        // Mark both containers and transferred content dirty
        tree.mark_dirty_flags(
            self.source_container,
            crate::dirty::DirtyFlags::LAYOUT | crate::dirty::DirtyFlags::CHILDREN_ORDER,
        );
        tree.mark_dirty_flags(
            self.target_container,
            crate::dirty::DirtyFlags::LAYOUT | crate::dirty::DirtyFlags::CHILDREN_ORDER,
        );
        tree.mark_dirty_flags(
            tab_content,
            crate::dirty::DirtyFlags::LAYOUT | crate::dirty::DirtyFlags::GEOMETRY,
        );

        Ok(())
    }

    fn rollback(&mut self, tree: &mut UiTree) -> DockResult<()> {
        let rollback = self.rollback_data.take().ok_or(DockError::NoRollbackData)?;

        // Remove tab from target
        {
            let target_mut = tree
                .get_widget_mut(self.target_container)
                .ok_or(DockError::NodeNotFound(self.target_container))?;

            let target_tabs = target_mut
                .as_any_mut()
                .downcast_mut::<DockTabs>()
                .ok_or(DockError::InvalidWidgetType)?;

            if rollback.target_index < target_tabs.tab_count() {
                target_tabs.remove_tab(rollback.target_index);
            }
        }

        // Re-add tab to source at original index
        {
            let source_mut = tree
                .get_widget_mut(self.source_container)
                .ok_or(DockError::NodeNotFound(self.source_container))?;

            let source_tabs = source_mut
                .as_any_mut()
                .downcast_mut::<DockTabs>()
                .ok_or(DockError::InvalidWidgetType)?;

            let insert_index = rollback.source_index.min(source_tabs.tab_count());
            source_tabs.insert_tab_at(insert_index, &rollback.tab_label, rollback.tab_content);
        }

        // Sync tree-level parent/child relationships
        tree.remove_child(self.target_container, rollback.tab_content);
        tree.add_child(self.source_container, rollback.tab_content);

        // Mark both containers and transferred content dirty
        tree.mark_dirty_flags(
            self.source_container,
            crate::dirty::DirtyFlags::LAYOUT | crate::dirty::DirtyFlags::CHILDREN_ORDER,
        );
        tree.mark_dirty_flags(
            self.target_container,
            crate::dirty::DirtyFlags::LAYOUT | crate::dirty::DirtyFlags::CHILDREN_ORDER,
        );
        tree.mark_dirty_flags(
            rollback.tab_content,
            crate::dirty::DirtyFlags::LAYOUT | crate::dirty::DirtyFlags::GEOMETRY,
        );

        Ok(())
    }
}

/// Split a container by wrapping the target in a new DockSplitter.
///
/// When a tab is dropped on an edge zone (Left/Right/Top/Bottom) of a target
/// DockTabs container, this operation:
/// 1. Extracts the dragged tab from the source container
/// 2. Creates a new DockTabs with just that tab
/// 3. Creates a new DockSplitter with the appropriate direction
/// 4. Replaces the target in its parent with the splitter
/// 5. Makes target + new tabs the splitter's children (order based on zone)
#[derive(Debug)]
pub struct SplitContainerOperation {
    /// Source container node (where the tab is dragged from).
    pub source_container: NodeId,
    /// Target container node (where the tab is dropped on an edge).
    pub target_container: NodeId,
    /// Index of tab in source container.
    pub source_tab_index: usize,
    /// The edge zone where the tab was dropped.
    pub zone: DockZone,
    /// Rollback data (stored after execution).
    rollback_data: Option<SplitRollback>,
}

#[derive(Debug)]
struct SplitRollback {
    /// The new DockTabs node that was created.
    new_tabs_node: NodeId,
    /// The new DockSplitter node that was created.
    splitter_node: NodeId,
    /// The tab label that was transferred.
    tab_label: String,
    /// The tab content node that was transferred.
    tab_content: NodeId,
    /// The parent of the target container (before the split), or None if target was root.
    target_parent: Option<NodeId>,
}

impl SplitContainerOperation {
    /// Create a new split container operation.
    pub fn new(
        source_container: NodeId,
        target_container: NodeId,
        source_tab_index: usize,
        zone: DockZone,
    ) -> Self {
        Self {
            source_container,
            target_container,
            source_tab_index,
            zone,
            rollback_data: None,
        }
    }
}

impl DockOperation for SplitContainerOperation {
    fn execute(&mut self, tree: &mut UiTree) -> DockResult<()> {
        // Validate zone is an edge zone
        if matches!(self.zone, DockZone::Center) {
            return Err(DockError::EdgeZoneRequired);
        }

        // Validate source container
        let source_widget = tree
            .get_widget(self.source_container)
            .ok_or(DockError::NodeNotFound(self.source_container))?;
        let source_tabs = source_widget
            .as_any()
            .downcast_ref::<DockTabs>()
            .ok_or(DockError::InvalidWidgetType)?;
        if self.source_tab_index >= source_tabs.tab_count() {
            return Err(DockError::InvalidTabIndex);
        }

        // Validate target container
        let _ = tree
            .get_widget(self.target_container)
            .ok_or(DockError::NodeNotFound(self.target_container))?
            .as_any()
            .downcast_ref::<DockTabs>()
            .ok_or(DockError::InvalidWidgetType)?;

        // Get target's parent info before making changes
        let target_parent = tree.get_node(self.target_container).and_then(|n| n.parent);
        let target_index_in_parent = if let Some(parent_id) = target_parent {
            tree.get_node(parent_id)
                .map(|p| {
                    p.children
                        .iter()
                        .position(|&c| c == self.target_container)
                        .unwrap_or(0)
                })
                .unwrap_or(0)
        } else {
            0
        };

        // Step 1: Extract tab from source
        let (tab_label, tab_content) = {
            let source_mut = tree
                .get_widget_mut(self.source_container)
                .ok_or(DockError::NodeNotFound(self.source_container))?;
            let source_tabs = source_mut
                .as_any_mut()
                .downcast_mut::<DockTabs>()
                .ok_or(DockError::InvalidWidgetType)?;

            source_tabs
                .remove_tab(self.source_tab_index)
                .ok_or(DockError::InvalidTabIndex)?
        };

        // Sync tree: remove tab content from source container
        tree.remove_child(self.source_container, tab_content);

        // Step 2: Create new DockTabs with the extracted tab
        let mut new_tabs = DockTabs::new();
        new_tabs.add_tab(&tab_label, tab_content);

        // Copy theme properties and content padding from source container
        if let Some(source_widget) = tree.get_widget(self.source_container)
            && let Some(source_tabs) = source_widget.as_any().downcast_ref::<DockTabs>()
        {
            new_tabs.theme = source_tabs.theme.clone();
            new_tabs.content_padding = source_tabs.content_padding;
        }

        // Copy layout style from target container to inherit sizing behavior
        if let Some(target_widget) = tree.get_widget(self.target_container) {
            new_tabs.style = target_widget.style().clone();
        }

        let new_tabs_node = tree.add_widget(Box::new(new_tabs));
        tree.add_child(new_tabs_node, tab_content);

        // Step 3: Create new DockSplitter
        let direction = self
            .zone
            .split_direction()
            .expect("Edge zone must have a split direction");
        let mut splitter = DockSplitter::new(direction);

        // Copy style from target container so splitter inherits sizing
        if let Some(target_widget) = tree.get_widget(self.target_container) {
            splitter.style = target_widget.style().clone();
        }

        // Order children based on zone
        if self.zone.is_before() {
            // Left/Top: new tabs first, then target
            splitter.children = vec![new_tabs_node, self.target_container];
        } else {
            // Right/Bottom: target first, then new tabs
            splitter.children = vec![self.target_container, new_tabs_node];
        }

        let splitter_node = tree.add_widget(Box::new(splitter));

        // Step 4: Set tree-level children for the splitter
        tree.add_child(splitter_node, new_tabs_node);

        // Move target from its current parent to splitter
        if let Some(parent_id) = target_parent {
            tree.remove_child(parent_id, self.target_container);
        }
        tree.add_child(splitter_node, self.target_container);

        // Step 5: Replace target in its parent with the splitter
        if let Some(parent_id) = target_parent {
            // Replace target with splitter in parent's widget children
            if let Some(parent_widget) = tree.get_widget_mut(parent_id)
                && let Some(children) = parent_widget.children_mut()
            {
                if let Some(pos) = children.iter().position(|&c| c == self.target_container) {
                    children[pos] = splitter_node;
                } else {
                    // Target was already removed from widget children by remove_child;
                    // insert splitter at the original index
                    let insert_pos = target_index_in_parent.min(children.len());
                    children.insert(insert_pos, splitter_node);
                }
            }
            // Add splitter as tree child of parent
            tree.add_child(parent_id, splitter_node);
            // Remove the duplicate — add_child appends, but we already positioned it in widget children
            // We need to ensure tree children match widget children for the parent
            // The simplest approach: just sync the tree children from widget children
            if let Some(parent_widget) = tree.get_widget(parent_id) {
                let widget_children: Vec<NodeId> = parent_widget.children().to_vec();
                tree.set_children(parent_id, &widget_children);
            }
        } else {
            // Target was root — make splitter the new root
            tree.set_root(splitter_node);
        }

        // Store rollback data
        self.rollback_data = Some(SplitRollback {
            new_tabs_node,
            splitter_node,
            tab_label,
            tab_content,
            target_parent,
        });

        // Mark everything dirty
        tree.mark_dirty_flags(
            self.source_container,
            crate::dirty::DirtyFlags::LAYOUT | crate::dirty::DirtyFlags::CHILDREN_ORDER,
        );
        tree.mark_dirty_flags(self.target_container, crate::dirty::DirtyFlags::LAYOUT);
        tree.mark_dirty_flags(
            new_tabs_node,
            crate::dirty::DirtyFlags::LAYOUT | crate::dirty::DirtyFlags::CHILDREN_ORDER,
        );
        tree.mark_dirty_flags(
            splitter_node,
            crate::dirty::DirtyFlags::LAYOUT | crate::dirty::DirtyFlags::CHILDREN_ORDER,
        );
        tree.mark_dirty_flags(
            tab_content,
            crate::dirty::DirtyFlags::LAYOUT | crate::dirty::DirtyFlags::GEOMETRY,
        );
        if let Some(parent_id) = target_parent {
            tree.mark_dirty_flags(
                parent_id,
                crate::dirty::DirtyFlags::LAYOUT | crate::dirty::DirtyFlags::CHILDREN_ORDER,
            );
        }

        Ok(())
    }

    fn rollback(&mut self, tree: &mut UiTree) -> DockResult<()> {
        let rollback = self.rollback_data.take().ok_or(DockError::NoRollbackData)?;

        // Step 1: Remove target from splitter
        tree.remove_child(rollback.splitter_node, self.target_container);

        // Step 2: Replace splitter with target in parent
        if let Some(parent_id) = rollback.target_parent {
            // Replace splitter with target in parent's widget children
            if let Some(parent_widget) = tree.get_widget_mut(parent_id)
                && let Some(children) = parent_widget.children_mut()
                && let Some(pos) = children.iter().position(|&c| c == rollback.splitter_node)
            {
                children[pos] = self.target_container;
            }
            // Sync tree children from widget children
            tree.remove_child(parent_id, rollback.splitter_node);
            tree.add_child(parent_id, self.target_container);
            if let Some(parent_widget) = tree.get_widget(parent_id) {
                let widget_children: Vec<NodeId> = parent_widget.children().to_vec();
                tree.set_children(parent_id, &widget_children);
            }
        } else {
            // Target was root
            tree.set_root(self.target_container);
        }

        // Step 3: Re-add tab to source container
        {
            let source_mut = tree
                .get_widget_mut(self.source_container)
                .ok_or(DockError::NodeNotFound(self.source_container))?;
            let source_tabs = source_mut
                .as_any_mut()
                .downcast_mut::<DockTabs>()
                .ok_or(DockError::InvalidWidgetType)?;

            let insert_index = self.source_tab_index.min(source_tabs.tab_count());
            source_tabs.insert_tab_at(insert_index, &rollback.tab_label, rollback.tab_content);
        }
        tree.add_child(self.source_container, rollback.tab_content);

        // Step 4: Remove the created nodes
        // Remove new_tabs_node first (child of splitter), then splitter
        tree.remove_node(rollback.new_tabs_node);
        tree.remove_node(rollback.splitter_node);

        // Mark dirty
        tree.mark_dirty_flags(
            self.source_container,
            crate::dirty::DirtyFlags::LAYOUT | crate::dirty::DirtyFlags::CHILDREN_ORDER,
        );
        tree.mark_dirty_flags(self.target_container, crate::dirty::DirtyFlags::LAYOUT);
        tree.mark_dirty_flags(
            rollback.tab_content,
            crate::dirty::DirtyFlags::LAYOUT | crate::dirty::DirtyFlags::GEOMETRY,
        );
        if let Some(parent_id) = rollback.target_parent {
            tree.mark_dirty_flags(
                parent_id,
                crate::dirty::DirtyFlags::LAYOUT | crate::dirty::DirtyFlags::CHILDREN_ORDER,
            );
        }

        Ok(())
    }
}

/// Collapse an empty DockTabs container by promoting its sibling.
///
/// When a DockTabs becomes empty (tab_count() == 0) after a transfer or close,
/// and its parent is a DockSplitter, the splitter should collapse — the remaining
/// child replaces the splitter in its grandparent (or becomes root).
///
/// Returns `Ok(true)` if the container was collapsed, `Ok(false)` if no action
/// was needed (non-empty, no parent, parent not a splitter).
pub fn collapse_empty_container(tree: &mut UiTree, container_id: NodeId) -> DockResult<bool> {
    // Step 1: Check if container is an empty DockTabs
    let is_empty = {
        let widget = tree
            .get_widget(container_id)
            .ok_or(DockError::NodeNotFound(container_id))?;
        let tabs = widget
            .as_any()
            .downcast_ref::<DockTabs>()
            .ok_or(DockError::InvalidWidgetType)?;
        tabs.tab_count() == 0
    };

    if !is_empty {
        return Ok(false);
    }

    // Step 2: Get parent
    let parent_id = match tree.get_node(container_id).and_then(|n| n.parent) {
        Some(id) => id,
        None => return Ok(false), // Empty root — leave as-is
    };

    // Step 3: Check parent is a DockSplitter
    let is_splitter = tree
        .get_widget(parent_id)
        .map(|w| w.as_any().downcast_ref::<DockSplitter>().is_some())
        .unwrap_or(false);

    if !is_splitter {
        return Ok(false);
    }

    // Step 4: Find the remaining sibling
    let splitter_children: Vec<NodeId> = tree
        .get_widget(parent_id)
        .map(|w| w.children().to_vec())
        .unwrap_or_default();

    let remaining_child = splitter_children
        .iter()
        .find(|&&c| c != container_id)
        .copied()
        .ok_or(DockError::NoRemainingSibling)?;

    // Step 5: Copy splitter's sizing style to remaining child
    let splitter_style = tree.get_widget(parent_id).map(|w| w.style().clone());

    if let Some(style) = splitter_style {
        if let Some(remaining_widget) = tree.get_widget_mut(remaining_child) {
            let child_style = remaining_widget.style_mut();
            child_style.layout.size = style.layout.size;
            child_style.layout.flex_grow = style.layout.flex_grow;
            child_style.layout.flex_shrink = style.layout.flex_shrink;
            // Copy constraints for viewport units
            child_style.constraints = style.constraints.clone();
        }
        // Sync the updated style to Taffy so layout picks up the new sizing
        tree.sync_taffy_style(remaining_child);
    }

    // Step 6: Get grandparent info
    let grandparent_id = tree.get_node(parent_id).and_then(|n| n.parent);

    // Step 7: Detach both children from splitter
    tree.remove_child(parent_id, container_id);
    tree.remove_child(parent_id, remaining_child);

    // Step 8: Replace splitter with remaining child
    if let Some(gp_id) = grandparent_id {
        // Replace splitter with remaining child in grandparent's widget children
        if let Some(gp_widget) = tree.get_widget_mut(gp_id)
            && let Some(children) = gp_widget.children_mut()
            && let Some(pos) = children.iter().position(|&c| c == parent_id)
        {
            children[pos] = remaining_child;
        }

        // Update tree-level relationships
        tree.remove_child(gp_id, parent_id);
        tree.add_child(gp_id, remaining_child);

        // Sync tree children from widget children to maintain order
        if let Some(gp_widget) = tree.get_widget(gp_id) {
            let widget_children: Vec<NodeId> = gp_widget.children().to_vec();
            tree.set_children(gp_id, &widget_children);
        }

        tree.mark_dirty_flags(
            gp_id,
            crate::dirty::DirtyFlags::LAYOUT
                | crate::dirty::DirtyFlags::CHILDREN_ORDER
                | crate::dirty::DirtyFlags::GEOMETRY,
        );
    } else {
        // Splitter was root — make remaining child the new root
        tree.set_root(remaining_child);
    }

    // Step 9: Remove the empty tabs node and the orphaned splitter
    tree.remove_node(container_id);
    tree.remove_node(parent_id);

    // Step 10: Mark remaining child dirty
    tree.mark_dirty_flags(
        remaining_child,
        crate::dirty::DirtyFlags::LAYOUT | crate::dirty::DirtyFlags::GEOMETRY,
    );

    Ok(true)
}

/// Merge all tabs from a source DockTabs into a target DockTabs (center-zone group drop).
///
/// After merging, the source container becomes empty and should be collapsed
/// via `collapse_empty_container`.
#[derive(Debug)]
pub struct MergeTabGroupOperation {
    /// Source container node (group being merged).
    pub source_container: NodeId,
    /// Target container node (receiving the tabs).
    pub target_container: NodeId,
    /// Index in target where tabs will be inserted.
    pub target_insert_index: usize,
    /// Rollback data.
    rollback_data: Option<MergeGroupRollback>,
}

#[derive(Debug)]
struct MergeGroupRollback {
    /// (label, content) pairs that were transferred.
    tabs: Vec<(String, NodeId)>,
    /// Insertion point in target.
    target_start_index: usize,
    /// Original active tab in source.
    source_active_tab: usize,
}

impl MergeTabGroupOperation {
    /// Create a new merge tab group operation.
    pub fn new(
        source_container: NodeId,
        target_container: NodeId,
        target_insert_index: usize,
    ) -> Self {
        Self {
            source_container,
            target_container,
            target_insert_index,
            rollback_data: None,
        }
    }
}

impl DockOperation for MergeTabGroupOperation {
    fn execute(&mut self, tree: &mut UiTree) -> DockResult<()> {
        // Validate both containers exist and are DockTabs
        let _ = tree
            .get_widget(self.source_container)
            .ok_or(DockError::NodeNotFound(self.source_container))?
            .as_any()
            .downcast_ref::<DockTabs>()
            .ok_or(DockError::InvalidWidgetType)?;

        let _ = tree
            .get_widget(self.target_container)
            .ok_or(DockError::NodeNotFound(self.target_container))?
            .as_any()
            .downcast_ref::<DockTabs>()
            .ok_or(DockError::InvalidWidgetType)?;

        if self.source_container == self.target_container {
            return Err(DockError::SameContainerTransfer);
        }

        // Extract all tabs from source
        let (tabs, source_active_tab) = {
            let source_mut = tree
                .get_widget_mut(self.source_container)
                .ok_or(DockError::NodeNotFound(self.source_container))?;
            let source_tabs = source_mut
                .as_any_mut()
                .downcast_mut::<DockTabs>()
                .ok_or(DockError::InvalidWidgetType)?;

            let active = source_tabs.active_tab;
            let all_tabs = source_tabs.remove_all_tabs();
            (all_tabs, active)
        };

        if tabs.is_empty() {
            return Ok(()); // Nothing to merge
        }

        // Insert into target at specified index
        let insert_idx = {
            let target_mut = tree
                .get_widget_mut(self.target_container)
                .ok_or(DockError::NodeNotFound(self.target_container))?;
            let target_tabs = target_mut
                .as_any_mut()
                .downcast_mut::<DockTabs>()
                .ok_or(DockError::InvalidWidgetType)?;

            let idx = self.target_insert_index.min(target_tabs.tab_count());
            target_tabs.insert_tabs_at(idx, &tabs);
            idx
        };

        // Sync tree children
        for (_, content) in &tabs {
            tree.remove_child(self.source_container, *content);
            tree.add_child(self.target_container, *content);
        }

        // Store rollback
        self.rollback_data = Some(MergeGroupRollback {
            tabs,
            target_start_index: insert_idx,
            source_active_tab,
        });

        // Mark dirty
        tree.mark_dirty_flags(
            self.source_container,
            crate::dirty::DirtyFlags::LAYOUT | crate::dirty::DirtyFlags::CHILDREN_ORDER,
        );
        tree.mark_dirty_flags(
            self.target_container,
            crate::dirty::DirtyFlags::LAYOUT
                | crate::dirty::DirtyFlags::CHILDREN_ORDER
                | crate::dirty::DirtyFlags::GEOMETRY,
        );

        Ok(())
    }

    fn rollback(&mut self, tree: &mut UiTree) -> DockResult<()> {
        let rollback = self.rollback_data.take().ok_or(DockError::NoRollbackData)?;

        // Remove merged tabs from target (in reverse to preserve indices)
        let tab_count = rollback.tabs.len();
        for i in (0..tab_count).rev() {
            let idx = rollback.target_start_index + i;
            let target_mut = tree
                .get_widget_mut(self.target_container)
                .ok_or(DockError::NodeNotFound(self.target_container))?;
            let target_tabs = target_mut
                .as_any_mut()
                .downcast_mut::<DockTabs>()
                .ok_or(DockError::InvalidWidgetType)?;
            target_tabs.remove_tab(idx);
        }

        // Re-add tabs to source
        {
            let source_mut = tree
                .get_widget_mut(self.source_container)
                .ok_or(DockError::NodeNotFound(self.source_container))?;
            let source_tabs = source_mut
                .as_any_mut()
                .downcast_mut::<DockTabs>()
                .ok_or(DockError::InvalidWidgetType)?;
            source_tabs.insert_tabs_at(0, &rollback.tabs);
            source_tabs.active_tab = rollback
                .source_active_tab
                .min(source_tabs.tab_count().saturating_sub(1));
        }

        // Sync tree children
        for (_, content) in &rollback.tabs {
            tree.remove_child(self.target_container, *content);
            tree.add_child(self.source_container, *content);
        }

        // Mark dirty
        tree.mark_dirty_flags(
            self.source_container,
            crate::dirty::DirtyFlags::LAYOUT | crate::dirty::DirtyFlags::CHILDREN_ORDER,
        );
        tree.mark_dirty_flags(
            self.target_container,
            crate::dirty::DirtyFlags::LAYOUT | crate::dirty::DirtyFlags::CHILDREN_ORDER,
        );

        Ok(())
    }
}

/// Move an entire tab group to an edge zone of another container.
///
/// Detaches the source from its parent splitter, collapses that splitter,
/// then wraps the target in a new splitter with the source as a sibling.
#[derive(Debug)]
pub struct MoveTabGroupOperation {
    /// Source container node (the group being moved).
    pub source_container: NodeId,
    /// Target container node (where the group is dropped).
    pub target_container: NodeId,
    /// The edge zone where the group was dropped.
    pub zone: DockZone,
    /// Rollback data.
    rollback_data: Option<MoveGroupRollback>,
}

#[derive(Debug)]
struct MoveGroupRollback {
    /// The new splitter node created for the split.
    _splitter_node: NodeId,
    /// The parent of the target before the move (or None if root).
    _target_parent: Option<NodeId>,
    /// The parent of the source before the move (or None if root).
    _source_parent: Option<NodeId>,
    /// The index of the source in its parent's children.
    _source_index_in_parent: usize,
}

impl MoveTabGroupOperation {
    /// Create a new move tab group operation.
    pub fn new(source_container: NodeId, target_container: NodeId, zone: DockZone) -> Self {
        Self {
            source_container,
            target_container,
            zone,
            rollback_data: None,
        }
    }
}

impl DockOperation for MoveTabGroupOperation {
    fn execute(&mut self, tree: &mut UiTree) -> DockResult<()> {
        // Validate zone is edge
        if matches!(self.zone, DockZone::Center) {
            return Err(DockError::EdgeZoneRequired);
        }

        // Validate both are DockTabs
        let _ = tree
            .get_widget(self.source_container)
            .ok_or(DockError::NodeNotFound(self.source_container))?
            .as_any()
            .downcast_ref::<DockTabs>()
            .ok_or(DockError::InvalidWidgetType)?;

        let _ = tree
            .get_widget(self.target_container)
            .ok_or(DockError::NodeNotFound(self.target_container))?
            .as_any()
            .downcast_ref::<DockTabs>()
            .ok_or(DockError::InvalidWidgetType)?;

        // Get source's parent info
        let source_parent = tree.get_node(self.source_container).and_then(|n| n.parent);
        let source_index_in_parent = if let Some(parent_id) = source_parent {
            tree.get_node(parent_id)
                .map(|p| {
                    p.children
                        .iter()
                        .position(|&c| c == self.source_container)
                        .unwrap_or(0)
                })
                .unwrap_or(0)
        } else {
            0
        };

        // Get target's parent info
        let target_parent = tree.get_node(self.target_container).and_then(|n| n.parent);
        let target_index_in_parent = if let Some(parent_id) = target_parent {
            tree.get_node(parent_id)
                .map(|p| {
                    p.children
                        .iter()
                        .position(|&c| c == self.target_container)
                        .unwrap_or(0)
                })
                .unwrap_or(0)
        } else {
            0
        };

        // Step 1: Detach source from its parent
        if let Some(parent_id) = source_parent {
            // Remove from parent's widget children
            if let Some(parent_widget) = tree.get_widget_mut(parent_id)
                && let Some(children) = parent_widget.children_mut()
            {
                children.retain(|&c| c != self.source_container);
            }
            tree.remove_child(parent_id, self.source_container);
        }

        // Step 2: Collapse source's parent splitter if it now has only one child
        if let Some(parent_id) = source_parent {
            let is_splitter = tree
                .get_widget(parent_id)
                .map(|w| w.as_any().downcast_ref::<DockSplitter>().is_some())
                .unwrap_or(false);

            if is_splitter {
                // Read from tree node children, not widget children.
                // After Step 1 the splitter has 1 child (intermediate state),
                // and DockSplitter::children() has a debug_assert requiring 0 or 2.
                let remaining_children: Vec<NodeId> = tree
                    .get_node(parent_id)
                    .map(|n| n.children.clone())
                    .unwrap_or_default();

                if remaining_children.len() == 1 {
                    let remaining = remaining_children[0];

                    // Copy splitter's sizing to remaining child
                    let splitter_style = tree.get_widget(parent_id).map(|w| w.style().clone());
                    if let Some(style) = splitter_style {
                        if let Some(remaining_widget) = tree.get_widget_mut(remaining) {
                            let child_style = remaining_widget.style_mut();
                            child_style.layout.size = style.layout.size;
                            child_style.layout.flex_grow = style.layout.flex_grow;
                            child_style.layout.flex_shrink = style.layout.flex_shrink;
                            child_style.constraints = style.constraints.clone();
                        }
                        tree.sync_taffy_style(remaining);
                    }

                    // Get grandparent
                    let grandparent = tree.get_node(parent_id).and_then(|n| n.parent);

                    // Detach remaining from splitter
                    tree.remove_child(parent_id, remaining);

                    // Replace splitter with remaining in grandparent
                    if let Some(gp_id) = grandparent {
                        if let Some(gp_widget) = tree.get_widget_mut(gp_id)
                            && let Some(children) = gp_widget.children_mut()
                            && let Some(pos) = children.iter().position(|&c| c == parent_id)
                        {
                            children[pos] = remaining;
                        }
                        tree.remove_child(gp_id, parent_id);
                        tree.add_child(gp_id, remaining);
                        if let Some(gp_widget) = tree.get_widget(gp_id) {
                            let widget_children: Vec<NodeId> = gp_widget.children().to_vec();
                            tree.set_children(gp_id, &widget_children);
                        }
                        tree.mark_dirty_flags(
                            gp_id,
                            crate::dirty::DirtyFlags::LAYOUT
                                | crate::dirty::DirtyFlags::CHILDREN_ORDER,
                        );
                    } else {
                        tree.set_root(remaining);
                    }

                    tree.remove_node(parent_id);
                    tree.mark_dirty_flags(
                        remaining,
                        crate::dirty::DirtyFlags::LAYOUT | crate::dirty::DirtyFlags::GEOMETRY,
                    );
                }
            }
        }

        // Step 3: Wrap target in a new DockSplitter
        let direction = self
            .zone
            .split_direction()
            .expect("Edge zone must have a split direction");
        let mut splitter = DockSplitter::new(direction);

        // Copy style from target container so splitter inherits sizing
        if let Some(target_widget) = tree.get_widget(self.target_container) {
            splitter.style = target_widget.style().clone();
        }

        // Also copy sizing to source from target so panels are balanced
        if let Some(target_widget) = tree.get_widget(self.target_container) {
            let target_style = target_widget.style().clone();
            if let Some(source_widget) = tree.get_widget_mut(self.source_container) {
                let source_style = source_widget.style_mut();
                source_style.layout.size = target_style.layout.size;
                source_style.layout.flex_grow = target_style.layout.flex_grow;
                source_style.layout.flex_shrink = target_style.layout.flex_shrink;
            }
        }

        // Order children based on zone
        if self.zone.is_before() {
            splitter.children = vec![self.source_container, self.target_container];
        } else {
            splitter.children = vec![self.target_container, self.source_container];
        }

        let splitter_node = tree.add_widget(Box::new(splitter));

        // Set tree children for splitter
        tree.add_child(splitter_node, self.source_container);

        // Re-read target_parent since the tree structure may have changed
        // due to collapsing the source's parent
        let current_target_parent = tree.get_node(self.target_container).and_then(|n| n.parent);

        // Move target from its current parent to splitter
        if let Some(tp_id) = current_target_parent {
            tree.remove_child(tp_id, self.target_container);
        }
        tree.add_child(splitter_node, self.target_container);

        // Replace target in its parent with the splitter
        if let Some(tp_id) = current_target_parent {
            if let Some(tp_widget) = tree.get_widget_mut(tp_id)
                && let Some(children) = tp_widget.children_mut()
            {
                if let Some(pos) = children.iter().position(|&c| c == self.target_container) {
                    children[pos] = splitter_node;
                } else {
                    let insert_pos = target_index_in_parent.min(children.len());
                    children.insert(insert_pos, splitter_node);
                }
            }
            tree.add_child(tp_id, splitter_node);
            if let Some(tp_widget) = tree.get_widget(tp_id) {
                let widget_children: Vec<NodeId> = tp_widget.children().to_vec();
                tree.set_children(tp_id, &widget_children);
            }
        } else {
            // Target was root
            tree.set_root(splitter_node);
        }

        // Store rollback data
        self.rollback_data = Some(MoveGroupRollback {
            _splitter_node: splitter_node,
            _target_parent: target_parent,
            _source_parent: source_parent,
            _source_index_in_parent: source_index_in_parent,
        });

        // Mark dirty
        tree.mark_dirty_flags(
            self.source_container,
            crate::dirty::DirtyFlags::LAYOUT | crate::dirty::DirtyFlags::GEOMETRY,
        );
        tree.mark_dirty_flags(self.target_container, crate::dirty::DirtyFlags::LAYOUT);
        tree.mark_dirty_flags(
            splitter_node,
            crate::dirty::DirtyFlags::LAYOUT | crate::dirty::DirtyFlags::CHILDREN_ORDER,
        );
        if let Some(tp_id) = current_target_parent {
            tree.mark_dirty_flags(
                tp_id,
                crate::dirty::DirtyFlags::LAYOUT | crate::dirty::DirtyFlags::CHILDREN_ORDER,
            );
        }

        Ok(())
    }

    fn rollback(&mut self, _tree: &mut UiTree) -> DockResult<()> {
        // Rollback for MoveTabGroupOperation is complex due to splitter collapse.
        // For now, return an error indicating rollback is not supported.
        Err(DockError::NoRollbackData)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transfer_tab_error_cases() {
        let mut tree = UiTree::new();

        let mut op = TransferTabOperation::new(NodeId(0), NodeId(1), 0, 0);

        // Should fail - nodes don't exist
        assert!(matches!(
            op.execute(&mut tree),
            Err(DockError::NodeNotFound(_))
        ));
    }

    #[test]
    fn test_rollback_without_execute() {
        let mut tree = UiTree::new();

        let mut op = TransferTabOperation::new(NodeId(0), NodeId(1), 0, 0);

        // Should fail - no rollback data
        assert!(matches!(
            op.rollback(&mut tree),
            Err(DockError::NoRollbackData)
        ));
    }
}
