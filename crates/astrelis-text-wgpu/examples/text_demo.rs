//! Text rendering demo.
//!
//! Renders plain text at various sizes, colors, and alignments using
//! the bitmap text renderer.
//!
//! Run with:
//! ```sh
//! cargo run -p astrelis-text-wgpu --example text_demo
//! ```

use astrelis_core::color::Color;
use astrelis_gpu::{Gpu, GpuConfig};
use astrelis_gpu::GpuError;
use astrelis_gpu::surface::SurfaceConfiguration;
use astrelis_gpu::types::{PresentMode, TextureFormat};


use astrelis_text::{FontSystem, Text};
use astrelis_text_wgpu::{BitmapTextRenderer, TextRendererConfig};
use astrelis_window::backend::{AppHandler, EventLoopContext};
use astrelis_window::control_flow::ControlFlow;
use astrelis_window::event::WindowEvent;
use astrelis_window::lifecycle::AppLifecycle;
use astrelis_window::types::LogicalInnerSize;
use astrelis_window::window_id::WindowId;
use astrelis_window::WindowBuilder;

struct App {
    window_id: Option<WindowId>,
    gpu: Option<Gpu>,
    surface: Option<astrelis_gpu::Surface>,
    renderer: Option<BitmapTextRenderer>,
    surface_format: TextureFormat,
    width: u32,
    height: u32,
}

impl AppHandler for App {
    fn on_lifecycle(&mut self, ctx: &mut dyn EventLoopContext, state: AppLifecycle) {
        astrelis_profiling::profile_function!();
        match state {
            AppLifecycle::Resumed => {
                let attrs = WindowBuilder::new()
                    .with_title("Astrelis — Text Demo")
                    .with_inner_size(LogicalInnerSize::new(800.0, 600.0))
                    .build();
                let win_id = ctx.create_window(attrs).expect("failed to create window");
                self.window_id = Some(win_id);

                let gpu =
                    Gpu::new(&GpuConfig::default()).expect("failed to create GPU backend");

                let window = ctx.window(win_id).expect("window not found");
                let mut surface = gpu.create_surface(window).expect("failed to create surface");

                let size = window.inner_size().physical();
                self.surface_format = surface.preferred_format();
                self.width = size.width as u32;
                self.height = size.height as u32;

                surface.configure(&SurfaceConfiguration {
                    format: self.surface_format,
                    width: self.width,
                    height: self.height,
                    present_mode: PresentMode::AutoVsync,
                    desired_maximum_frame_latency: 2,
                });

                // Create text renderer
                let font_system = FontSystem::with_system_fonts();
                let config = TextRendererConfig::new()
                    .with_surface_format(
                        astrelis_gpu::convert::types::texture_format(self.surface_format),
                    );
                let renderer = BitmapTextRenderer::new(&gpu, font_system, config);

                self.gpu = Some(gpu);
                self.surface = Some(surface);
                self.renderer = Some(renderer);
                ctx.set_control_flow(ControlFlow::Poll);
            }
            AppLifecycle::Suspended => {}
            AppLifecycle::Exiting => {}
        }
    }

    fn on_window_event(
        &mut self,
        ctx: &mut dyn EventLoopContext,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        astrelis_profiling::profile_function!();
        match event {
            WindowEvent::CloseRequested => ctx.exit(),
            WindowEvent::Resized(size) => {
                astrelis_profiling::profile_scope!("resize");
                let phys = size.physical();
                self.width = phys.width as u32;
                self.height = phys.height as u32;
                if self.width > 0 && self.height > 0 {
                    if let Some(surface) = &mut self.surface {
                        surface.configure(&SurfaceConfiguration {
                            format: self.surface_format,
                            width: self.width,
                            height: self.height,
                            present_mode: PresentMode::AutoVsync,
                            desired_maximum_frame_latency: 2,
                        });
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                astrelis_profiling::profile_scope!("redraw");
                self.render();
                if let Some(win) = ctx.window(window_id) {
                    win.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn on_events_cleared(&mut self, ctx: &mut dyn EventLoopContext) {
        astrelis_profiling::profile_function!();
        if let Some(gpu) = &self.gpu {
            gpu.process_profiling_frames();
        }
        astrelis_profiling::new_frame();
        if let Some(id) = self.window_id
            && let Some(win) = ctx.window(id)
        {
            win.request_redraw();
        }
    }
}

impl App {
    fn render(&mut self) {
        astrelis_profiling::profile_function!();
        let (Some(gpu), Some(surface), Some(renderer)) =
            (&self.gpu, &mut self.surface, &mut self.renderer)
        else {
            return;
        };

        astrelis_profiling::profile_scope!("acquire");
        let frame = match surface.acquire() {
            Ok(f) => f,
            Err(GpuError::SurfaceOutdated | GpuError::SurfaceLost | GpuError::Timeout) => return,
            Err(e) => panic!("failed to acquire: {e}"),
        };

        // Clear pass — goes through astrelis-gpu wrapper for GPU profiling.
        astrelis_profiling::profile_scope!("clear");
        let mut clear_encoder = gpu.device().create_command_encoder(Some("text_demo_clear"));
        {
            let _pass = clear_encoder.begin_render_pass(
                &astrelis_gpu::command::RenderPassDescriptor {
                    label: Some("clear"),
                    color_attachments: &[astrelis_gpu::command::ColorAttachment {
                        view: frame.view(),
                        resolve_target: None,
                        load_op: astrelis_gpu::types::LoadOp::Clear(Color::new(0.1, 0.1, 0.15, 1.0)),
                        store_op: astrelis_gpu::types::StoreOp::Store,
                    }],
                    depth_stencil_attachment: None,
                },
            );
        }
        gpu.submit(std::iter::once(clear_encoder));

        astrelis_profiling::profile_scope!("prepare_text");
        let texts = [
            Text::new("Hello, Astrelis!").size(32.0).color(Color::WHITE),
            Text::new("Small text (14px)")
                .size(14.0)
                .color(Color::new(0.8, 0.8, 0.8, 1.0)),
            Text::new("Medium text (20px)")
                .size(20.0)
                .color(Color::YELLOW),
            Text::new("Bold text").size(24.0).color(Color::CYAN).bold(),
            Text::new("Italic text")
                .size(24.0)
                .color(Color::GREEN)
                .italic(),
            Text::new("Red on dark background")
                .size(18.0)
                .color(Color::RED),
        ];

        let mut y = 30.0;
        for text in &texts {
            let mut buffer = renderer.prepare(text);
            renderer.draw_text(&mut buffer, astrelis_core::math::Vec2::new(30.0, y));
            y += text.font_size * 2.0;
        }

        // Text render pass — profiled via gpu_profile_scope.
        astrelis_profiling::profile_scope!("encode");
        let mut encoder =
            gpu.raw_device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("text_demo_text"),
                });
        let wgpu_view = frame.view().raw();
        let w = self.width;
        let h = self.height;
        gpu.device().gpu_profile_scope("text_render", &mut encoder, |enc| {
            renderer.render(gpu, enc, wgpu_view, w, h);
        });

        astrelis_profiling::profile_scope!("submit");
        gpu.raw_queue().submit(std::iter::once(encoder.finish()));
        astrelis_profiling::profile_scope!("present");
        frame.present();
    }
}

fn main() {
    astrelis_profiling::init();
    astrelis_profiling::set_thread_name("main");

    let mut app = App {
        window_id: None,
        gpu: None,
        surface: None,
        renderer: None,
        surface_format: TextureFormat::Bgra8UnormSrgb,
        width: 800,
        height: 600,
    };
    astrelis_window::run(&mut app).expect("event loop error");
}
