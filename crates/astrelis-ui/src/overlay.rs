//! Overlay management system for menus, tooltips, modals, and popovers.
//!
//! The overlay system provides:
//! - Z-layer based rendering order
//! - Automatic focus trapping for modals
//! - Click-outside-to-close behavior
//! - Escape key handling
//! - Position anchoring to widgets
//!
//! # Architecture
//!
//! Overlays are organized into Z-layers:
//! - Base (0): Normal UI content
//! - Tooltip (1000): Hover tooltips
//! - Dropdown (2000): Select menus, combo boxes
//! - Modal (3000): Dialog boxes, alerts
//! - Popover (4000): Context menus, floating panels
//!
//! Each layer can have multiple overlays, managed as a stack within that layer.
//!
//! # Example
//!
//! ```ignore
//! use astrelis_ui::overlay::{OverlayManager, OverlayConfig, ZLayer, OverlayPosition};
//!
//! let mut overlays = OverlayManager::new();
//!
//! // Show a modal dialog
//! let modal_id = overlays.show(
//!     &mut tree,
//!     modal_root_node,
//!     OverlayConfig {
//!         layer: ZLayer::Modal,
//!         position: OverlayPosition::Center,
//!         close_on_outside_click: false,
//!         close_on_escape: true,
//!         trap_focus: true,
//!         show_backdrop: true,
//!         backdrop_color: Color::rgba(0.0, 0.0, 0.0, 0.5),
//!     },
//! );
//!
//! // Later, close it
//! overlays.hide(&mut tree, modal_id);
//! ```

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use astrelis_core::alloc::HashMap;
use astrelis_core::math::Vec2;
use astrelis_render::Color;

use crate::draw_list::RenderLayer;
use crate::tree::{NodeId, UiTree};

/// Global counter for generating unique overlay IDs.
static OVERLAY_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Unique identifier for an overlay instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OverlayId(pub u64);

impl OverlayId {
    /// Generate a new unique overlay ID.
    pub fn new() -> Self {
        Self(OVERLAY_ID_COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl Default for OverlayId {
    fn default() -> Self {
        Self::new()
    }
}

/// Z-layer for overlay ordering.
///
/// Higher values render on top of lower values.
/// Custom layers can be created for special use cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ZLayer {
    /// Base UI layer (normal widgets).
    Base,
    /// Tooltip layer (hover information).
    Tooltip,
    /// Dropdown/select menu layer.
    Dropdown,
    /// Modal dialog layer.
    Modal,
    /// Popover/context menu layer.
    Popover,
    /// Debug/inspector layer (highest priority, renders on top of everything).
    Debug,
    /// Custom layer with explicit z-value.
    Custom(u16),
}

impl ZLayer {
    /// Get the z-index value for this layer.
    pub fn z_index(&self) -> u16 {
        match self {
            ZLayer::Base => 0,
            ZLayer::Tooltip => 1000,
            ZLayer::Dropdown => 2000,
            ZLayer::Modal => 3000,
            ZLayer::Popover => 4000,
            ZLayer::Debug => 5000,
            ZLayer::Custom(z) => *z,
        }
    }

    /// Create a custom layer at a specific z-index.
    pub fn custom(z: u16) -> Self {
        ZLayer::Custom(z)
    }

    /// Check if this layer is above another.
    pub fn is_above(&self, other: &ZLayer) -> bool {
        self.z_index() > other.z_index()
    }

    /// Convert this Z-layer to a render layer.
    ///
    /// Base maps to `RenderLayer::Base`, all other layers map to
    /// `RenderLayer::Overlay(n)` with increasing sub-order values.
    pub fn render_layer(&self) -> RenderLayer {
        match self {
            ZLayer::Base => RenderLayer::Base,
            ZLayer::Tooltip => RenderLayer::Overlay(1),
            ZLayer::Dropdown => RenderLayer::Overlay(2),
            ZLayer::Modal => RenderLayer::Overlay(3),
            ZLayer::Popover => RenderLayer::Overlay(4),
            ZLayer::Debug => RenderLayer::Overlay(5),
            ZLayer::Custom(z) => {
                let order = (*z / 1000).min(255) as u8;
                if order == 0 {
                    RenderLayer::Base
                } else {
                    RenderLayer::Overlay(order)
                }
            }
        }
    }
}

/// Position strategy for overlays.
#[derive(Debug, Clone)]
pub enum OverlayPosition {
    /// Position at absolute screen coordinates.
    Absolute { x: f32, y: f32 },
    /// Center in the viewport.
    Center,
    /// Anchor to a widget with offset.
    AnchorTo {
        anchor_node: NodeId,
        alignment: AnchorAlignment,
        offset: Vec2,
    },
    /// Position at mouse cursor.
    AtCursor { offset: Vec2 },
    /// Position relative to viewport edges.
    Viewport {
        horizontal: HorizontalAlign,
        vertical: VerticalAlign,
        margin: f32,
    },
}

/// Anchor alignment options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorAlignment {
    /// Below the anchor, left-aligned.
    BelowLeft,
    /// Below the anchor, centered.
    BelowCenter,
    /// Below the anchor, right-aligned.
    BelowRight,
    /// Above the anchor, left-aligned.
    AboveLeft,
    /// Above the anchor, centered.
    AboveCenter,
    /// Above the anchor, right-aligned.
    AboveRight,
    /// To the right of anchor, top-aligned.
    RightTop,
    /// To the right of anchor, centered.
    RightCenter,
    /// To the right of anchor, bottom-aligned.
    RightBottom,
    /// To the left of anchor, top-aligned.
    LeftTop,
    /// To the left of anchor, centered.
    LeftCenter,
    /// To the left of anchor, bottom-aligned.
    LeftBottom,
}

impl AnchorAlignment {
    /// Compute the position for an overlay based on anchor position, size, and alignment.
    pub fn compute_position(
        &self,
        anchor_pos: Vec2,
        anchor_size: Vec2,
        overlay_size: Vec2,
        viewport: Vec2,
    ) -> Vec2 {
        let (mut x, mut y) = match self {
            AnchorAlignment::BelowLeft => (anchor_pos.x, anchor_pos.y + anchor_size.y),
            AnchorAlignment::BelowCenter => (
                anchor_pos.x + (anchor_size.x - overlay_size.x) / 2.0,
                anchor_pos.y + anchor_size.y,
            ),
            AnchorAlignment::BelowRight => (
                anchor_pos.x + anchor_size.x - overlay_size.x,
                anchor_pos.y + anchor_size.y,
            ),
            AnchorAlignment::AboveLeft => (anchor_pos.x, anchor_pos.y - overlay_size.y),
            AnchorAlignment::AboveCenter => (
                anchor_pos.x + (anchor_size.x - overlay_size.x) / 2.0,
                anchor_pos.y - overlay_size.y,
            ),
            AnchorAlignment::AboveRight => (
                anchor_pos.x + anchor_size.x - overlay_size.x,
                anchor_pos.y - overlay_size.y,
            ),
            AnchorAlignment::RightTop => (anchor_pos.x + anchor_size.x, anchor_pos.y),
            AnchorAlignment::RightCenter => (
                anchor_pos.x + anchor_size.x,
                anchor_pos.y + (anchor_size.y - overlay_size.y) / 2.0,
            ),
            AnchorAlignment::RightBottom => (
                anchor_pos.x + anchor_size.x,
                anchor_pos.y + anchor_size.y - overlay_size.y,
            ),
            AnchorAlignment::LeftTop => (anchor_pos.x - overlay_size.x, anchor_pos.y),
            AnchorAlignment::LeftCenter => (
                anchor_pos.x - overlay_size.x,
                anchor_pos.y + (anchor_size.y - overlay_size.y) / 2.0,
            ),
            AnchorAlignment::LeftBottom => (
                anchor_pos.x - overlay_size.x,
                anchor_pos.y + anchor_size.y - overlay_size.y,
            ),
        };

        // Clamp to viewport bounds
        x = x.clamp(0.0, (viewport.x - overlay_size.x).max(0.0));
        y = y.clamp(0.0, (viewport.y - overlay_size.y).max(0.0));

        Vec2::new(x, y)
    }
}

/// Horizontal alignment for viewport positioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorizontalAlign {
    Left,
    Center,
    Right,
}

/// Vertical alignment for viewport positioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAlign {
    Top,
    Center,
    Bottom,
}

/// Configuration for an overlay.
#[derive(Debug, Clone)]
pub struct OverlayConfig {
    /// Z-layer for this overlay.
    pub layer: ZLayer,
    /// Position strategy.
    pub position: OverlayPosition,
    /// Close when clicking outside the overlay.
    pub close_on_outside_click: bool,
    /// Close when pressing Escape.
    pub close_on_escape: bool,
    /// Trap focus within the overlay (for modals).
    pub trap_focus: bool,
    /// Show a backdrop behind the overlay.
    pub show_backdrop: bool,
    /// Backdrop color (if show_backdrop is true).
    pub backdrop_color: Color,
    /// Animation on show (future use).
    pub animate_in: bool,
    /// Animation on hide (future use).
    pub animate_out: bool,
    /// Auto-dismiss after duration (None = manual dismiss).
    pub auto_dismiss: Option<std::time::Duration>,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            layer: ZLayer::Popover,
            position: OverlayPosition::Center,
            close_on_outside_click: true,
            close_on_escape: true,
            trap_focus: false,
            show_backdrop: false,
            backdrop_color: Color::rgba(0.0, 0.0, 0.0, 0.5),
            animate_in: false,
            animate_out: false,
            auto_dismiss: None,
        }
    }
}

impl OverlayConfig {
    /// Create configuration for a tooltip.
    pub fn tooltip() -> Self {
        Self {
            layer: ZLayer::Tooltip,
            position: OverlayPosition::AtCursor {
                offset: Vec2::new(10.0, 10.0),
            },
            close_on_outside_click: false,
            close_on_escape: false,
            trap_focus: false,
            show_backdrop: false,
            backdrop_color: Color::TRANSPARENT,
            animate_in: false,
            animate_out: false,
            auto_dismiss: None,
        }
    }

    /// Create configuration for a dropdown menu.
    pub fn dropdown(anchor: NodeId) -> Self {
        Self {
            layer: ZLayer::Dropdown,
            position: OverlayPosition::AnchorTo {
                anchor_node: anchor,
                alignment: AnchorAlignment::BelowLeft,
                offset: Vec2::ZERO,
            },
            close_on_outside_click: true,
            close_on_escape: true,
            trap_focus: true,
            show_backdrop: false,
            backdrop_color: Color::TRANSPARENT,
            animate_in: false,
            animate_out: false,
            auto_dismiss: None,
        }
    }

    /// Create configuration for a modal dialog.
    pub fn modal() -> Self {
        Self {
            layer: ZLayer::Modal,
            position: OverlayPosition::Center,
            close_on_outside_click: false,
            close_on_escape: true,
            trap_focus: true,
            show_backdrop: true,
            backdrop_color: Color::rgba(0.0, 0.0, 0.0, 0.5),
            animate_in: true,
            animate_out: true,
            auto_dismiss: None,
        }
    }

    /// Create configuration for a context menu.
    pub fn context_menu() -> Self {
        Self {
            layer: ZLayer::Popover,
            position: OverlayPosition::AtCursor { offset: Vec2::ZERO },
            close_on_outside_click: true,
            close_on_escape: true,
            trap_focus: true,
            show_backdrop: false,
            backdrop_color: Color::TRANSPARENT,
            animate_in: false,
            animate_out: false,
            auto_dismiss: None,
        }
    }
}

/// State of an active overlay.
#[derive(Debug)]
pub struct Overlay {
    /// Unique identifier.
    pub id: OverlayId,
    /// Root node of the overlay content.
    pub root_node: NodeId,
    /// Configuration.
    pub config: OverlayConfig,
    /// Computed absolute position.
    pub computed_position: Vec2,
    /// Computed size (from layout).
    pub computed_size: Vec2,
    /// Timestamp when overlay was shown.
    pub shown_at: std::time::Instant,
    /// Whether overlay is currently visible.
    pub visible: bool,
    /// Parent overlay (for nested menus).
    pub parent_overlay: Option<OverlayId>,
    /// Child overlays (for nested menus).
    pub child_overlays: Vec<OverlayId>,
}

impl Overlay {
    /// Create a new overlay.
    pub fn new(root_node: NodeId, config: OverlayConfig) -> Self {
        Self {
            id: OverlayId::new(),
            root_node,
            config,
            computed_position: Vec2::ZERO,
            computed_size: Vec2::ZERO,
            shown_at: std::time::Instant::now(),
            visible: true,
            parent_overlay: None,
            child_overlays: Vec::new(),
        }
    }

    /// Check if the overlay should auto-dismiss.
    pub fn should_auto_dismiss(&self) -> bool {
        if let Some(duration) = self.config.auto_dismiss {
            self.shown_at.elapsed() >= duration
        } else {
            false
        }
    }

    /// Check if a point is inside this overlay.
    pub fn contains_point(&self, point: Vec2) -> bool {
        point.x >= self.computed_position.x
            && point.x <= self.computed_position.x + self.computed_size.x
            && point.y >= self.computed_position.y
            && point.y <= self.computed_position.y + self.computed_size.y
    }
}

/// Overlay manager for handling all active overlays.
pub struct OverlayManager {
    /// All active overlays by ID.
    overlays: HashMap<OverlayId, Overlay>,
    /// Overlays organized by Z-layer.
    layer_stacks: BTreeMap<u16, Vec<OverlayId>>,
    /// Currently focused overlay.
    focused_overlay: Option<OverlayId>,
    /// Viewport size for positioning calculations.
    viewport_size: Vec2,
    /// Current mouse position.
    mouse_position: Vec2,
    /// Overlays to close this frame (deferred).
    pending_close: Vec<OverlayId>,
    /// Event that occurred (for external handling).
    last_event: Option<OverlayEvent>,
}

/// Events generated by the overlay system.
#[derive(Debug, Clone)]
pub enum OverlayEvent {
    /// Overlay was shown.
    Shown(OverlayId),
    /// Overlay was hidden.
    Hidden(OverlayId),
    /// Overlay was focused.
    Focused(OverlayId),
    /// Click outside all overlays.
    ClickedOutside,
    /// Escape pressed with overlay focused.
    EscapePressed(OverlayId),
}

impl OverlayManager {
    /// Create a new overlay manager.
    pub fn new() -> Self {
        Self {
            overlays: HashMap::new(),
            layer_stacks: BTreeMap::new(),
            focused_overlay: None,
            viewport_size: Vec2::new(800.0, 600.0),
            mouse_position: Vec2::ZERO,
            pending_close: Vec::new(),
            last_event: None,
        }
    }

    /// Set the viewport size for positioning calculations.
    pub fn set_viewport_size(&mut self, size: Vec2) {
        self.viewport_size = size;
    }

    /// Update mouse position (for cursor-relative positioning).
    pub fn set_mouse_position(&mut self, pos: Vec2) {
        self.mouse_position = pos;
    }

    /// Show an overlay with the given configuration.
    ///
    /// Returns the overlay ID for later reference.
    pub fn show(
        &mut self,
        tree: &mut UiTree,
        root_node: NodeId,
        config: OverlayConfig,
    ) -> OverlayId {
        let mut overlay = Overlay::new(root_node, config);
        let id = overlay.id;

        // Calculate initial position
        self.compute_position(&mut overlay, tree);

        // Add to layer stack
        let z_index = overlay.config.layer.z_index();
        self.layer_stacks.entry(z_index).or_default().push(id);

        // Set as focused if it traps focus
        if overlay.config.trap_focus {
            self.focused_overlay = Some(id);
        }

        self.overlays.insert(id, overlay);
        self.last_event = Some(OverlayEvent::Shown(id));

        id
    }

    /// Show an overlay as a child of another (for nested menus).
    pub fn show_child(
        &mut self,
        tree: &mut UiTree,
        parent_id: OverlayId,
        root_node: NodeId,
        config: OverlayConfig,
    ) -> Option<OverlayId> {
        let child_id = self.show(tree, root_node, config);

        // Link parent and child
        if let Some(child) = self.overlays.get_mut(&child_id) {
            child.parent_overlay = Some(parent_id);
        }
        if let Some(parent) = self.overlays.get_mut(&parent_id) {
            parent.child_overlays.push(child_id);
        }

        Some(child_id)
    }

    /// Hide an overlay by ID.
    pub fn hide(&mut self, tree: &mut UiTree, id: OverlayId) {
        // First, recursively hide all children
        if let Some(overlay) = self.overlays.get(&id) {
            let children: Vec<OverlayId> = overlay.child_overlays.clone();
            for child_id in children {
                self.hide(tree, child_id);
            }
        }

        // Remove from layer stack
        if let Some(overlay) = self.overlays.get(&id) {
            let z_index = overlay.config.layer.z_index();
            if let Some(stack) = self.layer_stacks.get_mut(&z_index) {
                stack.retain(|&oid| oid != id);
            }
        }

        // Remove from parent's child list
        if let Some(overlay) = self.overlays.get(&id)
            && let Some(parent_id) = overlay.parent_overlay
            && let Some(parent) = self.overlays.get_mut(&parent_id)
        {
            parent.child_overlays.retain(|&cid| cid != id);
        }

        // Update focused overlay
        if self.focused_overlay == Some(id) {
            self.focused_overlay = self.find_next_focusable();
        }

        self.overlays.remove(&id);
        self.last_event = Some(OverlayEvent::Hidden(id));

        // Mark tree dirty if needed
        let _ = tree; // Would update tree state here
    }

    /// Hide all overlays in a specific layer.
    pub fn hide_layer(&mut self, tree: &mut UiTree, layer: ZLayer) {
        let z_index = layer.z_index();
        if let Some(stack) = self.layer_stacks.get(&z_index).cloned() {
            for id in stack {
                self.hide(tree, id);
            }
        }
    }

    /// Hide all overlays.
    pub fn hide_all(&mut self, tree: &mut UiTree) {
        let all_ids: Vec<OverlayId> = self.overlays.keys().copied().collect();
        for id in all_ids {
            self.hide(tree, id);
        }
    }

    /// Get an overlay by ID.
    pub fn get(&self, id: OverlayId) -> Option<&Overlay> {
        self.overlays.get(&id)
    }

    /// Get mutable overlay by ID.
    pub fn get_mut(&mut self, id: OverlayId) -> Option<&mut Overlay> {
        self.overlays.get_mut(&id)
    }

    /// Check if any overlays are visible.
    pub fn has_overlays(&self) -> bool {
        !self.overlays.is_empty()
    }

    /// Get the topmost overlay at a screen position.
    pub fn hit_test(&self, pos: Vec2) -> Option<OverlayId> {
        // Check from highest to lowest layer
        for (_z, stack) in self.layer_stacks.iter().rev() {
            // Check from top of stack to bottom
            for &id in stack.iter().rev() {
                if let Some(overlay) = self.overlays.get(&id)
                    && overlay.visible
                    && overlay.contains_point(pos)
                {
                    return Some(id);
                }
            }
        }
        None
    }

    /// Handle a click event.
    ///
    /// Returns true if the click was handled by an overlay.
    pub fn handle_click(&mut self, _tree: &mut UiTree, pos: Vec2) -> bool {
        // Check if click hit any overlay
        if let Some(hit_id) = self.hit_test(pos) {
            // Focus the clicked overlay
            self.focused_overlay = Some(hit_id);
            self.last_event = Some(OverlayEvent::Focused(hit_id));
            return true;
        }

        // Click was outside all overlays
        self.last_event = Some(OverlayEvent::ClickedOutside);

        // Close overlays that should close on outside click
        let to_close: Vec<OverlayId> = self
            .overlays
            .iter()
            .filter(|(_, o)| o.config.close_on_outside_click && o.visible)
            .map(|(&id, _)| id)
            .collect();

        for id in to_close {
            self.pending_close.push(id);
        }

        !self.pending_close.is_empty()
    }

    /// Handle escape key press.
    ///
    /// Returns true if an overlay was closed.
    pub fn handle_escape(&mut self, tree: &mut UiTree) -> bool {
        // Find topmost overlay that closes on escape
        let to_close = self.find_topmost_escapable();

        if let Some(id) = to_close {
            self.last_event = Some(OverlayEvent::EscapePressed(id));
            self.hide(tree, id);
            return true;
        }

        false
    }

    /// Process pending operations (call at end of frame).
    pub fn flush(&mut self, tree: &mut UiTree) {
        // Process pending closes
        let to_close: Vec<OverlayId> = self.pending_close.drain(..).collect();
        for id in to_close {
            self.hide(tree, id);
        }

        // Check for auto-dismiss
        let auto_dismiss: Vec<OverlayId> = self
            .overlays
            .iter()
            .filter(|(_, o)| o.should_auto_dismiss())
            .map(|(&id, _)| id)
            .collect();

        for id in auto_dismiss {
            self.hide(tree, id);
        }
    }

    /// Update overlay positions (call when viewport or anchors change).
    pub fn update_positions(&mut self, tree: &UiTree) {
        let viewport_size = self.viewport_size;
        let mouse_position = self.mouse_position;

        for overlay in self.overlays.values_mut() {
            // Get overlay size from layout
            if let Some(layout) = tree.get_layout(overlay.root_node) {
                overlay.computed_size = Vec2::new(layout.width, layout.height);
            }

            let size = overlay.computed_size;

            let position = match &overlay.config.position {
                OverlayPosition::Absolute { x, y } => Vec2::new(*x, *y),
                OverlayPosition::Center => Vec2::new(
                    (viewport_size.x - size.x) / 2.0,
                    (viewport_size.y - size.y) / 2.0,
                ),
                OverlayPosition::AtCursor { offset } => mouse_position + *offset,
                OverlayPosition::AnchorTo {
                    anchor_node,
                    alignment,
                    offset,
                } => {
                    if let Some(anchor_layout) = tree.get_layout(*anchor_node) {
                        let anchor_pos = Vec2::new(anchor_layout.x, anchor_layout.y);
                        let anchor_size = Vec2::new(anchor_layout.width, anchor_layout.height);
                        alignment.compute_position(anchor_pos, anchor_size, size, viewport_size)
                            + *offset
                    } else {
                        Vec2::ZERO
                    }
                }
                OverlayPosition::Viewport {
                    horizontal,
                    vertical,
                    margin,
                } => {
                    let x = match horizontal {
                        HorizontalAlign::Left => *margin,
                        HorizontalAlign::Center => (viewport_size.x - size.x) / 2.0,
                        HorizontalAlign::Right => viewport_size.x - size.x - *margin,
                    };
                    let y = match vertical {
                        VerticalAlign::Top => *margin,
                        VerticalAlign::Center => (viewport_size.y - size.y) / 2.0,
                        VerticalAlign::Bottom => viewport_size.y - size.y - *margin,
                    };
                    Vec2::new(x, y)
                }
            };

            overlay.computed_position = position;
        }
    }

    /// Get the currently focused overlay.
    pub fn focused(&self) -> Option<OverlayId> {
        self.focused_overlay
    }

    /// Get last event that occurred.
    pub fn last_event(&self) -> Option<&OverlayEvent> {
        self.last_event.as_ref()
    }

    /// Clear last event.
    pub fn clear_event(&mut self) {
        self.last_event = None;
    }

    /// Get all visible overlays in render order (back to front).
    pub fn visible_overlays(&self) -> Vec<&Overlay> {
        let mut result = Vec::new();

        for stack in self.layer_stacks.values() {
            for &id in stack {
                if let Some(overlay) = self.overlays.get(&id)
                    && overlay.visible
                {
                    result.push(overlay);
                }
            }
        }

        result
    }

    /// Get backdrop quads for overlays that need them.
    pub fn backdrop_quads(&self) -> Vec<(Vec2, Vec2, Color)> {
        let mut backdrops = Vec::new();

        for overlay in self.overlays.values() {
            if overlay.visible && overlay.config.show_backdrop {
                backdrops.push((
                    Vec2::ZERO,
                    self.viewport_size,
                    overlay.config.backdrop_color,
                ));
            }
        }

        backdrops
    }

    // --- Private helper methods ---

    fn compute_position(&mut self, overlay: &mut Overlay, tree: &UiTree) {
        self.compute_position_internal(overlay, tree, self.viewport_size, self.mouse_position);
    }

    fn compute_position_internal(
        &self,
        overlay: &mut Overlay,
        tree: &UiTree,
        viewport: Vec2,
        mouse: Vec2,
    ) {
        // Get overlay size from layout
        if let Some(layout) = tree.get_layout(overlay.root_node) {
            overlay.computed_size = Vec2::new(layout.width, layout.height);
        }

        let size = overlay.computed_size;

        let position = match &overlay.config.position {
            OverlayPosition::Absolute { x, y } => Vec2::new(*x, *y),

            OverlayPosition::Center => {
                Vec2::new((viewport.x - size.x) / 2.0, (viewport.y - size.y) / 2.0)
            }

            OverlayPosition::AtCursor { offset } => mouse + *offset,

            OverlayPosition::AnchorTo {
                anchor_node,
                alignment,
                offset,
            } => {
                if let Some(anchor_layout) = tree.get_layout(*anchor_node) {
                    // Calculate absolute anchor position
                    let mut anchor_x = anchor_layout.x;
                    let mut anchor_y = anchor_layout.y;

                    // Walk up tree to get absolute position
                    let mut current = tree.get_node(*anchor_node).and_then(|n| n.parent);
                    while let Some(parent_id) = current {
                        if let Some(parent_layout) = tree.get_layout(parent_id) {
                            anchor_x += parent_layout.x;
                            anchor_y += parent_layout.y;
                        }
                        current = tree.get_node(parent_id).and_then(|n| n.parent);
                    }

                    let anchor_w = anchor_layout.width;
                    let anchor_h = anchor_layout.height;

                    let pos = match alignment {
                        AnchorAlignment::BelowLeft => Vec2::new(anchor_x, anchor_y + anchor_h),
                        AnchorAlignment::BelowCenter => {
                            Vec2::new(anchor_x + (anchor_w - size.x) / 2.0, anchor_y + anchor_h)
                        }
                        AnchorAlignment::BelowRight => {
                            Vec2::new(anchor_x + anchor_w - size.x, anchor_y + anchor_h)
                        }
                        AnchorAlignment::AboveLeft => Vec2::new(anchor_x, anchor_y - size.y),
                        AnchorAlignment::AboveCenter => {
                            Vec2::new(anchor_x + (anchor_w - size.x) / 2.0, anchor_y - size.y)
                        }
                        AnchorAlignment::AboveRight => {
                            Vec2::new(anchor_x + anchor_w - size.x, anchor_y - size.y)
                        }
                        AnchorAlignment::RightTop => Vec2::new(anchor_x + anchor_w, anchor_y),
                        AnchorAlignment::RightCenter => {
                            Vec2::new(anchor_x + anchor_w, anchor_y + (anchor_h - size.y) / 2.0)
                        }
                        AnchorAlignment::RightBottom => {
                            Vec2::new(anchor_x + anchor_w, anchor_y + anchor_h - size.y)
                        }
                        AnchorAlignment::LeftTop => Vec2::new(anchor_x - size.x, anchor_y),
                        AnchorAlignment::LeftCenter => {
                            Vec2::new(anchor_x - size.x, anchor_y + (anchor_h - size.y) / 2.0)
                        }
                        AnchorAlignment::LeftBottom => {
                            Vec2::new(anchor_x - size.x, anchor_y + anchor_h - size.y)
                        }
                    };

                    pos + *offset
                } else {
                    Vec2::ZERO
                }
            }

            OverlayPosition::Viewport {
                horizontal,
                vertical,
                margin,
            } => {
                let x = match horizontal {
                    HorizontalAlign::Left => *margin,
                    HorizontalAlign::Center => (viewport.x - size.x) / 2.0,
                    HorizontalAlign::Right => viewport.x - size.x - margin,
                };
                let y = match vertical {
                    VerticalAlign::Top => *margin,
                    VerticalAlign::Center => (viewport.y - size.y) / 2.0,
                    VerticalAlign::Bottom => viewport.y - size.y - margin,
                };
                Vec2::new(x, y)
            }
        };

        // Clamp to viewport
        overlay.computed_position = Vec2::new(
            position.x.max(0.0).min(viewport.x - size.x),
            position.y.max(0.0).min(viewport.y - size.y),
        );
    }

    fn find_next_focusable(&self) -> Option<OverlayId> {
        // Find the topmost overlay that traps focus
        for (_z, stack) in self.layer_stacks.iter().rev() {
            for &id in stack.iter().rev() {
                if let Some(overlay) = self.overlays.get(&id)
                    && overlay.visible
                    && overlay.config.trap_focus
                {
                    return Some(id);
                }
            }
        }
        None
    }

    fn find_topmost_escapable(&self) -> Option<OverlayId> {
        for (_z, stack) in self.layer_stacks.iter().rev() {
            for &id in stack.iter().rev() {
                if let Some(overlay) = self.overlays.get(&id)
                    && overlay.visible
                    && overlay.config.close_on_escape
                {
                    return Some(id);
                }
            }
        }
        None
    }
}

impl Default for OverlayManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlay_id_uniqueness() {
        let id1 = OverlayId::new();
        let id2 = OverlayId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_z_layer_ordering() {
        assert!(ZLayer::Tooltip.z_index() > ZLayer::Base.z_index());
        assert!(ZLayer::Dropdown.z_index() > ZLayer::Tooltip.z_index());
        assert!(ZLayer::Modal.z_index() > ZLayer::Dropdown.z_index());
        assert!(ZLayer::Popover.z_index() > ZLayer::Modal.z_index());
    }

    #[test]
    fn test_z_layer_custom() {
        let custom = ZLayer::custom(1500);
        assert_eq!(custom.z_index(), 1500);
        assert!(custom.is_above(&ZLayer::Tooltip));
        assert!(!custom.is_above(&ZLayer::Dropdown));
    }

    #[test]
    fn test_overlay_contains_point() {
        let mut overlay = Overlay::new(NodeId(1), OverlayConfig::default());
        overlay.computed_position = Vec2::new(100.0, 100.0);
        overlay.computed_size = Vec2::new(200.0, 150.0);

        assert!(overlay.contains_point(Vec2::new(150.0, 150.0)));
        assert!(overlay.contains_point(Vec2::new(100.0, 100.0)));
        assert!(overlay.contains_point(Vec2::new(299.0, 249.0)));
        assert!(!overlay.contains_point(Vec2::new(50.0, 50.0)));
        assert!(!overlay.contains_point(Vec2::new(350.0, 150.0)));
    }

    #[test]
    fn test_overlay_config_presets() {
        let tooltip = OverlayConfig::tooltip();
        assert_eq!(tooltip.layer, ZLayer::Tooltip);
        assert!(!tooltip.close_on_outside_click);
        assert!(!tooltip.trap_focus);

        let modal = OverlayConfig::modal();
        assert_eq!(modal.layer, ZLayer::Modal);
        assert!(!modal.close_on_outside_click);
        assert!(modal.trap_focus);
        assert!(modal.show_backdrop);

        let context = OverlayConfig::context_menu();
        assert_eq!(context.layer, ZLayer::Popover);
        assert!(context.close_on_outside_click);
        assert!(context.close_on_escape);
    }

    #[test]
    fn test_overlay_manager_basic() {
        let mut manager = OverlayManager::new();
        assert!(!manager.has_overlays());

        let mut tree = UiTree::new();
        let node_id = tree.add_widget(Box::new(crate::widgets::Container::new()));
        tree.set_root(node_id);

        let overlay_id = manager.show(&mut tree, node_id, OverlayConfig::default());
        assert!(manager.has_overlays());
        assert!(manager.get(overlay_id).is_some());

        manager.hide(&mut tree, overlay_id);
        assert!(!manager.has_overlays());
    }

    #[test]
    fn test_overlay_manager_layers() {
        let mut manager = OverlayManager::new();
        let mut tree = UiTree::new();

        let node1 = tree.add_widget(Box::new(crate::widgets::Container::new()));
        let node2 = tree.add_widget(Box::new(crate::widgets::Container::new()));

        let id1 = manager.show(
            &mut tree,
            node1,
            OverlayConfig {
                layer: ZLayer::Modal,
                ..Default::default()
            },
        );

        let id2 = manager.show(
            &mut tree,
            node2,
            OverlayConfig {
                layer: ZLayer::Popover,
                ..Default::default()
            },
        );

        // Popover should be on top
        let visible = manager.visible_overlays();
        assert_eq!(visible.len(), 2);
        assert_eq!(visible[0].id, id1); // Modal first
        assert_eq!(visible[1].id, id2); // Popover second (on top)

        manager.hide_layer(&mut tree, ZLayer::Popover);
        assert!(manager.get(id2).is_none());
        assert!(manager.get(id1).is_some());
    }

    #[test]
    fn test_overlay_hit_test() {
        let mut manager = OverlayManager::new();
        let mut tree = UiTree::new();

        let node1 = tree.add_widget(Box::new(crate::widgets::Container::new()));
        let node2 = tree.add_widget(Box::new(crate::widgets::Container::new()));

        let id1 = manager.show(&mut tree, node1, OverlayConfig::default());
        let id2 = manager.show(&mut tree, node2, OverlayConfig::default());

        // Set positions manually for testing
        manager.get_mut(id1).unwrap().computed_position = Vec2::new(0.0, 0.0);
        manager.get_mut(id1).unwrap().computed_size = Vec2::new(100.0, 100.0);
        manager.get_mut(id2).unwrap().computed_position = Vec2::new(50.0, 50.0);
        manager.get_mut(id2).unwrap().computed_size = Vec2::new(100.0, 100.0);

        // Point in both overlays - should hit id2 (higher in stack)
        let hit = manager.hit_test(Vec2::new(75.0, 75.0));
        assert_eq!(hit, Some(id2));

        // Point only in id1
        let hit = manager.hit_test(Vec2::new(25.0, 25.0));
        assert_eq!(hit, Some(id1));

        // Point outside both
        let hit = manager.hit_test(Vec2::new(200.0, 200.0));
        assert_eq!(hit, None);
    }

    #[test]
    fn test_z_layer_comparisons() {
        assert!(ZLayer::Popover.is_above(&ZLayer::Modal));
        assert!(ZLayer::Modal.is_above(&ZLayer::Dropdown));
        assert!(ZLayer::Dropdown.is_above(&ZLayer::Tooltip));
        assert!(ZLayer::Tooltip.is_above(&ZLayer::Base));

        assert!(!ZLayer::Base.is_above(&ZLayer::Tooltip));
    }

    #[test]
    fn test_overlay_position_variants() {
        let absolute = OverlayPosition::Absolute { x: 100.0, y: 200.0 };
        assert!(matches!(absolute, OverlayPosition::Absolute { .. }));

        let anchored = OverlayPosition::AnchorTo {
            anchor_node: NodeId(1),
            alignment: AnchorAlignment::BelowLeft,
            offset: Vec2::ZERO,
        };
        assert!(matches!(anchored, OverlayPosition::AnchorTo { .. }));

        let centered = OverlayPosition::Center;
        assert!(matches!(centered, OverlayPosition::Center));

        let at_cursor = OverlayPosition::AtCursor { offset: Vec2::ZERO };
        assert!(matches!(at_cursor, OverlayPosition::AtCursor { .. }));
    }

    #[test]
    fn test_anchor_alignment_variants() {
        let alignments = [
            AnchorAlignment::BelowLeft,
            AnchorAlignment::BelowCenter,
            AnchorAlignment::BelowRight,
            AnchorAlignment::AboveLeft,
            AnchorAlignment::AboveCenter,
            AnchorAlignment::AboveRight,
            AnchorAlignment::LeftCenter,
            AnchorAlignment::RightCenter,
        ];

        // Each alignment should compute a position
        let anchor_pos = Vec2::new(100.0, 100.0);
        let anchor_size = Vec2::new(200.0, 50.0);
        let overlay_size = Vec2::new(80.0, 40.0);
        let viewport = Vec2::new(800.0, 600.0);

        for alignment in &alignments {
            let pos = alignment.compute_position(anchor_pos, anchor_size, overlay_size, viewport);
            // Position should be a valid coordinate
            assert!(pos.x.is_finite());
            assert!(pos.y.is_finite());
        }
    }

    #[test]
    fn test_anchor_alignment_positioning() {
        let anchor_pos = Vec2::new(100.0, 100.0);
        let anchor_size = Vec2::new(200.0, 50.0);
        let overlay_size = Vec2::new(80.0, 40.0);
        let viewport = Vec2::new(800.0, 600.0);

        // BelowLeft should position below the anchor, aligned left
        let pos = AnchorAlignment::BelowLeft.compute_position(
            anchor_pos,
            anchor_size,
            overlay_size,
            viewport,
        );
        assert_eq!(pos.x, 100.0); // Same x as anchor
        assert_eq!(pos.y, 150.0); // Below anchor (100 + 50)

        // AboveRight should position above the anchor, aligned right
        let pos = AnchorAlignment::AboveRight.compute_position(
            anchor_pos,
            anchor_size,
            overlay_size,
            viewport,
        );
        assert_eq!(pos.x, 220.0); // Anchor right (100 + 200) - overlay width (80)
        assert_eq!(pos.y, 60.0); // Above anchor (100 - 40)
    }

    #[test]
    fn test_anchor_alignment_center_positioning() {
        let anchor_pos = Vec2::new(100.0, 100.0);
        let anchor_size = Vec2::new(200.0, 50.0);
        let overlay_size = Vec2::new(80.0, 40.0);
        let viewport = Vec2::new(800.0, 600.0);

        // BelowCenter should position below, centered horizontally
        let pos = AnchorAlignment::BelowCenter.compute_position(
            anchor_pos,
            anchor_size,
            overlay_size,
            viewport,
        );
        // Center x = 100 + (200 - 80) / 2 = 100 + 60 = 160
        assert_eq!(pos.x, 160.0);
        assert_eq!(pos.y, 150.0); // Below anchor
    }

    #[test]
    fn test_overlay_hide_all() {
        let mut manager = OverlayManager::new();
        let mut tree = UiTree::new();

        let node1 = tree.add_widget(Box::new(crate::widgets::Container::new()));
        let node2 = tree.add_widget(Box::new(crate::widgets::Container::new()));
        let node3 = tree.add_widget(Box::new(crate::widgets::Container::new()));

        manager.show(&mut tree, node1, OverlayConfig::default());
        manager.show(&mut tree, node2, OverlayConfig::modal());
        manager.show(&mut tree, node3, OverlayConfig::tooltip());

        assert_eq!(manager.visible_overlays().len(), 3);

        manager.hide_all(&mut tree);
        assert!(!manager.has_overlays());
    }

    #[test]
    fn test_overlay_get_mut() {
        let mut manager = OverlayManager::new();
        let mut tree = UiTree::new();

        let node = tree.add_widget(Box::new(crate::widgets::Container::new()));
        let id = manager.show(&mut tree, node, OverlayConfig::default());

        // Modify through get_mut
        if let Some(overlay) = manager.get_mut(id) {
            overlay.computed_position = Vec2::new(500.0, 300.0);
        }

        // Verify change persisted
        assert_eq!(
            manager.get(id).unwrap().computed_position,
            Vec2::new(500.0, 300.0)
        );
    }

    #[test]
    fn test_overlay_config_default() {
        let config = OverlayConfig::default();
        // Default layer is Popover
        assert_eq!(config.layer, ZLayer::Popover);
        assert!(!config.trap_focus);
        assert!(!config.show_backdrop);
    }

    #[test]
    fn test_overlay_dropdown_preset() {
        let config = OverlayConfig::dropdown(NodeId(1));
        assert_eq!(config.layer, ZLayer::Dropdown);
        assert!(config.close_on_outside_click);
    }

    #[test]
    fn test_overlay_visible_sorted_by_layer() {
        let mut manager = OverlayManager::new();
        let mut tree = UiTree::new();

        let node1 = tree.add_widget(Box::new(crate::widgets::Container::new()));
        let node2 = tree.add_widget(Box::new(crate::widgets::Container::new()));
        let node3 = tree.add_widget(Box::new(crate::widgets::Container::new()));

        // Add in reverse order of z-index
        let id_popover = manager.show(
            &mut tree,
            node1,
            OverlayConfig {
                layer: ZLayer::Popover,
                ..Default::default()
            },
        );
        let id_tooltip = manager.show(
            &mut tree,
            node2,
            OverlayConfig {
                layer: ZLayer::Tooltip,
                ..Default::default()
            },
        );
        let id_modal = manager.show(
            &mut tree,
            node3,
            OverlayConfig {
                layer: ZLayer::Modal,
                ..Default::default()
            },
        );

        let visible = manager.visible_overlays();
        // Should be sorted by z-index: tooltip < modal < popover
        assert_eq!(visible[0].id, id_tooltip);
        assert_eq!(visible[1].id, id_modal);
        assert_eq!(visible[2].id, id_popover);
    }

    #[test]
    fn test_overlay_custom_z_layer() {
        let custom_layer = ZLayer::custom(1500);
        let config = OverlayConfig {
            layer: custom_layer,
            ..Default::default()
        };

        assert_eq!(config.layer.z_index(), 1500);
    }
}
