//! Profiler viewer demo with real GPU + multi-thread CPU data.
//!
//! Opens an egui window, spawns three background worker threads that
//! emit nested CPU scopes, and uses real GPU timestamp queries from
//! `astrelis-gpu` so the viewer displays genuine GPU-lane profiling
//! alongside CPU spans.
//!
//! Run with:
//!
//!     cargo run -p astrelis-gpu-egui --example viewer_demo --release
//!
//! Interaction:
//!
//! - Drag to pan, two-finger horizontal swipe to pan.
//! - Scroll / pinch to zoom (cursor-anchored).
//! - Hover a span for name / duration / lane.
//! - `Reset` or `Home` snaps to the last 5 frames.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use astrelis_gpu::{Gpu, GpuConfig};
use astrelis_gpu::convert::types::texture_format;
use astrelis_gpu::surface::SurfaceConfiguration;
use astrelis_gpu::types::PresentMode;
use astrelis_gpu_egui::EguiIntegration;
use astrelis_profiling::profiler::Profiler;
use astrelis_profiling_egui::ProfilerWindow;
use astrelis_window::backend::{AppHandler, EventLoopContext};
use astrelis_window::control_flow::ControlFlow;
use astrelis_window::event::WindowEvent;
use astrelis_window::lifecycle::AppLifecycle;
use astrelis_window::types::LogicalInnerSize;
use astrelis_window::window_id::WindowId;

struct App {
    window_id: Option<WindowId>,
    gpu: Option<Gpu>,
    surface: Option<astrelis_gpu::Surface>,
    egui: Option<EguiIntegration>,
    profiler: ProfilerWindow,
    workers: Vec<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
}

impl App {
    fn spawn_workers(&mut self) {
        for i in 0..3 {
            let stop = Arc::clone(&self.stop);
            let handle = thread::Builder::new()
                .name(format!("worker-{i}"))
                .spawn(move || {
                    astrelis_profiling::set_thread_name(&format!("worker-{i}"));
                    while !stop.load(Ordering::Relaxed) {
                        astrelis_profiling::profile_scope!("worker_tick");
                        {
                            astrelis_profiling::profile_scope!("compute");
                            busy_for_us(200 + (i as u64) * 80);
                        }
                        {
                            astrelis_profiling::profile_scope!("update");
                            {
                                astrelis_profiling::profile_scope!("update.inner");
                                busy_for_us(60);
                            }
                            busy_for_us(30);
                        }
                        thread::sleep(Duration::from_micros(500));
                    }
                })
                .expect("spawn worker");
            self.workers.push(handle);
        }
    }
}

fn busy_for_us(us: u64) {
    // Small busy-wait so the recorded scope has a real duration
    // without letting the thread scheduler mask it with sleep.
    let end = std::time::Instant::now() + Duration::from_micros(us);
    while std::time::Instant::now() < end {
        std::hint::spin_loop();
    }
}

impl AppHandler for App {
    fn on_lifecycle(&mut self, ctx: &mut dyn EventLoopContext, state: AppLifecycle) {
        astrelis_profiling::profile_function!();
        match state {
            AppLifecycle::Resumed => {
                let attrs = astrelis_window::WindowBuilder::new()
                    .with_title("Astrelis — Profiler viewer demo")
                    .with_inner_size(LogicalInnerSize::new(1200.0, 720.0))
                    .build();
                let win_id = ctx.create_window(attrs).expect("failed to create window");
                self.window_id = Some(win_id);

                let gpu = Gpu::new(&GpuConfig::default()).expect("failed to create GPU backend");
                let window = ctx.window(win_id).expect("window not found");
                let mut surface = gpu
                    .create_surface(window)
                    .expect("failed to create surface");
                let size = window.inner_size().physical();
                let format = surface.preferred_format();
                surface.configure(&SurfaceConfiguration {
                    format,
                    width: size.width as u32,
                    height: size.height as u32,
                    present_mode: PresentMode::AutoVsync,
                    desired_maximum_frame_latency: 2,
                });
                let egui = EguiIntegration::new(&gpu, texture_format(format));

                self.gpu = Some(gpu);
                self.surface = Some(surface);
                self.egui = Some(egui);

                self.spawn_workers();
                ctx.set_control_flow(ControlFlow::Poll);
            }
            AppLifecycle::Suspended => {}
            AppLifecycle::Exiting => {
                self.stop.store(true, Ordering::Relaxed);
                for h in self.workers.drain(..) {
                    let _ = h.join();
                }
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
        if let Some(egui) = &mut self.egui {
            let consumed = egui.handle_window_event(&event);
            if consumed {
                return;
            }
        }
        match event {
            WindowEvent::CloseRequested => {
                self.stop.store(true, Ordering::Relaxed);
                ctx.exit();
            }
            WindowEvent::Resized(size) => {
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
            }
            WindowEvent::RedrawRequested => self.render(ctx, window_id),
            _ => {}
        }
    }

    fn on_events_cleared(&mut self, ctx: &mut dyn EventLoopContext) {
        astrelis_profiling::profile_function!();
        // Collect completed GPU timestamp queries into the timeline.
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
    fn render(&mut self, ctx: &mut dyn EventLoopContext, window_id: WindowId) {
        astrelis_profiling::profile_function!();
        let (Some(gpu), Some(surface), Some(egui)) =
            (&self.gpu, &mut self.surface, &mut self.egui)
        else {
            return;
        };

        {
            astrelis_profiling::profile_scope!("main.simulate");
            busy_for_us(400);
            {
                astrelis_profiling::profile_scope!("main.simulate.phys");
                busy_for_us(120);
            }
            {
                astrelis_profiling::profile_scope!("main.simulate.ai");
                busy_for_us(90);
            }
        }

        astrelis_profiling::profile_scope!("acquire");
        let frame = match surface.acquire() {
            Ok(f) => f,
            Err(_) => return,
        };
        let window = ctx.window(window_id).expect("window not found");
        let size = window.inner_size().physical();

        astrelis_profiling::profile_scope!("egui_frame");
        egui.begin_frame(window);

        egui::Window::new("Profiler timeline")
            .default_size([1100.0, 520.0])
            .show(egui.context(), |ui| {
                ui.label(
                    "Drag to pan · Scroll to zoom (cursor-anchored) · \
                     Hover a span for name/duration/lane · \
                     Reset or Home to snap to full retained range.",
                );
                ui.separator();
                self.profiler.ui(ui);
            });

        let view = frame.view().raw();

        // Use the astrelis-gpu CommandEncoder for the clear pass so
        // GPU profiler timestamp queries are attached automatically.
        astrelis_profiling::profile_scope!("encode_clear");
        let mut clear_encoder = gpu.device().create_command_encoder(Some("viewer_demo_clear"));
        {
            let _pass = clear_encoder.begin_render_pass(
                &astrelis_gpu::command::RenderPassDescriptor {
                    label: Some("clear"),
                    color_attachments: &[astrelis_gpu::command::ColorAttachment {
                        view: frame.view(),
                        resolve_target: None,
                        load_op: astrelis_gpu::types::LoadOp::Clear(
                            astrelis_core::color::Color::new(0.08, 0.08, 0.1, 1.0),
                        ),
                        store_op: astrelis_gpu::types::StoreOp::Store,
                    }],
                    depth_stencil_attachment: None,
                },
            );
        }
        gpu.submit(std::iter::once(clear_encoder));

        // egui pass — profiled via gpu_profile_scope.
        astrelis_profiling::profile_scope!("encode_egui");
        let mut egui_encoder =
            gpu.raw_device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("viewer_demo_egui"),
                });
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [size.width as u32, size.height as u32],
            pixels_per_point: window.scale_factor(),
        };
        gpu.device().gpu_profile_scope("egui_render", &mut egui_encoder, |enc| {
            egui.end_frame_and_render(gpu, enc, view, screen_descriptor, Some(window));
        });
        astrelis_profiling::profile_scope!("submit");
        gpu.raw_queue().submit(std::iter::once(egui_encoder.finish()));
        astrelis_profiling::profile_scope!("present");
        frame.present();
    }
}

fn main() {
    astrelis_profiling::init();
    astrelis_profiling::set_thread_name("main");

    // Keep 3000 frames (~50 s at 60 fps) so there's enough history
    // to pan around when auto-follow is off.
    {
        let p = Profiler::get();
        let mut tl = p.timeline.write().unwrap();
        tl.retention.max_frames = 3000;
    }

    let mut app = App {
        window_id: None,
        gpu: None,
        surface: None,
        egui: None,
        profiler: ProfilerWindow::new(),
        workers: Vec::new(),
        stop: Arc::new(AtomicBool::new(false)),
    };
    astrelis_window::run(&mut app).expect("event loop error");
}
