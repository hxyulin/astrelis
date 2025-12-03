/// A globally shared graphics context.
pub struct GraphicsContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl GraphicsContext {
    /// Creates a new graphics context synchronously.
    ///
    /// See [`GraphicsContext::new`] for the asynchronous version.
    pub fn new_sync() -> &'static Self {
        pollster::block_on(Self::new())
    }

    /// Creates a new graphics context asynchronously.
    ///
    /// This returns a static reference to simplify the public API and lifecycle
    pub async fn new() -> &'static Self {
        Self::new_with_descriptor(GraphicsContextDescriptor::default()).await
    }

    /// Creates a new graphics context with custom descriptor.
    pub async fn new_with_descriptor(descriptor: GraphicsContextDescriptor) -> &'static Self {
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

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: descriptor.features,
                required_limits: descriptor.limits.clone(),
                label: descriptor.label,
                ..Default::default()
            })
            .await
            .expect("Failed to create device");

        Box::leak(Box::new(Self {
            instance,
            adapter,
            device,
            queue,
        }))
    }

    /// Get device info
    pub fn info(&self) -> wgpu::AdapterInfo {
        self.adapter.get_info()
    }

    /// Get device limits
    pub fn limits(&self) -> wgpu::Limits {
        self.device.limits()
    }

    /// Get device features
    pub fn features(&self) -> wgpu::Features {
        self.device.features()
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
    /// Required device features
    pub features: wgpu::Features,
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
            features: wgpu::Features::empty(),
            limits: wgpu::Limits::default(),
            label: None,
        }
    }
}
