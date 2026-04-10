//! Bitmap glyph atlas management.
//!
//! Manages an R8Unorm texture atlas for bitmap glyph rendering, with
//! CPU-side data mirroring and dirty tracking for efficient GPU uploads.

use std::collections::HashMap;

use cosmic_text::CacheKey;

use super::{AtlasEntry, AtlasPacker, GlyphPlacement};

/// Bitmap glyph atlas.
///
/// Stores rasterized glyphs in an R8Unorm texture with a CPU-side mirror
/// for incremental upload.
pub struct BitmapAtlas {
    /// CPU-side atlas data.
    pub data: Vec<u8>,
    /// Atlas dimensions.
    pub size: u32,
    /// Glyph entries keyed by cache key.
    pub entries: HashMap<CacheKey, (AtlasEntry, GlyphPlacement)>,
    /// Rectangle packer.
    pub packer: AtlasPacker,
    /// Whether the atlas data has been modified since last GPU upload.
    pub dirty: bool,
}

impl BitmapAtlas {
    /// Create a new bitmap atlas of the given size.
    pub fn new(size: u32) -> Self {
        astrelis_profiling::profile_function!();
        Self {
            data: vec![0u8; (size * size) as usize],
            size,
            entries: HashMap::new(),
            packer: AtlasPacker::new(size, size),
            dirty: false,
        }
    }

    /// Insert a rasterized glyph into the atlas.
    ///
    /// Returns `None` if the atlas is full.
    pub fn insert(
        &mut self,
        cache_key: CacheKey,
        glyph_data: &[u8],
        width: u32,
        height: u32,
        placement: GlyphPlacement,
    ) -> Option<AtlasEntry> {
        astrelis_profiling::profile_function!();
        if let Some((entry, _)) = self.entries.get(&cache_key) {
            return Some(*entry);
        }

        let (x, y) = self.packer.allocate(width, height)?;

        // Copy glyph data into atlas
        for row in 0..height {
            let src_start = (row * width) as usize;
            let dst_start = ((y + row) * self.size + x) as usize;
            let len = width as usize;

            if src_start + len <= glyph_data.len() && dst_start + len <= self.data.len() {
                self.data[dst_start..dst_start + len]
                    .copy_from_slice(&glyph_data[src_start..src_start + len]);
            }
        }

        let entry = AtlasEntry::new(x, y, width, height);
        self.entries.insert(cache_key, (entry, placement));
        self.dirty = true;

        Some(entry)
    }

    /// Look up an existing glyph entry.
    pub fn get(&self, cache_key: &CacheKey) -> Option<&(AtlasEntry, GlyphPlacement)> {
        self.entries.get(cache_key)
    }

    /// Clear the atlas.
    pub fn clear(&mut self) {
        self.data.fill(0);
        self.entries.clear();
        self.packer.reset();
        self.dirty = true;
    }

    /// Get the number of cached glyphs.
    pub fn glyph_count(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cache_key() -> CacheKey {
        CacheKey {
            font_id: cosmic_text::fontdb::ID::dummy(),
            glyph_id: 65,
            font_size_bits: 16.0_f32.to_bits(),
            font_weight: cosmic_text::Weight(400),
            x_bin: cosmic_text::SubpixelBin::Zero,
            y_bin: cosmic_text::SubpixelBin::Zero,
            flags: cosmic_text::CacheKeyFlags::empty(),
        }
    }

    #[test]
    fn test_bitmap_atlas_creation() {
        let atlas = BitmapAtlas::new(256);
        assert_eq!(atlas.size, 256);
        assert_eq!(atlas.data.len(), 256 * 256);
        assert_eq!(atlas.glyph_count(), 0);
        assert!(!atlas.dirty);
    }

    #[test]
    fn test_bitmap_atlas_insert() {
        let mut atlas = BitmapAtlas::new(256);
        let key = test_cache_key();
        let glyph_data = vec![255u8; 10 * 12];
        let placement = GlyphPlacement {
            left: 0,
            top: 10,
            width: 10,
            height: 12,
        };

        let entry = atlas.insert(key, &glyph_data, 10, 12, placement);
        assert!(entry.is_some());
        assert_eq!(atlas.glyph_count(), 1);
        assert!(atlas.dirty);
    }

    #[test]
    fn test_bitmap_atlas_duplicate_insert() {
        let mut atlas = BitmapAtlas::new(256);
        let key = test_cache_key();
        let glyph_data = vec![255u8; 10 * 12];
        let placement = GlyphPlacement {
            left: 0,
            top: 10,
            width: 10,
            height: 12,
        };

        let entry1 = atlas.insert(key, &glyph_data, 10, 12, placement);
        let entry2 = atlas.insert(key, &glyph_data, 10, 12, placement);

        assert_eq!(entry1.unwrap().x, entry2.unwrap().x);
        assert_eq!(atlas.glyph_count(), 1);
    }

    #[test]
    fn test_bitmap_atlas_clear() {
        let mut atlas = BitmapAtlas::new(256);
        let key = test_cache_key();
        let glyph_data = vec![255u8; 10 * 12];
        let placement = GlyphPlacement {
            left: 0,
            top: 10,
            width: 10,
            height: 12,
        };

        atlas.insert(key, &glyph_data, 10, 12, placement);
        atlas.clear();

        assert_eq!(atlas.glyph_count(), 0);
        assert!(atlas.dirty);
    }
}
