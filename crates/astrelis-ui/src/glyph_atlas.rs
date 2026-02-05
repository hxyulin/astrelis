//! Glyph atlas integration for converting shaped glyphs to GPU instances.
//!
//! This module bridges the gap between cosmic-text's shaped glyph data and
//! our GPU TextInstance format. It manages the conversion from abstract glyph
//! IDs to concrete atlas UV coordinates for rendering.
//!
//! ## Coordinate System
//!
//! The text rendering system uses a **top-left origin coordinate system**:
//! - Origin (0, 0) is at the top-left corner
//! - X increases rightward
//! - Y increases downward
//!
//! This is consistent with UI layout systems (CSS, Flutter) and Taffy's layout model.
//!
//! ## Baseline to Top-Left Conversion
//!
//! Internally, glyphs are positioned relative to a baseline (typography standard).
//! This module performs the critical conversion from baseline-relative positioning
//! to top-left positioning via the `-placement.top` operation. See the positioning
//! comments in [`glyphs_to_instances`] for details.

use astrelis_core::math::Vec2;
use astrelis_core::profiling::profile_function;
use astrelis_render::Color;
use astrelis_text::{AtlasEntry, FontRenderer, ShapedGlyph};

use crate::gpu_types::TextInstance;

/// Convert shaped glyphs to TextInstances with atlas coordinates.
///
/// This function rasterizes glyphs into the font atlas if needed and
/// generates GPU-ready TextInstance data with proper atlas UV coordinates.
pub fn glyphs_to_instances(
    font_renderer: &mut FontRenderer,
    glyphs: &[ShapedGlyph],
    base_position: Vec2,
    color: Color,
    z_depth: f32,
) -> Vec<TextInstance> {
    let mut instances = Vec::with_capacity(glyphs.len());
    glyphs_to_instances_into(font_renderer, glyphs, base_position, color, z_depth, &mut instances);
    instances
}

/// Convert shaped glyphs to TextInstances, appending to an existing Vec.
///
/// This is an optimized version that avoids allocating a new Vec each call.
/// Use this when processing multiple text commands to reuse allocations.
pub fn glyphs_to_instances_into(
    font_renderer: &mut FontRenderer,
    glyphs: &[ShapedGlyph],
    base_position: Vec2,
    color: Color,
    z_depth: f32,
    out: &mut Vec<TextInstance>,
) {
    profile_function!();
    let atlas_size = font_renderer.atlas_size() as f32;

    for glyph in glyphs {
        // Get or rasterize the glyph into the atlas and get placement info
        if let Some((atlas_entry, placement)) =
            font_renderer.ensure_glyph_with_placement(glyph.cache_key)
        {
            // Calculate screen position with placement offsets.
            //
            // POSITIONING EXPLANATION:
            // - base_position: Top-left corner of the text bounding box (from layout)
            // - glyph.position: Contains (horizontal_advance, baseline_y) from shaping
            // - placement.left: Horizontal bearing (distance from cursor to left edge of glyph)
            // - placement.top: Distance from baseline DOWN to top of glyph (positive downward)
            //
            // The key transformation is `-placement.top`:
            // Since placement.top is the distance FROM baseline TO top (positive = down),
            // we negate it to move FROM baseline UP to the top edge of the glyph.
            // This converts baseline-relative positioning to top-left positioning.
            let screen_pos =
                base_position + glyph.position + Vec2::new(placement.left, -placement.top);

            // Convert atlas pixel coordinates to UV coordinates
            let atlas_uv_min = [
                atlas_entry.x as f32 / atlas_size,
                atlas_entry.y as f32 / atlas_size,
            ];
            let atlas_uv_max = [
                (atlas_entry.x + atlas_entry.width) as f32 / atlas_size,
                (atlas_entry.y + atlas_entry.height) as f32 / atlas_size,
            ];

            // Use placement dimensions for accurate glyph size
            let size = Vec2::new(placement.width, placement.height);

            out.push(TextInstance::new(
                screen_pos,
                size,
                atlas_uv_min,
                atlas_uv_max,
                color,
                z_depth,
            ));
        }
        // Skip glyphs that fail to rasterize (whitespace, missing glyphs, etc.)
    }
}

/// Convert a single glyph to a TextInstance.
///
/// Returns None if the glyph cannot be rasterized or is not in the atlas.
pub fn glyph_to_instance(
    font_renderer: &mut FontRenderer,
    glyph: &ShapedGlyph,
    base_position: Vec2,
    color: Color,
    z_depth: f32,
) -> Option<TextInstance> {
    let atlas_size = font_renderer.atlas_size() as f32;
    let (atlas_entry, placement) = font_renderer.ensure_glyph_with_placement(glyph.cache_key)?;

    // Calculate screen position with placement offsets for correct baseline alignment.
    // The -placement.top converts from baseline-relative to top-left positioning.
    let screen_pos = base_position + glyph.position + Vec2::new(placement.left, -placement.top);

    let atlas_uv_min = [
        atlas_entry.x as f32 / atlas_size,
        atlas_entry.y as f32 / atlas_size,
    ];
    let atlas_uv_max = [
        (atlas_entry.x + atlas_entry.width) as f32 / atlas_size,
        (atlas_entry.y + atlas_entry.height) as f32 / atlas_size,
    ];

    let size = Vec2::new(placement.width, placement.height);

    Some(TextInstance::new(
        screen_pos,
        size,
        atlas_uv_min,
        atlas_uv_max,
        color,
        z_depth,
    ))
}

/// Batch glyphs by atlas page for efficient rendering.
///
/// Since we currently use a single atlas, this just returns all glyphs in one batch.
/// Future enhancement: support multiple atlas pages and batch accordingly.
pub struct GlyphBatch {
    /// Atlas page index (0 for single atlas)
    pub atlas_page: u32,
    /// Start index in the instance buffer
    pub start_index: u32,
    /// Number of instances in this batch
    pub count: u32,
}

impl GlyphBatch {
    /// Create a new glyph batch.
    pub fn new(atlas_page: u32, start_index: u32, count: u32) -> Self {
        Self {
            atlas_page,
            start_index,
            count,
        }
    }
}

/// Create batches from glyphs (currently single batch, future: multi-atlas).
pub fn create_glyph_batches(instance_count: usize) -> Vec<GlyphBatch> {
    if instance_count == 0 {
        return Vec::new();
    }

    // Single atlas for now
    vec![GlyphBatch::new(0, 0, instance_count as u32)]
}

/// Helper to calculate UV coordinates for an atlas entry.
pub fn atlas_entry_uv_coords(entry: &AtlasEntry, atlas_size: u32) -> ([f32; 2], [f32; 2]) {
    let atlas_size_f = atlas_size as f32;
    let uv_min = [entry.x as f32 / atlas_size_f, entry.y as f32 / atlas_size_f];
    let uv_max = [
        (entry.x + entry.width) as f32 / atlas_size_f,
        (entry.y + entry.height) as f32 / atlas_size_f,
    ];
    (uv_min, uv_max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atlas_entry_uv_coords() {
        let entry = AtlasEntry {
            x: 100,
            y: 200,
            width: 50,
            height: 60,
        };
        let atlas_size = 1024;

        let (uv_min, uv_max) = atlas_entry_uv_coords(&entry, atlas_size);

        assert_eq!(uv_min[0], 100.0 / 1024.0);
        assert_eq!(uv_min[1], 200.0 / 1024.0);
        assert_eq!(uv_max[0], 150.0 / 1024.0);
        assert_eq!(uv_max[1], 260.0 / 1024.0);
    }

    #[test]
    fn test_create_glyph_batches_empty() {
        let batches = create_glyph_batches(0);
        assert!(batches.is_empty());
    }

    #[test]
    fn test_create_glyph_batches_single() {
        let batches = create_glyph_batches(10);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].atlas_page, 0);
        assert_eq!(batches[0].start_index, 0);
        assert_eq!(batches[0].count, 10);
    }

    #[test]
    fn test_glyph_batch_creation() {
        let batch = GlyphBatch::new(2, 100, 50);
        assert_eq!(batch.atlas_page, 2);
        assert_eq!(batch.start_index, 100);
        assert_eq!(batch.count, 50);
    }
}
