//! Rich text demo with mixed styles.
//!
//! Shows rich text with bold, italic, colored, and differently-sized spans
//! using the hybrid font renderer.
//!
//! Run with:
//! ```sh
//! cargo run -p astrelis-text-wgpu --example rich_text_demo
//! ```

use astrelis_core::color::Color;
use astrelis_core::math::Vec2;
use astrelis_gpu::backend::{GpuBackend, GpuConfig};
use astrelis_gpu::error::GpuError;
use astrelis_gpu::surface::{GpuSurface, SurfaceConfiguration, SurfaceTexture};
use astrelis_gpu::types::{PresentMode, TextureFormat};
use astrelis_gpu_wgpu::WgpuBackend;
use astrelis_text::{FontSystem, RichTextBuilder, Text};
use astrelis_text_wgpu::{FontRenderer, TextRendererConfig};
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
    renderer: Option<FontRenderer>,
    surface_format: TextureFormat,
    width: u32,
    height: u32,
}

impl AppHandler for App {
    fn on_lifecycle(&mut self, ctx: &mut dyn EventLoopContext, state: AppLifecycle) {
        if let AppLifecycle::Resumed = state {
            let attrs = WindowBuilder::new()
                .with_title("Astrelis — Rich Text Demo")
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
            let renderer = FontRenderer::new(gpu.device(), font_system, config);

            self.gpu = Some(gpu);
            self.surface = Some(surface);
            self.renderer = Some(renderer);
            ctx.set_control_flow(ControlFlow::Poll);
        }
    }

    fn on_window_event(
        &mut self,
        ctx: &mut dyn EventLoopContext,
        window_id: WindowId,
        event: WindowEvent,
    ) {
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
        if let Some(id) = self.window_id
            && let Some(win) = ctx.window(id)
        {
            win.request_redraw();
        }
    }
}

impl App {
    fn render(&mut self) {
        let (Some(gpu), Some(surface), Some(renderer)) =
            (&self.gpu, &mut self.surface, &mut self.renderer)
        else {
            return;
        };

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
                    label: Some("rich_text_encoder"),
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
                            r: 0.08,
                            g: 0.08,
                            b: 0.12,
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

        // Build rich text with the builder API
        let rich = RichTextBuilder::new()
            .text("This is ")
            .bold("bold")
            .text(", ")
            .italic("italic")
            .text(", and ")
            .colored("colored", Color::CYAN)
            .text(" text.")
            .default_size(24.0)
            .build();

        // Render each span as a separate Text at advancing x positions
        let segments = rich.to_text_segments();
        let mut x = 30.0f32;
        let y = 40.0f32;

        for (text, _style) in &segments {
            let mut buffer = renderer.prepare(text);
            renderer.draw_text(text, &mut buffer, Vec2::new(x, y));
            let bounds = buffer.bounds();
            x += bounds.0;
        }

        // Render plain text examples at different sizes
        let examples = [
            ("Title (Bitmap - 16px)", 16.0, Color::WHITE),
            ("Subtitle (SDF - 32px)", 32.0, Color::new(0.7, 0.9, 1.0, 1.0)),
            ("Heading (SDF - 48px)", 48.0, Color::YELLOW),
        ];

        let mut y_pos = 120.0;
        for (content, size, color) in &examples {
            let text = Text::new(*content).size(*size).color(*color);
            let mut buffer = renderer.prepare(&text);
            renderer.draw_text(&text, &mut buffer, Vec2::new(30.0, y_pos));
            y_pos += size * 2.0;
        }

        // Render a span with custom style
        let custom = Text::new("Custom: scaled + colored")
            .size(20.0)
            .color(Color::GREEN)
            .bold();
        let mut buffer = renderer.prepare(&custom);
        renderer.draw_text(&custom, &mut buffer, Vec2::new(30.0, y_pos));

        renderer.render(wgpu_device, &mut encoder, wgpu_view, self.width, self.height);

        wgpu_device
            .wgpu_queue()
            .submit(std::iter::once(encoder.finish()));
        frame.present();
    }
}

fn main() {
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
