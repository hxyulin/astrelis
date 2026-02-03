//! GPU feature detection and management.
//!
//! This module provides a type-safe wrapper around wgpu features with support for
//! required vs requested features.

use bitflags::bitflags;

bitflags! {
    /// GPU features that can be requested or required.
    ///
    /// These map to common wgpu features that are useful for rendering applications.
    /// Use `GpuFeatures::to_wgpu()` to convert to `wgpu::Features`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct GpuFeatures: u32 {
        /// Allows the use of `first_instance` parameter in indirect draw calls.
        /// Required for GPU-driven rendering with indirect buffers.
        const INDIRECT_FIRST_INSTANCE = 1 << 0;

        /// Allows `multi_draw_indirect_count` for GPU-driven draw count.
        /// In wgpu 27+, `multi_draw_indirect()` requires no feature flag
        /// (only `DownlevelFlags::INDIRECT_EXECUTION`), but the count variant
        /// (`multi_draw_indirect_count`) requires this feature.
        const MULTI_DRAW_INDIRECT_COUNT = 1 << 1;

        /// Allows push constants in shaders for small, frequently updated data.
        /// More efficient than uniform buffers for small amounts of per-draw data.
        const PUSH_CONSTANTS = 1 << 2;

        /// BC texture compression (DXT1, DXT3, DXT5).
        /// Common on desktop platforms, reduces texture memory usage.
        const TEXTURE_COMPRESSION_BC = 1 << 3;

        /// Allows disabling depth clipping in the rasterizer.
        /// Useful for certain shadow mapping techniques.
        const DEPTH_CLIP_CONTROL = 1 << 4;

        /// 16-bit floating point support in shaders.
        /// Can improve performance on some GPUs for certain workloads.
        const SHADER_F16 = 1 << 5;

        /// Allows non-zero `first_vertex` and `first_instance` in indirect draw calls.
        /// Note: This is a subset of INDIRECT_FIRST_INSTANCE on some platforms.
        const INDIRECT_FIRST_VERTEX = 1 << 6;

        /// Polygon mode: line (wireframe rendering).
        const POLYGON_MODE_LINE = 1 << 7;

        /// Polygon mode: point.
        const POLYGON_MODE_POINT = 1 << 8;

        /// Conservative rasterization for better triangle coverage.
        const CONSERVATIVE_RASTERIZATION = 1 << 9;

        /// Texture binding arrays (bindless textures).
        /// Enables more flexible texture access patterns in shaders.
        const TEXTURE_BINDING_ARRAY = 1 << 10;

        /// Sampled texture and storage buffer binding arrays.
        const BUFFER_BINDING_ARRAY = 1 << 11;

        /// Storage resource binding arrays with dynamic indexing.
        const STORAGE_RESOURCE_BINDING_ARRAY = 1 << 12;

        /// Partially bound binding arrays.
        /// Allows leaving some array slots unbound.
        const PARTIALLY_BOUND_BINDING_ARRAY = 1 << 13;

        /// 32-bit floating point texture filtering.
        const FLOAT32_FILTERABLE = 1 << 14;

        /// RG11B10 unsigned floating point render target format.
        const RG11B10UFLOAT_RENDERABLE = 1 << 15;

        /// BGRA8 unorm storage texture support.
        const BGRA8UNORM_STORAGE = 1 << 16;

        /// Timestamp queries for GPU profiling.
        /// Allows measuring GPU execution time with high precision.
        /// Alone, this only allows timestamp writes on pass definition
        /// (via `timestamp_writes` in render/compute pass descriptors).
        const TIMESTAMP_QUERY = 1 << 17;

        /// Allows timestamp write commands at arbitrary points within command encoders.
        /// Implies `TIMESTAMP_QUERY` is supported.
        /// Required by `wgpu-profiler` for scopes on command encoders.
        const TIMESTAMP_QUERY_INSIDE_ENCODERS = 1 << 19;

        /// Allows timestamp write commands at arbitrary points within render/compute passes.
        /// Implies `TIMESTAMP_QUERY` and `TIMESTAMP_QUERY_INSIDE_ENCODERS` are supported.
        /// Required by `wgpu-profiler` for scopes on render/compute passes.
        const TIMESTAMP_QUERY_INSIDE_PASSES = 1 << 20;

        /// Non-uniform indexing of sampled texture and storage buffer binding arrays.
        /// Required for dynamic indexing into `binding_array<texture_2d<f32>>` with
        /// values that are not uniform across a draw call (e.g., per-instance texture index).
        const SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING = 1 << 18;
    }
}

impl GpuFeatures {
    /// Convert to wgpu::Features.
    pub fn to_wgpu(self) -> wgpu::Features {
        let mut features = wgpu::Features::empty();

        if self.contains(GpuFeatures::INDIRECT_FIRST_INSTANCE) {
            features |= wgpu::Features::INDIRECT_FIRST_INSTANCE;
        }
        if self.contains(GpuFeatures::MULTI_DRAW_INDIRECT_COUNT) {
            features |= wgpu::Features::MULTI_DRAW_INDIRECT_COUNT;
        }
        if self.contains(GpuFeatures::PUSH_CONSTANTS) {
            features |= wgpu::Features::PUSH_CONSTANTS;
        }
        if self.contains(GpuFeatures::TEXTURE_COMPRESSION_BC) {
            features |= wgpu::Features::TEXTURE_COMPRESSION_BC;
        }
        if self.contains(GpuFeatures::DEPTH_CLIP_CONTROL) {
            features |= wgpu::Features::DEPTH_CLIP_CONTROL;
        }
        if self.contains(GpuFeatures::SHADER_F16) {
            features |= wgpu::Features::SHADER_F16;
        }
        if self.contains(GpuFeatures::POLYGON_MODE_LINE) {
            features |= wgpu::Features::POLYGON_MODE_LINE;
        }
        if self.contains(GpuFeatures::POLYGON_MODE_POINT) {
            features |= wgpu::Features::POLYGON_MODE_POINT;
        }
        if self.contains(GpuFeatures::CONSERVATIVE_RASTERIZATION) {
            features |= wgpu::Features::CONSERVATIVE_RASTERIZATION;
        }
        if self.contains(GpuFeatures::TEXTURE_BINDING_ARRAY) {
            features |= wgpu::Features::TEXTURE_BINDING_ARRAY;
        }
        if self.contains(GpuFeatures::BUFFER_BINDING_ARRAY) {
            features |= wgpu::Features::BUFFER_BINDING_ARRAY;
        }
        if self.contains(GpuFeatures::STORAGE_RESOURCE_BINDING_ARRAY) {
            features |= wgpu::Features::STORAGE_RESOURCE_BINDING_ARRAY;
        }
        if self.contains(GpuFeatures::PARTIALLY_BOUND_BINDING_ARRAY) {
            features |= wgpu::Features::PARTIALLY_BOUND_BINDING_ARRAY;
        }
        if self.contains(GpuFeatures::FLOAT32_FILTERABLE) {
            features |= wgpu::Features::FLOAT32_FILTERABLE;
        }
        if self.contains(GpuFeatures::RG11B10UFLOAT_RENDERABLE) {
            features |= wgpu::Features::RG11B10UFLOAT_RENDERABLE;
        }
        if self.contains(GpuFeatures::BGRA8UNORM_STORAGE) {
            features |= wgpu::Features::BGRA8UNORM_STORAGE;
        }
        if self.contains(GpuFeatures::TIMESTAMP_QUERY) {
            features |= wgpu::Features::TIMESTAMP_QUERY;
        }
        if self.contains(GpuFeatures::TIMESTAMP_QUERY_INSIDE_ENCODERS) {
            features |= wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
        }
        if self.contains(GpuFeatures::TIMESTAMP_QUERY_INSIDE_PASSES) {
            features |= wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES;
        }
        if self.contains(GpuFeatures::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING) {
            features |= wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING;
        }

        features
    }

    /// Convert from wgpu::Features to GpuFeatures.
    ///
    /// Note: Only features that have a corresponding GpuFeatures flag will be included.
    pub fn from_wgpu(features: wgpu::Features) -> Self {
        let mut gpu_features = GpuFeatures::empty();

        if features.contains(wgpu::Features::INDIRECT_FIRST_INSTANCE) {
            gpu_features |= GpuFeatures::INDIRECT_FIRST_INSTANCE;
        }
        if features.contains(wgpu::Features::MULTI_DRAW_INDIRECT_COUNT) {
            gpu_features |= GpuFeatures::MULTI_DRAW_INDIRECT_COUNT;
        }
        if features.contains(wgpu::Features::PUSH_CONSTANTS) {
            gpu_features |= GpuFeatures::PUSH_CONSTANTS;
        }
        if features.contains(wgpu::Features::TEXTURE_COMPRESSION_BC) {
            gpu_features |= GpuFeatures::TEXTURE_COMPRESSION_BC;
        }
        if features.contains(wgpu::Features::DEPTH_CLIP_CONTROL) {
            gpu_features |= GpuFeatures::DEPTH_CLIP_CONTROL;
        }
        if features.contains(wgpu::Features::SHADER_F16) {
            gpu_features |= GpuFeatures::SHADER_F16;
        }
        if features.contains(wgpu::Features::POLYGON_MODE_LINE) {
            gpu_features |= GpuFeatures::POLYGON_MODE_LINE;
        }
        if features.contains(wgpu::Features::POLYGON_MODE_POINT) {
            gpu_features |= GpuFeatures::POLYGON_MODE_POINT;
        }
        if features.contains(wgpu::Features::CONSERVATIVE_RASTERIZATION) {
            gpu_features |= GpuFeatures::CONSERVATIVE_RASTERIZATION;
        }
        if features.contains(wgpu::Features::TEXTURE_BINDING_ARRAY) {
            gpu_features |= GpuFeatures::TEXTURE_BINDING_ARRAY;
        }
        if features.contains(wgpu::Features::BUFFER_BINDING_ARRAY) {
            gpu_features |= GpuFeatures::BUFFER_BINDING_ARRAY;
        }
        if features.contains(wgpu::Features::STORAGE_RESOURCE_BINDING_ARRAY) {
            gpu_features |= GpuFeatures::STORAGE_RESOURCE_BINDING_ARRAY;
        }
        if features.contains(wgpu::Features::PARTIALLY_BOUND_BINDING_ARRAY) {
            gpu_features |= GpuFeatures::PARTIALLY_BOUND_BINDING_ARRAY;
        }
        if features.contains(wgpu::Features::FLOAT32_FILTERABLE) {
            gpu_features |= GpuFeatures::FLOAT32_FILTERABLE;
        }
        if features.contains(wgpu::Features::RG11B10UFLOAT_RENDERABLE) {
            gpu_features |= GpuFeatures::RG11B10UFLOAT_RENDERABLE;
        }
        if features.contains(wgpu::Features::BGRA8UNORM_STORAGE) {
            gpu_features |= GpuFeatures::BGRA8UNORM_STORAGE;
        }
        if features.contains(wgpu::Features::TIMESTAMP_QUERY) {
            gpu_features |= GpuFeatures::TIMESTAMP_QUERY;
        }
        if features.contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS) {
            gpu_features |= GpuFeatures::TIMESTAMP_QUERY_INSIDE_ENCODERS;
        }
        if features.contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES) {
            gpu_features |= GpuFeatures::TIMESTAMP_QUERY_INSIDE_PASSES;
        }
        if features.contains(wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING) {
            gpu_features |= GpuFeatures::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING;
        }

        gpu_features
    }

    /// Check if all the specified features are supported by the adapter.
    pub fn check_support(self, adapter: &wgpu::Adapter) -> FeatureSupportResult {
        let adapter_features = GpuFeatures::from_wgpu(adapter.features());
        let missing = self - (self & adapter_features);

        if missing.is_empty() {
            FeatureSupportResult::Supported
        } else {
            FeatureSupportResult::Missing(missing)
        }
    }
}

impl Default for GpuFeatures {
    fn default() -> Self {
        GpuFeatures::empty()
    }
}

/// Result of checking feature support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureSupportResult {
    /// All requested features are supported.
    Supported,
    /// Some features are missing.
    Missing(GpuFeatures),
}

impl FeatureSupportResult {
    /// Returns true if all features are supported.
    pub fn is_supported(&self) -> bool {
        matches!(self, FeatureSupportResult::Supported)
    }

    /// Returns the missing features, if any.
    pub fn missing(&self) -> Option<GpuFeatures> {
        match self {
            FeatureSupportResult::Supported => None,
            FeatureSupportResult::Missing(features) => Some(*features),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_features_empty() {
        let features = GpuFeatures::empty();
        assert!(features.is_empty());
        assert_eq!(features.to_wgpu(), wgpu::Features::empty());
    }

    #[test]
    fn test_gpu_features_roundtrip() {
        let features = GpuFeatures::INDIRECT_FIRST_INSTANCE
            | GpuFeatures::MULTI_DRAW_INDIRECT_COUNT
            | GpuFeatures::PUSH_CONSTANTS
            | GpuFeatures::TIMESTAMP_QUERY;

        let wgpu_features = features.to_wgpu();
        let back = GpuFeatures::from_wgpu(wgpu_features);

        assert_eq!(features, back);
    }

    #[test]
    fn test_gpu_features_contains() {
        let features = GpuFeatures::INDIRECT_FIRST_INSTANCE | GpuFeatures::PUSH_CONSTANTS;

        assert!(features.contains(GpuFeatures::INDIRECT_FIRST_INSTANCE));
        assert!(features.contains(GpuFeatures::PUSH_CONSTANTS));
        assert!(!features.contains(GpuFeatures::MULTI_DRAW_INDIRECT_COUNT));
    }
}
