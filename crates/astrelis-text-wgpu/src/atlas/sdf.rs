//! SDF glyph atlas management.
//!
//! Similar to the bitmap atlas but stores Signed Distance Field data
//! and uses size-independent cache keys for resolution-independent rendering.

use std::collections::HashMap;

use super::{AtlasEntry, AtlasPacker, GlyphPlacement};

/// Size-independent cache key for SDF glyphs.
///
/// Unlike bitmap rendering, SDF glyphs are rasterized at a fixed base size
/// and scaled in the shader, so the cache key omits the font size bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SdfCacheKey {
    /// Font face ID.
    pub font_id: cosmic_text::fontdb::ID,
    /// Glyph index.
    pub glyph_id: u16,
}

impl SdfCacheKey {
    /// Create from a cosmic-text cache key (discards size/subpixel info).
    pub fn from_cache_key(key: &cosmic_text::CacheKey) -> Self {
        Self {
            font_id: key.font_id,
            glyph_id: key.glyph_id,
        }
    }
}

/// SDF atlas entry with scaling metadata.
#[derive(Debug, Clone, Copy)]
pub struct SdfAtlasEntry {
    /// Atlas position and size.
    pub atlas_entry: AtlasEntry,
    /// Glyph placement at the base rasterization size.
    pub placement: GlyphPlacement,
    /// The base size this glyph was rasterized at.
    pub base_size: f32,
    /// The SDF spread used during generation.
    pub spread: f32,
}

/// SDF glyph atlas.
pub struct SdfAtlas {
    /// CPU-side atlas data.
    pub data: Vec<u8>,
    /// Atlas dimensions.
    pub size: u32,
    /// Glyph entries keyed by size-independent key.
    pub entries: HashMap<SdfCacheKey, SdfAtlasEntry>,
    /// Rectangle packer.
    pub packer: AtlasPacker,
    /// Whether the atlas data has been modified since last GPU upload.
    pub dirty: bool,
}

impl SdfAtlas {
    /// Create a new SDF atlas of the given size.
    pub fn new(size: u32) -> Self {
        Self {
            data: vec![0u8; (size * size) as usize],
            size,
            entries: HashMap::new(),
            packer: AtlasPacker::new(size, size),
            dirty: false,
        }
    }

    /// Insert an SDF glyph into the atlas.
    ///
    /// Returns `None` if the atlas is full.
    pub fn insert(
        &mut self,
        cache_key: SdfCacheKey,
        sdf_data: &[u8],
        width: u32,
        height: u32,
        placement: GlyphPlacement,
        base_size: f32,
        spread: f32,
    ) -> Option<SdfAtlasEntry> {
        if let Some(entry) = self.entries.get(&cache_key) {
            return Some(*entry);
        }

        let (x, y) = self.packer.allocate(width, height)?;

        // Copy SDF data into atlas
        for row in 0..height {
            let src_start = (row * width) as usize;
            let dst_start = ((y + row) * self.size + x) as usize;
            let len = width as usize;

            if src_start + len <= sdf_data.len() && dst_start + len <= self.data.len() {
                self.data[dst_start..dst_start + len]
                    .copy_from_slice(&sdf_data[src_start..src_start + len]);
            }
        }

        let atlas_entry = AtlasEntry::new(x, y, width, height);
        let sdf_entry = SdfAtlasEntry {
            atlas_entry,
            placement,
            base_size,
            spread,
        };

        self.entries.insert(cache_key, sdf_entry);
        self.dirty = true;

        Some(sdf_entry)
    }

    /// Look up an existing SDF glyph entry.
    pub fn get(&self, cache_key: &SdfCacheKey) -> Option<&SdfAtlasEntry> {
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
