use std::sync::Arc;

use crate::{
    graphics::{Framebuffer, texture::Texture},
    profiling::profile_function,
};
use wgpu::SurfaceTexture;
pub use wgpu::{Backends, PresentMode};
use winit::{dpi::PhysicalSize, window::Window};

pub struct GraphicsContextOpts {
    backends: Backends,
    present_mode: PresentMode,
}

impl Default for GraphicsContextOpts {
    fn default() -> Self {
        let backends = Backends::from_env().unwrap_or(Backends::all());
        let present_mode = PresentMode::AutoVsync;
        Self {
            backends,
            present_mode,
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

pub enum RenderableSurface<'a> {
    WindowSurface,
    Framebuffer(&'a Framebuffer),
}

impl<'a> RenderableSurface<'a> {
    pub(crate) fn get_color(&'a self, ctx: &'a GraphicsContext) -> &'a wgpu::TextureView {
        match self {
            Self::WindowSurface => &ctx.frame.as_ref().unwrap().surface.view,
            Self::Framebuffer(fb) => &fb.color.view(),
        }
    }

    pub(crate) fn get_depth(&'a self, ctx: &'a GraphicsContext) -> Option<&'a wgpu::TextureView> {
        match self {
            Self::WindowSurface => Some(&ctx.depth.view),
            Self::Framebuffer(fb) => Some(&fb.depth.as_ref()?.view()),
        }
    }
}

pub struct Surface {
    pub(crate) texture: SurfaceTexture,
    pub(crate) view: wgpu::TextureView,
}

pub struct GraphicsContext {
    pub(crate) window: Arc<Window>,
    pub(super) instance: wgpu::Instance,
    pub(super) surface: wgpu::Surface<'static>,
    pub(super) adapter: wgpu::Adapter,
    pub(super) device: wgpu::Device,
    pub(super) device_features: wgpu::Features,
    pub(super) queue: wgpu::Queue,
    pub(super) config: wgpu::SurfaceConfiguration,
    pub(super) sample_count: u32,
    pub(super) depth: Texture,

    pub(super) backend: wgpu::Backend,
    pub(super) frame: Option<GraphicsContextFrame>,

    reconfigure: PendingReconfigure,
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
        /*
                let sample_count = format_features
                    .flags
                    .supported_sample_counts()
                    .iter()
                    .max()
                    .cloned()
                    .unwrap_or(1);
        */
        // We only support sample count of 1, no MSAA is supported yet
        let sample_count = 1;

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
            sample_count,
            depth,

            backend,
            frame: None,
            reconfigure: PendingReconfigure::new(),
        })
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
}
