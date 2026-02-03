//! Sampler cache for efficient GPU sampler reuse.
//!
//! Creating GPU samplers is expensive. This module provides a cache
//! that reuses samplers with identical descriptors.

use astrelis_core::profiling::profile_function;
use ahash::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, RwLock};

/// A hashable key for sampler descriptors.
///
/// wgpu::SamplerDescriptor doesn't implement Hash, so we need this wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SamplerKey {
    /// Address mode for U coordinate
    pub address_mode_u: wgpu::AddressMode,
    /// Address mode for V coordinate
    pub address_mode_v: wgpu::AddressMode,
    /// Address mode for W coordinate
    pub address_mode_w: wgpu::AddressMode,
    /// Magnification filter
    pub mag_filter: wgpu::FilterMode,
    /// Minification filter
    pub min_filter: wgpu::FilterMode,
    /// Mipmap filter
    pub mipmap_filter: wgpu::FilterMode,
    /// Minimum LOD clamp
    pub lod_min_clamp: u32, // f32 bits
    /// Maximum LOD clamp
    pub lod_max_clamp: u32, // f32 bits
    /// Comparison function (if any)
    pub compare: Option<wgpu::CompareFunction>,
    /// Anisotropy clamp (1-16)
    pub anisotropy_clamp: u16,
    /// Border color (for ClampToBorder address mode)
    pub border_color: Option<wgpu::SamplerBorderColor>,
}

impl Hash for SamplerKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.address_mode_u.hash(state);
        self.address_mode_v.hash(state);
        self.address_mode_w.hash(state);
        self.mag_filter.hash(state);
        self.min_filter.hash(state);
        self.mipmap_filter.hash(state);
        self.lod_min_clamp.hash(state);
        self.lod_max_clamp.hash(state);
        self.compare.hash(state);
        self.anisotropy_clamp.hash(state);
        self.border_color.hash(state);
    }
}

impl SamplerKey {
    /// Create a key for a repeating nearest (point) sampler.
    pub fn nearest_repeat() -> Self {
        Self {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: 0.0f32.to_bits(),
            lod_max_clamp: f32::MAX.to_bits(),
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        }
    }

    /// Create a key for a mirrored linear sampler.
    pub fn linear_mirror() -> Self {
        Self {
            address_mode_u: wgpu::AddressMode::MirrorRepeat,
            address_mode_v: wgpu::AddressMode::MirrorRepeat,
            address_mode_w: wgpu::AddressMode::MirrorRepeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            lod_min_clamp: 0.0f32.to_bits(),
            lod_max_clamp: f32::MAX.to_bits(),
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        }
    }

    /// Create a key for a mirrored nearest (point) sampler.
    pub fn nearest_mirror() -> Self {
        Self {
            address_mode_u: wgpu::AddressMode::MirrorRepeat,
            address_mode_v: wgpu::AddressMode::MirrorRepeat,
            address_mode_w: wgpu::AddressMode::MirrorRepeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: 0.0f32.to_bits(),
            lod_max_clamp: f32::MAX.to_bits(),
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        }
    }

    /// Create a key from a sampler descriptor.
    pub fn from_descriptor(desc: &wgpu::SamplerDescriptor) -> Self {
        Self {
            address_mode_u: desc.address_mode_u,
            address_mode_v: desc.address_mode_v,
            address_mode_w: desc.address_mode_w,
            mag_filter: desc.mag_filter,
            min_filter: desc.min_filter,
            mipmap_filter: desc.mipmap_filter,
            lod_min_clamp: desc.lod_min_clamp.to_bits(),
            lod_max_clamp: desc.lod_max_clamp.to_bits(),
            compare: desc.compare,
            anisotropy_clamp: desc.anisotropy_clamp,
            border_color: desc.border_color,
        }
    }

    /// Create a descriptor from this key.
    pub fn to_descriptor<'a>(&self, label: Option<&'a str>) -> wgpu::SamplerDescriptor<'a> {
        wgpu::SamplerDescriptor {
            label,
            address_mode_u: self.address_mode_u,
            address_mode_v: self.address_mode_v,
            address_mode_w: self.address_mode_w,
            mag_filter: self.mag_filter,
            min_filter: self.min_filter,
            mipmap_filter: self.mipmap_filter,
            lod_min_clamp: f32::from_bits(self.lod_min_clamp),
            lod_max_clamp: f32::from_bits(self.lod_max_clamp),
            compare: self.compare,
            anisotropy_clamp: self.anisotropy_clamp,
            border_color: self.border_color,
        }
    }

    /// Create a key for a default linear sampler.
    pub fn linear() -> Self {
        Self {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            lod_min_clamp: 0.0f32.to_bits(),
            lod_max_clamp: f32::MAX.to_bits(),
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        }
    }

    /// Create a key for a default nearest (point) sampler.
    pub fn nearest() -> Self {
        Self {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: 0.0f32.to_bits(),
            lod_max_clamp: f32::MAX.to_bits(),
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        }
    }

    /// Create a key for a repeating linear sampler.
    pub fn linear_repeat() -> Self {
        Self {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            lod_min_clamp: 0.0f32.to_bits(),
            lod_max_clamp: f32::MAX.to_bits(),
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        }
    }
}

/// Sampling mode for image textures.
///
/// This is a user-friendly enum for selecting common texture sampling configurations.
/// It maps to underlying `SamplerKey` configurations for cache lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ImageSampling {
    /// Smooth bilinear filtering (default). Good for photos and gradients.
    #[default]
    Linear,
    /// Pixel-perfect nearest-neighbor filtering. Ideal for pixel art.
    Nearest,
    /// Linear filtering with UV wrapping (repeat). For tiled textures.
    LinearRepeat,
    /// Nearest filtering with UV wrapping. For tiled pixel art.
    NearestRepeat,
    /// Linear filtering with mirrored UV wrapping.
    LinearMirror,
    /// Nearest filtering with mirrored UV wrapping.
    NearestMirror,
}

impl ImageSampling {
    /// Convert to a SamplerKey for cache lookup.
    pub fn to_sampler_key(&self) -> SamplerKey {
        match self {
            Self::Linear => SamplerKey::linear(),
            Self::Nearest => SamplerKey::nearest(),
            Self::LinearRepeat => SamplerKey::linear_repeat(),
            Self::NearestRepeat => SamplerKey::nearest_repeat(),
            Self::LinearMirror => SamplerKey::linear_mirror(),
            Self::NearestMirror => SamplerKey::nearest_mirror(),
        }
    }
}

/// A thread-safe cache of GPU samplers.
///
/// This cache ensures that identical sampler configurations share the same
/// GPU sampler object, reducing memory usage and creation overhead.
///
/// # Example
///
/// ```ignore
/// use astrelis_render::{SamplerCache, SamplerKey};
///
/// let cache = SamplerCache::new();
///
/// // Get or create a linear sampler
/// let sampler = cache.get_or_create(&device, SamplerKey::linear());
///
/// // The same sampler is returned for identical keys
/// let sampler2 = cache.get_or_create(&device, SamplerKey::linear());
/// // sampler and sampler2 point to the same GPU sampler
/// ```
pub struct SamplerCache {
    cache: RwLock<HashMap<SamplerKey, Arc<wgpu::Sampler>>>,
}

impl Default for SamplerCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SamplerCache {
    /// Create a new empty sampler cache.
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::default()),
        }
    }

    /// Get a sampler from the cache or create a new one.
    ///
    /// If a sampler with the given key already exists in the cache,
    /// it is returned. Otherwise, a new sampler is created and cached.
    ///
    /// # Panics
    /// Panics if the internal RwLock is poisoned (another thread panicked while holding the lock).
    pub fn get_or_create(&self, device: &wgpu::Device, key: SamplerKey) -> Arc<wgpu::Sampler> {
        profile_function!();
        // Try read lock first (fast path)
        {
            let cache = self.cache.read()
                .expect("SamplerCache lock poisoned - a thread panicked while accessing the cache");
            if let Some(sampler) = cache.get(&key) {
                return Arc::clone(sampler);
            }
        }

        // Slow path: create sampler and insert
        let mut cache = self.cache.write()
            .expect("SamplerCache lock poisoned - a thread panicked while accessing the cache");

        // Double-check in case another thread inserted while we waited
        if let Some(sampler) = cache.get(&key) {
            return Arc::clone(sampler);
        }

        // Create the sampler
        let descriptor = key.to_descriptor(Some("Cached Sampler"));
        let sampler = Arc::new(device.create_sampler(&descriptor));
        cache.insert(key, Arc::clone(&sampler));
        sampler
    }

    /// Get a sampler from the cache or create one using a descriptor.
    ///
    /// This is a convenience method that converts the descriptor to a key.
    pub fn get_or_create_from_descriptor(
        &self,
        device: &wgpu::Device,
        descriptor: &wgpu::SamplerDescriptor,
    ) -> Arc<wgpu::Sampler> {
        let key = SamplerKey::from_descriptor(descriptor);
        self.get_or_create(device, key)
    }

    /// Get a default linear sampler.
    pub fn linear(&self, device: &wgpu::Device) -> Arc<wgpu::Sampler> {
        self.get_or_create(device, SamplerKey::linear())
    }

    /// Get a default nearest (point) sampler.
    pub fn nearest(&self, device: &wgpu::Device) -> Arc<wgpu::Sampler> {
        self.get_or_create(device, SamplerKey::nearest())
    }

    /// Get a repeating linear sampler.
    pub fn linear_repeat(&self, device: &wgpu::Device) -> Arc<wgpu::Sampler> {
        self.get_or_create(device, SamplerKey::linear_repeat())
    }

    /// Get a repeating nearest sampler.
    pub fn nearest_repeat(&self, device: &wgpu::Device) -> Arc<wgpu::Sampler> {
        self.get_or_create(device, SamplerKey::nearest_repeat())
    }

    /// Get a mirrored linear sampler.
    pub fn linear_mirror(&self, device: &wgpu::Device) -> Arc<wgpu::Sampler> {
        self.get_or_create(device, SamplerKey::linear_mirror())
    }

    /// Get a mirrored nearest sampler.
    pub fn nearest_mirror(&self, device: &wgpu::Device) -> Arc<wgpu::Sampler> {
        self.get_or_create(device, SamplerKey::nearest_mirror())
    }

    /// Get a sampler for the given sampling mode.
    pub fn from_sampling(&self, device: &wgpu::Device, sampling: ImageSampling) -> Arc<wgpu::Sampler> {
        self.get_or_create(device, sampling.to_sampler_key())
    }

    /// Get the number of cached samplers.
    ///
    /// # Panics
    /// Panics if the internal RwLock is poisoned.
    pub fn len(&self) -> usize {
        self.cache.read()
            .expect("SamplerCache lock poisoned")
            .len()
    }

    /// Check if the cache is empty.
    ///
    /// # Panics
    /// Panics if the internal RwLock is poisoned.
    pub fn is_empty(&self) -> bool {
        self.cache.read()
            .expect("SamplerCache lock poisoned")
            .is_empty()
    }

    /// Clear the cache, releasing all cached samplers.
    ///
    /// This should only be called when you're sure no references to
    /// cached samplers are still in use.
    ///
    /// # Panics
    /// Panics if the internal RwLock is poisoned.
    pub fn clear(&self) {
        self.cache.write()
            .expect("SamplerCache lock poisoned")
            .clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sampler_key_hash_equality() {
        let key1 = SamplerKey::linear();
        let key2 = SamplerKey::linear();
        let key3 = SamplerKey::nearest();

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);

        // Test hashing
        use std::collections::hash_map::DefaultHasher;
        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();
        key1.hash(&mut hasher1);
        key2.hash(&mut hasher2);
        assert_eq!(hasher1.finish(), hasher2.finish());
    }

    #[test]
    fn test_sampler_key_roundtrip() {
        let key = SamplerKey::linear();
        let desc = key.to_descriptor(Some("Test"));
        let key2 = SamplerKey::from_descriptor(&desc);
        assert_eq!(key, key2);
    }
}
