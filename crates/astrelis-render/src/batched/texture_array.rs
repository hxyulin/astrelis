//! Texture array management for standard (per-texture) and bindless bind groups.

use ahash::HashMap;

use super::pipeline;
use super::types::TextureSlot2D;

/// Manages texture bind groups for both standard (Tier 1-2) and bindless (Tier 3) paths.
pub struct TextureArray {
    /// Cached per-texture bind groups keyed by texture id. Used by Tier 1-2.
    bind_group_cache: HashMap<u64, wgpu::BindGroup>,
    /// The standard bind group layout (texture + sampler).
    standard_layout: wgpu::BindGroupLayout,
    /// Fallback bind group for solid quads.
    fallback_bind_group: wgpu::BindGroup,
    /// Fallback texture view (1x1 white).
    _fallback_texture: wgpu::Texture,
    _fallback_view: wgpu::TextureView,
    _fallback_sampler: wgpu::Sampler,
}

impl TextureArray {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let standard_layout = pipeline::create_standard_texture_bind_group_layout(device);
        let (fallback_texture, fallback_view, fallback_sampler) =
            pipeline::create_fallback_texture(device, queue);

        let fallback_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("batched_fallback_bg"),
            layout: &standard_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&fallback_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&fallback_sampler),
                },
            ],
        });

        Self {
            bind_group_cache: HashMap::default(),
            standard_layout,
            fallback_bind_group,
            _fallback_texture: fallback_texture,
            _fallback_view: fallback_view,
            _fallback_sampler: fallback_sampler,
        }
    }

    /// Get the standard bind group layout.
    pub fn standard_layout(&self) -> &wgpu::BindGroupLayout {
        &self.standard_layout
    }

    /// Get the fallback bind group for solid quads.
    pub fn fallback_bind_group(&self) -> &wgpu::BindGroup {
        &self.fallback_bind_group
    }

    /// Update standard bind groups for the given textures. Creates bind groups
    /// for any textures not already cached.
    pub fn update_standard(&mut self, device: &wgpu::Device, textures: &[TextureSlot2D]) {
        for slot in textures {
            self.bind_group_cache.entry(slot.id).or_insert_with(|| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("batched_texture_bg"),
                    layout: &self.standard_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&slot.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&slot.sampler),
                        },
                    ],
                })
            });
        }
    }

    /// Get a standard bind group by texture ID.
    pub fn get_standard_bind_group(&self, texture_id: u64) -> Option<&wgpu::BindGroup> {
        self.bind_group_cache.get(&texture_id)
    }

    /// Evict cached bind groups for textures no longer in use.
    pub fn evict(&mut self, active_ids: &[u64]) {
        self.bind_group_cache
            .retain(|id, _| active_ids.contains(id));
    }
}

/// Manages a bindless texture array bind group for Tier 3.
pub struct BindlessTextureArray {
    /// Bind group layout for binding_array.
    layout: wgpu::BindGroupLayout,
    /// Current bind group.
    bind_group: Option<wgpu::BindGroup>,
    /// Current texture IDs in the array (for change detection).
    current_ids: Vec<u64>,
    /// Shared sampler for all textures.
    sampler: wgpu::Sampler,
    /// Fallback texture view.
    _fallback_texture: wgpu::Texture,
    fallback_view: wgpu::TextureView,
    /// Maximum textures supported.
    max_textures: u32,
}

impl BindlessTextureArray {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, max_textures: u32) -> Self {
        let layout = pipeline::create_bindless_texture_bind_group_layout(device, max_textures);
        let (fallback_texture, fallback_view, _) = pipeline::create_fallback_texture(device, queue);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("batched_bindless_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            layout,
            bind_group: None,
            current_ids: Vec::new(),
            sampler,
            _fallback_texture: fallback_texture,
            fallback_view,
            max_textures,
        }
    }

    /// Get the bindless bind group layout.
    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    /// Update the binding array if the texture set has changed.
    pub fn update(&mut self, device: &wgpu::Device, textures: &[TextureSlot2D]) {
        let new_ids: Vec<u64> = textures.iter().map(|s| s.id).collect();
        if new_ids == self.current_ids && self.bind_group.is_some() {
            return;
        }

        // Build texture view references, padding with fallback for unused slots
        let mut views: Vec<&wgpu::TextureView> = Vec::with_capacity(self.max_textures as usize);
        for slot in textures {
            views.push(&slot.view);
        }
        // Fill remaining slots with fallback
        while views.len() < self.max_textures as usize {
            views.push(&self.fallback_view);
        }

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("batched_bindless_bg"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureViewArray(&views),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        self.bind_group = Some(bind_group);
        self.current_ids = new_ids;
    }

    /// Get the current bindless bind group.
    pub fn bind_group(&self) -> Option<&wgpu::BindGroup> {
        self.bind_group.as_ref()
    }
}
