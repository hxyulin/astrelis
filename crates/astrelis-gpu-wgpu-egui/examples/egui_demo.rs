//! Demonstrates egui integration with the astrelis windowing and GPU backends.
//!
//! Opens a window and renders an egui demo UI with interactive widgets.

use astrelis_gpu::backend::{GpuBackend, GpuConfig};
use astrelis_gpu::surface::{GpuSurface, SurfaceConfiguration, SurfaceTexture};
use astrelis_gpu::types::PresentMode;
use astrelis_gpu_wgpu::WgpuBackend;
use astrelis_gpu_wgpu::convert::types::texture_format;
use astrelis_gpu_wgpu_egui::EguiIntegration;
use astrelis_window::backend::{AppHandler, EventLoopContext, WindowBackend};
use astrelis_window::control_flow::ControlFlow;
use astrelis_window::event::WindowEvent;
use astrelis_window::lifecycle::AppLifecycle;
use astrelis_window::types::LogicalInnerSize;
use astrelis_window::window_id::WindowId;
use astrelis_window_winit::WinitBackend;

struct App {
    window_id: Option<WindowId>,
    gpu: Option<WgpuBackend>,
    surface: Option<<WgpuBackend as GpuBackend>::Surface>,
    egui: Option<EguiIntegration>,

    // Demo state
    name: String,
    counter: i32,
    slider_value: f32,
    checkbox: bool,
}

impl AppHandler for App {
    fn on_lifecycle(&mut self, ctx: &mut dyn EventLoopContext, state: AppLifecycle) {
        match state {
            AppLifecycle::Resumed => {
                let attrs = astrelis_window::WindowBuilder::new()
                    .with_title("Astrelis — egui Demo")
                    .with_inner_size(LogicalInnerSize::new(1024.0, 768.0))
                    .build();
                let win_id = ctx.create_window(attrs).expect("failed to create window");
                self.window_id = Some(win_id);

                let gpu =
                    WgpuBackend::new(&GpuConfig::default()).expect("failed to create GPU backend");

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

                let egui = EguiIntegration::new(gpu.device(), texture_format(format));

                self.gpu = Some(gpu);
                self.surface = Some(surface);
                self.egui = Some(egui);
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
        // Let egui process the event first.
        if let Some(egui) = &mut self.egui {
            let consumed = egui.handle_window_event(&event);
            if consumed {
                return;
            }
        }

        match event {
            WindowEvent::CloseRequested => ctx.exit(),
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
            WindowEvent::RedrawRequested => {
                self.render(ctx, window_id);
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
    fn render(&mut self, ctx: &mut dyn EventLoopContext, window_id: WindowId) {
        let (Some(gpu), Some(surface), Some(egui)) =
            (&self.gpu, &mut self.surface, &mut self.egui)
        else {
            return;
        };

        let frame = match surface.acquire() {
            Ok(f) => f,
            Err(_) => return,
        };

        let window = ctx.window(window_id).expect("window not found");
        let size = window.inner_size().physical();

        // Begin egui frame.
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

        // EguiIntegration::end_frame_and_render requires a raw wgpu::CommandEncoder
        // and wgpu::TextureView, so we use the WgpuDevice escape-hatch methods here
        // instead of the abstract GpuDevice API.
        let device = gpu.device();
        let mut encoder =
            device
                .wgpu_device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("egui_demo"),
                });

        // Resolve the surface texture view from the device's resource map.
        let view_id = frame.view();
        let views = device.texture_views();
        let view = views.get(view_id).expect("surface view not found");

        // Clear pass.
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
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

        // End egui frame and render.
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [size.width as u32, size.height as u32],
            pixels_per_point: window.scale_factor(),
        };
        egui.end_frame_and_render(
            device,
            &mut encoder,
            view,
            screen_descriptor,
            Some(window),
        );

        device.wgpu_queue().submit(std::iter::once(encoder.finish()));
        frame.present();
    }
}

fn main() {
    let backend = WinitBackend::new().expect("failed to create windowing backend");
    let mut app = App {
        window_id: None,
        gpu: None,
        surface: None,
        egui: None,
        name: String::new(),
        counter: 0,
        slider_value: 50.0,
        checkbox: false,
    };
    backend.run(&mut app).expect("event loop error");
}
