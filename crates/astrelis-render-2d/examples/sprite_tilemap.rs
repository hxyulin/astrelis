//! Direct-window sprite, atlas, tilemap, camera, and batching demo.

use std::time::Instant;

use astrelis_core::{
    color::Color,
    geometry::Size,
    math::{Affine2, UVec2, Vec2},
};
use astrelis_gpu::{
    CompositeAlphaMode, DeviceDescriptor, PresentMode, RequestAdapterOptions, SurfaceConfiguration,
    SurfaceFrameStatus, SurfaceTarget, TextureUsages,
};
use astrelis_platform::{
    Application, PlatformContext, Window, WindowAttributes, WindowEvent, WindowId,
};
use astrelis_render::RenderTarget;
use astrelis_render_2d::{
    Camera2D, DrawList2D, Renderer2D, SpriteDraw, TextureOptions, TileAtlas, Tilemap, TilemapDraw,
};

struct GpuState {
    surface: astrelis_gpu::Surface,
    device: astrelis_gpu::Device,
    queue: astrelis_gpu::Queue,
    configuration: SurfaceConfiguration,
    renderer: Renderer2D,
    atlas: TileAtlas,
    map: Tilemap,
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
            .expect("configure surface");
    }

    fn redraw(&mut self) {
        let Some(gpu) = &mut self.gpu else { return };
        let frame = match gpu.surface.acquire().expect("acquire frame") {
            SurfaceFrameStatus::Ready(frame) | SurfaceFrameStatus::Suboptimal(frame) => frame,
            SurfaceFrameStatus::Outdated | SurfaceFrameStatus::Lost => {
                gpu.surface
                    .configure(&gpu.device, gpu.configuration.clone())
                    .expect("recover surface");
                return;
            }
            SurfaceFrameStatus::Timeout | SurfaceFrameStatus::Occluded => return,
            _ => return,
        };
        let scale = self
            .window
            .as_ref()
            .map_or(1.0, |window| window.scale_factor() as f32);
        let logical = Vec2::new(
            gpu.configuration.width as f32 / scale,
            gpu.configuration.height as f32 / scale,
        );
        let time = self.started.elapsed().as_secs_f32();
        let camera = Camera2D {
            center: Vec2::new(256.0, 192.0),
            rotation: time.sin() * 0.08,
            zoom: 1.0 + time.cos() * 0.08,
        };
        let mut list = DrawList2D::new();
        gpu.map.record_visible(
            &mut list,
            gpu.atlas,
            camera,
            logical,
            TilemapDraw::default(),
        );
        list.draw_sprite(SpriteDraw {
            texture: gpu.atlas.texture,
            source: gpu.atlas.source(3),
            transform: Affine2::from_scale_angle_translation(
                Vec2::splat(2.5),
                time,
                Vec2::new(256.0, 192.0),
            ),
            size: Vec2::splat(32.0),
            pivot: Vec2::splat(0.5),
            tint: Color::WHITE,
            layer: 1,
        });
        let view = frame.texture().create_view(Default::default());
        let target = RenderTarget {
            view,
            allocation_size: Size::new(gpu.configuration.width, gpu.configuration.height),
            render_size: Size::new(gpu.configuration.width, gpu.configuration.height),
            scale_factor: scale,
            clear_color: Color::rgb(0.02, 0.03, 0.07),
        };
        let mut encoder = gpu.device.create_command_encoder(Default::default());
        gpu.renderer
            .render(&mut encoder, &target, &camera, &list)
            .expect("render 2D scene");
        gpu.queue
            .submit([encoder.finish().expect("finish encoder")])
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
                title: "Astrelis 2D scene renderer".into(),
                inner_size: Some(Size::new(900.0, 650.0)),
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
            view_formats: vec![],
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
            Renderer2D::new(device.clone(), queue.clone(), Default::default()).expect("renderer");
        let pixels = [
            255, 80, 80, 255, 80, 255, 130, 255, 80, 140, 255, 255, 255, 220, 80, 255,
        ];
        let texture = renderer
            .create_texture_rgba8(Size::new(2, 2), &pixels, TextureOptions::default())
            .expect("atlas");
        let atlas = TileAtlas {
            texture,
            texture_size: Size::new(2, 2),
            tile_size: Size::new(1, 1),
            margin: UVec2::ZERO,
            spacing: UVec2::ZERO,
        };
        let mut map =
            Tilemap::with_default_chunks(UVec2::new(16, 12), Vec2::splat(32.0)).expect("tilemap");
        for y in 0..12 {
            for x in 0..16 {
                map.set_tile(UVec2::new(x, y), Some((x + y) % 4));
            }
        }
        self.window = Some(window);
        self.gpu = Some(GpuState {
            surface,
            device,
            queue,
            configuration,
            renderer,
            atlas,
            map,
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
