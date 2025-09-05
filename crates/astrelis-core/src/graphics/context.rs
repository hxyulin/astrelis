use std::sync::Arc;

use crate::{
    alloc::{IndexSlot, SparseSet},
    graphics::{Framebuffer, FramebufferOpts, ViewConfig, texture::Texture},
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
}
