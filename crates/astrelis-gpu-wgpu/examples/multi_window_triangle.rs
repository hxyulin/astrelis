//! Draws one triangle into two independently resized windows using one device.

use astrelis_gpu::{
    BufferUsages, Color, ColorTargetState, ColorWrites, CommandEncoderDescriptor,
    CompositeAlphaMode, DeviceDescriptor, FragmentState, LoadOp, PresentMode, PrimitiveState,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor,
    RequestAdapterOptions, ShaderModuleDescriptor, StoreOp, SurfaceConfiguration,
    SurfaceFrameStatus, SurfaceTarget, TextureUsages, TextureViewDescriptor, VertexAttribute,
    VertexBufferLayout, VertexFormat, VertexState, VertexStepMode,
};
use astrelis_platform::{
    Application, PlatformContext, Window, WindowAttributes, WindowEvent, WindowId,
};

struct WindowSurface {
    window: Window,
    surface: astrelis_gpu::Surface,
    configuration: SurfaceConfiguration,
}

struct GpuState {
    device: astrelis_gpu::Device,
    queue: astrelis_gpu::Queue,
    vertex_buffer: astrelis_gpu::Buffer,
    pipeline: astrelis_gpu::RenderPipeline,
    windows: Vec<WindowSurface>,
}

struct App {
    instance: astrelis_gpu::Instance,
    gpu: Option<GpuState>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            instance: astrelis_gpu_wgpu::create_instance(Default::default()),
            gpu: None,
        }
    }
}

impl App {
    fn redraw(&mut self, id: WindowId) {
        let Some(gpu) = &mut self.gpu else { return };
        let Some(window) = gpu.windows.iter_mut().find(|entry| entry.window.id() == id) else {
            return;
        };
        let frame = match window.surface.acquire().expect("acquire frame") {
            SurfaceFrameStatus::Ready(frame) => frame,
            SurfaceFrameStatus::Suboptimal(frame) => {
                window
                    .surface
                    .configure(&gpu.device, window.configuration.clone())
                    .expect("reconfigure surface");
                frame
            }
            SurfaceFrameStatus::Outdated | SurfaceFrameStatus::Lost => {
                window
                    .surface
                    .configure(&gpu.device, window.configuration.clone())
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
        {
            let mut pass = encoder
                .begin_render_pass(RenderPassDescriptor {
                    label: Some("multi-window triangle".into()),
                    color_attachments: vec![Some(RenderPassColorAttachment {
                        view,
                        resolve_target: None,
                        load: LoadOp::Clear(Color {
                            r: 0.02,
                            g: 0.02,
                            b: 0.04,
                            a: 1.0,
                        }),
                        store: StoreOp::Store,
                    })],
                    timestamp_writes: None,
                })
                .expect("begin pass");
            pass.set_pipeline(&gpu.pipeline).expect("set pipeline");
            pass.set_vertex_buffer(0, &gpu.vertex_buffer, 0..gpu.vertex_buffer.size())
                .expect("set vertex buffer");
            pass.draw(0..3, 0..1);
        }
        gpu.queue
            .submit([encoder.finish().expect("finish encoder")])
            .expect("submit");
        frame.present().expect("present");
    }
}

impl Application for App {
    type UserEvent = ();

    fn resumed(&mut self, context: &mut PlatformContext<'_, ()>) {
        if self.gpu.is_some() {
            return;
        }
        let windows: Vec<Window> = ["Astrelis triangle A", "Astrelis triangle B"]
            .into_iter()
            .map(|title| {
                context
                    .create_window(WindowAttributes {
                        title: title.into(),
                        ..Default::default()
                    })
                    .expect("create window")
            })
            .collect();
        let surfaces: Vec<_> = windows
            .iter()
            .map(|window| {
                self.instance
                    .create_surface(SurfaceTarget::new(window.clone()))
                    .expect("create surface")
            })
            .collect();
        let adapter = pollster::block_on(self.instance.request_adapter(RequestAdapterOptions {
            compatible_surface: Some(surfaces[0].clone()),
            ..Default::default()
        }))
        .expect("request adapter");
        let (device, queue) =
            pollster::block_on(adapter.request_device(DeviceDescriptor::default()))
                .expect("request device");
        let capabilities: Vec<_> = surfaces
            .iter()
            .map(|surface| {
                surface
                    .capabilities(&adapter)
                    .expect("surface capabilities")
            })
            .collect();
        let format = capabilities[0]
            .formats
            .iter()
            .copied()
            .find(|format| {
                capabilities[1].formats.contains(format)
                    && matches!(
                        format,
                        astrelis_gpu::TextureFormat::Bgra8UnormSrgb
                            | astrelis_gpu::TextureFormat::Rgba8UnormSrgb
                    )
            })
            .or_else(|| {
                capabilities[0]
                    .formats
                    .iter()
                    .copied()
                    .find(|format| capabilities[1].formats.contains(format))
            })
            .expect("surfaces have no common format");
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("triangle shader".into()),
            wgsl: r#"
                struct Output {
                    @builtin(position) position: vec4<f32>,
                    @location(0) color: vec3<f32>,
                };
                @vertex
                fn vs_main(
                    @location(0) position: vec2<f32>,
                    @location(1) color: vec3<f32>,
                ) -> Output {
                    var output: Output;
                    output.position = vec4<f32>(position, 0.0, 1.0);
                    output.color = color;
                    return output;
                }
                @fragment
                fn fs_main(input: Output) -> @location(0) vec4<f32> {
                    return vec4<f32>(input.color, 1.0);
                }
            "#
            .into(),
        });
        let pipeline = device
            .create_render_pipeline(RenderPipelineDescriptor {
                label: Some("triangle pipeline".into()),
                layout: None,
                vertex: VertexState {
                    module: shader.clone(),
                    entry_point: "vs_main".into(),
                    buffers: vec![VertexBufferLayout {
                        array_stride: 20,
                        step_mode: VertexStepMode::Vertex,
                        attributes: vec![
                            VertexAttribute {
                                offset: 0,
                                shader_location: 0,
                                format: VertexFormat::Float32x2,
                            },
                            VertexAttribute {
                                offset: 8,
                                shader_location: 1,
                                format: VertexFormat::Float32x3,
                            },
                        ],
                    }],
                },
                primitive: PrimitiveState::default(),
                fragment: Some(FragmentState {
                    module: shader,
                    entry_point: "fs_main".into(),
                    targets: vec![Some(ColorTargetState {
                        format,
                        write_mask: ColorWrites::ALL,
                    })],
                }),
            })
            .expect("create pipeline");
        let vertices: [[f32; 5]; 3] = [
            [-0.7, -0.6, 1.0, 0.1, 0.2],
            [0.7, -0.6, 0.1, 1.0, 0.3],
            [0.0, 0.7, 0.2, 0.4, 1.0],
        ];
        let vertex_buffer = device
            .create_buffer_init(
                &queue,
                Some("triangle vertices".into()),
                bytemuck::cast_slice(&vertices),
                BufferUsages::VERTEX,
            )
            .expect("create vertex buffer");
        let window_surfaces = windows
            .into_iter()
            .zip(surfaces)
            .zip(capabilities)
            .map(|((window, surface), capabilities)| {
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
                WindowSurface {
                    window,
                    surface,
                    configuration,
                }
            })
            .collect();
        self.gpu = Some(GpuState {
            device,
            queue,
            vertex_buffer,
            pipeline,
            windows: window_surfaces,
        });
    }

    fn window_event(
        &mut self,
        context: &mut PlatformContext<'_, ()>,
        id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.windows.retain(|entry| entry.window.id() != id);
                    if gpu.windows.is_empty() {
                        self.gpu = None;
                        context.exit();
                    }
                }
            }
            WindowEvent::Resized(size) => {
                if size.width > 0
                    && size.height > 0
                    && let Some(gpu) = &mut self.gpu
                    && let Some(window) =
                        gpu.windows.iter_mut().find(|entry| entry.window.id() == id)
                {
                    window.configuration.width = size.width;
                    window.configuration.height = size.height;
                    window
                        .surface
                        .configure(&gpu.device, window.configuration.clone())
                        .expect("resize surface");
                    window.window.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { inner_size, .. } => {
                if let Some(gpu) = &mut self.gpu
                    && let Some(window) =
                        gpu.windows.iter_mut().find(|entry| entry.window.id() == id)
                    && inner_size.width > 0
                    && inner_size.height > 0
                {
                    window.configuration.width = inner_size.width;
                    window.configuration.height = inner_size.height;
                    window
                        .surface
                        .configure(&gpu.device, window.configuration.clone())
                        .expect("DPI resize surface");
                    window.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => self.redraw(id),
            _ => {}
        }
    }
}

fn main() -> Result<(), astrelis_platform::PlatformError> {
    astrelis_platform_winit::run(App::default())
}
