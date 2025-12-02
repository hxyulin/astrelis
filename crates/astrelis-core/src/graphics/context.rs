use std::sync::Arc;

use crate::{
    alloc::{IndexSlot, SparseSet},
    graphics::{Framebuffer, FramebufferOpts, ViewConfig, frame::FrameContext, texture::Texture},
    profiling::profile_function,
};
use wgpu::SurfaceTexture;
pub use wgpu::{Backends, PresentMode};
use winit::{dpi::PhysicalSize, window::Window};

pub struct GraphicsContextOpts {
    backends: Backends,
    present_mode: PresentMode,
    multisample: bool,
}

impl Default for GraphicsContextOpts {
    fn default() -> Self {
        Self {
            backends: Backends::from_env().unwrap_or(Backends::all()),
            present_mode: PresentMode::AutoVsync,
            multisample: false,
        }
    }
}

struct PendingReconfigure {
    pub resize: Option<PhysicalSize<u32>>,
}

impl PendingReconfigure {
    const fn new() -> Self {
        Self { resize: None }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RenderTarget {
    Window,
    Target(RenderTargetId),
}

impl RenderTarget {
    pub(crate) fn get_config(&self, ctx: &GraphicsContext) -> ViewConfig {
        match self {
            Self::Window => ViewConfig {
                extent: wgpu::Extent3d {
                    width: ctx.config.width,
                    height: ctx.config.height,
                    depth_or_array_layers: 1,
                },
                format: ctx.config.format,
                sample_count: ctx.sample_count(),
            },
            Self::Target(fb) => ctx.get_framebuffer(*fb).config,
        }
    }

    pub(crate) fn get_color<'a>(&self, ctx: &'a GraphicsContext) -> &'a wgpu::TextureView {
        match self {
            Self::Window => &ctx.frame.as_ref().unwrap().surface.view,
            Self::Target(fb) => ctx.get_framebuffer(*fb).color.view(),
        }
    }

    pub(crate) fn get_depth<'a>(
        &'a self,
        ctx: &'a GraphicsContext,
    ) -> Option<&'a wgpu::TextureView> {
        match self {
            Self::Window => Some(&ctx.depth.view),
            Self::Target(fb) => Some(ctx.get_framebuffer(*fb).depth.as_ref()?.view()),
        }
    }
}

pub struct Surface {
    pub(crate) texture: SurfaceTexture,
    pub(crate) view: wgpu::TextureView,
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct RenderTargetId(IndexSlot);

#[allow(dead_code)]
pub struct GraphicsContext {
    pub(crate) window: Arc<Window>,
    pub(crate) framebuffers: SparseSet<Framebuffer>,

    pub(super) instance: wgpu::Instance,
    pub(super) surface: wgpu::Surface<'static>,
    pub(super) adapter: wgpu::Adapter,
    pub(super) device: wgpu::Device,
    pub(super) device_features: wgpu::Features,
    pub(super) queue: wgpu::Queue,
    pub(super) config: wgpu::SurfaceConfiguration,
    pub(super) depth: Texture,

    pub(super) backend: wgpu::Backend,
    pub(super) frame: Option<GraphicsContextFrame>,

    multisample: Option<MultisampleState>,
    reconfigure: PendingReconfigure,

    pub(crate) buffer_pool: BufferPool,
    pub(crate) frame_number: u64,
}

pub struct MultisampleState {
    pub sample_count: u32,
    pub texture: Texture,
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum GraphicsContextCreationError {
    #[error(transparent)]
    CreateSurfaceError(#[from] wgpu::CreateSurfaceError),
    #[error("no adapter available")]
    NoAdapter,
    #[error(transparent)]
    RequestDeviceError(#[from] wgpu::RequestDeviceError),
}

pub(super) struct GraphicsContextFrame {
    pub(super) surface: Surface,
    // We need to keep track, because if there are multiple passes, we need to not clear the frame,
    // or if there is none, we need to submit an arbitrary command buffer.
    pub(super) passes: usize,
}

impl GraphicsContext {
    pub fn new(
        window: Arc<Window>,
        opts: GraphicsContextOpts,
    ) -> Result<Self, GraphicsContextCreationError> {
        profile_function!();
        use wgpu::{Instance, InstanceDescriptor, RequestAdapterOptions};
        let instance = Instance::new(&InstanceDescriptor {
            backends: opts.backends,
            ..Default::default()
        });

        let size = window.inner_size();
        let surface = instance.create_surface(window.clone())?;

        let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .ok_or(GraphicsContextCreationError::NoAdapter)?;

        let backend = adapter.get_info().backend;
        let device_features = wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                //trace: wgpu::Trace::Off,
                required_features: device_features,
                required_limits: if cfg!(target_arch = "wasm32") {
                    wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits())
                } else {
                    wgpu::Limits::default().using_resolution(adapter.limits())
                },
                memory_hints: wgpu::MemoryHints::MemoryUsage,
            },
            None,
        ))?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .unwrap();
        config.format = surface_format;
        config.present_mode = opts.present_mode;
        surface.configure(&device, &config);

        let format_features = config.format.guaranteed_format_features(device_features);
        let multisample = opts.multisample.then(|| {
            let sample_count = format_features
                .flags
                .supported_sample_counts()
                .iter()
                .max()
                .cloned()
                .unwrap_or(1);
            MultisampleState {
                sample_count,
                texture: Texture::create_msaa_texture(&device, &config, sample_count),
            }
        });

        let depth = Texture::create_depth_texture(&device, &config);

        Ok(Self {
            window,
            instance,
            surface,
            adapter,
            device,
            device_features,
            queue,
            config,
            depth,
            multisample,

            backend,
            frame: None,
            reconfigure: PendingReconfigure::new(),
            framebuffers: SparseSet::new(),
            buffer_pool: BufferPool::new(),
            frame_number: 0,
        })
    }

    pub fn get_framebuffer(&self, id: RenderTargetId) -> &Framebuffer {
        self.framebuffers.get(id.0)
    }

    pub fn begin_render(&mut self) {
        profile_function!();

        let mut configure_needed = false;
        if let Some(new_size) = self.reconfigure.resize {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            configure_needed = true;
        }
        if configure_needed {
            self.surface.configure(&self.device, &self.config);
        }

        let frame = self.surface.get_current_texture().unwrap();
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.frame.replace(GraphicsContextFrame {
            surface: Surface {
                texture: frame,
                view,
            },
            passes: 0,
        });
    }

    pub fn end_render(&mut self) {
        profile_function!();
        let frame = self
            .frame
            .take()
            .expect("you need to call begin_render first");
        debug_assert!(
            frame.passes > 0,
            "at least 1 pass is required to render a frame"
        );
        frame.surface.texture.present();

        // Increment frame counter and cleanup buffer pool
        self.frame_number += 1;
        if self.frame_number % 60 == 0 {
            self.buffer_pool.trim(&self.device, self.frame_number);
        }
    }

    /// This should be called to resize the surface for the new resolution
    pub fn resized(&mut self, new_size: PhysicalSize<u32>) {
        self.reconfigure.resize = Some(new_size);
    }

    /// Internal API
    pub(crate) fn sample_count(&self) -> u32 {
        self.multisample
            .as_ref()
            .map(|msaa| msaa.sample_count)
            .unwrap_or(1)
    }

    pub(crate) fn create_framebuffer(&mut self, opts: FramebufferOpts) -> RenderTargetId {
        RenderTargetId(self.framebuffers.push(Framebuffer::new_internal(
            &self.device,
            &self.config,
            opts,
        )))
    }

    /// Get a reference to the device
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// Get a reference to the queue
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Get a reference to the surface configuration
    pub fn surface_config(&self) -> &wgpu::SurfaceConfiguration {
        &self.config
    }

    /// Get the current frame's surface view for rendering
    pub fn get_surface_view(&self) -> &wgpu::TextureView {
        &self
            .frame
            .as_ref()
            .expect("frame should be available during render")
            .surface
            .view
    }

    /// Begin a new frame with the new API (returns FrameContext)
    pub fn begin_frame(&mut self) -> FrameContext<'_> {
        profile_function!();

        // Configure surface if needed
        let mut configure_needed = false;
        if let Some(new_size) = self.reconfigure.resize {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            configure_needed = true;
        }
        if configure_needed {
            self.surface.configure(&self.device, &self.config);
        }

        // Get surface texture
        let frame = self.surface.get_current_texture().unwrap();
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.frame.replace(GraphicsContextFrame {
            surface: Surface {
                texture: frame,
                view,
            },
            passes: 0,
        });

        FrameContext::new(self)
    }

    /// Allocate a staging buffer from the pool
    pub fn allocate_staging_buffer(&mut self, size: u64) -> wgpu::Buffer {
        self.buffer_pool
            .allocate(&self.device, size, self.frame_number)
    }

    /// Return a staging buffer to the pool
    pub fn free_staging_buffer(&mut self, buffer: wgpu::Buffer) {
        self.buffer_pool.free(buffer, self.frame_number);
    }

    /// Get current frame number
    pub fn frame_number(&self) -> u64 {
        self.frame_number
    }
}

/// Pool for reusing staging buffers across frames
pub struct BufferPool {
    free_buffers: Vec<(wgpu::Buffer, u64, u64)>, // (buffer, size, last_used_frame)
}

impl BufferPool {
    pub fn new() -> Self {
        Self {
            free_buffers: Vec::new(),
        }
    }

    pub fn allocate(&mut self, device: &wgpu::Device, size: u64, frame: u64) -> wgpu::Buffer {
        // Try to find a suitable buffer
        if let Some(idx) = self
            .free_buffers
            .iter()
            .position(|(_, buf_size, _)| *buf_size >= size)
        {
            let (buffer, _, _) = self.free_buffers.swap_remove(idx);
            return buffer;
        }

        // Create new buffer
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size,
            usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::MAP_WRITE,
            mapped_at_creation: false,
        })
    }

    pub fn free(&mut self, buffer: wgpu::Buffer, frame: u64) {
        let size = buffer.size();
        self.free_buffers.push((buffer, size, frame));
    }

    pub fn trim(&mut self, device: &wgpu::Device, current_frame: u64) {
        // Remove buffers unused for 60 frames
        self.free_buffers
            .retain(|(_, _, last_used)| current_frame - *last_used < 60);

        // Log pool stats
        tracing::trace!(free_buffers = self.free_buffers.len(), "BufferPool stats");
    }
}
