//! Direct-window Lambert-lit mesh, transparency, and debug geometry demo.

use std::time::Instant;

use astrelis_core::{
    color::Color,
    geometry::Size,
    math::{Mat4, Quat, Vec3},
};
use astrelis_gpu::{
    CompositeAlphaMode, DeviceDescriptor, PresentMode, RequestAdapterOptions, SurfaceConfiguration,
    SurfaceFrameStatus, SurfaceTarget, TextureUsages,
};
use astrelis_platform::{
    Application, PlatformContext, Window, WindowAttributes, WindowEvent, WindowId,
};
use astrelis_render::RenderTarget;
use astrelis_render_3d::{
    AlphaMode, Camera3D, DrawList3D, Lighting, MaterialDescriptor, MaterialHandle, MeshDraw,
    MeshHandle, Renderer3D, cube, plane, uv_sphere,
};

struct GpuState {
    surface: astrelis_gpu::Surface,
    device: astrelis_gpu::Device,
    queue: astrelis_gpu::Queue,
    configuration: SurfaceConfiguration,
    renderer: Renderer3D,
    cube: MeshHandle,
    sphere: MeshHandle,
    plane: MeshHandle,
    red: MaterialHandle,
    blue: MaterialHandle,
    ground: MaterialHandle,
}
struct Demo {
    instance: astrelis_gpu::Instance,
    window: Option<Window>,
    gpu: Option<GpuState>,
    started: Instant,
}
impl Default for Demo {
    fn default() -> Self {
        Self {
            instance: astrelis_gpu_wgpu::create_instance(Default::default()),
            window: None,
            gpu: None,
            started: Instant::now(),
        }
    }
}

impl Demo {
    fn configure(&mut self, width: u32, height: u32) {
        let Some(gpu) = &mut self.gpu else { return };
        if width == 0 || height == 0 {
            return;
        }
        gpu.configuration.width = width;
        gpu.configuration.height = height;
        gpu.surface
            .configure(&gpu.device, gpu.configuration.clone())
            .expect("configure");
    }
    fn redraw(&mut self) {
        let Some(gpu) = &mut self.gpu else { return };
        let frame = match gpu.surface.acquire().expect("acquire") {
            SurfaceFrameStatus::Ready(frame) | SurfaceFrameStatus::Suboptimal(frame) => frame,
            SurfaceFrameStatus::Outdated | SurfaceFrameStatus::Lost => {
                gpu.surface
                    .configure(&gpu.device, gpu.configuration.clone())
                    .expect("recover");
                return;
            }
            SurfaceFrameStatus::Timeout | SurfaceFrameStatus::Occluded => return,
            _ => return,
        };
        let time = self.started.elapsed().as_secs_f32();
        let mut camera = Camera3D {
            position: Vec3::new(time.sin() * 7.0, 4.0, time.cos() * 7.0),
            ..Default::default()
        };
        camera.look_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y);
        let mut list = DrawList3D::new();
        list.draw_mesh(MeshDraw {
            mesh: gpu.plane,
            material: gpu.ground,
            transform: Mat4::from_translation(Vec3::new(0.0, -1.0, 0.0)),
            tint: Color::WHITE,
        });
        list.draw_mesh(MeshDraw {
            mesh: gpu.cube,
            material: gpu.red,
            transform: Mat4::from_rotation_translation(Quat::from_rotation_y(time), Vec3::ZERO),
            tint: Color::WHITE,
        });
        list.draw_mesh(MeshDraw {
            mesh: gpu.sphere,
            material: gpu.blue,
            transform: Mat4::from_scale_rotation_translation(
                Vec3::splat(0.8),
                Quat::IDENTITY,
                Vec3::new(time.cos() * 2.3, 0.5, time.sin() * 2.3),
            ),
            tint: Color::WHITE,
        });
        list.draw_grid(8, 0.5, Color::new(0.2, 0.3, 0.45, 0.55));
        list.draw_axes(Mat4::IDENTITY, 1.5);
        let target = RenderTarget {
            view: frame.texture().create_view(Default::default()),
            allocation_size: Size::new(gpu.configuration.width, gpu.configuration.height),
            render_size: Size::new(gpu.configuration.width, gpu.configuration.height),
            scale_factor: 1.0,
            clear_color: Color::rgb(0.015, 0.025, 0.06),
        };
        let mut encoder = gpu.device.create_command_encoder(Default::default());
        gpu.renderer
            .render(&mut encoder, &target, &camera, &Lighting::default(), &list)
            .expect("render 3D scene");
        gpu.queue
            .submit([encoder.finish().expect("finish")])
            .expect("submit");
        frame.present().expect("present");
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

impl Application for Demo {
    type UserEvent = ();
    fn resumed(&mut self, context: &mut PlatformContext<'_, ()>) {
        if self.window.is_some() {
            return;
        }
        let window = context
            .create_window(WindowAttributes {
                title: "Astrelis 3D scene renderer".into(),
                inner_size: Some(Size::new(960.0, 680.0)),
                ..Default::default()
            })
            .expect("window");
        let surface = self
            .instance
            .create_surface(SurfaceTarget::new(window.clone()))
            .expect("surface");
        let adapter = pollster::block_on(self.instance.request_adapter(RequestAdapterOptions {
            compatible_surface: Some(surface.clone()),
            ..Default::default()
        }))
        .expect("adapter");
        let (device, queue) =
            pollster::block_on(adapter.request_device(DeviceDescriptor::default()))
                .expect("device");
        let capabilities = surface.capabilities(&adapter).expect("capabilities");
        let size = window.inner_size().expect("size");
        let configuration = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: capabilities.formats[0],
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
            .expect("configure");
        let mut renderer =
            Renderer3D::new(device.clone(), queue.clone(), Default::default()).expect("renderer");
        let cube = renderer.create_mesh(&cube(1.8)).expect("cube");
        let sphere = renderer
            .create_mesh(&uv_sphere(1.0, 24, 12))
            .expect("sphere");
        let plane = renderer.create_mesh(&plane(12.0, 12.0)).expect("plane");
        let red = renderer
            .create_material(MaterialDescriptor {
                base_color: Color::rgb(0.95, 0.2, 0.12),
                ..Default::default()
            })
            .expect("red");
        let blue = renderer
            .create_material(MaterialDescriptor {
                base_color: Color::new(0.15, 0.5, 1.0, 0.72),
                alpha_mode: AlphaMode::Blend,
                ..Default::default()
            })
            .expect("blue");
        let ground = renderer
            .create_material(MaterialDescriptor {
                base_color: Color::rgb(0.12, 0.16, 0.22),
                double_sided: true,
                ..Default::default()
            })
            .expect("ground");
        self.window = Some(window);
        self.gpu = Some(GpuState {
            surface,
            device,
            queue,
            configuration,
            renderer,
            cube,
            sphere,
            plane,
            red,
            blue,
            ground,
        });
        self.window.as_ref().unwrap().request_redraw();
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
                self.configure(inner_size.width, inner_size.height)
            }
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }
}

fn main() -> Result<(), astrelis_platform::PlatformError> {
    astrelis_platform_winit::run(Demo::default())
}
