//! Clears a window only when the platform requests a redraw.

use astrelis_gpu::{
    Color, CommandEncoderDescriptor, CompositeAlphaMode, DeviceDescriptor, LoadOp, PresentMode,
    RenderPassColorAttachment, RenderPassDescriptor, RequestAdapterOptions, StoreOp,
    SurfaceConfiguration, SurfaceFrameStatus, SurfaceTarget, TextureUsages, TextureViewDescriptor,
};
use astrelis_platform::{
    Application, PlatformContext, Window, WindowAttributes, WindowEvent, WindowId,
};

struct GpuState {
    surface: astrelis_gpu::Surface,
    device: astrelis_gpu::Device,
    queue: astrelis_gpu::Queue,
    configuration: SurfaceConfiguration,
}

struct App {
    instance: astrelis_gpu::Instance,
    window: Option<Window>,
    gpu: Option<GpuState>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            instance: astrelis_gpu_wgpu::create_instance(Default::default()),
            window: None,
            gpu: None,
        }
    }
}

impl App {
    fn configure(&mut self, width: u32, height: u32) {
        let Some(gpu) = &mut self.gpu else { return };
        if width == 0 || height == 0 {
            return;
        }
        gpu.configuration.width = width;
        gpu.configuration.height = height;
        gpu.surface
            .configure(&gpu.device, gpu.configuration.clone())
            .expect("configure surface");
    }

    fn redraw(&mut self) {
        let Some(gpu) = &mut self.gpu else { return };
        let frame = match gpu.surface.acquire().expect("acquire surface frame") {
            SurfaceFrameStatus::Ready(frame) => frame,
            SurfaceFrameStatus::Suboptimal(frame) => {
                gpu.surface
                    .configure(&gpu.device, gpu.configuration.clone())
                    .expect("reconfigure suboptimal surface");
                frame
            }
            SurfaceFrameStatus::Outdated | SurfaceFrameStatus::Lost => {
                gpu.surface
                    .configure(&gpu.device, gpu.configuration.clone())
                    .expect("recover surface");
                return;
            }
            SurfaceFrameStatus::Timeout | SurfaceFrameStatus::Occluded => return,
            _ => return,
        };
        let view = frame
            .texture()
            .create_view(TextureViewDescriptor::default());
        let mut encoder = gpu
            .device
            .create_command_encoder(CommandEncoderDescriptor::default());
        encoder
            .render_pass(RenderPassDescriptor {
                label: Some("clear window".into()),
                color_attachments: vec![Some(RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    load: LoadOp::Clear(Color {
                        r: 0.03,
                        g: 0.06,
                        b: 0.12,
                        a: 1.0,
                    }),
                    store: StoreOp::Store,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
            })
            .expect("record clear");
        gpu.queue
            .submit([encoder.finish().expect("finish encoder")])
            .expect("submit clear");
        frame.present().expect("present");
        gpu.device
            .poll(astrelis_gpu::PollMode::Poll)
            .expect("poll device");
    }
}

impl Application for App {
    type UserEvent = ();

    fn resumed(&mut self, context: &mut PlatformContext<'_, ()>) {
        if self.window.is_some() {
            return;
        }
        let window = context
            .create_window(WindowAttributes {
                title: "Astrelis GPU clear".into(),
                ..Default::default()
            })
            .expect("create window");
        let surface = self
            .instance
            .create_surface(SurfaceTarget::new(window.clone()))
            .expect("create surface");
        let adapter = pollster::block_on(self.instance.request_adapter(RequestAdapterOptions {
            compatible_surface: Some(surface.clone()),
            ..Default::default()
        }))
        .expect("request adapter");
        let (device, queue) =
            pollster::block_on(adapter.request_device(DeviceDescriptor::default()))
                .expect("request device");
        let capabilities = surface
            .capabilities(&adapter)
            .expect("surface capabilities");
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
        let size = window.inner_size().expect("window size");
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
            .expect("configure surface");
        window.request_redraw();
        self.window = Some(window);
        self.gpu = Some(GpuState {
            surface,
            device,
            queue,
            configuration,
        });
    }

    fn window_event(
        &mut self,
        context: &mut PlatformContext<'_, ()>,
        _id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                self.gpu = None;
                self.window = None;
                context.exit();
            }
            WindowEvent::Resized(size) => self.configure(size.width, size.height),
            WindowEvent::ScaleFactorChanged { inner_size, .. } => {
                self.configure(inner_size.width, inner_size.height);
            }
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }
}

fn main() -> Result<(), astrelis_platform::PlatformError> {
    astrelis_platform_winit::run(App::default())
}
