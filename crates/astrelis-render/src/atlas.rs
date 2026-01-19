//! Texture atlas with non-uniform rectangle packing.
//!
//! Provides efficient texture packing for UI elements, sprites, and other 2D graphics.
//!
//! # Example
//!
//! ```ignore
//! use astrelis_render::{TextureAtlas, GraphicsContext};
//!
//! let context = GraphicsContext::new_owned_sync();
//! let mut atlas = TextureAtlas::new(context.clone(), 512, wgpu::TextureFormat::Rgba8UnormSrgb);
//!
//! // Insert images
//! let key1 = AtlasKey::new("icon1");
//! if let Some(entry) = atlas.insert(key1, &image_data, Vec2::new(32.0, 32.0)) {
//!     println!("Inserted at UV: {:?}", entry.uv_rect);
//! }
//!
//! // Upload to GPU
//! atlas.upload(&context);
//!
//! // Retrieve UV coordinates
//! if let Some(entry) = atlas.get(&key1) {
//!     // Use entry.uv_rect for rendering
//! }
//! ```

use crate::GraphicsContext;
use ahash::HashMap;
use astrelis_core::geometry::Rect as GenericRect;
use std::sync::Arc;

/// Rectangle type for atlas packing.
type Rect = GenericRect<f32>;

/// Unique key for an atlas entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AtlasKey(u64);

impl AtlasKey {
    /// Create a new atlas key from a string.
    pub fn new(s: &str) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        Self(hasher.finish())
    }

    /// Create an atlas key from a u64.
    pub fn from_u64(id: u64) -> Self {
        Self(id)
    }

    /// Get the raw u64 value.
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// An entry in the texture atlas.
#[derive(Debug, Clone, Copy)]
pub struct AtlasEntry {
    /// Rectangle in pixel coordinates within the atlas.
    pub rect: Rect,
    /// Rectangle in normalized UV coordinates (0.0 to 1.0).
    pub uv_rect: Rect,
}

impl AtlasEntry {
    /// Create a new atlas entry.
    pub fn new(rect: Rect, atlas_size: f32) -> Self {
        let uv_rect = Rect {
            x: rect.x / atlas_size,
            y: rect.y / atlas_size,
            width: rect.width / atlas_size,
            height: rect.height / atlas_size,
        };

        Self { rect, uv_rect }
    }
}

/// Rectangle packing algorithm.
#[derive(Debug, Clone)]
enum PackerNode {
    /// Empty node that can be split.
    Empty {
        rect: Rect,
    },
    /// Filled node with an entry.
    Filled {
        rect: Rect,
        key: AtlasKey,
    },
    /// Split node with two children.
    Split {
        rect: Rect,
        left: Box<PackerNode>,
        right: Box<PackerNode>,
    },
}

impl PackerNode {
    /// Create a new empty node.
    fn new(rect: Rect) -> Self {
        Self::Empty { rect }
    }

    /// Try to insert a rectangle into this node.
    fn insert(&mut self, key: AtlasKey, width: f32, height: f32) -> Option<Rect> {
        match self {
            PackerNode::Empty { rect } => {
                // Check if the rectangle fits
                if width > rect.width || height > rect.height {
                    return None;
                }

                // Perfect fit
                if width == rect.width && height == rect.height {
                    let result = *rect;
                    *self = PackerNode::Filled { rect: *rect, key };
                    return Some(result);
                }

                // Split the node
                let rect_copy = *rect;

                // Decide whether to split horizontally or vertically
                let horizontal_waste = rect.width - width;
                let vertical_waste = rect.height - height;

                let (left_rect, right_rect) = if horizontal_waste > vertical_waste {
                    // Split horizontally (left/right)
                    (
                        Rect {
                            x: rect.x,
                            y: rect.y,
                            width,
                            height: rect.height,
                        },
                        Rect {
                            x: rect.x + width,
                            y: rect.y,
                            width: rect.width - width,
                            height: rect.height,
                        },
                    )
                } else {
                    // Split vertically (top/bottom)
                    (
                        Rect {
                            x: rect.x,
                            y: rect.y,
                            width: rect.width,
                            height,
                        },
                        Rect {
                            x: rect.x,
                            y: rect.y + height,
                            width: rect.width,
                            height: rect.height - height,
                        },
                    )
                };

                let mut left = Box::new(PackerNode::new(left_rect));
                let right = Box::new(PackerNode::new(right_rect));

                // Insert into the left node
                let result = left.insert(key, width, height);

                *self = PackerNode::Split {
                    rect: rect_copy,
                    left,
                    right,
                };

                result
            }
            PackerNode::Filled { .. } => None,
            PackerNode::Split { left, right, .. } => {
                // Try left first, then right
                left.insert(key, width, height)
                    .or_else(|| right.insert(key, width, height))
            }
        }
    }
}

/// Texture atlas with dynamic rectangle packing.
pub struct TextureAtlas {
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    entries: HashMap<AtlasKey, AtlasEntry>,
    packer: PackerNode,
    format: wgpu::TextureFormat,
    size: u32,
    context: Arc<GraphicsContext>,
    /// Pending uploads (key, data, rect)
    pending_uploads: Vec<(AtlasKey, Vec<u8>, Rect)>,
    dirty: bool,
}

impl TextureAtlas {
    /// Create a new texture atlas.
    ///
    /// # Arguments
    ///
    /// * `context` - Graphics context
    /// * `size` - Size of the atlas texture (must be power of 2)
    /// * `format` - Texture format
    pub fn new(context: Arc<GraphicsContext>, size: u32, format: wgpu::TextureFormat) -> Self {
        let texture = context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("TextureAtlas"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let packer = PackerNode::new(Rect {
            x: 0.0,
            y: 0.0,
            width: size as f32,
            height: size as f32,
        });

        Self {
            texture,
            texture_view,
            entries: HashMap::default(),
            packer,
            format,
            size,
            context,
            pending_uploads: Vec::new(),
            dirty: false,
        }
    }

    /// Insert an image into the atlas.
    ///
    /// Returns the atlas entry if the image was successfully inserted.
    /// Returns None if there's no space in the atlas.
    ///
    /// # Arguments
    ///
    /// * `key` - Unique key for this image
    /// * `image_data` - Raw pixel data (must match atlas format)
    /// * `size` - Size of the image in pixels
    pub fn insert(
        &mut self,
        key: AtlasKey,
        image_data: &[u8],
        width: u32,
        height: u32,
    ) -> Option<AtlasEntry> {
        // Check if already exists
        if let Some(entry) = self.entries.get(&key) {
            return Some(*entry);
        }

        // Try to pack the rectangle
        let rect = self.packer.insert(key, width as f32, height as f32)?;

        // Create entry
        let entry = AtlasEntry::new(rect, self.size as f32);
        self.entries.insert(key, entry);

        // Queue upload
        self.pending_uploads
            .push((key, image_data.to_vec(), rect));
        self.dirty = true;

        Some(entry)
    }

    /// Get an atlas entry by key.
    pub fn get(&self, key: &AtlasKey) -> Option<&AtlasEntry> {
        self.entries.get(key)
    }

    /// Check if the atlas contains a key.
    pub fn contains(&self, key: &AtlasKey) -> bool {
        self.entries.contains_key(key)
    }

    /// Upload all pending data to the GPU.
    pub fn upload(&mut self) {
        if !self.dirty {
            return;
        }

        for (_, data, rect) in &self.pending_uploads {
            let bytes_per_pixel = match self.format {
                wgpu::TextureFormat::Rgba8UnormSrgb | wgpu::TextureFormat::Rgba8Unorm => 4,
                wgpu::TextureFormat::Bgra8UnormSrgb | wgpu::TextureFormat::Bgra8Unorm => 4,
                wgpu::TextureFormat::R8Unorm => 1,
                _ => 4, // Default to 4 bytes
            };

            self.context.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: rect.x as u32,
                        y: rect.y as u32,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(rect.width as u32 * bytes_per_pixel),
                    rows_per_image: Some(rect.height as u32),
                },
                wgpu::Extent3d {
                    width: rect.width as u32,
                    height: rect.height as u32,
                    depth_or_array_layers: 1,
                },
            );
        }

        self.pending_uploads.clear();
        self.dirty = false;
    }

    /// Get the texture view for binding.
    pub fn texture_view(&self) -> &wgpu::TextureView {
        &self.texture_view
    }

    /// Get the texture for advanced use cases.
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    /// Get the size of the atlas.
    pub fn size(&self) -> u32 {
        self.size
    }

    /// Get the texture format.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Get the number of entries in the atlas.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the atlas is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries from the atlas.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.pending_uploads.clear();
        self.packer = PackerNode::new(Rect {
            x: 0.0,
            y: 0.0,
            width: self.size as f32,
            height: self.size as f32,
        });
        self.dirty = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atlas_key() {
        let key1 = AtlasKey::new("test");
        let key2 = AtlasKey::new("test");
        let key3 = AtlasKey::new("other");

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_atlas_entry_uv() {
        let rect = Rect {
            x: 0.0,
            y: 0.0,
            width: 64.0,
            height: 64.0,
        };
        let entry = AtlasEntry::new(rect, 256.0);

        assert_eq!(entry.uv_rect.x, 0.0);
        assert_eq!(entry.uv_rect.y, 0.0);
        assert_eq!(entry.uv_rect.width, 0.25);
        assert_eq!(entry.uv_rect.height, 0.25);
    }

    #[test]
    fn test_packer_insertion() {
        let mut packer = PackerNode::new(Rect {
            x: 0.0,
            y: 0.0,
            width: 256.0,
            height: 256.0,
        });

        let key1 = AtlasKey::new("rect1");
        let rect1 = packer.insert(key1, 64.0, 64.0);
        assert!(rect1.is_some());

        let key2 = AtlasKey::new("rect2");
        let rect2 = packer.insert(key2, 32.0, 32.0);
        assert!(rect2.is_some());

        // Try to insert something too large
        let key3 = AtlasKey::new("rect3");
        let rect3 = packer.insert(key3, 512.0, 512.0);
        assert!(rect3.is_none());
    }

    #[test]
    fn test_atlas_basic() {
        let context = GraphicsContext::new_owned_sync();
        let mut atlas = TextureAtlas::new(context, 256, wgpu::TextureFormat::Rgba8UnormSrgb);

        assert_eq!(atlas.size(), 256);
        assert_eq!(atlas.len(), 0);
        assert!(atlas.is_empty());

        // Create a 32x32 red square
        let mut image_data = vec![0u8; 32 * 32 * 4];
        for i in 0..(32 * 32) {
            image_data[i * 4] = 255; // R
            image_data[i * 4 + 1] = 0; // G
            image_data[i * 4 + 2] = 0; // B
            image_data[i * 4 + 3] = 255; // A
        }

        let key = AtlasKey::new("red_square");
        let entry = atlas.insert(key, &image_data, 32, 32);
        assert!(entry.is_some());
        assert_eq!(atlas.len(), 1);

        // Check retrieval
        let retrieved = atlas.get(&key);
        assert!(retrieved.is_some());

        // Upload to GPU
        atlas.upload();
    }

    #[test]
    fn test_atlas_multiple_inserts() {
        let context = GraphicsContext::new_owned_sync();
        let mut atlas = TextureAtlas::new(context, 256, wgpu::TextureFormat::Rgba8UnormSrgb);

        // Insert multiple images
        for i in 0..10 {
            let image_data = vec![0u8; 16 * 16 * 4];
            let key = AtlasKey::new(&format!("image_{}", i));
            let entry = atlas.insert(key, &image_data, 16, 16);
            assert!(entry.is_some());
        }

        assert_eq!(atlas.len(), 10);
        atlas.upload();
    }

    #[test]
    fn test_atlas_duplicate_key() {
        let context = GraphicsContext::new_owned_sync();
        let mut atlas = TextureAtlas::new(context, 256, wgpu::TextureFormat::Rgba8UnormSrgb);

        let image_data = vec![0u8; 32 * 32 * 4];
        let key = AtlasKey::new("duplicate");

        let entry1 = atlas.insert(key, &image_data, 32, 32);
        assert!(entry1.is_some());

        let entry2 = atlas.insert(key, &image_data, 32, 32);
        assert!(entry2.is_some());

        // Should only have one entry
        assert_eq!(atlas.len(), 1);
    }

    #[test]
    fn test_atlas_clear() {
        let context = GraphicsContext::new_owned_sync();
        let mut atlas = TextureAtlas::new(context, 256, wgpu::TextureFormat::Rgba8UnormSrgb);

        let image_data = vec![0u8; 32 * 32 * 4];
        let key = AtlasKey::new("test");
        atlas.insert(key, &image_data, 32, 32);

        assert_eq!(atlas.len(), 1);

        atlas.clear();

        assert_eq!(atlas.len(), 0);
        assert!(atlas.is_empty());
    }
}
