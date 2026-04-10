//! Text effects demo using SDF rendering.
//!
//! Currently demonstrates SDF text rendering at various sizes.
//! Effects (shadows, outlines, glows) are not yet implemented in the renderer.
//!
//! Run with:
//! ```sh
//! cargo run -p astrelis-text-wgpu --example text_effects
//! ```

use astrelis_core::color::Color;
use astrelis_core::math::Vec2;
use astrelis_gpu::backend::{GpuBackend, GpuConfig};
use astrelis_gpu::error::GpuError;
use astrelis_gpu::surface::{GpuSurface, SurfaceConfiguration, SurfaceTexture};
use astrelis_gpu::types::{PresentMode, TextureFormat};
use astrelis_gpu_wgpu::WgpuBackend;
use astrelis_text::{FontSystem, Text};
use astrelis_text_wgpu::{SdfTextRenderer, TextRendererConfig};
use astrelis_window::backend::{AppHandler, EventLoopContext, WindowBackend};
use astrelis_window::control_flow::ControlFlow;
use astrelis_window::event::WindowEvent;
use astrelis_window::lifecycle::AppLifecycle;
use astrelis_window::types::LogicalInnerSize;
use astrelis_window::window_id::WindowId;
use astrelis_window::WindowBuilder;
use astrelis_window_winit::WinitBackend;

type WgpuSurface = <WgpuBackend as GpuBackend>::Surface;
type WgpuSurfaceTexture = <WgpuSurface as GpuSurface>::Texture;

struct App {
    window_id: Option<WindowId>,
    gpu: Option<WgpuBackend>,
    surface: Option<WgpuSurface>,
    renderer: Option<SdfTextRenderer>,
    surface_format: TextureFormat,
    width: u32,
    height: u32,
}

impl AppHandler for App {
    fn on_lifecycle(&mut self, ctx: &mut dyn EventLoopContext, state: AppLifecycle) {
        astrelis_profiling::profile_function!();
        if let AppLifecycle::Resumed = state {
            let attrs = WindowBuilder::new()
                .with_title("Astrelis — SDF Text Demo")
                .with_inner_size(LogicalInnerSize::new(800.0, 600.0))
                .build();
            let win_id = ctx.create_window(attrs).expect("failed to create window");
            self.window_id = Some(win_id);

            let gpu = WgpuBackend::new(&GpuConfig::default()).expect("GPU init failed");
            let window = ctx.window(win_id).expect("window not found");
            let mut surface = gpu.create_surface(window).expect("surface creation failed");

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

            let font_system = FontSystem::with_system_fonts();
            let config = TextRendererConfig::new().with_surface_format(
                astrelis_gpu_wgpu::convert::types::texture_format(self.surface_format),
            );
            let renderer = SdfTextRenderer::new(gpu.device(), font_system, config);

            self.gpu = Some(gpu);
            self.surface = Some(surface);
            self.renderer = Some(renderer);
            ctx.set_control_flow(ControlFlow::Wait);
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
            gpu.device().process_gpu_profiling_frames();
        }
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
        let frame: WgpuSurfaceTexture = match surface.acquire() {
            Ok(f) => f,
            Err(GpuError::SurfaceOutdated | GpuError::SurfaceLost | GpuError::Timeout) => return,
            Err(e) => panic!("failed to acquire: {e}"),
        };

        let wgpu_device = gpu.device();
        let views = wgpu_device.texture_views();
        let wgpu_view = views.get(frame.view()).expect("texture view not found");

        let mut encoder =
            wgpu_device
                .wgpu_device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("text_effects_encoder"),
                });

        // Clear
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: wgpu_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.05,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }

        astrelis_profiling::profile_scope!("prepare_text");
        // SDF text at various sizes - demonstrates resolution-independent rendering
        let texts = [
            ("SDF Text — 24px", 24.0, Color::WHITE),
            ("SDF Text — 32px", 32.0, Color::CYAN),
            ("SDF Text — 48px", 48.0, Color::YELLOW),
            ("SDF Text — 64px", 64.0, Color::GREEN),
            ("SDF Text — 96px", 96.0, Color::new(1.0, 0.5, 0.3, 1.0)),
        ];

        let mut y = 20.0;
        for (content, size, color) in &texts {
            let text = Text::new(*content).size(*size).color(*color);
            let mut buf = renderer.prepare(&text);
            renderer.draw_text(&mut buf, Vec2::new(30.0, y));
            y += size * 1.5;
        }

        astrelis_profiling::profile_scope!("encode");
        renderer.render(wgpu_device, &mut encoder, wgpu_view, self.width, self.height);

        astrelis_profiling::profile_scope!("submit");
        wgpu_device
            .wgpu_queue()
            .submit(std::iter::once(encoder.finish()));
        astrelis_profiling::profile_scope!("present");
        frame.present();
    }
}

fn main() {
    astrelis_profiling::init();
    let backend = WinitBackend::new().expect("failed to create windowing backend");
    let mut app = App {
        window_id: None,
        gpu: None,
        surface: None,
        renderer: None,
        surface_format: TextureFormat::Bgra8UnormSrgb,
        width: 800,
        height: 600,
    };
    backend.run(&mut app).expect("event loop error");
}
