//! Clear-color example.
//!
//! Opens a window and clears it to a cycling color each frame.
//! This is the simplest possible GPU example — no shaders, no geometry,
//! just surface acquisition, a render pass with a clear color, and present.
//!
//! Run with:
//! ```sh
//! cargo run -p astrelis-gpu --example clear_color
//! ```

use astrelis_gpu::{Gpu, GpuConfig, GpuError};
use astrelis_gpu::command::{ColorAttachment, RenderPassDescriptor};
use astrelis_gpu::surface::SurfaceConfiguration;
use astrelis_gpu::types::{LoadOp, PresentMode, StoreOp};
use astrelis_window::backend::{AppHandler, EventLoopContext};
use astrelis_window::control_flow::ControlFlow;
use astrelis_window::event::WindowEvent;
use astrelis_window::lifecycle::AppLifecycle;
use astrelis_window::types::LogicalInnerSize;
use astrelis_window::window_id::WindowId;
use astrelis_window::WindowBuilder;

use astrelis_core::color::Color;

struct App {
    window_id: Option<WindowId>,
    gpu: Option<Gpu>,
    surface: Option<astrelis_gpu::Surface<'static>>,
    frame_count: u64,
}

impl AppHandler for App {
    fn on_lifecycle(&mut self, ctx: &mut dyn EventLoopContext, state: AppLifecycle) {
        astrelis_profiling::profile_function!();
        match state {
            AppLifecycle::Resumed => {
                let attrs = WindowBuilder::new()
                    .with_title("Astrelis — Clear Color")
                    .with_inner_size(LogicalInnerSize::new(800.0, 600.0))
                    .build();
                let win_id = ctx.create_window(attrs).expect("failed to create window");
                self.window_id = Some(win_id);

                // Initialize GPU.
                let gpu =
                    Gpu::new(&GpuConfig::default()).expect("failed to create GPU backend");
                println!("GPU: {} ({:?})", gpu.device().adapter_info().name, gpu.device().adapter_info().backend);

                // Create surface from window.
                let window = ctx.window(win_id).expect("window not found");
                let mut surface = gpu.create_surface(window).expect("failed to create surface");

                // Configure the surface.
                let size = window.inner_size().physical();
                let config = SurfaceConfiguration {
                    format: surface.preferred_format(),
                    width: size.width as u32,
                    height: size.height as u32,
                    present_mode: PresentMode::AutoVsync,
                    desired_maximum_frame_latency: 2,
                };
                surface.configure(&config);

                // SAFETY: surface lifetime is managed alongside gpu lifetime
                let surface: astrelis_gpu::Surface<'static> = unsafe { std::mem::transmute(surface) };

                self.gpu = Some(gpu);
                self.surface = Some(surface);
                ctx.set_control_flow(ControlFlow::Wait);
            }
            AppLifecycle::Suspended => {}
            AppLifecycle::Exiting => {
                println!("Rendered {frames} frames. Goodbye!", frames = self.frame_count);
            }
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
                if let Some(surface) = &mut self.surface {
                    let phys = size.physical();
                    let w = phys.width as u32;
                    let h = phys.height as u32;
                    if w > 0 && h > 0 {
                        let config = SurfaceConfiguration {
                            format: surface.preferred_format(),
                            width: w,
                            height: h,
                            present_mode: PresentMode::AutoVsync,
                            desired_maximum_frame_latency: 2,
                        };
                        surface.configure(&config);
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
        let (Some(gpu), Some(surface)) = (&self.gpu, &mut self.surface) else {
            return;
        };

        // Process GPU profiling results from prior frames.
        gpu.process_profiling_frames();

        // Acquire the next surface texture.
        astrelis_profiling::profile_scope!("acquire");
        let frame = match surface.acquire() {
            Ok(f) => f,
            Err(GpuError::SurfaceOutdated | GpuError::SurfaceLost) => return,
            Err(GpuError::Timeout) => return,
            Err(e) => panic!("failed to acquire surface texture: {e}"),
        };

        // Cycle through colors based on frame count.
        let t = self.frame_count as f32 / 120.0;
        let r = (t.sin() * 0.5 + 0.5).clamp(0.0, 1.0);
        let g = ((t + 2.0).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
        let b = ((t + 4.0).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
        let clear_color = Color::new(r, g, b, 1.0);

        // Record a render pass that just clears.
        astrelis_profiling::profile_scope!("encode");
        let mut encoder = gpu.device().create_command_encoder(Some("clear"));
        {
            let _pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("clear_pass"),
                color_attachments: &[ColorAttachment {
                    view: frame.view(),
                    resolve_target: None,
                    load_op: LoadOp::Clear(clear_color),
                    store_op: StoreOp::Store,
                }],
                depth_stencil_attachment: None,
            });
            // No draw calls — just clear.
        }

        astrelis_profiling::profile_scope!("submit");
        gpu.submit(std::iter::once(encoder));
        astrelis_profiling::profile_scope!("present");
        frame.present();
        self.frame_count += 1;
    }
}

fn main() {
    astrelis_profiling::init();
    
    let mut app = App {
        window_id: None,
        gpu: None,
        surface: None,
        frame_count: 0,
    };
    astrelis_window::run(&mut app).expect("event loop error");
}
