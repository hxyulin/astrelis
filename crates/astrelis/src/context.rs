use crate::{Error, Mesh, RenderTarget, Vertex};

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
    /// Width and height are physical pixels. A zero dimension creates a suspended
    /// target. Window creation and event handling remain application-owned.
    /// Passing an owned handle such as `Arc<Window>` allows a `'static` target;
    /// passing a borrowed window ties the target's lifetime to that borrow.
    ///
    /// # Errors
    ///
    /// Returns an error if surface creation, adapter selection, device creation, or
    /// target configuration fails. Dimensions must fit the device's texture limit.
    pub async fn with_surface<'window>(
        window: impl Into<wgpu::SurfaceTarget<'window>>,
        width: u32,
        height: u32,
    ) -> Result<(Self, RenderTarget<'window>), Error> {
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window)
            .map_err(Error::CreateSurface)?;
        let graphics = Self::request(&instance, Some(&surface)).await?;
        let target = RenderTarget::surface(&graphics, surface, width, height)?;
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
    /// can render to any of them, reusing meshes and cached pipelines. Width and
    /// height are physical pixels; a zero dimension suspends the target.
    /// The returned target borrows only a borrowed window, not the context.
    ///
    /// # Errors
    ///
    /// Returns [`Error::CreateSurface`] if wgpu cannot create the surface,
    /// [`Error::UnsupportedSurface`] if this adapter cannot present to it, or
    /// [`Error::InvalidTargetSize`] if the dimensions exceed the device limit.
    pub fn create_surface<'window>(
        &self,
        window: impl Into<wgpu::SurfaceTarget<'window>>,
        width: u32,
        height: u32,
    ) -> Result<RenderTarget<'window>, Error> {
        let surface = self
            .instance
            .create_surface(window)
            .map_err(Error::CreateSurface)?;
        RenderTarget::surface(self, surface, width, height)
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
