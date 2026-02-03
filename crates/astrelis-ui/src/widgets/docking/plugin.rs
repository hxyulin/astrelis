//! Docking plugin providing DockSplitter and DockTabs widget types.
//!
//! This plugin registers render, traversal, and overflow handlers for
//! the docking widgets. It also owns cross-widget docking state
//! (drag management, drop zone detection, animations).

use crate::clip::ClipRect;
use crate::widgets::docking::tabs::{CHAR_WIDTH_FACTOR, CLOSE_BUTTON_MARGIN};
use crate::widgets::docking::{
    DockAnimationState, DockSplitter, DockTabs, DockingContext, DragManager, DropZoneDetector,
    DEFAULT_CLOSE_BUTTON_SIZE, DEFAULT_TAB_PADDING,
};
use crate::draw_list::{DrawCommand, QuadCommand, TextCommand};
use crate::plugin::registry::{
    TraversalBehavior, WidgetOverflow, WidgetRenderContext, WidgetTypeDescriptor,
    WidgetTypeRegistry,
};
use crate::plugin::UiPlugin;
use crate::style::Overflow;
use crate::tree::{NodeId, UiTree};
use astrelis_core::math::Vec2;
use astrelis_render::Color;
use std::any::Any;

// ---------------------------------------------------------------------------
// DockingPlugin
// ---------------------------------------------------------------------------

/// Plugin providing docking widget types and cross-widget docking state.
///
/// Owns drag management, drop zone detection, animations, and the
/// docking context (container registry cache).
pub struct DockingPlugin {
    /// Manages drag state for splitter resizing and tab operations.
    pub drag_manager: DragManager,
    /// Tracks which splitter separator is under the mouse.
    pub hovered_splitter: Option<NodeId>,
    /// Detects drop zones for tab drag operations.
    pub drop_zone_detector: DropZoneDetector,
    /// Active cross-container drop preview state.
    pub cross_container_preview: Option<CrossContainerPreview>,
    /// Container registry cache for efficient drag lookups.
    pub docking_context: DockingContext,
    /// Animation state for ghost tabs, groups, and drop previews.
    pub dock_animations: DockAnimationState,
    /// DockTabs node whose scrollbar thumb is being dragged.
    pub scrollbar_drag_node: Option<NodeId>,
}

/// State for a cross-container drop preview.
#[derive(Debug, Clone, Copy)]
pub struct CrossContainerPreview {
    /// The target DockTabs container.
    pub target_node: NodeId,
    /// Absolute layout of the target container.
    pub target_layout: crate::tree::LayoutRect,
    /// The detected drop zone within the target.
    pub zone: crate::widgets::docking::DockZone,
    /// Preview bounds (where the tab will be inserted).
    pub preview_bounds: crate::tree::LayoutRect,
    /// Insertion index for center zone (tab bar position).
    pub insert_index: Option<usize>,
}

impl DockingPlugin {
    /// Create a new docking plugin with default state.
    pub fn new() -> Self {
        Self {
            drag_manager: DragManager::new(),
            hovered_splitter: None,
            drop_zone_detector: DropZoneDetector::new(),
            cross_container_preview: None,
            docking_context: DockingContext::new(),
            dock_animations: DockAnimationState::new(),
            scrollbar_drag_node: None,
        }
    }

    /// Invalidate the docking container cache.
    pub fn invalidate_cache(&mut self) {
        self.docking_context.invalidate();
    }

    /// Update all docking animations with the given delta time.
    /// Returns `true` if any animation is still active.
    pub fn update_animations(&mut self, dt: f32) -> bool {
        self.dock_animations.update(dt)
    }

    /// Check if there is an active drag operation.
    pub fn is_dragging(&self) -> bool {
        self.drag_manager.is_dragging()
    }

    /// Invalidate any references to nodes that no longer exist.
    pub fn invalidate_removed_nodes(&mut self, tree: &UiTree) {
        if let Some(id) = self.hovered_splitter
            && !tree.node_exists(id)
        {
            self.hovered_splitter = None;
        }
        if let Some(ref p) = self.cross_container_preview
            && !tree.node_exists(p.target_node)
        {
            self.cross_container_preview = None;
        }
        if let Some(id) = self.scrollbar_drag_node
            && !tree.node_exists(id)
        {
            self.scrollbar_drag_node = None;
        }
    }
}

impl Default for DockingPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl UiPlugin for DockingPlugin {
    fn name(&self) -> &str {
        "docking"
    }

    fn register_widgets(&self, registry: &mut WidgetTypeRegistry) {
        registry.register::<DockSplitter>(
            WidgetTypeDescriptor::new("DockSplitter").with_render(render_dock_splitter),
        );
        registry.register::<DockTabs>(
            WidgetTypeDescriptor::new("DockTabs")
                .with_render(render_dock_tabs)
                .with_traversal(dock_tabs_traversal)
                .with_overflow(dock_tabs_overflow),
        );
    }

    fn post_layout(&mut self, _tree: &mut UiTree) {
        // Docking post-layout processing (splitter ratios, tab sizing)
        // is currently handled in tree.rs post_process_docking_layouts.
        // Will be migrated in a future phase.
    }

    fn update(&mut self, dt: f32, _tree: &mut UiTree) {
        self.update_animations(dt);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ---------------------------------------------------------------------------
// Render: DockSplitter
// ---------------------------------------------------------------------------

pub fn render_dock_splitter(
    widget: &dyn Any,
    ctx: &mut WidgetRenderContext<'_>,
) -> Vec<DrawCommand> {
    let splitter = widget.downcast_ref::<DockSplitter>().unwrap();
    let mut commands = Vec::new();

    let sep_bounds = splitter.separator_bounds(&crate::tree::LayoutRect {
        x: ctx.abs_position.x,
        y: ctx.abs_position.y,
        width: ctx.layout_size.x,
        height: ctx.layout_size.y,
    });

    let sep_color = splitter.current_separator_color();

    commands.push(DrawCommand::Quad(
        QuadCommand::filled(
            Vec2::new(sep_bounds.x, sep_bounds.y),
            Vec2::new(sep_bounds.width, sep_bounds.height),
            sep_color,
            0,
        )
        .with_clip(ctx.clip_rect),
    ));

    commands
}

// ---------------------------------------------------------------------------
// Render: DockTabs
// ---------------------------------------------------------------------------

pub fn render_dock_tabs(widget: &dyn Any, ctx: &mut WidgetRenderContext<'_>) -> Vec<DrawCommand> {
    let tabs = widget.downcast_ref::<DockTabs>().unwrap();
    let mut commands = Vec::new();

    let abs_layout = crate::tree::LayoutRect {
        x: ctx.abs_position.x,
        y: ctx.abs_position.y,
        width: ctx.layout_size.x,
        height: ctx.layout_size.y,
    };

    // Tab bar background
    let bar_bounds = tabs.tab_bar_bounds(&abs_layout);
    commands.push(DrawCommand::Quad(
        QuadCommand::filled(
            Vec2::new(bar_bounds.x, bar_bounds.y),
            Vec2::new(bar_bounds.width, bar_bounds.height),
            tabs.theme.tab_bar_color,
            0,
        )
        .with_clip(ctx.clip_rect),
    ));

    // Compute clip rect for the tab row area (excludes scrollbar strip).
    let tab_row = tabs.tab_row_bounds(&abs_layout);
    let tab_row_clip = ClipRect::from_bounds(tab_row.x, tab_row.y, tab_row.width, tab_row.height);
    let tab_clip = ctx.clip_rect.intersect(&tab_row_clip);

    // Render individual tabs
    for i in 0..tabs.tab_count() {
        if let Some(tab_rect) = tabs.tab_bounds(i, &abs_layout) {
            // Skip tabs entirely outside the visible tab row area
            let tab_right = tab_rect.x + tab_rect.width;
            let bar_right = tab_row.x + tab_row.width;
            if tab_right < tab_row.x || tab_rect.x > bar_right {
                continue;
            }

            let tab_color = tabs.tab_background_color(i);

            // Tab background
            commands.push(DrawCommand::Quad(
                QuadCommand::rounded(
                    Vec2::new(tab_rect.x, tab_rect.y),
                    Vec2::new(tab_rect.width, tab_rect.height),
                    tab_color,
                    4.0,
                    0,
                )
                .with_clip(tab_clip),
            ));

            // Tab label text
            if let Some(label) = tabs.tab_label(i) {
                let request_id = ctx.text_pipeline.request_shape(
                    label.to_string(),
                    0,
                    tabs.theme.tab_font_size,
                    None,
                );

                if let Some(shaped) = ctx.text_pipeline.get_completed(request_id) {
                    let text_height = shaped.bounds().1;
                    let text_x = tab_rect.x + DEFAULT_TAB_PADDING;
                    let text_y = tab_rect.y + (tab_rect.height - text_height) * 0.5;

                    commands.push(DrawCommand::Text(
                        TextCommand::new(
                            Vec2::new(text_x, text_y),
                            shaped,
                            tabs.theme.tab_text_color,
                            1,
                        )
                        .with_clip(tab_clip),
                    ));
                }

                // Close button if closable
                if tabs.theme.closable
                    && let Some(close_rect) = tabs.close_button_bounds(i, &abs_layout) {
                        commands.push(DrawCommand::Quad(
                            QuadCommand::rounded(
                                Vec2::new(close_rect.x, close_rect.y),
                                Vec2::new(close_rect.width, close_rect.height),
                                Color::rgba(1.0, 1.0, 1.0, 0.1),
                                close_rect.width / 2.0,
                                0,
                            )
                            .with_clip(tab_clip),
                        ));

                        // Render X for close button
                        let x_request = ctx.text_pipeline.request_shape(
                            "×".to_string(),
                            0,
                            tabs.theme.tab_font_size * 0.9,
                            None,
                        );

                        if let Some(x_shaped) = ctx.text_pipeline.get_completed(x_request) {
                            let x_width = x_shaped.bounds().0;
                            let x_height = x_shaped.bounds().1;
                            let x_x = close_rect.x + (close_rect.width - x_width) * 0.5;
                            let x_y = close_rect.y + (close_rect.height - x_height) * 0.5;

                            commands.push(DrawCommand::Text(
                                TextCommand::new(
                                    Vec2::new(x_x, x_y),
                                    x_shaped,
                                    tabs.theme.tab_text_color,
                                    2,
                                )
                                .with_clip(tab_clip),
                            ));
                        }
                    }
            }
        }
    }

    // Render scrollbar track + thumb when scrollbar mode is active
    if tabs.should_show_scrollbar() {
        let track = tabs.scrollbar_track_bounds(&abs_layout);
        commands.push(DrawCommand::Quad(
            QuadCommand::filled(
                Vec2::new(track.x, track.y),
                Vec2::new(track.width, track.height),
                tabs.theme.scrollbar_theme.track_color,
                2,
            )
            .with_clip(ctx.clip_rect),
        ));

        let thumb = tabs.scrollbar_thumb_bounds(&abs_layout);
        let thumb_color = tabs.scrollbar_thumb_color();
        commands.push(DrawCommand::Quad(
            QuadCommand::rounded(
                Vec2::new(thumb.x, thumb.y),
                Vec2::new(thumb.width, thumb.height),
                thumb_color,
                tabs.theme.scrollbar_theme.thumb_border_radius,
                3,
            )
            .with_clip(ctx.clip_rect),
        ));
    }

    // Render arrow scroll indicators when arrows mode is active
    if tabs.should_show_arrows() {
        let arrow_color = Color::from_rgba_u8(180, 180, 180, 200);
        let arrow_size = tabs.theme.tab_font_size;
        let arrow_row = tabs.tab_row_bounds(&abs_layout);

        // Left arrow (visible when scrolled past start)
        if tabs.tab_scroll_offset > 0.0 {
            let arrow_request = ctx.text_pipeline.request_shape(
                "\u{25C0}".to_string(), // ◀
                0,
                arrow_size,
                None,
            );
            if let Some(shaped) = ctx.text_pipeline.get_completed(arrow_request) {
                let arrow_h = shaped.bounds().1;
                let ax = arrow_row.x + 2.0;
                let ay = arrow_row.y + (arrow_row.height - arrow_h) * 0.5;
                commands.push(DrawCommand::Text(
                    TextCommand::new(Vec2::new(ax, ay), shaped, arrow_color, 3)
                        .with_clip(ctx.clip_rect),
                ));
            }
        }

        // Right arrow (visible when more tabs are off-screen right)
        let max_offset = tabs.max_tab_scroll_offset(abs_layout.width);
        if tabs.tab_scroll_offset < max_offset {
            let arrow_request = ctx.text_pipeline.request_shape(
                "\u{25B6}".to_string(), // ▶
                0,
                arrow_size,
                None,
            );
            if let Some(shaped) = ctx.text_pipeline.get_completed(arrow_request) {
                let arrow_w = shaped.bounds().0;
                let arrow_h = shaped.bounds().1;
                let ax = arrow_row.x + arrow_row.width - arrow_w - 2.0;
                let ay = arrow_row.y + (arrow_row.height - arrow_h) * 0.5;
                commands.push(DrawCommand::Text(
                    TextCommand::new(Vec2::new(ax, ay), shaped, arrow_color, 3)
                        .with_clip(ctx.clip_rect),
                ));
            }
        }
    }

    // Render drop indicator
    if let Some(indicator_bounds) = tabs.drop_indicator_bounds(&abs_layout) {
        let indicator_color = Color::from_rgba_u8(100, 150, 255, 200);
        commands.push(DrawCommand::Quad(
            QuadCommand::filled(
                Vec2::new(indicator_bounds.x, indicator_bounds.y),
                Vec2::new(indicator_bounds.width, indicator_bounds.height),
                indicator_color,
                3,
            )
            .with_clip(ctx.clip_rect),
        ));
    }

    // Render ghost tab at cursor
    if let Some(dragging_index) = tabs.drag.dragging_tab_index
        && let Some(cursor_pos) = tabs.drag.drag_cursor_pos
    {
        let ghost_label = tabs.tab_label(dragging_index).unwrap_or("");

        let char_width = tabs.theme.tab_font_size * CHAR_WIDTH_FACTOR;
        let text_width = ghost_label.len() as f32 * char_width;
        let close_width = if tabs.theme.closable {
            DEFAULT_CLOSE_BUTTON_SIZE + CLOSE_BUTTON_MARGIN
        } else {
            0.0
        };
        let tab_width = text_width + DEFAULT_TAB_PADDING * 2.0 + close_width;

        let ghost_pos = cursor_pos - Vec2::new(tab_width / 2.0, tabs.theme.tab_bar_height / 2.0);
        let ghost_size = Vec2::new(tab_width, tabs.theme.tab_bar_height);
        let ghost_color = Color::from_rgba_u8(80, 100, 140, 180);

        commands.push(DrawCommand::Quad(
            QuadCommand::rounded(ghost_pos, ghost_size, ghost_color, 4.0, 3).with_clip(ctx.clip_rect),
        ));

        // Ghost text
        let request_id = ctx.text_pipeline.request_shape(
            ghost_label.to_string(),
            0,
            tabs.theme.tab_font_size,
            None,
        );

        if let Some(shaped) = ctx.text_pipeline.get_completed(request_id) {
            let text_height = shaped.bounds().1;
            let text_x = ghost_pos.x + DEFAULT_TAB_PADDING;
            let text_y = ghost_pos.y + (tabs.theme.tab_bar_height - text_height) * 0.5;
            let ghost_text_color = Color::from_rgba_u8(200, 200, 200, 180);

            commands.push(DrawCommand::Text(
                TextCommand::new(Vec2::new(text_x, text_y), shaped, ghost_text_color, 4)
                    .with_clip(ctx.clip_rect),
            ));
        }
    }

    commands
}

// ---------------------------------------------------------------------------
// Traversal: DockTabs
// ---------------------------------------------------------------------------

/// DockTabs only renders the active tab's children.
pub fn dock_tabs_traversal(widget: &dyn Any) -> TraversalBehavior {
    let tabs = widget.downcast_ref::<DockTabs>().unwrap();
    TraversalBehavior::OnlyChild(tabs.active_tab)
}

// ---------------------------------------------------------------------------
// Overflow: DockTabs
// ---------------------------------------------------------------------------

pub fn dock_tabs_overflow(_widget: &dyn Any) -> WidgetOverflow {
    WidgetOverflow {
        overflow_x: Overflow::Hidden,
        overflow_y: Overflow::Hidden,
    }
}
