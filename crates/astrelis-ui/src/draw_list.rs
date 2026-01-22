//! High-level retained draw list for UI rendering.
//!
//! This module implements Phase 5 of the UI refactor plan: retained-mode rendering.
//! The draw list is API-agnostic and can be encoded to different GPU backends.
//! It tracks which nodes contribute which draw commands for efficient updates.

use crate::clip::ClipRect;
use crate::dirty_ranges::DirtyRanges;
use crate::tree::NodeId;
use crate::widgets::{ImageTexture, ImageUV};
use astrelis_core::alloc::HashMap;
use astrelis_core::math::Vec2;
use astrelis_core::profiling::profile_function;
use astrelis_render::Color;
use astrelis_text::PipelineShapedTextResult as ShapedTextResult;
use astrelis_text::{TextEffects, TextRenderMode};
use std::sync::Arc;

/// High-level draw command for a UI element.
#[derive(Debug, Clone)]
pub enum DrawCommand {
    /// Draw a filled or bordered rectangle
    Quad(QuadCommand),
    /// Draw shaped text
    Text(TextCommand),
    /// Draw a textured image
    Image(ImageCommand),
}

impl DrawCommand {
    /// Get the node ID that owns this command.
    pub fn node_id(&self) -> NodeId {
        match self {
            DrawCommand::Quad(q) => q.node_id,
            DrawCommand::Text(t) => t.node_id,
            DrawCommand::Image(i) => i.node_id,
        }
    }
}

impl DrawCommand {
    /// Get the z-index for sorting.
    pub fn z_index(&self) -> u16 {
        match self {
            DrawCommand::Quad(q) => q.z_index,
            DrawCommand::Text(t) => t.z_index,
            DrawCommand::Image(i) => i.z_index,
        }
    }

    /// Check if this is an opaque draw command.
    pub fn is_opaque(&self) -> bool {
        match self {
            DrawCommand::Quad(q) => q.color.a >= 1.0,
            DrawCommand::Text(t) => t.color.a >= 1.0,
            DrawCommand::Image(i) => i.tint.a >= 1.0,
        }
    }

    /// Set the node ID for this command.
    pub fn set_node_id(&mut self, node_id: NodeId) {
        match self {
            DrawCommand::Quad(q) => q.node_id = node_id,
            DrawCommand::Text(t) => t.node_id = node_id,
            DrawCommand::Image(i) => i.node_id = node_id,
        }
    }

    /// Get the clip rectangle for this command.
    pub fn clip_rect(&self) -> &ClipRect {
        match self {
            DrawCommand::Quad(q) => &q.clip_rect,
            DrawCommand::Text(t) => &t.clip_rect,
            DrawCommand::Image(i) => &i.clip_rect,
        }
    }

    /// Set the clip rectangle for this command.
    pub fn set_clip_rect(&mut self, clip_rect: ClipRect) {
        match self {
            DrawCommand::Quad(q) => q.clip_rect = clip_rect,
            DrawCommand::Text(t) => t.clip_rect = clip_rect,
            DrawCommand::Image(i) => i.clip_rect = clip_rect,
        }
    }
}

/// Command to draw a quad (rectangle).
#[derive(Debug, Clone)]
pub struct QuadCommand {
    /// Node that owns this command
    pub node_id: NodeId,
    /// Position in screen space
    pub position: Vec2,
    /// Size of the quad
    pub size: Vec2,
    /// Fill or border color
    pub color: Color,
    /// Border radius for rounded corners (0 = sharp)
    pub border_radius: f32,
    /// Border thickness (0 = filled, >0 = outline)
    pub border_thickness: f32,
    /// Z-index for depth sorting
    pub z_index: u16,
    /// Clip rectangle for scissor clipping
    pub clip_rect: ClipRect,
}

impl QuadCommand {
    /// Create a new filled quad command.
    pub fn filled(position: Vec2, size: Vec2, color: Color, z_index: u16) -> Self {
        Self {
            node_id: NodeId(0), // Will be set by DrawList
            position,
            size,
            color,
            border_radius: 0.0,
            border_thickness: 0.0,
            z_index,
            clip_rect: ClipRect::infinite(),
        }
    }

    /// Create a new rounded quad command.
    pub fn rounded(
        position: Vec2,
        size: Vec2,
        color: Color,
        border_radius: f32,
        z_index: u16,
    ) -> Self {
        Self {
            node_id: NodeId(0), // Will be set by DrawList
            position,
            size,
            color,
            border_radius,
            border_thickness: 0.0,
            z_index,
            clip_rect: ClipRect::infinite(),
        }
    }

    /// Create a new bordered quad command.
    pub fn bordered(
        position: Vec2,
        size: Vec2,
        color: Color,
        border_thickness: f32,
        border_radius: f32,
        z_index: u16,
    ) -> Self {
        Self {
            node_id: NodeId(0), // Will be set by DrawList
            position,
            size,
            color,
            border_radius,
            border_thickness,
            z_index,
            clip_rect: ClipRect::infinite(),
        }
    }

    /// Set the clip rectangle for this quad.
    pub fn with_clip(mut self, clip_rect: ClipRect) -> Self {
        self.clip_rect = clip_rect;
        self
    }
}

/// Command to draw shaped text.
#[derive(Debug, Clone)]
pub struct TextCommand {
    /// Node that owns this command
    pub node_id: NodeId,
    /// Position in screen space
    pub position: Vec2,
    /// Shaped text result with glyphs
    pub shaped_text: Arc<ShapedTextResult>,
    /// Text color
    pub color: Color,
    /// Z-index for depth sorting
    pub z_index: u16,
    /// Optional text effects (shadows, outlines, glows)
    pub effects: Option<TextEffects>,
    /// Render mode (Bitmap or SDF) - auto-selected when effects are present
    pub render_mode: TextRenderMode,
    /// Clip rectangle for scissor clipping
    pub clip_rect: ClipRect,
}

impl TextCommand {
    /// Create a new text command.
    pub fn new(
        position: Vec2,
        shaped_text: Arc<ShapedTextResult>,
        color: Color,
        z_index: u16,
    ) -> Self {
        Self {
            node_id: NodeId(0), // Will be set by DrawList
            position,
            shaped_text,
            color,
            z_index,
            effects: None,
            render_mode: TextRenderMode::Bitmap,
            clip_rect: ClipRect::infinite(),
        }
    }

    /// Create a text command with effects (automatically uses SDF rendering).
    ///
    /// Effects require SDF (Signed Distance Field) rendering to work properly.
    /// This constructor automatically sets the render mode to SDF.
    ///
    /// # Arguments
    ///
    /// * `position` - Top-left position to draw the text
    /// * `shaped_text` - Pre-shaped text buffer with glyph layout
    /// * `color` - Base text color
    /// * `z_index` - Depth sorting order
    /// * `effects` - Collection of text effects (shadows, outlines, glows)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use astrelis_ui::{TextCommand, TextEffectsBuilder};
    /// use astrelis_core::math::Vec2;
    /// use astrelis_render::Color;
    ///
    /// let effects = TextEffectsBuilder::new()
    ///     .shadow(Vec2::new(2.0, 2.0), Color::BLACK)
    ///     .outline(1.5, Color::WHITE)
    ///     .build();
    ///
    /// let cmd = TextCommand::with_effects(
    ///     Vec2::new(100.0, 100.0),
    ///     shaped_text,
    ///     Color::WHITE,
    ///     1,
    ///     effects
    /// );
    /// ```
    pub fn with_effects(
        position: Vec2,
        shaped_text: Arc<ShapedTextResult>,
        color: Color,
        z_index: u16,
        effects: TextEffects,
    ) -> Self {
        Self {
            node_id: NodeId(0),
            position,
            shaped_text,
            color,
            z_index,
            effects: Some(effects),
            render_mode: TextRenderMode::SDF { spread: 4.0 },
            clip_rect: ClipRect::infinite(),
        }
    }

    /// Check if this text command requires SDF rendering.
    ///
    /// Returns `true` if:
    /// - The render mode is explicitly set to SDF, OR
    /// - The text has effects (which require SDF)
    ///
    /// This is used by the renderer to select the appropriate rendering pipeline.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let cmd = TextCommand::new(...);
    /// assert!(!cmd.requires_sdf());  // Bitmap by default
    ///
    /// let cmd = TextCommand::with_effects(..., effects);
    /// assert!(cmd.requires_sdf());  // SDF required for effects
    /// ```
    pub fn requires_sdf(&self) -> bool {
        self.render_mode.is_sdf() || self.effects.is_some()
    }

    /// Set the clip rectangle for this text.
    pub fn with_clip(mut self, clip_rect: ClipRect) -> Self {
        self.clip_rect = clip_rect;
        self
    }
}

/// Command to draw an image (textured quad).
#[derive(Clone)]
pub struct ImageCommand {
    /// Node that owns this command
    pub node_id: NodeId,
    /// Position in screen space
    pub position: Vec2,
    /// Size of the image
    pub size: Vec2,
    /// The texture to draw
    pub texture: ImageTexture,
    /// UV coordinates for sprite regions
    pub uv: ImageUV,
    /// Tint color (multiplied with texture)
    pub tint: Color,
    /// Border radius for rounded corners
    pub border_radius: f32,
    /// Sampling mode for texture filtering
    pub sampling: astrelis_render::ImageSampling,
    /// Z-index for depth sorting
    pub z_index: u16,
    /// Clip rectangle for scissor clipping
    pub clip_rect: ClipRect,
}

impl std::fmt::Debug for ImageCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageCommand")
            .field("node_id", &self.node_id)
            .field("position", &self.position)
            .field("size", &self.size)
            .field("uv", &self.uv)
            .field("tint", &self.tint)
            .field("border_radius", &self.border_radius)
            .field("sampling", &self.sampling)
            .field("z_index", &self.z_index)
            .field("clip_rect", &self.clip_rect)
            .finish()
    }
}

impl ImageCommand {
    /// Create a new image command.
    pub fn new(
        position: Vec2,
        size: Vec2,
        texture: ImageTexture,
        uv: ImageUV,
        tint: Color,
        border_radius: f32,
        sampling: astrelis_render::ImageSampling,
        z_index: u16,
    ) -> Self {
        Self {
            node_id: NodeId(0), // Will be set by DrawList
            position,
            size,
            texture,
            uv,
            tint,
            border_radius,
            sampling,
            z_index,
            clip_rect: ClipRect::infinite(),
        }
    }

    /// Create a simple image command with default UV and sampling (full texture, linear filtering).
    pub fn simple(position: Vec2, size: Vec2, texture: ImageTexture, z_index: u16) -> Self {
        Self::new(
            position,
            size,
            texture,
            ImageUV::default(),
            Color::WHITE,
            0.0,
            astrelis_render::ImageSampling::default(),
            z_index,
        )
    }

    /// Set the clip rectangle for this image.
    pub fn with_clip(mut self, clip_rect: ClipRect) -> Self {
        self.clip_rect = clip_rect;
        self
    }
}

/// Retained draw list for efficient UI rendering.
///
/// Maintains a list of draw commands and tracks which nodes contribute
/// which commands. Supports incremental updates where only dirty nodes
/// need to be re-encoded.
pub struct DrawList {
    /// All draw commands in the list
    commands: Vec<DrawCommand>,
    /// Mapping from node ID to command indices
    node_to_commands: HashMap<NodeId, Vec<usize>>,
    /// Ranges of commands that have been modified
    dirty_ranges: DirtyRanges,
    /// Whether the entire list needs re-sorting
    needs_sort: bool,
    /// Total number of updates since creation
    update_count: u64,
}

impl DrawList {
    /// Create a new empty draw list.
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            node_to_commands: HashMap::new(),
            dirty_ranges: DirtyRanges::new(),
            needs_sort: false,
            update_count: 0,
        }
    }

    /// Create a draw list with initial capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            commands: Vec::with_capacity(capacity),
            node_to_commands: HashMap::with_capacity(capacity / 2),
            dirty_ranges: DirtyRanges::new(),
            needs_sort: false,
            update_count: 0,
        }
    }

    /// Update commands for a node.
    ///
    /// Replaces all existing commands for the node with the new ones.
    /// Marks affected ranges as dirty for GPU update.
    pub fn update_node(&mut self, node_id: NodeId, mut new_commands: Vec<DrawCommand>) {
        self.update_count += 1;

        // Remove old commands for this node
        if let Some(old_indices) = self.node_to_commands.get(&node_id) {
            // Mark old ranges as dirty and clear them
            for &idx in old_indices.iter().rev() {
                if idx < self.commands.len() {
                    self.dirty_ranges.mark_dirty(idx, idx + 1);
                    // Don't remove yet - we'll compact later
                }
            }
        }

        if new_commands.is_empty() {
            // Node has no commands anymore
            self.node_to_commands.remove(&node_id);
            self.needs_sort = true; // May need to compact
            return;
        }

        // Add new commands
        let start_idx = self.commands.len();
        let mut new_indices = Vec::with_capacity(new_commands.len());

        for mut cmd in new_commands.drain(..) {
            cmd.set_node_id(node_id);
            let idx = self.commands.len();
            self.commands.push(cmd);
            new_indices.push(idx);
        }

        let end_idx = self.commands.len();

        // Mark new range as dirty
        self.dirty_ranges.mark_dirty(start_idx, end_idx);

        // Update mapping
        self.node_to_commands.insert(node_id, new_indices);

        // May need sorting if z-index changed
        self.needs_sort = true;
    }

    /// Update only the color of a node's commands (fast path for paint-only changes).
    ///
    /// This is much faster than full command replacement when only colors change.
    pub fn update_node_colors(&mut self, node_id: NodeId, color: Color) {
        if let Some(indices) = self.node_to_commands.get(&node_id) {
            for &idx in indices {
                if let Some(cmd) = self.commands.get_mut(idx) {
                    match cmd {
                        DrawCommand::Quad(q) => q.color = color,
                        DrawCommand::Text(t) => t.color = color,
                        DrawCommand::Image(i) => i.tint = color,
                    }
                    self.dirty_ranges.mark_dirty(idx, idx + 1);
                }
            }
            self.update_count += 1;
        }
    }

    /// Remove all commands for a node.
    pub fn remove_node(&mut self, node_id: NodeId) {
        if let Some(indices) = self.node_to_commands.remove(&node_id) {
            // Mark ranges as dirty
            for idx in indices {
                if idx < self.commands.len() {
                    self.dirty_ranges.mark_dirty(idx, idx + 1);
                }
            }
            self.needs_sort = true; // Need to compact
            self.update_count += 1;
        }
    }

    /// Sort commands by z-index and prepare for rendering.
    ///
    /// Should be called before encoding to GPU to ensure proper draw order.
    pub fn sort_if_needed(&mut self) {
        profile_function!();

        if !self.needs_sort {
            return;
        }

        // Compact: remove invalidated commands
        self.compact();

        // Sort by z-index (stable sort preserves order for same z-index)
        self.commands
            .sort_by_key(|cmd| (cmd.z_index(), !cmd.is_opaque()));

        // Rebuild node mapping
        self.rebuild_node_mapping();

        // Mark entire list as dirty after sort
        if !self.commands.is_empty() {
            self.dirty_ranges.mark_dirty(0, self.commands.len());
        }

        self.needs_sort = false;
    }

    /// Compact the command list by removing invalidated entries.
    fn compact(&mut self) {
        // Collect all valid command indices
        let mut valid_indices: Vec<usize> = self
            .node_to_commands
            .values()
            .flat_map(|indices| indices.iter().copied())
            .collect();

        if valid_indices.len() == self.commands.len() {
            // No compaction needed
            return;
        }

        valid_indices.sort_unstable();
        valid_indices.dedup();

        // Build new command list with only valid commands
        let mut new_commands = Vec::with_capacity(valid_indices.len());
        for idx in valid_indices {
            if let Some(cmd) = self.commands.get(idx) {
                new_commands.push(cmd.clone());
            }
        }

        self.commands = new_commands;
    }

    /// Rebuild node-to-command mapping after sorting/compacting.
    fn rebuild_node_mapping(&mut self) {
        // Clear old mapping
        self.node_to_commands.clear();

        // Rebuild from commands (now that they track their node_id)
        for (idx, cmd) in self.commands.iter().enumerate() {
            self.node_to_commands
                .entry(cmd.node_id())
                .or_default()
                .push(idx);
        }
    }

    /// Get all commands for rendering.
    pub fn commands(&self) -> &[DrawCommand] {
        &self.commands
    }

    /// Get dirty ranges for partial GPU updates.
    pub fn dirty_ranges(&self) -> &DirtyRanges {
        &self.dirty_ranges
    }

    /// Clear all dirty ranges.
    pub fn clear_dirty(&mut self) {
        self.dirty_ranges.clear();
    }

    /// Clear the entire draw list.
    pub fn clear(&mut self) {
        self.commands.clear();
        self.node_to_commands.clear();
        self.dirty_ranges.clear();
        self.needs_sort = false;
    }

    /// Get the number of commands in the list.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Check if the draw list is empty.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Get statistics about the draw list.
    pub fn stats(&self) -> DrawListStats {
        let mut num_quads = 0;
        let mut num_text = 0;
        let mut num_images = 0;
        let mut num_opaque = 0;
        let mut num_transparent = 0;

        for cmd in &self.commands {
            match cmd {
                DrawCommand::Quad(_) => num_quads += 1,
                DrawCommand::Text(_) => num_text += 1,
                DrawCommand::Image(_) => num_images += 1,
            }
            if cmd.is_opaque() {
                num_opaque += 1;
            } else {
                num_transparent += 1;
            }
        }

        DrawListStats {
            total_commands: self.commands.len(),
            num_quads,
            num_text,
            num_images,
            num_opaque,
            num_transparent,
            num_nodes: self.node_to_commands.len(),
            dirty_ranges: self.dirty_ranges.stats().num_ranges,
            needs_sort: self.needs_sort,
            update_count: self.update_count,
        }
    }

    /// Separate commands into opaque and transparent batches.
    ///
    /// Returns (opaque_commands, transparent_commands) for two-pass rendering.
    pub fn separate_by_opacity(&self) -> (Vec<&DrawCommand>, Vec<&DrawCommand>) {
        let mut opaque = Vec::new();
        let mut transparent = Vec::new();

        for cmd in &self.commands {
            if cmd.is_opaque() {
                opaque.push(cmd);
            } else {
                transparent.push(cmd);
            }
        }

        (opaque, transparent)
    }

    /// Get commands in a specific z-index range.
    pub fn commands_in_z_range(&self, min_z: u16, max_z: u16) -> Vec<&DrawCommand> {
        self.commands
            .iter()
            .filter(|cmd| {
                let z = cmd.z_index();
                z >= min_z && z <= max_z
            })
            .collect()
    }
}

impl Default for DrawList {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about a draw list.
#[derive(Debug, Clone, Copy)]
pub struct DrawListStats {
    pub total_commands: usize,
    pub num_quads: usize,
    pub num_text: usize,
    pub num_images: usize,
    pub num_opaque: usize,
    pub num_transparent: usize,
    pub num_nodes: usize,
    pub dirty_ranges: usize,
    pub needs_sort: bool,
    pub update_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use astrelis_text::PipelineShapedTextResult as ShapedTextResult;
    use astrelis_text::ShapedTextResult as BaseShapedTextResult;

    #[test]
    fn test_add_and_get_commands() {
        let mut draw_list = DrawList::new();
        let node_id = NodeId(1);

        let cmd = DrawCommand::Quad(QuadCommand::filled(
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 50.0),
            Color::WHITE,
            0,
        ));

        draw_list.update_node(node_id, vec![cmd]);

        assert_eq!(draw_list.len(), 1);
        assert!(!draw_list.is_empty());
    }

    #[test]
    fn test_update_replaces_commands() {
        let mut draw_list = DrawList::new();
        let node_id = NodeId(1);

        // First update
        let cmd1 = DrawCommand::Quad(QuadCommand::filled(
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 50.0),
            Color::WHITE,
            0,
        ));
        draw_list.update_node(node_id, vec![cmd1]);

        assert_eq!(draw_list.len(), 1);

        // Second update with different command
        let cmd2 = DrawCommand::Quad(QuadCommand::filled(
            Vec2::new(10.0, 10.0),
            Vec2::new(200.0, 100.0),
            Color::RED,
            0,
        ));
        draw_list.update_node(node_id, vec![cmd2]);

        // Should still be 2 commands (old ones not immediately removed)
        // Compaction happens on sort
        assert!(draw_list.len() >= 1);
    }

    #[test]
    fn test_remove_node() {
        let mut draw_list = DrawList::new();
        let node_id = NodeId(1);

        let cmd = DrawCommand::Quad(QuadCommand::filled(
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 50.0),
            Color::WHITE,
            0,
        ));
        draw_list.update_node(node_id, vec![cmd]);

        draw_list.remove_node(node_id);

        // Commands marked for removal but not compacted yet
        assert!(draw_list.needs_sort);
    }

    #[test]
    fn test_clear() {
        let mut draw_list = DrawList::new();
        draw_list.update_node(
            NodeId(1),
            vec![DrawCommand::Quad(QuadCommand::filled(
                Vec2::ZERO,
                Vec2::new(100.0, 100.0),
                Color::WHITE,
                0,
            ))],
        );

        draw_list.clear();

        assert_eq!(draw_list.len(), 0);
        assert!(draw_list.is_empty());
    }

    #[test]
    fn test_dirty_ranges() {
        let mut draw_list = DrawList::new();
        let node_id = NodeId(1);

        draw_list.update_node(
            node_id,
            vec![DrawCommand::Quad(QuadCommand::filled(
                Vec2::ZERO,
                Vec2::new(100.0, 100.0),
                Color::WHITE,
                0,
            ))],
        );

        assert!(!draw_list.dirty_ranges().is_empty());

        draw_list.clear_dirty();

        assert!(draw_list.dirty_ranges().is_empty());
    }

    #[test]
    fn test_update_colors_fast_path() {
        let mut draw_list = DrawList::new();
        let node_id = NodeId(1);

        draw_list.update_node(
            node_id,
            vec![DrawCommand::Quad(QuadCommand::filled(
                Vec2::ZERO,
                Vec2::new(100.0, 100.0),
                Color::WHITE,
                0,
            ))],
        );

        draw_list.clear_dirty();
        draw_list.update_node_colors(node_id, Color::RED);

        assert!(!draw_list.dirty_ranges().is_empty());
    }

    #[test]
    fn test_stats() {
        let mut draw_list = DrawList::new();

        draw_list.update_node(
            NodeId(1),
            vec![DrawCommand::Quad(QuadCommand::filled(
                Vec2::ZERO,
                Vec2::new(100.0, 100.0),
                Color::WHITE,
                0,
            ))],
        );

        let shaped = Arc::new(ShapedTextResult::new(
            1,
            BaseShapedTextResult::new((100.0, 20.0), vec![]),
        ));
        draw_list.update_node(
            NodeId(2),
            vec![DrawCommand::Text(TextCommand::new(
                Vec2::ZERO,
                shaped,
                Color::BLACK,
                0,
            ))],
        );

        let stats = draw_list.stats();
        assert_eq!(stats.num_quads, 1);
        assert_eq!(stats.num_text, 1);
        assert_eq!(stats.total_commands, 2);
    }

    #[test]
    fn test_opacity_separation() {
        let mut draw_list = DrawList::new();

        // Opaque quad
        draw_list.update_node(
            NodeId(1),
            vec![DrawCommand::Quad(QuadCommand::filled(
                Vec2::ZERO,
                Vec2::new(100.0, 100.0),
                Color::WHITE,
                0,
            ))],
        );

        // Transparent quad
        draw_list.update_node(
            NodeId(2),
            vec![DrawCommand::Quad(QuadCommand::filled(
                Vec2::ZERO,
                Vec2::new(100.0, 100.0),
                Color::rgba(1.0, 1.0, 1.0, 0.5),
                0,
            ))],
        );

        let (opaque, transparent) = draw_list.separate_by_opacity();
        assert_eq!(opaque.len(), 1);
        assert_eq!(transparent.len(), 1);
    }

    #[test]
    fn test_text_command_new() {
        let shaped = Arc::new(ShapedTextResult::new(
            1,
            BaseShapedTextResult::new((100.0, 20.0), vec![]),
        ));
        let cmd = TextCommand::new(Vec2::new(10.0, 20.0), shaped, Color::BLACK, 5);

        assert_eq!(cmd.position, Vec2::new(10.0, 20.0));
        assert_eq!(cmd.color, Color::BLACK);
        assert_eq!(cmd.z_index, 5);
        assert!(cmd.effects.is_none());
        assert_eq!(cmd.render_mode, TextRenderMode::Bitmap);
        assert!(!cmd.requires_sdf());
    }

    #[test]
    fn test_text_command_with_effects() {
        use astrelis_text::{TextEffect, TextEffects};

        let mut effects = TextEffects::new();
        effects.add(TextEffect::shadow(Vec2::new(2.0, 2.0), Color::BLACK));

        let shaped = Arc::new(ShapedTextResult::new(
            1,
            BaseShapedTextResult::new((100.0, 20.0), vec![]),
        ));
        let cmd = TextCommand::with_effects(
            Vec2::new(10.0, 20.0),
            shaped,
            Color::WHITE,
            5,
            effects,
        );

        assert!(cmd.effects.is_some());
        assert_eq!(cmd.render_mode, TextRenderMode::SDF { spread: 4.0 });
        assert!(cmd.requires_sdf());
    }

    #[test]
    fn test_text_command_requires_sdf_with_effects() {
        use astrelis_text::{TextEffect, TextEffects};

        let mut effects = TextEffects::new();
        effects.add(TextEffect::outline(1.0, Color::BLACK));

        let shaped = Arc::new(ShapedTextResult::new(
            1,
            BaseShapedTextResult::new((100.0, 20.0), vec![]),
        ));
        let mut cmd = TextCommand::new(Vec2::ZERO, shaped, Color::WHITE, 0);
        cmd.effects = Some(effects);

        // Has effects, should require SDF
        assert!(cmd.requires_sdf());
    }

    #[test]
    fn test_text_command_requires_sdf_with_sdf_mode() {
        let shaped = Arc::new(ShapedTextResult::new(
            1,
            BaseShapedTextResult::new((100.0, 20.0), vec![]),
        ));
        let mut cmd = TextCommand::new(Vec2::ZERO, shaped, Color::WHITE, 0);
        cmd.render_mode = TextRenderMode::SDF { spread: 6.0 };

        // SDF mode set, should require SDF
        assert!(cmd.requires_sdf());
    }

    #[test]
    fn test_text_command_requires_sdf_bitmap_no_effects() {
        let shaped = Arc::new(ShapedTextResult::new(
            1,
            BaseShapedTextResult::new((100.0, 20.0), vec![]),
        ));
        let cmd = TextCommand::new(Vec2::ZERO, shaped, Color::WHITE, 0);

        // Bitmap mode, no effects, should not require SDF
        assert!(!cmd.requires_sdf());
    }

    #[test]
    fn test_text_command_multiple_effects() {
        use astrelis_text::{TextEffect, TextEffects};

        let mut effects = TextEffects::new();
        effects.add(TextEffect::shadow(Vec2::new(1.0, 1.0), Color::BLACK));
        effects.add(TextEffect::outline(1.0, Color::WHITE));
        effects.add(TextEffect::glow(3.0, Color::BLUE, 0.5));

        let shaped = Arc::new(ShapedTextResult::new(
            1,
            BaseShapedTextResult::new((100.0, 20.0), vec![]),
        ));
        let cmd = TextCommand::with_effects(Vec2::ZERO, shaped, Color::WHITE, 0, effects.clone());

        assert!(cmd.requires_sdf());
        assert_eq!(cmd.effects.as_ref().unwrap().effects().len(), 3);
    }
}
