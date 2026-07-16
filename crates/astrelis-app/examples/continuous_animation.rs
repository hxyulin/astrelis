//! Continuously animates a GPU-cleared window through the shared runtime.

use std::{io, time::Duration};

use astrelis_app::{
    App, AppContext, FixedStep, FixedUpdateInfo, Runtime, RuntimeConfig, RuntimePolicy,
};
use astrelis_gpu::{
    Color, CommandEncoderDescriptor, CompositeAlphaMode, DeviceDescriptor, LoadOp, PresentMode,
    RenderPassColorAttachment, RenderPassDescriptor, RequestAdapterOptions, StoreOp,
    SurfaceConfiguration, SurfaceFrameStatus, SurfaceTarget, TextureUsages, TextureViewDescriptor,
};
use astrelis_platform::{Window, WindowAttributes, WindowEvent, WindowId};

struct GpuState {
    surface: astrelis_gpu::Surface,
    device: astrelis_gpu::Device,
    queue: astrelis_gpu::Queue,
    configuration: SurfaceConfiguration,
}

struct Animation {
    instance: astrelis_gpu::Instance,
    window: Option<Window>,
    gpu: Option<GpuState>,
    phase: f64,
}

impl Default for Animation {
    fn default() -> Self {
        Self {
            instance: astrelis_gpu_wgpu::create_instance(Default::default()),
            window: None,
            gpu: None,
            phase: 0.0,
        }
    }
}

impl Animation {
    fn configure(&mut self, width: u32, height: u32) -> io::Result<()> {
        let Some(gpu) = &mut self.gpu else {
            return Ok(());
        };
        if width == 0 || height == 0 {
            return Ok(());
        }
        gpu.configuration.width = width;
        gpu.configuration.height = height;
        gpu.surface
            .configure(&gpu.device, gpu.configuration.clone())
            .map_err(io::Error::other)
    }

    fn render(&mut self) -> io::Result<()> {
        let Some(gpu) = &mut self.gpu else {
            return Ok(());
        };
        let frame = match gpu.surface.acquire().map_err(io::Error::other)? {
            SurfaceFrameStatus::Ready(frame) => frame,
            SurfaceFrameStatus::Suboptimal(frame) => {
                gpu.surface
                    .configure(&gpu.device, gpu.configuration.clone())
                    .map_err(io::Error::other)?;
                frame
            }
            SurfaceFrameStatus::Outdated | SurfaceFrameStatus::Lost => {
                gpu.surface
                    .configure(&gpu.device, gpu.configuration.clone())
                    .map_err(io::Error::other)?;
                return Ok(());
            }
            SurfaceFrameStatus::Timeout | SurfaceFrameStatus::Occluded => return Ok(()),
            _ => return Ok(()),
        };
        let wave = self.phase.sin() * 0.5 + 0.5;
        let view = frame
            .texture()
            .create_view(TextureViewDescriptor::default());
        let mut encoder = gpu
            .device
            .create_command_encoder(CommandEncoderDescriptor::default());
        encoder
            .render_pass(RenderPassDescriptor {
                label: Some("continuous animation".into()),
                color_attachments: vec![Some(RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    load: LoadOp::Clear(Color {
                        r: 0.03 + wave * 0.25,
                        g: 0.05 + (1.0 - wave) * 0.15,
                        b: 0.16 + wave * 0.5,
                        a: 1.0,
                    }),
                    store: StoreOp::Store,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
            })
            .map_err(io::Error::other)?;
        gpu.queue
            .submit([encoder.finish().map_err(io::Error::other)?])
            .map_err(io::Error::other)?;
        frame.present().map_err(io::Error::other)?;
        gpu.device
            .poll(astrelis_gpu::PollMode::Poll)
            .map_err(io::Error::other)
    }
}

impl App for Animation {
    type Error = io::Error;

    fn resumed(&mut self, context: &mut AppContext<'_, '_, Self>) -> Result<(), Self::Error> {
        if self.window.is_some() {
            return Ok(());
        }
        let window = context
            .create_window(WindowAttributes {
                title: "Astrelis continuous animation".into(),
                ..Default::default()
            })
            .map_err(io::Error::other)?;
        let surface = self
            .instance
            .create_surface(SurfaceTarget::new(window.clone()))
            .map_err(io::Error::other)?;
        let adapter = pollster::block_on(self.instance.request_adapter(RequestAdapterOptions {
            compatible_surface: Some(surface.clone()),
            ..Default::default()
        }))
        .map_err(io::Error::other)?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(DeviceDescriptor::default()))
                .map_err(io::Error::other)?;
        let capabilities = surface.capabilities(&adapter).map_err(io::Error::other)?;
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(|format| {
                matches!(
                    format,
                    astrelis_gpu::TextureFormat::Bgra8UnormSrgb
                        | astrelis_gpu::TextureFormat::Rgba8UnormSrgb
                )
            })
            .unwrap_or(capabilities.formats[0]);
        let size = window.inner_size().map_err(io::Error::other)?;
        let configuration = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: PresentMode::Fifo,
            alpha_mode: capabilities
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(CompositeAlphaMode::Opaque),
            desired_maximum_frame_latency: 2,
        };
        surface
            .configure(&device, configuration.clone())
            .map_err(io::Error::other)?;
        self.window = Some(window);
        self.gpu = Some(GpuState {
            surface,
            device,
            queue,
            configuration,
        });
        Ok(())
    }

    fn fixed_update(
        &mut self,
        _context: &mut AppContext<'_, '_, Self>,
        info: FixedUpdateInfo,
    ) -> Result<(), Self::Error> {
        self.phase = info.elapsed.as_secs_f64() * 2.0;
        Ok(())
    }

    fn window_event(
        &mut self,
        context: &mut AppContext<'_, '_, Self>,
        window: WindowId,
        event: WindowEvent,
    ) -> Result<(), Self::Error> {
        match event {
            WindowEvent::CloseRequested => {
                self.gpu = None;
                context.unregister_window(window);
                self.window = None;
                context.exit();
            }
            WindowEvent::Resized(size) => self.configure(size.width, size.height)?,
            WindowEvent::ScaleFactorChanged { inner_size, .. } => {
                self.configure(inner_size.width, inner_size.height)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn redraw(
        &mut self,
        _context: &mut AppContext<'_, '_, Self>,
        _window: WindowId,
    ) -> Result<(), Self::Error> {
        self.render()
    }
}

fn main() -> Result<(), astrelis_app::RuntimeError<io::Error>> {
    let policy = RuntimePolicy::Continuous {
        frame_interval: Some(Duration::from_secs_f64(1.0 / 60.0)),
        fixed_step: Some(FixedStep::new(Duration::from_secs_f64(1.0 / 60.0))),
    };
    Runtime::finish(astrelis_platform_winit::run_return(Runtime::new(
        Animation::default(),
        RuntimeConfig {
            policy,
            ..Default::default()
        },
    )))
    .map(|_| ())
}
