//! Demonstrates `ControlFlow::Wait` with demand-driven rendering.
//!
//! Unlike the other examples that use `ControlFlow::Poll` (rendering
//! every frame), this example only redraws when something changes:
//! window resize, mouse movement, keyboard input, or focus changes.
//! When idle, the event loop blocks and uses zero CPU.
//!
//! Watch the terminal — each redraw prints a line with the reason and
//! a running frame counter so you can see exactly when rendering
//! happens.
//!
//! Try: move the mouse over the window, resize it, press keys,
//! then leave it alone and watch the redraws stop.
//!
//! Run with:
//! ```sh
//! cargo run -p astrelis-gpu --example wait_mode
//! ```

use std::time::Instant;

use astrelis_core::color::Color;
use astrelis_gpu::command::{ColorAttachment, RenderPassDescriptor};
use astrelis_gpu::surface::SurfaceConfiguration;
use astrelis_gpu::types::{LoadOp, PresentMode, StoreOp};
use astrelis_gpu::{Gpu, GpuConfig, GpuError};
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
    frame_count: u64,
    start_time: Instant,
    /// The color to clear with — changes on input events so you
    /// can see the effect of each redraw.
    hue: f32,
    /// Why the most recent redraw was requested. Printed to the
    /// terminal so you can correlate redraws with events.
    redraw_reason: &'static str,
}

impl AppHandler for App {
    fn on_lifecycle(&mut self, ctx: &mut dyn EventLoopContext, state: AppLifecycle) {
        astrelis_profiling::profile_function!();
        match state {
            AppLifecycle::Resumed => {
                let attrs = WindowBuilder::new()
                    .with_title("Astrelis — Wait Mode (watch terminal)")
                    .with_inner_size(LogicalInnerSize::new(800.0, 600.0))
                    .build();
                let win_id = ctx.create_window(attrs).expect("failed to create window");
                self.window_id = Some(win_id);

                let gpu =
                    Gpu::new(&GpuConfig::default()).expect("failed to create GPU backend");
                let window = ctx.window(win_id).expect("window not found");
                let mut surface =
                    gpu.create_surface(window).expect("failed to create surface");

                let size = window.inner_size().physical();
                surface.configure(&SurfaceConfiguration {
                    format: surface.preferred_format(),
                    width: size.width as u32,
                    height: size.height as u32,
                    present_mode: PresentMode::AutoVsync,
                    desired_maximum_frame_latency: 2,
                });

                self.gpu = Some(gpu);
                self.surface = Some(surface);

                // Key: use Wait mode — the event loop blocks until
                // an OS event arrives. No CPU usage while idle.
                ctx.set_control_flow(ControlFlow::Wait);

                // Request one initial redraw so the window isn't
                // blank after creation.
                self.redraw_reason = "initial";
                if let Some(win) = ctx.window(win_id) {
                    win.request_redraw();
                }
            }
            AppLifecycle::Suspended => {}
            AppLifecycle::Exiting => {
                let elapsed = self.start_time.elapsed().as_secs_f32();
                let avg_fps = self.frame_count as f32 / elapsed;
                println!(
                    "\nRendered {frames} frames in {elapsed:.1}s ({avg_fps:.1} avg FPS). Goodbye!",
                    frames = self.frame_count,
                );
                println!(
                    "With Poll mode this would have been ~{poll_frames} frames.",
                    poll_frames = (elapsed * 60.0) as u64,
                );
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
        // Helper: request a redraw and record the reason.
        let mut needs_redraw: Option<&'static str> = None;

        match event {
            WindowEvent::CloseRequested => ctx.exit(),

            WindowEvent::Resized(size) => {
                astrelis_profiling::profile_scope!("resize");
                if let Some(surface) = &mut self.surface {
                    let phys = size.physical();
                    let w = phys.width as u32;
                    let h = phys.height as u32;
                    if w > 0 && h > 0 {
                        surface.configure(&SurfaceConfiguration {
                            format: surface.preferred_format(),
                            width: w,
                            height: h,
                            present_mode: PresentMode::AutoVsync,
                            desired_maximum_frame_latency: 2,
                        });
                    }
                }
                needs_redraw = Some("resize");
            }

            WindowEvent::CursorMoved(_) => {
                self.hue = (self.hue + 0.01) % 1.0;
                needs_redraw = Some("cursor_moved");
            }

            WindowEvent::KeyboardInput(key) => {
                if key.state.is_pressed() {
                    self.hue = (self.hue + 0.05) % 1.0;
                    needs_redraw = Some("key_press");
                }
            }

            WindowEvent::MouseButtonInput { state, .. } => {
                if state.is_pressed() {
                    self.hue = (self.hue + 0.1) % 1.0;
                    needs_redraw = Some("mouse_click");
                }
            }

            WindowEvent::Focused(_) => {
                needs_redraw = Some("focus_change");
            }

            WindowEvent::RedrawRequested => {
                astrelis_profiling::profile_scope!("redraw");
                self.render();
            }

            _ => {}
        }

        if let Some(reason) = needs_redraw {
            self.redraw_reason = reason;
            if let Some(win) = ctx.window(window_id) {
                win.request_redraw();
            }
        }
    }

    fn on_events_cleared(&mut self, _ctx: &mut dyn EventLoopContext) {
        astrelis_profiling::profile_function!();
        if let Some(gpu) = &self.gpu {
            gpu.process_profiling_frames();
        }
        astrelis_profiling::new_frame();
        // Note: we do NOT call request_redraw() here. In Wait mode,
        // redraws only happen in response to input events.
    }
}

impl App {
    fn render(&mut self) {
        astrelis_profiling::profile_function!();
        let (Some(gpu), Some(surface)) = (&self.gpu, &mut self.surface) else {
            return;
        };

        gpu.process_profiling_frames();

        astrelis_profiling::profile_scope!("acquire");
        let frame = match surface.acquire() {
            Ok(f) => f,
            Err(GpuError::SurfaceOutdated | GpuError::SurfaceLost | GpuError::Timeout) => return,
            Err(e) => panic!("failed to acquire surface texture: {e}"),
        };

        // Convert HSV hue to RGB for the clear color.
        let (r, g, b) = hsv_to_rgb(self.hue, 0.6, 0.8);
        let clear_color = Color::new(r, g, b, 1.0);

        astrelis_profiling::profile_scope!("encode");
        let mut encoder = gpu.device().create_command_encoder(Some("wait_mode"));
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
        }

        astrelis_profiling::profile_scope!("submit");
        gpu.submit(std::iter::once(encoder));
        astrelis_profiling::profile_scope!("present");
        frame.present();

        self.frame_count += 1;
        let elapsed = self.start_time.elapsed().as_secs_f32();
        println!(
            "frame {count:>5} @ {elapsed:>8.3}s  reason={reason}  hue={hue:.2}",
            count = self.frame_count,
            reason = self.redraw_reason,
            hue = self.hue,
        );
    }
}

/// Simple HSV to RGB conversion.
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let c = v * s;
    let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h * 6.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (r + m, g + m, b + m)
}

fn main() {
    astrelis_profiling::init();
    astrelis_profiling::set_thread_name("main");
    astrelis_core::logging::init_default();

    let mut app = App {
        window_id: None,
        gpu: None,
        surface: None,
        frame_count: 0,
        start_time: Instant::now(),
        hue: 0.0,
        redraw_reason: "none",
    };
    astrelis_window::run(&mut app).expect("event loop error");
}
