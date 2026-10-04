use crate::{
    Error, Framebuffer, FramebufferOptions, Material, MaterialOptions, Mesh, RenderTarget,
    SurfaceOptions, Vertex,
};

/// Shared wgpu initialization and GPU resources for one or more render targets.
///
/// A context owns a [`wgpu::Instance`], a selected [`wgpu::Adapter`], and its
/// [`wgpu::Device`] and [`wgpu::Queue`]. The instance creates surfaces and finds
/// adapters; the device creates resources, and the queue uploads and submits work.
/// Each [`RenderTarget`] owns its own surface, size, and presentation configuration.
///
/// Start with [`Self::with_surface`] to select an adapter compatible with the first
/// window, then use [`Self::create_surface`] for additional windows. All targets,
/// meshes, and renderers using this context share the same device. Additional
/// surfaces must be supported by the selected adapter; a context does not select
/// another adapter automatically when a surface is incompatible.
///
/// Cloning shares all wgpu handles without creating another instance or device.
/// Default initialization requests no optional device features, including timestamp
/// queries. Use [`Self::from_wgpu`] for application-controlled device configuration.
#[derive(Clone, Debug)]
pub struct GraphicsContext {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl GraphicsContext {
    /// Initializes wgpu and a first surface, selecting a compatible adapter.
    ///
    /// Options select physical dimensions and the initial sample count. A zero
    /// dimension creates a suspended target. Window creation and event handling
    /// remain application-owned. Target creation validates MSAA and caches usable
    /// counts before returning; unsupported requests do not fall back implicitly.
    /// Passing an owned handle such as `Arc<Window>` allows a `'static` target;
    /// passing a borrowed window ties the target's lifetime to that borrow.
    ///
    /// # Errors
    ///
    /// Returns an error if surface creation, adapter selection, device creation, or
    /// target configuration fails. Dimensions must fit the device's texture limit.
    /// Unsupported MSAA returns [`Error::UnsupportedSampleCount`].
    pub async fn with_surface<'window>(
        window: impl Into<wgpu::SurfaceTarget<'window>>,
        options: SurfaceOptions,
    ) -> Result<(Self, RenderTarget<'window>), Error> {
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window)
            .map_err(Error::CreateSurface)?;
        let graphics = Self::request(&instance, Some(&surface)).await?;
        let target = RenderTarget::surface(&graphics, surface, options)?;
        Ok((graphics, target))
    }

    /// Initializes wgpu without a presentation surface.
    ///
    /// Useful for compute or offscreen GPU work through the raw device and queue.
    /// Surfaces can be added later with [`Self::create_surface`], but the selected
    /// adapter is not guaranteed to support them. Prefer [`Self::with_surface`]
    /// when the application will present to a window.
    ///
    /// # Errors
    ///
    /// Returns an error if no suitable adapter or device can be created.
    pub async fn headless() -> Result<Self, Error> {
        Self::request(&wgpu::Instance::default(), None).await
    }

    /// Creates and configures another surface using this context's device.
    ///
    /// Targets have independent sizes and presentation state. A single renderer
    /// can render to any of them, reusing meshes and cached pipelines. Options use
    /// physical pixels; a zero dimension suspends the target. The initial sample
    /// count is validated for this surface's format and usable counts are cached.
    /// The returned target borrows only a borrowed window, not the context.
    ///
    /// # Errors
    ///
    /// Returns [`Error::CreateSurface`] if wgpu cannot create the surface,
    /// [`Error::UnsupportedSurface`] if this adapter cannot present to it, or
    /// [`Error::InvalidTargetSize`] if the dimensions exceed the device limit, or
    /// [`Error::UnsupportedSampleCount`] if the initial MSAA request is unsupported.
    pub fn create_surface<'window>(
        &self,
        window: impl Into<wgpu::SurfaceTarget<'window>>,
        options: SurfaceOptions,
    ) -> Result<RenderTarget<'window>, Error> {
        let surface = self
            .instance
            .create_surface(window)
            .map_err(Error::CreateSurface)?;
        RenderTarget::surface(self, surface, options)
    }

    /// Validates and uploads an immutable indexed triangle mesh on this device.
    ///
    /// Every three Uint32 indices describe a triangle. Vertex positions use clip
    /// space, and colors use linear RGB with straight alpha. Upload once and reuse
    /// the mesh across frames and renderers on this device.
    ///
    /// # Errors
    ///
    /// Rejects empty geometry, incomplete triangles, out-of-bounds indices,
    /// nonfinite vertex components, and data exceeding the device's buffer limits.
    pub fn create_mesh(&self, vertices: &[Vertex], indices: &[u32]) -> Result<Mesh, Error> {
        Mesh::upload(self, vertices, indices)
    }

    /// Creates a reusable offscreen color framebuffer with optional MSAA.
    ///
    /// Validates dimensions, enabled format features, output usages, and sample
    /// counts before allocating. Zero dimensions create a suspended resource.
    /// Defaults allow rendering and shader sampling; add `COPY_SRC` for readback.
    /// Usable sample counts are cached at creation. Unsupported requests return
    /// an error without choosing a fallback or creating GPU attachments.
    pub fn create_framebuffer(&self, options: FramebufferOptions) -> Result<Framebuffer, Error> {
        Framebuffer::create(self, options)
    }

    /// Creates an immutable mesh material from an application-created shader.
    ///
    /// The shader and binding layouts must belong to this device. The material
    /// retains their GPU handles and owns its entry-point names and render state.
    /// Uniform buffers, textures, bind groups, and their updates remain owned by
    /// the application. Shaders use the fixed [`Vertex`] attribute layout.
    ///
    /// As with raw wgpu resource creation, invalid layouts use wgpu's error
    /// reporting. Shader interfaces are validated when a renderer first creates
    /// a pipeline; use [`crate::MeshRenderer::prepare_material`] to do that before
    /// rendering. This factory does not record, submit, or wait for GPU work.
    pub fn create_material(&self, options: MaterialOptions<'_>) -> Material {
        Material::create(self, options)
    }

    /// Requests a device from an application-created wgpu instance.
    ///
    /// The context retains a clone of the instance for later surface creation.
    /// Pass `Some(&surface)` to select an adapter compatible with an existing
    /// surface created by this instance; `None` selects without presentation
    /// constraints. This is the initialization path for custom instance settings.
    ///
    /// # Errors
    ///
    /// Returns an error if adapter selection or device creation fails.
    pub async fn request(
        instance: &wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self, Error> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: surface,
                ..Default::default()
            })
            .await
            .map_err(Error::Adapter)?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Astrelis device"),
                ..Default::default()
            })
            .await
            .map_err(Error::Device)?;
        Ok(Self::from_wgpu(instance.clone(), adapter, device, queue))
    }

    /// Adopts application-created wgpu handles, preserving their configuration.
    ///
    /// The caller must supply the instance that created the adapter, the adapter
    /// that created the device, and the queue returned with that device. Optional
    /// features and device limits remain under the application's control.
    pub fn from_wgpu(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Self {
        Self {
            instance,
            adapter,
            device,
            queue,
        }
    }

    /// Returns the wgpu instance for adapter discovery and custom surface creation.
    pub fn instance(&self) -> &wgpu::Instance {
        &self.instance
    }

    /// Returns the selected wgpu adapter.
    pub fn adapter(&self) -> &wgpu::Adapter {
        &self.adapter
    }

    /// Returns the shared wgpu device for custom resource creation and GPU work.
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// Returns the shared wgpu queue for uploads, submission, and callbacks.
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }
}
