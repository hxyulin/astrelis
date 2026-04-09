//! Atlas texture management for text rendering.
//!
//! Provides packing, entry tracking, and glyph placement for both
//! bitmap and SDF texture atlases.

pub mod bitmap;
pub mod sdf;

/// A rectangle packer for atlas allocation.
///
/// Uses a simple shelf-based algorithm to pack rectangles into the atlas.
#[derive(Debug)]
pub struct AtlasPacker {
    width: u32,
    height: u32,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
}

impl AtlasPacker {
    /// Create a new atlas packer for the given dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
        }
    }

    /// Try to allocate a rectangle of the given size.
    ///
    /// Returns `Some((x, y))` if the rectangle was allocated, `None` if the atlas is full.
    pub fn allocate(&mut self, glyph_width: u32, glyph_height: u32) -> Option<(u32, u32)> {
        // Add 1px padding to prevent bleed
        let padded_width = glyph_width + 1;
        let padded_height = glyph_height + 1;

        // Check if we need to wrap to next row
        if self.cursor_x + padded_width > self.width {
            self.cursor_x = 0;
            self.cursor_y += self.row_height;
            self.row_height = 0;
        }

        // Check if we've run out of vertical space
        if self.cursor_y + padded_height > self.height {
            return None;
        }

        let x = self.cursor_x;
        let y = self.cursor_y;

        self.cursor_x += padded_width;
        self.row_height = self.row_height.max(padded_height);

        Some((x, y))
    }

    /// Reset the packer (does not clear the atlas data).
    pub fn reset(&mut self) {
        self.cursor_x = 0;
        self.cursor_y = 0;
        self.row_height = 0;
    }

    /// Get the atlas dimensions.
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Estimate utilization as a fraction (0.0 to 1.0).
    pub fn utilization(&self) -> f32 {
        let used_area = self.cursor_y * self.width + self.cursor_x * self.row_height;
        let total_area = self.width * self.height;
        if total_area == 0 {
            0.0
        } else {
            used_area as f32 / total_area as f32
        }
    }
}

/// Position and size of a glyph in the atlas texture.
#[derive(Debug, Clone, Copy)]
pub struct AtlasEntry {
    /// X position in the atlas.
    pub x: u32,
    /// Y position in the atlas.
    pub y: u32,
    /// Width of the glyph in the atlas.
    pub width: u32,
    /// Height of the glyph in the atlas.
    pub height: u32,
}

impl AtlasEntry {
    /// Create a new atlas entry.
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Get UV coordinates for the atlas entry given the atlas dimensions.
    ///
    /// Returns `(u_min, v_min, u_max, v_max)`.
    pub fn uv(&self, atlas_width: u32, atlas_height: u32) -> (f32, f32, f32, f32) {
        let u_min = self.x as f32 / atlas_width as f32;
        let v_min = self.y as f32 / atlas_height as f32;
        let u_max = (self.x + self.width) as f32 / atlas_width as f32;
        let v_max = (self.y + self.height) as f32 / atlas_height as f32;
        (u_min, v_min, u_max, v_max)
    }
}

/// Glyph placement metrics from the rasterizer.
#[derive(Debug, Clone, Copy)]
pub struct GlyphPlacement {
    /// Horizontal offset from the glyph origin.
    pub left: i32,
    /// Vertical offset from the glyph origin (positive = up from baseline).
    pub top: i32,
    /// Width of the rasterized glyph.
    pub width: u32,
    /// Height of the rasterized glyph.
    pub height: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atlas_packer_basic() {
        let mut packer = AtlasPacker::new(256, 256);

        let pos = packer.allocate(32, 32);
        assert!(pos.is_some());
        assert_eq!(pos.unwrap(), (0, 0));

        let pos = packer.allocate(32, 32);
        assert!(pos.is_some());
        assert_eq!(pos.unwrap(), (33, 0)); // 32 + 1 padding
    }

    #[test]
    fn test_atlas_packer_wrap() {
        let mut packer = AtlasPacker::new(64, 128);

        // Fill first row
        let _ = packer.allocate(30, 20);
        let _ = packer.allocate(30, 20);

        // Should wrap to next row
        let pos = packer.allocate(30, 20);
        assert!(pos.is_some());
        let (x, y) = pos.unwrap();
        assert_eq!(x, 0);
        assert!(y > 0);
    }

    #[test]
    fn test_atlas_packer_full() {
        let mut packer = AtlasPacker::new(32, 32);

        let pos1 = packer.allocate(30, 30);
        assert!(pos1.is_some());

        // Atlas should be too full for another 30x30 glyph
        let pos2 = packer.allocate(30, 30);
        assert!(pos2.is_none());
    }

    #[test]
    fn test_atlas_packer_reset() {
        let mut packer = AtlasPacker::new(64, 64);
        let _ = packer.allocate(32, 32);
        packer.reset();
        let pos = packer.allocate(32, 32);
        assert_eq!(pos, Some((0, 0)));
    }

    #[test]
    fn test_atlas_entry_uv() {
        let entry = AtlasEntry::new(10, 20, 30, 40);
        let (u_min, v_min, u_max, v_max) = entry.uv(256, 256);

        assert!((u_min - 10.0 / 256.0).abs() < f32::EPSILON);
        assert!((v_min - 20.0 / 256.0).abs() < f32::EPSILON);
        assert!((u_max - 40.0 / 256.0).abs() < f32::EPSILON);
        assert!((v_max - 60.0 / 256.0).abs() < f32::EPSILON);
    }
}
