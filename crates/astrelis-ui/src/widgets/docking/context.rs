//! Container registry cache for efficient docking lookups.
//!
//! During cross-container tab dragging, the event handler needs to find all
//! DockTabs containers to test for drop targets. Without caching, this requires
//! an O(N) tree walk on every mouse move. `DockingContext` caches this data and
//! invalidates it when the tree structure changes.

use super::DockTabs;
use super::splitter::{DEFAULT_SEPARATOR_SIZE, default_separator_color, default_separator_hover_color};
use super::tabs::{
    DEFAULT_TAB_BAR_HEIGHT, default_active_tab_color, default_inactive_tab_color,
    default_tab_bar_color, default_tab_hover_color, default_tab_text_color,
};
use crate::tree::{LayoutRect, NodeId, UiTree};
use astrelis_core::alloc::HashMap;
use astrelis_render::Color;

/// Centralized styling defaults for the docking system.
///
/// Controls separator appearance, tab bar colors/sizing, and content padding.
/// Set on [`DockingContext`] to apply defaults to all docking widgets.
/// Individual widgets can override specific values (e.g. per-widget `content_padding`).
#[derive(Debug, Clone)]
pub struct DockingStyle {
    /// Width of splitter separators in pixels.
    pub separator_size: f32,
    /// Normal separator color.
    pub separator_color: Color,
    /// Separator color when hovered or dragged.
    pub separator_hover_color: Color,
    /// Height of the tab bar in pixels.
    pub tab_bar_height: f32,
    /// Tab bar background color.
    pub tab_bar_color: Color,
    /// Active tab background color.
    pub active_tab_color: Color,
    /// Inactive tab background color.
    pub inactive_tab_color: Color,
    /// Tab label text color.
    pub tab_text_color: Color,
    /// Tab hover background color.
    pub tab_hover_color: Color,
    /// Tab label font size.
    pub tab_font_size: f32,
    /// Whether tabs show a close button by default.
    pub closable: bool,
    /// Padding between the tab content area edges and child content (pixels).
    pub content_padding: f32,
    /// Extra hit-test tolerance around the separator (pixels per side).
    ///
    /// The separator visual is `separator_size` wide, but the grabbable area
    /// extends by this many pixels on each side perpendicular to the separator.
    pub separator_tolerance: f32,
}

impl Default for DockingStyle {
    fn default() -> Self {
        Self {
            separator_size: DEFAULT_SEPARATOR_SIZE,
            separator_color: default_separator_color(),
            separator_hover_color: default_separator_hover_color(),
            tab_bar_height: DEFAULT_TAB_BAR_HEIGHT,
            tab_bar_color: default_tab_bar_color(),
            active_tab_color: default_active_tab_color(),
            inactive_tab_color: default_inactive_tab_color(),
            tab_text_color: default_tab_text_color(),
            tab_hover_color: default_tab_hover_color(),
            tab_font_size: 11.0,
            closable: false,
            content_padding: 4.0,
            separator_tolerance: 4.0,
        }
    }
}

impl DockingStyle {
    /// Set the separator size.
    pub fn separator_size(mut self, size: f32) -> Self {
        self.separator_size = size;
        self
    }

    /// Set the separator colors.
    pub fn separator_colors(mut self, normal: Color, hover: Color) -> Self {
        self.separator_color = normal;
        self.separator_hover_color = hover;
        self
    }

    /// Set the tab bar height.
    pub fn tab_bar_height(mut self, height: f32) -> Self {
        self.tab_bar_height = height;
        self
    }

    /// Set the tab bar background color.
    pub fn tab_bar_color(mut self, color: Color) -> Self {
        self.tab_bar_color = color;
        self
    }

    /// Set the active tab color.
    pub fn active_tab_color(mut self, color: Color) -> Self {
        self.active_tab_color = color;
        self
    }

    /// Set the inactive tab color.
    pub fn inactive_tab_color(mut self, color: Color) -> Self {
        self.inactive_tab_color = color;
        self
    }

    /// Set the tab text color.
    pub fn tab_text_color(mut self, color: Color) -> Self {
        self.tab_text_color = color;
        self
    }

    /// Set the tab hover color.
    pub fn tab_hover_color(mut self, color: Color) -> Self {
        self.tab_hover_color = color;
        self
    }

    /// Set the tab font size.
    pub fn tab_font_size(mut self, size: f32) -> Self {
        self.tab_font_size = size;
        self
    }

    /// Set whether tabs are closable by default.
    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }

    /// Set the content padding (inset from panel edges).
    pub fn content_padding(mut self, padding: f32) -> Self {
        self.content_padding = padding;
        self
    }

    /// Set the separator hit-test tolerance (extra pixels per side).
    pub fn separator_tolerance(mut self, tolerance: f32) -> Self {
        self.separator_tolerance = tolerance;
        self
    }
}

/// Cached information about a DockTabs container.
#[derive(Debug, Clone)]
pub struct CachedContainerInfo {
    /// Absolute layout of the container.
    pub layout: LayoutRect,
    /// Number of tabs in the container.
    pub tab_count: usize,
}

/// Registry of DockTabs containers for efficient lookup during drag operations.
///
/// Caches the locations and metadata of all DockTabs widgets in the tree
/// so that cross-container drop target detection does not need a full tree
/// traversal on every mouse move.
pub struct DockingContext {
    /// Cached container info keyed by NodeId.
    tab_containers: HashMap<NodeId, CachedContainerInfo>,
    /// Whether the cache needs rebuilding.
    cache_dirty: bool,
    /// Centralized docking style defaults.
    style: DockingStyle,
}

impl DockingContext {
    /// Create a new empty docking context.
    pub fn new() -> Self {
        Self {
            tab_containers: HashMap::new(),
            cache_dirty: true,
            style: DockingStyle::default(),
        }
    }

    /// Get a reference to the docking style.
    pub fn style(&self) -> &DockingStyle {
        &self.style
    }

    /// Get a mutable reference to the docking style.
    pub fn style_mut(&mut self) -> &mut DockingStyle {
        &mut self.style
    }

    /// Replace the docking style.
    pub fn set_style(&mut self, style: DockingStyle) {
        self.style = style;
    }

    /// Mark the cache as needing a rebuild.
    ///
    /// Call this after any tree structure change (node add/remove, tab transfer, split).
    pub fn invalidate(&mut self) {
        self.cache_dirty = true;
    }

    /// Rebuild the container cache from the tree.
    ///
    /// Walks the tree to find all DockTabs widgets and caches their layout and tab count.
    pub fn rebuild_cache(&mut self, tree: &UiTree) {
        self.tab_containers.clear();

        let all_tabs = tree.find_widgets_with_layout::<DockTabs>();
        for (node_id, layout) in all_tabs {
            let tab_count = tree
                .get_widget(node_id)
                .and_then(|w| w.as_any().downcast_ref::<DockTabs>())
                .map(|t| t.tab_count())
                .unwrap_or(0);

            self.tab_containers
                .insert(node_id, CachedContainerInfo { layout, tab_count });
        }

        self.cache_dirty = false;
    }

    /// Get the cached tab containers, rebuilding if necessary.
    pub fn find_tab_containers(&mut self, tree: &UiTree) -> &HashMap<NodeId, CachedContainerInfo> {
        if self.cache_dirty {
            self.rebuild_cache(tree);
        }
        &self.tab_containers
    }

    /// Check if the cache is dirty and needs a rebuild.
    pub fn is_dirty(&self) -> bool {
        self.cache_dirty
    }

    /// Get the number of cached containers.
    pub fn container_count(&self) -> usize {
        self.tab_containers.len()
    }
}

impl Default for DockingContext {
    fn default() -> Self {
        Self::new()
    }
}
