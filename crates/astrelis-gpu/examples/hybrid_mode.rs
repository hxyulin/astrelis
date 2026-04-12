//! Demonstrates hybrid `Wait`/`Poll` rendering.
//!
//! This example shows how a desktop app can save CPU by defaulting to
//! `ControlFlow::Wait` (idle, zero CPU) while temporarily switching
//! to `ControlFlow::Poll` (continuous rendering) when an animation is
//! active. This is the recommended pattern for desktop apps that mix
//! static UI with occasional animations or game-like rendering.
//!
//! Behavior:
//! - **Idle (Wait):** window shows a static color, event loop sleeps.
//!   Only redraws on input events (mouse, keyboard, resize).
//! - **Animating (Poll):** press Space to start a 3-second color
//!   animation. The loop switches to Poll, renders every frame, then
//!   automatically switches back to Wait when the animation ends.
//!
//! Watch the terminal — it prints the current mode and frame count
//! so you can see the transitions.
//!
//! Run with:
//! ```sh
//! cargo run -p astrelis-gpu --example hybrid_mode
//! ```

use std::time::{Duration, Instant};

use astrelis_core::color::Color;
use astrelis_gpu::command::{ColorAttachment, RenderPassDescriptor};
use astrelis_gpu::surface::SurfaceConfiguration;
use astrelis_gpu::types::{LoadOp, PresentMode, StoreOp};
use astrelis_gpu::{Gpu, GpuConfig, GpuError};
use astrelis_window::backend::{AppHandler, EventLoopContext};
use astrelis_window::control_flow::ControlFlow;
use astrelis_window::event::WindowEvent;
use astrelis_window::keyboard::KeyCode;
use astrelis_window::lifecycle::AppLifecycle;
use astrelis_window::types::LogicalInnerSize;
use astrelis_window::window_id::WindowId;
use astrelis_window::WindowBuilder;

/// How long the color animation runs after pressing Space.
const ANIMATION_DURATION: Duration = Duration::from_secs(3);

struct App {
    window_id: Option<WindowId>,
    gpu: Option<Gpu>,
    surface: Option<astrelis_gpu::Surface>,
    frame_count: u64,
    /// `Some(deadline)` while an animation is active.
    /// When `Instant::now()` passes the deadline, the animation ends
    /// and the loop switches back to Wait.
    animation_end: Option<Instant>,
    /// Current color hue [0, 1). Advanced by input or animation.
    hue: f32,
    /// Whether we're currently in Poll mode.
    is_polling: bool,
}

impl AppHandler for App {
    fn on_lifecycle(&mut self, ctx: &mut dyn EventLoopContext, state: AppLifecycle) {
        astrelis_profiling::profile_function!();
        match state {
            AppLifecycle::Resumed => {
                let attrs = WindowBuilder::new()
                    .with_title("Astrelis — Hybrid Mode (Space = animate)")
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

                // Start in Wait mode — idle until input.
                ctx.set_control_flow(ControlFlow::Wait);

                if let Some(win) = ctx.window(win_id) {
                    win.request_redraw();
                }
            }
            AppLifecycle::Suspended => {}
            AppLifecycle::Exiting => {
                println!("Rendered {} frames. Goodbye!", self.frame_count);
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
        let mut needs_redraw = false;

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
                needs_redraw = true;
            }

            WindowEvent::KeyboardInput(key) if key.state.is_pressed() => {
                if key.key_code == KeyCode::Space {
                    // Start (or restart) the animation.
                    self.animation_end = Some(Instant::now() + ANIMATION_DURATION);
                    if !self.is_polling {
                        self.is_polling = true;
                        ctx.set_control_flow(ControlFlow::Poll);
                        println!("[mode] Wait -> Poll (animation started)");
                    }
                } else {
                    // Other keys: nudge the color.
                    self.hue = (self.hue + 0.05) % 1.0;
                    needs_redraw = true;
                }
            }

            WindowEvent::CursorMoved(_) => {
                self.hue = (self.hue + 0.005) % 1.0;
                needs_redraw = true;
            }

            WindowEvent::RedrawRequested => {
                astrelis_profiling::profile_scope!("redraw");
                self.render();
            }

            _ => {}
        }

        if needs_redraw {
            if let Some(win) = ctx.window(window_id) {
                win.request_redraw();
            }
        }
    }

    fn on_events_cleared(&mut self, ctx: &mut dyn EventLoopContext) {
        astrelis_profiling::profile_function!();
        if let Some(gpu) = &self.gpu {
            gpu.process_profiling_frames();
        }
        astrelis_profiling::new_frame();

        // Check if the animation is still active.
        if let Some(end) = self.animation_end {
            if Instant::now() >= end {
                // Animation finished — switch back to Wait.
                self.animation_end = None;
                self.is_polling = false;
                ctx.set_control_flow(ControlFlow::Wait);
                println!("[mode] Poll -> Wait (animation ended, {} frames total)", self.frame_count);
                // One final redraw to show the resting state.
                if let Some(id) = self.window_id
                    && let Some(win) = ctx.window(id)
                {
                    win.request_redraw();
                }
            } else {
                // Animation in progress — advance hue and redraw.
                self.hue = (self.hue + 0.01) % 1.0;
                if let Some(id) = self.window_id
                    && let Some(win) = ctx.window(id)
                {
                    win.request_redraw();
                }
            }
        }
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

        let (r, g, b) = hsv_to_rgb(self.hue, 0.6, 0.8);
        let clear_color = Color::new(r, g, b, 1.0);

        astrelis_profiling::profile_scope!("encode");
        let mut encoder = gpu.device().create_command_encoder(Some("hybrid_mode"));
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
        let mode = if self.is_polling { "POLL" } else { "WAIT" };
        println!(
            "frame {count:>5}  mode={mode}  hue={hue:.2}",
            count = self.frame_count,
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

    println!("Press Space to start a 3-second color animation.");
    println!("Move the mouse or press other keys for single-frame redraws.");
    println!("Watch the mode transitions in the log.\n");

    let mut app = App {
        window_id: None,
        gpu: None,
        surface: None,
        frame_count: 0,
        animation_end: None,
        hue: 0.0,
        is_polling: false,
    };
    astrelis_window::run(&mut app).expect("event loop error");
}
