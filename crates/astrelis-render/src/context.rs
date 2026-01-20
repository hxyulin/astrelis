use crate::features::GpuFeatures;
use std::sync::Arc;

/// A globally shared graphics context.
///
/// # Ownership Pattern
///
/// This type uses Arc for shared ownership:
///
/// ```rust,no_run
/// use astrelis_render::GraphicsContext;
/// use std::sync::Arc;
///
/// // Synchronous creation (blocks on async internally)
/// let ctx = GraphicsContext::new_owned_sync(); // Returns Arc<Self>
/// let ctx2 = ctx.clone(); // Cheap clone (Arc)
///
/// // Asynchronous creation (for async contexts)
/// # async fn example() {
/// let ctx = GraphicsContext::new_owned().await; // Returns Arc<Self>
/// # }
/// ```
///
/// Benefits of the Arc pattern:
/// - No memory leak
/// - Proper cleanup on drop
/// - Better for testing (can create/destroy contexts)
/// - Arc internally makes cloning cheap
pub struct GraphicsContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    /// The GPU features that were enabled on this context.
    enabled_features: GpuFeatures,
}

impl GraphicsContext {
    /// Creates a new graphics context with owned ownership (recommended).
    ///
    /// Returns `Arc<Self>` which can be cheaply cloned and shared.
    /// This is the preferred method for new code as it doesn't leak memory.
    ///
    /// # Example
    /// ```rust,no_run
    /// use astrelis_render::GraphicsContext;
    ///
    /// # async fn example() {
    /// let ctx = GraphicsContext::new_owned().await;
    /// let ctx2 = ctx.clone(); // Cheap clone
    /// # }
    /// ```
    pub async fn new_owned() -> Arc<Self> {
        Self::new_owned_with_descriptor(GraphicsContextDescriptor::default()).await
    }

    /// Creates a new graphics context synchronously with owned ownership (recommended).
    ///
    /// This blocks the current thread until the context is created.
    pub fn new_owned_sync() -> Arc<Self> {
        pollster::block_on(Self::new_owned())
    }

    /// Creates a new graphics context with custom descriptor (owned).
    pub async fn new_owned_with_descriptor(descriptor: GraphicsContextDescriptor) -> Arc<Self> {
        let context = Self::create_context_internal(descriptor).await;
        Arc::new(context)
    }

    /// Internal method to create context without deciding on ownership pattern.
    async fn create_context_internal(descriptor: GraphicsContextDescriptor) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: descriptor.backends,
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: descriptor.power_preference,
                compatible_surface: None,
                force_fallback_adapter: descriptor.force_fallback_adapter,
            })
            .await
            .expect("Failed to find a suitable GPU adapter");

        // Check required features
        let required_result = descriptor.required_gpu_features.check_support(&adapter);
        if let Some(missing) = required_result.missing() {
            panic!(
                "Required GPU features are not supported by the adapter: {:?}\n\
                 Adapter: {:?}\n\
                 Supported features: {:?}",
                missing,
                adapter.get_info().name,
                GpuFeatures::from_wgpu(adapter.features())
            );
        }

        // Determine which requested features are available
        let available_requested = descriptor.requested_gpu_features
            & GpuFeatures::from_wgpu(adapter.features());

        // Log which requested features were not available
        let unavailable_requested =
            descriptor.requested_gpu_features - available_requested;
        if !unavailable_requested.is_empty() {
            tracing::warn!(
                "Some requested GPU features are not available: {:?}",
                unavailable_requested
            );
        }

        // Combine all features to enable
        let enabled_features = descriptor.required_gpu_features | available_requested;
        let wgpu_features = enabled_features.to_wgpu() | descriptor.additional_wgpu_features;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: wgpu_features,
                required_limits: descriptor.limits.clone(),
                label: descriptor.label,
                ..Default::default()
            })
            .await
            .expect("Failed to create device");

        tracing::info!(
            "Created graphics context with features: {:?}",
            enabled_features
        );

        Self {
            instance,
            adapter,
            device,
            queue,
            enabled_features,
        }
    }

    /// Get device info
    pub fn info(&self) -> wgpu::AdapterInfo {
        self.adapter.get_info()
    }

    /// Get device limits
    pub fn limits(&self) -> wgpu::Limits {
        self.device.limits()
    }

    /// Get raw wgpu device features
    pub fn wgpu_features(&self) -> wgpu::Features {
        self.device.features()
    }

    /// Get the enabled GPU features (high-level wrapper).
    pub fn gpu_features(&self) -> GpuFeatures {
        self.enabled_features
    }

    /// Check if a specific GPU feature is enabled.
    pub fn has_feature(&self, feature: GpuFeatures) -> bool {
        self.enabled_features.contains(feature)
    }

    /// Check if all specified GPU features are enabled.
    pub fn has_all_features(&self, features: GpuFeatures) -> bool {
        self.enabled_features.contains(features)
    }

    /// Assert that a feature is available, panicking with a clear message if not.
    ///
    /// Use this before operations that require specific features.
    pub fn require_feature(&self, feature: GpuFeatures) {
        if !self.has_feature(feature) {
            panic!(
                "GPU feature {:?} is required but not enabled.\n\
                 Enabled features: {:?}\n\
                 To use this feature, add it to `required_gpu_features` in GraphicsContextDescriptor.",
                feature, self.enabled_features
            );
        }
    }

    // =========================================================================
    // Texture Format Support Queries
    // =========================================================================

    /// Check if a texture format is supported for the given usages.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let supported = ctx.supports_texture_format(
    ///     wgpu::TextureFormat::Rgba8Unorm,
    ///     wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING
    /// );
    /// ```
    pub fn supports_texture_format(
        &self,
        format: wgpu::TextureFormat,
        usages: wgpu::TextureUsages,
    ) -> bool {
        let capabilities = self.adapter.get_texture_format_features(format);

        // Check if all requested usages are supported
        if usages.contains(wgpu::TextureUsages::TEXTURE_BINDING)
            && !capabilities
                .allowed_usages
                .contains(wgpu::TextureUsages::TEXTURE_BINDING)
        {
            return false;
        }
        if usages.contains(wgpu::TextureUsages::STORAGE_BINDING)
            && !capabilities
                .allowed_usages
                .contains(wgpu::TextureUsages::STORAGE_BINDING)
        {
            return false;
        }
        if usages.contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
            && !capabilities
                .allowed_usages
                .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
        {
            return false;
        }
        if usages.contains(wgpu::TextureUsages::COPY_SRC)
            && !capabilities
                .allowed_usages
                .contains(wgpu::TextureUsages::COPY_SRC)
        {
            return false;
        }
        if usages.contains(wgpu::TextureUsages::COPY_DST)
            && !capabilities
                .allowed_usages
                .contains(wgpu::TextureUsages::COPY_DST)
        {
            return false;
        }

        true
    }

    /// Get texture format capabilities.
    ///
    /// Returns detailed information about what operations are supported
    /// for a given texture format.
    pub fn texture_format_capabilities(
        &self,
        format: wgpu::TextureFormat,
    ) -> wgpu::TextureFormatFeatures {
        self.adapter.get_texture_format_features(format)
    }

    // =========================================================================
    // Limit Queries (Convenience Methods)
    // =========================================================================

    /// Get the maximum 2D texture dimension.
    ///
    /// This is the maximum width and height for 2D textures.
    #[inline]
    pub fn max_texture_dimension_2d(&self) -> u32 {
        self.device.limits().max_texture_dimension_2d
    }

    /// Get the maximum buffer size in bytes.
    ///
    /// This is the maximum size for any buffer.
    #[inline]
    pub fn max_buffer_size(&self) -> u64 {
        self.device.limits().max_buffer_size
    }

    /// Get the minimum uniform buffer offset alignment.
    ///
    /// When using dynamic uniform buffers, offsets must be aligned to this value.
    #[inline]
    pub fn min_uniform_buffer_offset_alignment(&self) -> u32 {
        self.device.limits().min_uniform_buffer_offset_alignment
    }

    /// Get the minimum storage buffer offset alignment.
    ///
    /// When using dynamic storage buffers, offsets must be aligned to this value.
    #[inline]
    pub fn min_storage_buffer_offset_alignment(&self) -> u32 {
        self.device.limits().min_storage_buffer_offset_alignment
    }

    /// Get the maximum push constant size in bytes.
    ///
    /// Push constants require the `PUSH_CONSTANTS` feature.
    /// Returns 0 if push constants are not supported.
    #[inline]
    pub fn max_push_constant_size(&self) -> u32 {
        self.device.limits().max_push_constant_size
    }

    /// Get the maximum 1D texture dimension.
    #[inline]
    pub fn max_texture_dimension_1d(&self) -> u32 {
        self.device.limits().max_texture_dimension_1d
    }

    /// Get the maximum 3D texture dimension.
    #[inline]
    pub fn max_texture_dimension_3d(&self) -> u32 {
        self.device.limits().max_texture_dimension_3d
    }

    /// Get the maximum texture array layers.
    #[inline]
    pub fn max_texture_array_layers(&self) -> u32 {
        self.device.limits().max_texture_array_layers
    }

    /// Get the maximum bind groups.
    #[inline]
    pub fn max_bind_groups(&self) -> u32 {
        self.device.limits().max_bind_groups
    }

    /// Get the maximum bindings per bind group.
    #[inline]
    pub fn max_bindings_per_bind_group(&self) -> u32 {
        self.device.limits().max_bindings_per_bind_group
    }

    /// Get the maximum dynamic uniform buffers per pipeline layout.
    #[inline]
    pub fn max_dynamic_uniform_buffers_per_pipeline_layout(&self) -> u32 {
        self.device
            .limits()
            .max_dynamic_uniform_buffers_per_pipeline_layout
    }

    /// Get the maximum dynamic storage buffers per pipeline layout.
    #[inline]
    pub fn max_dynamic_storage_buffers_per_pipeline_layout(&self) -> u32 {
        self.device
            .limits()
            .max_dynamic_storage_buffers_per_pipeline_layout
    }

    /// Get the maximum sampled textures per shader stage.
    #[inline]
    pub fn max_sampled_textures_per_shader_stage(&self) -> u32 {
        self.device.limits().max_sampled_textures_per_shader_stage
    }

    /// Get the maximum samplers per shader stage.
    #[inline]
    pub fn max_samplers_per_shader_stage(&self) -> u32 {
        self.device.limits().max_samplers_per_shader_stage
    }

    /// Get the maximum storage buffers per shader stage.
    #[inline]
    pub fn max_storage_buffers_per_shader_stage(&self) -> u32 {
        self.device.limits().max_storage_buffers_per_shader_stage
    }

    /// Get the maximum storage textures per shader stage.
    #[inline]
    pub fn max_storage_textures_per_shader_stage(&self) -> u32 {
        self.device.limits().max_storage_textures_per_shader_stage
    }

    /// Get the maximum uniform buffers per shader stage.
    #[inline]
    pub fn max_uniform_buffers_per_shader_stage(&self) -> u32 {
        self.device.limits().max_uniform_buffers_per_shader_stage
    }

    /// Get the maximum uniform buffer binding size.
    #[inline]
    pub fn max_uniform_buffer_binding_size(&self) -> u32 {
        self.device.limits().max_uniform_buffer_binding_size
    }

    /// Get the maximum storage buffer binding size.
    #[inline]
    pub fn max_storage_buffer_binding_size(&self) -> u32 {
        self.device.limits().max_storage_buffer_binding_size
    }

    /// Get the maximum vertex buffers.
    #[inline]
    pub fn max_vertex_buffers(&self) -> u32 {
        self.device.limits().max_vertex_buffers
    }

    /// Get the maximum vertex attributes.
    #[inline]
    pub fn max_vertex_attributes(&self) -> u32 {
        self.device.limits().max_vertex_attributes
    }

    /// Get the maximum vertex buffer array stride.
    #[inline]
    pub fn max_vertex_buffer_array_stride(&self) -> u32 {
        self.device.limits().max_vertex_buffer_array_stride
    }

    /// Get the maximum compute workgroup storage size.
    #[inline]
    pub fn max_compute_workgroup_storage_size(&self) -> u32 {
        self.device.limits().max_compute_workgroup_storage_size
    }

    /// Get the maximum compute invocations per workgroup.
    #[inline]
    pub fn max_compute_invocations_per_workgroup(&self) -> u32 {
        self.device.limits().max_compute_invocations_per_workgroup
    }

    /// Get the maximum compute workgroup size X.
    #[inline]
    pub fn max_compute_workgroup_size_x(&self) -> u32 {
        self.device.limits().max_compute_workgroup_size_x
    }

    /// Get the maximum compute workgroup size Y.
    #[inline]
    pub fn max_compute_workgroup_size_y(&self) -> u32 {
        self.device.limits().max_compute_workgroup_size_y
    }

    /// Get the maximum compute workgroup size Z.
    #[inline]
    pub fn max_compute_workgroup_size_z(&self) -> u32 {
        self.device.limits().max_compute_workgroup_size_z
    }

    /// Get the maximum compute workgroups per dimension.
    #[inline]
    pub fn max_compute_workgroups_per_dimension(&self) -> u32 {
        self.device.limits().max_compute_workgroups_per_dimension
    }
}

/// Descriptor for configuring graphics context creation.
pub struct GraphicsContextDescriptor {
    /// GPU backends to use
    pub backends: wgpu::Backends,
    /// Power preference for adapter selection
    pub power_preference: wgpu::PowerPreference,
    /// Whether to force fallback adapter
    pub force_fallback_adapter: bool,
    /// Required GPU features (panics if not available).
    ///
    /// Use this for features that your application cannot function without.
    pub required_gpu_features: GpuFeatures,
    /// Requested GPU features (best-effort, logs warning if unavailable).
    ///
    /// Use this for features that would be nice to have but are not essential.
    pub requested_gpu_features: GpuFeatures,
    /// Additional raw wgpu features to enable (for features not covered by GpuFeatures).
    pub additional_wgpu_features: wgpu::Features,
    /// Required device limits
    pub limits: wgpu::Limits,
    /// Optional label for debugging
    pub label: Option<&'static str>,
}

impl Default for GraphicsContextDescriptor {
    fn default() -> Self {
        Self {
            backends: wgpu::Backends::all(),
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            required_gpu_features: GpuFeatures::empty(),
            requested_gpu_features: GpuFeatures::empty(),
            additional_wgpu_features: wgpu::Features::empty(),
            limits: wgpu::Limits::default(),
            label: None,
        }
    }
}

impl GraphicsContextDescriptor {
    /// Create a new descriptor with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set required GPU features (panics if not available).
    pub fn require_features(mut self, features: GpuFeatures) -> Self {
        self.required_gpu_features = features;
        self
    }

    /// Set requested GPU features (best-effort, warns if unavailable).
    pub fn request_features(mut self, features: GpuFeatures) -> Self {
        self.requested_gpu_features = features;
        self
    }

    /// Add additional required features.
    pub fn with_required_features(mut self, features: GpuFeatures) -> Self {
        self.required_gpu_features |= features;
        self
    }

    /// Add additional requested features.
    pub fn with_requested_features(mut self, features: GpuFeatures) -> Self {
        self.requested_gpu_features |= features;
        self
    }

    /// Set additional raw wgpu features (for features not covered by GpuFeatures).
    pub fn with_wgpu_features(mut self, features: wgpu::Features) -> Self {
        self.additional_wgpu_features = features;
        self
    }

    /// Set the power preference.
    pub fn power_preference(mut self, preference: wgpu::PowerPreference) -> Self {
        self.power_preference = preference;
        self
    }

    /// Set the backends to use.
    pub fn backends(mut self, backends: wgpu::Backends) -> Self {
        self.backends = backends;
        self
    }

    /// Set the device limits.
    pub fn limits(mut self, limits: wgpu::Limits) -> Self {
        self.limits = limits;
        self
    }

    /// Set the debug label.
    pub fn label(mut self, label: &'static str) -> Self {
        self.label = Some(label);
        self
    }
}
