//! Resizable vector-style paint demo using the neutral platform and GPU APIs.

use astrelis_core::{
    color::Color,
    geometry::{Point, Rect, Size},
    math::{Affine2, Vec2},
};
use astrelis_gpu::{
    CompositeAlphaMode, DeviceDescriptor, PresentMode, RequestAdapterOptions, SurfaceConfiguration,
    SurfaceFrameStatus, SurfaceTarget, TextureUsages, TextureViewDescriptor,
};
use astrelis_paint::{
    Brush, CornerRadii, FillRule, Image, ImageOptions, Painter, Path, RoundedRect, StrokeStyle,
};
use astrelis_paint_gpu::{RenderTarget, Renderer, RendererOptions};
use astrelis_platform::{
    Application, PlatformContext, Window, WindowAttributes, WindowEvent, WindowId,
};

struct GpuState {
    window: Window,
    surface: astrelis_gpu::Surface,
    configuration: SurfaceConfiguration,
    device: astrelis_gpu::Device,
    queue: astrelis_gpu::Queue,
    renderer: Renderer,
}

struct Demo {
    instance: astrelis_gpu::Instance,
    gpu: Option<GpuState>,
}

impl Default for Demo {
    fn default() -> Self {
        Self {
            instance: astrelis_gpu_wgpu::create_instance(Default::default()),
            gpu: None,
        }
    }
}

fn display_list(width: f32, height: f32) -> astrelis_paint::DisplayList {
    let mut star = Path::builder();
    star.move_to(Point::new(0.0, -54.0)).unwrap();
    for index in 1..10 {
        let angle = -std::f32::consts::FRAC_PI_2 + index as f32 * std::f32::consts::PI / 5.0;
        let radius = if index % 2 == 0 { 54.0 } else { 24.0 };
        star.line_to(Point::new(angle.cos() * radius, angle.sin() * radius))
            .unwrap();
    }
    star.close().unwrap();
    let star = star.finish();
    let checker = Image::from_rgba8(
        Size::new(2, 2),
        vec![
            255, 255, 255, 255, 40, 80, 180, 255, 40, 80, 180, 255, 255, 255, 255, 255,
        ],
    )
    .unwrap();

    let mut painter = Painter::new();
    painter
        .fill_rounded_rect(
            RoundedRect::new(
                Rect::from_xywh(
                    24.0,
                    24.0,
                    (width - 48.0).max(0.0),
                    (height - 48.0).max(0.0),
                ),
                CornerRadii::uniform(22.0),
            )
            .unwrap(),
            Brush::Solid(Color::new(0.08, 0.11, 0.18, 1.0)),
        )
        .unwrap();
    painter
        .with_save(|painter| {
            painter.clip_rect(Rect::from_xywh(
                48.0,
                48.0,
                (width - 96.0).max(0.0),
                (height - 96.0).max(0.0),
            ))?;
            painter.transform(Affine2::from_translation(Vec2::new(
                width * 0.5,
                height * 0.5,
            )))?;
            painter.transform(Affine2::from_angle(0.22))?;
            painter.clip_path(&star, FillRule::NonZero)?;
            painter.draw_image(
                &checker,
                Rect::from_xywh(-90.0, -90.0, 180.0, 180.0),
                ImageOptions::default(),
            )?;
            painter.stroke_path(
                &star,
                StrokeStyle {
                    width: 7.0,
                    ..Default::default()
                },
                Brush::Solid(Color::new(0.2, 0.9, 1.0, 0.9)),
            )
        })
        .unwrap();
    painter.finish().unwrap()
}

impl Demo {
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
        let scale = gpu.window.scale_factor() as f32;
        let logical_width = gpu.configuration.width as f32 / scale;
        let logical_height = gpu.configuration.height as f32 / scale;
        let list = display_list(logical_width, logical_height);
        let view = frame
            .texture()
            .create_view(TextureViewDescriptor::default());
        let mut encoder = gpu.device.create_command_encoder(Default::default());
        gpu.renderer
            .render(
                &mut encoder,
                &list,
                RenderTarget {
                    view,
                    format: gpu.configuration.format,
                    size: Size::new(gpu.configuration.width, gpu.configuration.height),
                    scale_factor: scale,
                    clear_color: Color::new(0.015, 0.02, 0.04, 1.0),
                },
            )
            .expect("render display list");
        gpu.queue
            .submit([encoder.finish().expect("finish encoder")])
            .expect("submit");
        frame.present().expect("present");
    }
}

impl Application for Demo {
    type UserEvent = ();

    fn resumed(&mut self, context: &mut PlatformContext<'_, ()>) {
        if self.gpu.is_some() {
            return;
        }
        let window = context
            .create_window(WindowAttributes {
                title: "Astrelis vector paint".into(),
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
        let renderer = Renderer::new(device.clone(), queue.clone(), RendererOptions::default())
            .expect("paint renderer");
        window.request_redraw();
        self.gpu = Some(GpuState {
            window,
            surface,
            configuration,
            device,
            queue,
            renderer,
        });
    }

    fn window_event(
        &mut self,
        context: &mut PlatformContext<'_, ()>,
        _id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => context.exit(),
            WindowEvent::RedrawRequested => self.redraw(),
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.gpu
                    && size.width > 0
                    && size.height > 0
                {
                    gpu.configuration.width = size.width;
                    gpu.configuration.height = size.height;
                    gpu.surface
                        .configure(&gpu.device, gpu.configuration.clone())
                        .expect("resize surface");
                    gpu.window.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { inner_size, .. } => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.configuration.width = inner_size.width.max(1);
                    gpu.configuration.height = inner_size.height.max(1);
                    gpu.surface
                        .configure(&gpu.device, gpu.configuration.clone())
                        .expect("DPI surface resize");
                    gpu.window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

fn main() {
    astrelis_platform_winit::run(Demo::default()).expect("run vector demo");
}
