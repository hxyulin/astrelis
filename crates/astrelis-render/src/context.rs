//! GPU context management and resource creation.
//!
//! This module provides [`GraphicsContext`], the core GPU abstraction that manages
//! the WGPU device, queue, and adapter. It uses `Arc<GraphicsContext>` for cheap
//! cloning and shared ownership across windows and rendering subsystems.
//!
//! # Lifecycle
//!
//! 1. Create with [`GraphicsContext::new_owned_sync()`] (blocking) or [`GraphicsContext::new_owned()`] (async)
//! 2. Clone the `Arc<GraphicsContext>` to share with windows, renderers, etc.
//! 3. Use helper methods to create GPU resources (shaders, buffers, pipelines)
//! 4. Drop when all Arc references are released
//!
//! # Example
//!
//! ```rust,no_run
//! use astrelis_render::GraphicsContext;
//!
//! let graphics = GraphicsContext::new_owned_sync()
//!     .expect("Failed to create GPU context");
//!
//! // Clone for sharing (cheap Arc clone)
//! let graphics_clone = graphics.clone();
//!
//! // Use for resource creation
//! let shader = graphics.create_shader_module(/* ... */);
//! ```
//!
//! # Thread Safety
//!
//! `GraphicsContext` is `Send + Sync` and can be safely shared across threads
//! via `Arc<GraphicsContext>`.

use astrelis_core::profiling::{profile_function, profile_scope};

use crate::capability::{clamp_limits_to_adapter, RenderCapability};
use crate::features::GpuFeatures;
use astrelis_test_utils::{
    GpuBindGroup, GpuBindGroupLayout, GpuBuffer, GpuComputePipeline, GpuRenderPipeline,
    GpuSampler, GpuShaderModule, GpuTexture, RenderContext,
};
use std::sync::Arc;
use wgpu::{
    BindGroupDescriptor, BindGroupLayoutDescriptor, BufferDescriptor, ComputePipelineDescriptor,
    RenderPipelineDescriptor, SamplerDescriptor, ShaderModuleDescriptor, TextureDescriptor,
};

/// Errors that can occur during graphics context creation.
#[derive(Debug, Clone)]
pub enum GraphicsError {
    /// No suitable GPU adapter was found.
    NoAdapter,

    /// Failed to create a device.
    DeviceCreationFailed(String),

    /// Required GPU features are not supported by the adapter.
    MissingRequiredFeatures {
        missing: GpuFeatures,
        adapter_name: String,
        supported: GpuFeatures,
    },

    /// Failed to create a surface.
    SurfaceCreationFailed(String),

    /// Failed to get surface configuration.
    SurfaceConfigurationFailed(String),

    /// Failed to acquire surface texture.
    SurfaceTextureAcquisitionFailed(String),

    /// Surface is lost and needs to be recreated.
    /// This is a recoverable condition - call `reconfigure_surface()` and retry.
    SurfaceLost,

    /// Surface texture is outdated (e.g., window was resized).
    /// This is a recoverable condition - call `reconfigure_surface()` and retry.
    SurfaceOutdated,

    /// Not enough memory to acquire surface texture.
    SurfaceOutOfMemory,

    /// Surface acquisition timed out.
    SurfaceTimeout,
}

impl std::fmt::Display for GraphicsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphicsError::NoAdapter => {
                write!(f, "Failed to find a suitable GPU adapter")
            }
            GraphicsError::DeviceCreationFailed(msg) => {
                write!(f, "Failed to create device: {}", msg)
            }
            GraphicsError::MissingRequiredFeatures { missing, adapter_name, supported } => {
                write!(
                    f,
                    "Required GPU features not supported by adapter '{}': {:?}\nSupported: {:?}",
                    adapter_name, missing, supported
                )
            }
            GraphicsError::SurfaceCreationFailed(msg) => {
                write!(f, "Failed to create surface: {}", msg)
            }
            GraphicsError::SurfaceConfigurationFailed(msg) => {
                write!(f, "Failed to get surface configuration: {}", msg)
            }
            GraphicsError::SurfaceTextureAcquisitionFailed(msg) => {
                write!(f, "Failed to acquire surface texture: {}", msg)
            }
            GraphicsError::SurfaceLost => {
                write!(f, "Surface lost - needs recreation (window minimize, GPU reset, etc.)")
            }
            GraphicsError::SurfaceOutdated => {
                write!(f, "Surface outdated - needs reconfiguration (window resized)")
            }
            GraphicsError::SurfaceOutOfMemory => {
                write!(f, "Out of memory acquiring surface texture")
            }
            GraphicsError::SurfaceTimeout => {
                write!(f, "Timeout acquiring surface texture")
            }
        }
    }
}

impl std::error::Error for GraphicsError {}

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
/// let ctx = GraphicsContext::new_owned_sync()
///     .expect("Failed to create graphics context"); // Returns Arc<Self>
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
    pub(crate) instance: wgpu::Instance,
    pub(crate) adapter: wgpu::Adapter,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
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
    pub async fn new_owned() -> Result<Arc<Self>, GraphicsError> {
        profile_function!();
        Self::new_owned_with_descriptor(GraphicsContextDescriptor::default()).await
    }

    /// Creates a new graphics context synchronously with owned ownership (recommended).
    ///
    /// **Warning:** This blocks the current thread until the context is created.
    /// For async contexts, use [`new_owned()`](Self::new_owned) instead.
    ///
    /// # Errors
    ///
    /// Returns `GraphicsError` if:
    /// - No suitable GPU adapter is found
    /// - Required GPU features are not supported
    /// - Device creation fails
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use astrelis_render::GraphicsContext;
    ///
    /// // For examples/tests: use .expect() for simplicity
    /// let ctx = GraphicsContext::new_owned_sync()
    ///     .expect("Failed to create graphics context");
    ///
    /// // For production: handle the error properly
    /// let ctx = match GraphicsContext::new_owned_sync() {
    ///     Ok(ctx) => ctx,
    ///     Err(e) => {
    ///         eprintln!("GPU initialization failed: {:?}", e);
    ///         return;
    ///     }
    /// };
    /// ```
    pub fn new_owned_sync() -> Result<Arc<Self>, GraphicsError> {
        profile_function!();
        pollster::block_on(Self::new_owned())
    }

    /// Creates a new graphics context with custom descriptor (owned).
    pub async fn new_owned_with_descriptor(descriptor: GraphicsContextDescriptor) -> Result<Arc<Self>, GraphicsError> {
        let context = Self::create_context_internal(descriptor).await?;
        Ok(Arc::new(context))
    }

    /// Internal method to create context without deciding on ownership pattern.
    async fn create_context_internal(descriptor: GraphicsContextDescriptor) -> Result<Self, GraphicsError> {
        profile_function!();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: descriptor.backends,
            ..Default::default()
        });

        let adapter = {
            profile_scope!("request_adapter");
            instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: descriptor.power_preference,
                    compatible_surface: None,
                    force_fallback_adapter: descriptor.force_fallback_adapter,
                })
                .await
                .map_err(|_| GraphicsError::NoAdapter)?
        };

        // Check required features
        let required_result = descriptor.required_gpu_features.check_support(&adapter);
        if let Some(missing) = required_result.missing() {
            return Err(GraphicsError::MissingRequiredFeatures {
                missing,
                adapter_name: adapter.get_info().name.clone(),
                supported: GpuFeatures::from_wgpu(adapter.features()),
            });
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

        // Clamp requested limits to adapter capabilities to prevent device creation failure
        let adapter_limits = adapter.limits();
        let clamped_limits = clamp_limits_to_adapter(&descriptor.limits, &adapter_limits);

        let (device, queue) = {
            profile_scope!("request_device");
            adapter
                .request_device(&wgpu::DeviceDescriptor {
                    required_features: wgpu_features,
                    required_limits: clamped_limits,
                    label: descriptor.label,
                    ..Default::default()
                })
                .await
                .map_err(|e| GraphicsError::DeviceCreationFailed(e.to_string()))?
        };

        tracing::info!(
            "Created graphics context with features: {:?}",
            enabled_features
        );

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            enabled_features,
        })
    }

    /// Get a reference to the wgpu device.
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// Get a reference to the wgpu queue.
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Get a reference to the wgpu adapter.
    pub fn adapter(&self) -> &wgpu::Adapter {
        &self.adapter
    }

    /// Get a reference to the wgpu instance.
    pub fn instance(&self) -> &wgpu::Instance {
        &self.instance
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
        self.adapter
            .get_texture_format_features(format)
            .allowed_usages
            .contains(usages)
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

    /// Require a capability — its features become required, limits are merged.
    ///
    /// If the adapter doesn't support the capability's required features,
    /// device creation will fail with [`GraphicsError::MissingRequiredFeatures`].
    ///
    /// # Example
    ///
    /// ```ignore
    /// use astrelis_render::{GraphicsContextDescriptor, GpuProfiler};
    ///
    /// let desc = GraphicsContextDescriptor::new()
    ///     .require_capability::<GpuProfiler>();
    /// ```
    pub fn require_capability<T: RenderCapability>(mut self) -> Self {
        let reqs = T::requirements();
        self.required_gpu_features |= reqs.required_features;
        self.required_gpu_features |= reqs.requested_features;
        self.additional_wgpu_features |= reqs.additional_wgpu_features;
        crate::capability::merge_limits_max(&mut self.limits, &reqs.min_limits);
        tracing::trace!("Required capability: {}", T::name());
        self
    }

    /// Request a capability — required features stay required, requested features
    /// stay optional, limits are merged.
    ///
    /// The capability's required features are added as required, and its
    /// requested features are added as requested (best-effort). Limits are
    /// merged and clamped to adapter capabilities during device creation.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use astrelis_render::GraphicsContextDescriptor;
    /// use astrelis_render::batched::BestBatchCapability;
    ///
    /// let desc = GraphicsContextDescriptor::new()
    ///     .request_capability::<BestBatchCapability>();
    /// ```
    pub fn request_capability<T: RenderCapability>(mut self) -> Self {
        let reqs = T::requirements();
        self.required_gpu_features |= reqs.required_features;
        self.requested_gpu_features |= reqs.requested_features;
        self.additional_wgpu_features |= reqs.additional_wgpu_features;
        crate::capability::merge_limits_max(&mut self.limits, &reqs.min_limits);
        tracing::trace!("Requested capability: {}", T::name());
        self
    }
}

// ============================================================================
// RenderContext trait implementation
// ============================================================================

impl RenderContext for GraphicsContext {
    fn create_buffer(&self, desc: &BufferDescriptor) -> GpuBuffer {
        let buffer = self.device().create_buffer(desc);
        GpuBuffer::from_wgpu(buffer)
    }

    fn write_buffer(&self, buffer: &GpuBuffer, offset: u64, data: &[u8]) {
        let wgpu_buffer = buffer.as_wgpu();
        self.queue().write_buffer(wgpu_buffer, offset, data);
    }

    fn create_texture(&self, desc: &TextureDescriptor) -> GpuTexture {
        let texture = self.device().create_texture(desc);
        GpuTexture::from_wgpu(texture)
    }

    fn create_shader_module(&self, desc: &ShaderModuleDescriptor) -> GpuShaderModule {
        let module = self.device().create_shader_module(desc.clone());
        GpuShaderModule::from_wgpu(module)
    }

    fn create_render_pipeline(&self, desc: &RenderPipelineDescriptor) -> GpuRenderPipeline {
        let pipeline = self.device().create_render_pipeline(desc);
        GpuRenderPipeline::from_wgpu(pipeline)
    }

    fn create_compute_pipeline(&self, desc: &ComputePipelineDescriptor) -> GpuComputePipeline {
        let pipeline = self.device().create_compute_pipeline(desc);
        GpuComputePipeline::from_wgpu(pipeline)
    }

    fn create_bind_group_layout(&self, desc: &BindGroupLayoutDescriptor) -> GpuBindGroupLayout {
        let layout = self.device().create_bind_group_layout(desc);
        GpuBindGroupLayout::from_wgpu(layout)
    }

    fn create_bind_group(&self, desc: &BindGroupDescriptor) -> GpuBindGroup {
        let bind_group = self.device().create_bind_group(desc);
        GpuBindGroup::from_wgpu(bind_group)
    }

    fn create_sampler(&self, desc: &SamplerDescriptor) -> GpuSampler {
        let sampler = self.device().create_sampler(desc);
        GpuSampler::from_wgpu(sampler)
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "mock")]
    use super::*;
    #[cfg(feature = "mock")]
    use astrelis_test_utils::MockRenderContext;

    #[test]
    #[cfg(feature = "mock")]
    fn test_render_context_trait_object() {
        // Test that we can use both GraphicsContext and MockRenderContext
        // polymorphically through the RenderContext trait

        let mock_ctx = MockRenderContext::new();

        fn uses_render_context(ctx: &dyn RenderContext) {
            let buffer = ctx.create_buffer(&BufferDescriptor {
                label: Some("Test Buffer"),
                size: 256,
                usage: wgpu::BufferUsages::UNIFORM,
                mapped_at_creation: false,
            });

            ctx.write_buffer(&buffer, 0, &[0u8; 256]);
        }

        // Should work with mock context
        uses_render_context(&mock_ctx);

        // Verify the mock recorded the calls
        let calls = mock_ctx.calls();
        assert_eq!(calls.len(), 2); // create_buffer + write_buffer
    }
}
