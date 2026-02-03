//! Render capability system for declaring GPU feature/limit requirements.
//!
//! Renderers implement [`RenderCapability`] to declare their GPU requirements.
//! These are collected via [`GraphicsContextDescriptor::require_capability`] and
//! [`GraphicsContextDescriptor::request_capability`] to configure device creation.
//!
//! # Example
//!
//! ```ignore
//! use astrelis_render::{GraphicsContext, GraphicsContextDescriptor, GpuRequirements, RenderCapability};
//! use astrelis_render::batched::BestBatchCapability;
//!
//! let ctx = pollster::block_on(
//!     GraphicsContext::new_owned_with_descriptor(
//!         GraphicsContextDescriptor::new()
//!             .request_capability::<BestBatchCapability>()
//!     )
//! ).unwrap();
//! ```

use crate::features::GpuFeatures;

/// A trait for renderers to declare their GPU feature and limit requirements.
///
/// Implement this on renderer types (or dedicated marker types) so that
/// [`GraphicsContextDescriptor::require_capability`] and
/// [`GraphicsContextDescriptor::request_capability`] can automatically
/// gather the necessary GPU configuration.
pub trait RenderCapability {
    /// GPU requirements (features + limits) for this component.
    fn requirements() -> GpuRequirements;

    /// Human-readable name for diagnostics.
    fn name() -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// GPU requirements for a render capability.
///
/// Contains required features (must be present), requested features (best-effort),
/// additional raw wgpu features, and minimum device limits.
#[derive(Debug, Clone)]
pub struct GpuRequirements {
    /// Features that must be present (device creation fails if missing).
    pub required_features: GpuFeatures,
    /// Features that are desired but not essential (warns if missing).
    pub requested_features: GpuFeatures,
    /// Additional raw wgpu features not covered by [`GpuFeatures`].
    pub additional_wgpu_features: wgpu::Features,
    /// Minimum device limits. Merged with other requirements via field-wise max.
    pub min_limits: wgpu::Limits,
}

impl GpuRequirements {
    /// Create requirements with no features or elevated limits.
    pub fn none() -> Self {
        Self {
            required_features: GpuFeatures::empty(),
            requested_features: GpuFeatures::empty(),
            additional_wgpu_features: wgpu::Features::empty(),
            min_limits: wgpu::Limits::default(),
        }
    }

    /// Create a new requirements builder starting from no requirements.
    pub fn new() -> Self {
        Self::none()
    }

    /// Set required GPU features.
    pub fn require_features(mut self, features: GpuFeatures) -> Self {
        self.required_features |= features;
        self
    }

    /// Set requested (best-effort) GPU features.
    pub fn request_features(mut self, features: GpuFeatures) -> Self {
        self.requested_features |= features;
        self
    }

    /// Set additional raw wgpu features.
    pub fn with_wgpu_features(mut self, features: wgpu::Features) -> Self {
        self.additional_wgpu_features |= features;
        self
    }

    /// Modify minimum limits via a closure.
    ///
    /// # Example
    ///
    /// ```ignore
    /// GpuRequirements::new().with_min_limits(|l| {
    ///     l.max_binding_array_elements_per_shader_stage = 256;
    /// })
    /// ```
    pub fn with_min_limits(mut self, f: impl FnOnce(&mut wgpu::Limits)) -> Self {
        f(&mut self.min_limits);
        self
    }

    /// Merge another set of requirements into this one.
    ///
    /// Features are unioned, limits are merged via field-wise max.
    pub fn merge(&mut self, other: &GpuRequirements) {
        self.required_features |= other.required_features;
        self.requested_features |= other.requested_features;
        self.additional_wgpu_features |= other.additional_wgpu_features;
        merge_limits_max(&mut self.min_limits, &other.min_limits);
    }
}

impl Default for GpuRequirements {
    fn default() -> Self {
        Self::none()
    }
}

/// Merge two [`wgpu::Limits`] by taking the maximum of each "maximum" field
/// and the minimum of each "minimum" field (alignment fields).
///
/// This ensures the merged limits satisfy both sets of requirements.
pub fn merge_limits_max(target: &mut wgpu::Limits, other: &wgpu::Limits) {
    // "Maximum" fields: take the larger value
    target.max_texture_dimension_1d = target.max_texture_dimension_1d.max(other.max_texture_dimension_1d);
    target.max_texture_dimension_2d = target.max_texture_dimension_2d.max(other.max_texture_dimension_2d);
    target.max_texture_dimension_3d = target.max_texture_dimension_3d.max(other.max_texture_dimension_3d);
    target.max_texture_array_layers = target.max_texture_array_layers.max(other.max_texture_array_layers);
    target.max_bind_groups = target.max_bind_groups.max(other.max_bind_groups);
    target.max_bindings_per_bind_group = target.max_bindings_per_bind_group.max(other.max_bindings_per_bind_group);
    target.max_dynamic_uniform_buffers_per_pipeline_layout = target.max_dynamic_uniform_buffers_per_pipeline_layout.max(other.max_dynamic_uniform_buffers_per_pipeline_layout);
    target.max_dynamic_storage_buffers_per_pipeline_layout = target.max_dynamic_storage_buffers_per_pipeline_layout.max(other.max_dynamic_storage_buffers_per_pipeline_layout);
    target.max_sampled_textures_per_shader_stage = target.max_sampled_textures_per_shader_stage.max(other.max_sampled_textures_per_shader_stage);
    target.max_samplers_per_shader_stage = target.max_samplers_per_shader_stage.max(other.max_samplers_per_shader_stage);
    target.max_storage_buffers_per_shader_stage = target.max_storage_buffers_per_shader_stage.max(other.max_storage_buffers_per_shader_stage);
    target.max_storage_textures_per_shader_stage = target.max_storage_textures_per_shader_stage.max(other.max_storage_textures_per_shader_stage);
    target.max_uniform_buffers_per_shader_stage = target.max_uniform_buffers_per_shader_stage.max(other.max_uniform_buffers_per_shader_stage);
    target.max_uniform_buffer_binding_size = target.max_uniform_buffer_binding_size.max(other.max_uniform_buffer_binding_size);
    target.max_storage_buffer_binding_size = target.max_storage_buffer_binding_size.max(other.max_storage_buffer_binding_size);
    target.max_vertex_buffers = target.max_vertex_buffers.max(other.max_vertex_buffers);
    target.max_buffer_size = target.max_buffer_size.max(other.max_buffer_size);
    target.max_vertex_attributes = target.max_vertex_attributes.max(other.max_vertex_attributes);
    target.max_vertex_buffer_array_stride = target.max_vertex_buffer_array_stride.max(other.max_vertex_buffer_array_stride);
    target.max_push_constant_size = target.max_push_constant_size.max(other.max_push_constant_size);
    target.max_compute_workgroup_storage_size = target.max_compute_workgroup_storage_size.max(other.max_compute_workgroup_storage_size);
    target.max_compute_invocations_per_workgroup = target.max_compute_invocations_per_workgroup.max(other.max_compute_invocations_per_workgroup);
    target.max_compute_workgroup_size_x = target.max_compute_workgroup_size_x.max(other.max_compute_workgroup_size_x);
    target.max_compute_workgroup_size_y = target.max_compute_workgroup_size_y.max(other.max_compute_workgroup_size_y);
    target.max_compute_workgroup_size_z = target.max_compute_workgroup_size_z.max(other.max_compute_workgroup_size_z);
    target.max_compute_workgroups_per_dimension = target.max_compute_workgroups_per_dimension.max(other.max_compute_workgroups_per_dimension);
    target.max_binding_array_elements_per_shader_stage = target.max_binding_array_elements_per_shader_stage.max(other.max_binding_array_elements_per_shader_stage);

    // "Minimum" alignment fields: take the smaller value (stricter alignment)
    target.min_uniform_buffer_offset_alignment = target.min_uniform_buffer_offset_alignment.min(other.min_uniform_buffer_offset_alignment);
    target.min_storage_buffer_offset_alignment = target.min_storage_buffer_offset_alignment.min(other.min_storage_buffer_offset_alignment);
}

/// Clamp requested limits to what the adapter actually supports.
///
/// For "maximum" fields, takes `min(requested, adapter)` so we never request
/// more than the adapter can provide. For "minimum" alignment fields, takes
/// `max(requested, adapter)` since the adapter's alignment is a lower bound.
pub fn clamp_limits_to_adapter(requested: &wgpu::Limits, adapter: &wgpu::Limits) -> wgpu::Limits {
    let mut result = requested.clone();

    // "Maximum" fields: don't exceed adapter
    result.max_texture_dimension_1d = result.max_texture_dimension_1d.min(adapter.max_texture_dimension_1d);
    result.max_texture_dimension_2d = result.max_texture_dimension_2d.min(adapter.max_texture_dimension_2d);
    result.max_texture_dimension_3d = result.max_texture_dimension_3d.min(adapter.max_texture_dimension_3d);
    result.max_texture_array_layers = result.max_texture_array_layers.min(adapter.max_texture_array_layers);
    result.max_bind_groups = result.max_bind_groups.min(adapter.max_bind_groups);
    result.max_bindings_per_bind_group = result.max_bindings_per_bind_group.min(adapter.max_bindings_per_bind_group);
    result.max_dynamic_uniform_buffers_per_pipeline_layout = result.max_dynamic_uniform_buffers_per_pipeline_layout.min(adapter.max_dynamic_uniform_buffers_per_pipeline_layout);
    result.max_dynamic_storage_buffers_per_pipeline_layout = result.max_dynamic_storage_buffers_per_pipeline_layout.min(adapter.max_dynamic_storage_buffers_per_pipeline_layout);
    result.max_sampled_textures_per_shader_stage = result.max_sampled_textures_per_shader_stage.min(adapter.max_sampled_textures_per_shader_stage);
    result.max_samplers_per_shader_stage = result.max_samplers_per_shader_stage.min(adapter.max_samplers_per_shader_stage);
    result.max_storage_buffers_per_shader_stage = result.max_storage_buffers_per_shader_stage.min(adapter.max_storage_buffers_per_shader_stage);
    result.max_storage_textures_per_shader_stage = result.max_storage_textures_per_shader_stage.min(adapter.max_storage_textures_per_shader_stage);
    result.max_uniform_buffers_per_shader_stage = result.max_uniform_buffers_per_shader_stage.min(adapter.max_uniform_buffers_per_shader_stage);
    result.max_uniform_buffer_binding_size = result.max_uniform_buffer_binding_size.min(adapter.max_uniform_buffer_binding_size);
    result.max_storage_buffer_binding_size = result.max_storage_buffer_binding_size.min(adapter.max_storage_buffer_binding_size);
    result.max_vertex_buffers = result.max_vertex_buffers.min(adapter.max_vertex_buffers);
    result.max_buffer_size = result.max_buffer_size.min(adapter.max_buffer_size);
    result.max_vertex_attributes = result.max_vertex_attributes.min(adapter.max_vertex_attributes);
    result.max_vertex_buffer_array_stride = result.max_vertex_buffer_array_stride.min(adapter.max_vertex_buffer_array_stride);
    result.max_push_constant_size = result.max_push_constant_size.min(adapter.max_push_constant_size);
    result.max_compute_workgroup_storage_size = result.max_compute_workgroup_storage_size.min(adapter.max_compute_workgroup_storage_size);
    result.max_compute_invocations_per_workgroup = result.max_compute_invocations_per_workgroup.min(adapter.max_compute_invocations_per_workgroup);
    result.max_compute_workgroup_size_x = result.max_compute_workgroup_size_x.min(adapter.max_compute_workgroup_size_x);
    result.max_compute_workgroup_size_y = result.max_compute_workgroup_size_y.min(adapter.max_compute_workgroup_size_y);
    result.max_compute_workgroup_size_z = result.max_compute_workgroup_size_z.min(adapter.max_compute_workgroup_size_z);
    result.max_compute_workgroups_per_dimension = result.max_compute_workgroups_per_dimension.min(adapter.max_compute_workgroups_per_dimension);
    result.max_binding_array_elements_per_shader_stage = result.max_binding_array_elements_per_shader_stage.min(adapter.max_binding_array_elements_per_shader_stage);
    result.max_color_attachments = result.max_color_attachments.min(adapter.max_color_attachments);
    result.max_color_attachment_bytes_per_sample = result.max_color_attachment_bytes_per_sample.min(adapter.max_color_attachment_bytes_per_sample);
    result.max_inter_stage_shader_components = result.max_inter_stage_shader_components.min(adapter.max_inter_stage_shader_components);
    result.max_non_sampler_bindings = result.max_non_sampler_bindings.min(adapter.max_non_sampler_bindings);

    // "Minimum" alignment fields: adapter's value is the floor
    result.min_uniform_buffer_offset_alignment = result.min_uniform_buffer_offset_alignment.max(adapter.min_uniform_buffer_offset_alignment);
    result.min_storage_buffer_offset_alignment = result.min_storage_buffer_offset_alignment.max(adapter.min_storage_buffer_offset_alignment);

    // Subgroup sizes: use adapter values
    result.min_subgroup_size = adapter.min_subgroup_size;
    result.max_subgroup_size = adapter.max_subgroup_size;

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_requirements_none() {
        let req = GpuRequirements::none();
        assert!(req.required_features.is_empty());
        assert!(req.requested_features.is_empty());
        assert!(req.additional_wgpu_features.is_empty());
    }

    #[test]
    fn test_gpu_requirements_builder() {
        let req = GpuRequirements::new()
            .require_features(GpuFeatures::INDIRECT_FIRST_INSTANCE)
            .request_features(GpuFeatures::TIMESTAMP_QUERY)
            .with_min_limits(|l| {
                l.max_binding_array_elements_per_shader_stage = 256;
            });

        assert!(req.required_features.contains(GpuFeatures::INDIRECT_FIRST_INSTANCE));
        assert!(req.requested_features.contains(GpuFeatures::TIMESTAMP_QUERY));
        assert_eq!(req.min_limits.max_binding_array_elements_per_shader_stage, 256);
    }

    #[test]
    fn test_gpu_requirements_merge() {
        let mut a = GpuRequirements::new()
            .require_features(GpuFeatures::INDIRECT_FIRST_INSTANCE)
            .with_min_limits(|l| {
                l.max_binding_array_elements_per_shader_stage = 128;
            });

        let b = GpuRequirements::new()
            .require_features(GpuFeatures::TEXTURE_BINDING_ARRAY)
            .request_features(GpuFeatures::TIMESTAMP_QUERY)
            .with_min_limits(|l| {
                l.max_binding_array_elements_per_shader_stage = 256;
            });

        a.merge(&b);

        assert!(a.required_features.contains(GpuFeatures::INDIRECT_FIRST_INSTANCE));
        assert!(a.required_features.contains(GpuFeatures::TEXTURE_BINDING_ARRAY));
        assert!(a.requested_features.contains(GpuFeatures::TIMESTAMP_QUERY));
        assert_eq!(a.min_limits.max_binding_array_elements_per_shader_stage, 256);
    }

    #[test]
    fn test_merge_limits_max() {
        let mut a = wgpu::Limits::default();
        let mut b = wgpu::Limits::default();

        a.max_texture_dimension_2d = 4096;
        b.max_texture_dimension_2d = 8192;
        b.max_bind_groups = 8;

        merge_limits_max(&mut a, &b);

        assert_eq!(a.max_texture_dimension_2d, 8192);
        assert_eq!(a.max_bind_groups, 8);
    }

    #[test]
    fn test_clamp_limits_to_adapter() {
        let mut requested = wgpu::Limits::default();
        let mut adapter = wgpu::Limits::default();

        // Request more than adapter supports
        requested.max_binding_array_elements_per_shader_stage = 1024;
        adapter.max_binding_array_elements_per_shader_stage = 256;

        // Request less than adapter supports
        requested.max_texture_dimension_2d = 4096;
        adapter.max_texture_dimension_2d = 8192;

        let clamped = clamp_limits_to_adapter(&requested, &adapter);

        // Should be clamped to adapter max
        assert_eq!(clamped.max_binding_array_elements_per_shader_stage, 256);
        // Should keep requested value (it's within adapter range)
        assert_eq!(clamped.max_texture_dimension_2d, 4096);
    }
}
