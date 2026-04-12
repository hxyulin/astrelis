//! Demonstrates egui integration with the astrelis windowing and GPU backends.
//!
//! Opens a window and renders an egui demo UI with interactive widgets.

use astrelis_gpu::{Gpu, GpuConfig};
use astrelis_gpu::convert::types::texture_format;
use astrelis_gpu::surface::SurfaceConfiguration;
use astrelis_gpu::types::PresentMode;
use astrelis_gpu_egui::EguiIntegration;
use astrelis_profiling_egui::{LastFrameFlameGraph, ProfilerWindow};
use astrelis_window::backend::{AppHandler, EventLoopContext};
use astrelis_window::control_flow::ControlFlow;
use astrelis_window::event::WindowEvent;
use astrelis_window::lifecycle::AppLifecycle;
use astrelis_window::types::LogicalInnerSize;
use astrelis_window::window_id::WindowId;

/// Which profiler widget is currently displayed. Toggled with F2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProfilerView {
    /// Scrollable timeline — the default.
    Timeline,
    /// Last-frame flame graph, kept for comparison.
    LastFrame,
}

struct App {
    window_id: Option<WindowId>,
    gpu: Option<Gpu>,
    surface: Option<astrelis_gpu::Surface>,
    egui: Option<EguiIntegration>,

    // Demo state
    name: String,
    counter: i32,
    slider_value: f32,
    checkbox: bool,

    // Profiler viewers. Both instances are kept so the user can
    // toggle between them with F2 without losing view state.
    profiler_window: ProfilerWindow,
    flame_graph: LastFrameFlameGraph,
    view: ProfilerView,
}

impl AppHandler for App {
    fn on_lifecycle(&mut self, ctx: &mut dyn EventLoopContext, state: AppLifecycle) {
        astrelis_profiling::profile_function!();
        match state {
            AppLifecycle::Resumed => {
                let attrs = astrelis_window::WindowBuilder::new()
                    .with_title("Astrelis — egui Demo")
                    .with_inner_size(LogicalInnerSize::new(1024.0, 768.0))
                    .build();
                let win_id = ctx.create_window(attrs).expect("failed to create window");
                self.window_id = Some(win_id);

                let gpu =
                    Gpu::new(&GpuConfig::default()).expect("failed to create GPU backend");

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
                // Poll so the event loop ticks continuously and the
                // profiler flame graph updates live rather than
                // only on input events.
                ctx.set_control_flow(ControlFlow::Poll);
            }
            AppLifecycle::Suspended | AppLifecycle::Exiting => {}
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
            }
            WindowEvent::RedrawRequested => {
                astrelis_profiling::profile_scope!("redraw");
                self.render(ctx, window_id);
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
    fn render(&mut self, ctx: &mut dyn EventLoopContext, window_id: WindowId) {
        astrelis_profiling::profile_function!();
        let (Some(gpu), Some(surface), Some(egui)) =
            (&self.gpu, &mut self.surface, &mut self.egui)
        else {
            return;
        };

        astrelis_profiling::profile_scope!("acquire");
        let frame = match surface.acquire() {
            Ok(f) => f,
            Err(_) => return,
        };

        let window = ctx.window(window_id).expect("window not found");
        let size = window.inner_size().physical();

        // Begin egui frame.
        astrelis_profiling::profile_scope!("egui_frame");
        egui.begin_frame(window);

        // Build UI.
        egui::Window::new("egui Demo").show(egui.context(), |ui| {
            ui.heading("Astrelis + egui");
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Your name:");
                ui.text_edit_singleline(&mut self.name);
            });

            ui.add(egui::Slider::new(&mut self.slider_value, 0.0..=100.0).text("Slider"));
            ui.checkbox(&mut self.checkbox, "Check me");

            ui.horizontal(|ui| {
                if ui.button("  -  ").clicked() {
                    self.counter -= 1;
                }
                ui.label(format!("Counter: {}", self.counter));
                if ui.button("  +  ").clicked() {
                    self.counter += 1;
                }
            });

            ui.separator();
            ui.label(format!("Hello, {}!", if self.name.is_empty() { "world" } else { &self.name }));
        });

        // F2 toggles between the scrollable timeline (default) and
        // the last-frame flame graph. Both widgets read the same
        // global timeline.
        if egui.context().input(|i| i.key_pressed(egui::Key::F2)) {
            self.view = match self.view {
                ProfilerView::Timeline => ProfilerView::LastFrame,
                ProfilerView::LastFrame => ProfilerView::Timeline,
            };
        }

        let title = match self.view {
            ProfilerView::Timeline => "Profiler (timeline — F2 to toggle)",
            ProfilerView::LastFrame => "Profiler (flame graph — F2 to toggle)",
        };
        egui::Window::new(title)
            .default_size([900.0, 320.0])
            .show(egui.context(), |ui| match self.view {
                ProfilerView::Timeline => self.profiler_window.ui(ui),
                ProfilerView::LastFrame => self.flame_graph.ui(ui),
            });

        // Clear pass — profiled via astrelis-gpu wrapper.
        astrelis_profiling::profile_scope!("encode_clear");
        let mut clear_encoder = gpu.device().create_command_encoder(Some("egui_demo_clear"));
        {
            let _pass = clear_encoder.begin_render_pass(
                &astrelis_gpu::command::RenderPassDescriptor {
                    label: Some("clear"),
                    color_attachments: &[astrelis_gpu::command::ColorAttachment {
                        view: frame.view(),
                        resolve_target: None,
                        load_op: astrelis_gpu::types::LoadOp::Clear(
                            astrelis_core::color::Color::new(0.1, 0.1, 0.1, 1.0),
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
        let mut encoder =
            gpu.raw_device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("egui_demo_egui"),
                });
        let view = frame.view().raw();
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [size.width as u32, size.height as u32],
            pixels_per_point: window.scale_factor(),
        };
        gpu.device().gpu_profile_scope("egui_render", &mut encoder, |enc| {
            egui.end_frame_and_render(gpu, enc, view, screen_descriptor, Some(window));
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
        egui: None,
        name: String::new(),
        counter: 0,
        slider_value: 50.0,
        checkbox: false,
        profiler_window: ProfilerWindow::new(),
        flame_graph: LastFrameFlameGraph::new(),
        view: ProfilerView::Timeline,
    };
    astrelis_window::run(&mut app).expect("event loop error");
}
