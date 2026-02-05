//! Constraint System Showcase
//!
//! This example demonstrates the responsive UI constraint system:
//! - Aspect ratio constraints (16:9 video, 1:1 square)
//! - Viewport units (vw, vh, vmin, vmax)
//! - Constraint expressions (calc, min, max, clamp) - API demonstration
//!
//! Controls:
//! - Resize the window to see responsive behavior

use astrelis_core::logging;
use astrelis_core::math::Vec2;
use astrelis_core::profiling::{ProfilingBackend, init_profiling, new_frame};
use astrelis_render::{
    Color, GraphicsContext, RenderableWindow, WindowContextDescriptor, wgpu,
};
use astrelis_ui::constraint_builder::*;
use astrelis_ui::constraint_resolver::{ConstraintResolver, ResolveContext};
use astrelis_ui::{NodeId, UiBuilder, UiSystem};
use astrelis_winit::{
    FrameTime, WindowId,
    app::{App, AppCtx, run_app},
    event::{Event, EventBatch, HandleStatus},
    window::{WindowBackend, WindowDescriptor, WinitPhysicalSize},
};

struct ConstraintShowcaseApp {
    window: RenderableWindow,
    window_id: WindowId,
    ui: UiSystem,
}

fn main() {
    logging::init();
    init_profiling(ProfilingBackend::PuffinHttp);

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_owned_sync().expect("Failed to create graphics context");

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Constraint System Showcase".to_string(),
                size: Some(WinitPhysicalSize::new(900.0, 700.0)),
                ..Default::default()
            })
            .expect("Failed to create window");

        let window = RenderableWindow::new_with_descriptor(
            window,
            graphics_ctx.clone(),
            WindowContextDescriptor {
                format: Some(wgpu::TextureFormat::Bgra8UnormSrgb),
                ..Default::default()
            },
        )
        .expect("Failed to create renderable window");

        let window_id = window.id();

        // Create UI system
        let mut ui = UiSystem::new(graphics_ctx.clone());
        ui.set_viewport(window.viewport());

        // Build initial UI
        build_showcase_ui(&mut ui, &window);

        tracing::info!("Constraint showcase initialized");
        tracing::info!("Resize the window to see responsive behavior");

        Box::new(ConstraintShowcaseApp {
            window,
            window_id,
            ui,
        })
    });
}

fn build_showcase_ui(ui: &mut UiSystem, window: &RenderableWindow) {
    let size = window.physical_size();
    let width = size.width as f32;
    let height = size.height as f32;

    // Create resolve context for constraint demonstration
    let viewport = Vec2::new(width, height);
    let ctx = ResolveContext::new(viewport, Some(width * 0.8)); // 80% parent width

    let theme = ui.theme().clone();
    let colors = &theme.colors;

    ui.build(|root| {
        // Main container - full viewport
        root.container()
            .width(width)
            .height(height)
            .padding(20.0)
            .background_color(colors.background)
            .child(|root| {
                root.column()
                    .gap(15.0)
                    .child(|root| {
                        // Title
                        root.text("Constraint System Showcase")
                            .size(24.0)
                            .color(colors.text_primary)
                            .bold()
                            .build()
                    })
                    .child(|root| {
                        // Viewport info
                        root.text(format!("Viewport: {}x{} px", width as i32, height as i32))
                            .size(12.0)
                            .color(colors.text_secondary)
                            .build()
                    })
                    .child(|root| {
                        // Row of demonstration panels
                        root.row()
                            .gap(15.0)
                            .child(|root| build_aspect_ratio_panel(root, &theme))
                            .child(|root| build_constraint_expressions_panel(root, &theme, &ctx))
                            .build()
                    })
                    .child(|root| {
                        root.row()
                            .gap(15.0)
                            .child(|root| build_viewport_units_info(root, &theme, viewport))
                            .build()
                    })
                    .build()
            })
            .build();
    });
}

fn build_aspect_ratio_panel(root: &mut UiBuilder, theme: &astrelis_ui::Theme) -> NodeId {
    let colors = &theme.colors;
    root.container()
        .width(250.0)
        .background_color(colors.surface)
        .border_color(colors.border)
        .border_width(2.0)
        .border_radius(8.0)
        .padding(12.0)
        .child(|root| {
            root.column()
                .gap(10.0)
                .child(|root| {
                    // Section title accent color - keep as content-specific
                    root.text("Aspect Ratios")
                        .size(16.0)
                        .color(Color::from_rgb_u8(100, 180, 255))
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.text("16:9 Video Container")
                        .size(11.0)
                        .color(colors.text_secondary)
                        .build()
                })
                .child(|root| {
                    // 16:9 aspect ratio box - content-specific demo colors
                    root.container()
                        .width(200.0)
                        .aspect_ratio(16.0 / 9.0)
                        .background_color(Color::from_rgb_u8(60, 60, 75))
                        .border_color(Color::from_rgb_u8(100, 100, 130))
                        .border_width(1.0)
                        .border_radius(4.0)
                        .child(|root| {
                            root.text("16:9")
                                .size(14.0)
                                .color(Color::from_rgb_u8(150, 150, 170))
                                .margin(8.0)
                                .build()
                        })
                        .build()
                })
                .child(|root| {
                    root.text("1:1 Square Avatar")
                        .size(11.0)
                        .color(colors.text_secondary)
                        .build()
                })
                .child(|root| {
                    // 1:1 square box - content-specific demo colors
                    root.container()
                        .width(80.0)
                        .aspect_ratio(1.0)
                        .background_color(Color::from_rgb_u8(75, 60, 60))
                        .border_color(Color::from_rgb_u8(130, 100, 100))
                        .border_width(1.0)
                        .border_radius(40.0) // Circle
                        .child(|root| {
                            root.text("1:1")
                                .size(12.0)
                                .color(Color::from_rgb_u8(170, 150, 150))
                                .margin(25.0)
                                .build()
                        })
                        .build()
                })
                .child(|root| {
                    root.text("4:3 Photo Frame")
                        .size(11.0)
                        .color(colors.text_secondary)
                        .build()
                })
                .child(|root| {
                    // 4:3 aspect ratio box - content-specific demo colors
                    root.container()
                        .width(120.0)
                        .aspect_ratio(4.0 / 3.0)
                        .background_color(Color::from_rgb_u8(60, 75, 60))
                        .border_color(Color::from_rgb_u8(100, 130, 100))
                        .border_width(1.0)
                        .border_radius(4.0)
                        .child(|root| {
                            root.text("4:3")
                                .size(12.0)
                                .color(Color::from_rgb_u8(150, 170, 150))
                                .margin(8.0)
                                .build()
                        })
                        .build()
                })
                .build()
        })
        .build()
}

fn build_constraint_expressions_panel(
    root: &mut UiBuilder,
    theme: &astrelis_ui::Theme,
    ctx: &ResolveContext,
) -> NodeId {
    let colors = &theme.colors;
    // Demonstrate constraint resolution
    let calc_result = ConstraintResolver::resolve(&calc(percent(100.0) - px(40.0)), ctx);
    let min_result = ConstraintResolver::resolve(&min2(percent(50.0), px(400.0)), ctx);
    let max_result = ConstraintResolver::resolve(&max2(px(200.0), percent(30.0)), ctx);
    let clamp_result =
        ConstraintResolver::resolve(&clamp(px(100.0), percent(50.0), px(300.0)), ctx);

    root.container()
        .width(350.0)
        .background_color(colors.surface)
        .border_color(colors.border)
        .border_width(2.0)
        .border_radius(8.0)
        .padding(12.0)
        .child(|root| {
            root.column()
                .gap(8.0)
                .child(|root| {
                    // Section title accent color - keep as content-specific
                    root.text("Constraint Expressions")
                        .size(16.0)
                        .color(Color::from_rgb_u8(180, 100, 255))
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.text(format!(
                        "Parent width: {:.0}px",
                        ctx.parent_size.unwrap_or(0.0)
                    ))
                    .size(11.0)
                    .color(colors.text_secondary)
                    .build()
                })
                .child(|root| {
                    root.text(format!(
                        "calc(100% - 40px) = {:.0}px",
                        calc_result.unwrap_or(0.0)
                    ))
                    .size(12.0)
                    .color(Color::from_rgb_u8(200, 180, 220))
                    .build()
                })
                .child(|root| {
                    root.text(format!(
                        "min(50%, 400px) = {:.0}px",
                        min_result.unwrap_or(0.0)
                    ))
                    .size(12.0)
                    .color(Color::from_rgb_u8(200, 180, 220))
                    .build()
                })
                .child(|root| {
                    root.text(format!(
                        "max(200px, 30%) = {:.0}px",
                        max_result.unwrap_or(0.0)
                    ))
                    .size(12.0)
                    .color(Color::from_rgb_u8(200, 180, 220))
                    .build()
                })
                .child(|root| {
                    root.text(format!(
                        "clamp(100px, 50%, 300px) = {:.0}px",
                        clamp_result.unwrap_or(0.0)
                    ))
                    .size(12.0)
                    .color(Color::from_rgb_u8(200, 180, 220))
                    .build()
                })
                .child(|root| {
                    root.text("Resize window to see values change!")
                        .size(11.0)
                        .color(colors.warning)
                        .margin(5.0)
                        .build()
                })
                .build()
        })
        .build()
}

fn build_viewport_units_info(
    root: &mut UiBuilder,
    theme: &astrelis_ui::Theme,
    viewport: Vec2,
) -> NodeId {
    let colors = &theme.colors;
    // Calculate viewport unit values
    let vw_10 = viewport.x * 0.1;
    let vh_10 = viewport.y * 0.1;
    let vmin_10 = viewport.x.min(viewport.y) * 0.1;
    let vmax_10 = viewport.x.max(viewport.y) * 0.1;

    root.container()
        .width(400.0)
        .background_color(colors.surface)
        .border_color(colors.border)
        .border_width(2.0)
        .border_radius(8.0)
        .padding(12.0)
        .child(|root| {
            root.column()
                .gap(6.0)
                .child(|root| {
                    // Section title accent color - keep as content-specific
                    root.text("Viewport Units")
                        .size(16.0)
                        .color(Color::from_rgb_u8(100, 255, 140))
                        .bold()
                        .build()
                })
                .child(|root| {
                    root.text(format!("10vw = {:.0}px (10% of {:.0})", vw_10, viewport.x))
                        .size(12.0)
                        .color(Color::from_rgb_u8(180, 220, 190))
                        .build()
                })
                .child(|root| {
                    root.text(format!("10vh = {:.0}px (10% of {:.0})", vh_10, viewport.y))
                        .size(12.0)
                        .color(Color::from_rgb_u8(180, 220, 190))
                        .build()
                })
                .child(|root| {
                    root.text(format!(
                        "10vmin = {:.0}px (10% of min({:.0}, {:.0}))",
                        vmin_10, viewport.x, viewport.y
                    ))
                    .size(12.0)
                    .color(Color::from_rgb_u8(180, 220, 190))
                    .build()
                })
                .child(|root| {
                    root.text(format!(
                        "10vmax = {:.0}px (10% of max({:.0}, {:.0}))",
                        vmax_10, viewport.x, viewport.y
                    ))
                    .size(12.0)
                    .color(Color::from_rgb_u8(180, 220, 190))
                    .build()
                })
                .build()
        })
        .build()
}

impl App for ConstraintShowcaseApp {
    fn update(&mut self, _ctx: &mut AppCtx, time: &FrameTime) {
        new_frame();
        self.ui.update(time.delta_seconds());
    }

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        if window_id != self.window_id {
            return;
        }

        // Handle window resize events
        events.dispatch(|event| {
            if let Event::WindowResized(size) = event {
                self.window.resized(*size);
                self.ui.set_viewport(self.window.viewport());

                tracing::info!(
                    "Window resized to {}x{} - rebuilding UI",
                    size.width,
                    size.height,
                );

                // Rebuild UI with new viewport size
                build_showcase_ui(&mut self.ui, &self.window);

                return HandleStatus::consumed();
            }
            HandleStatus::ignored()
        });

        // Handle UI events
        self.ui.handle_events(events);

        // Begin frame and render with depth buffer for proper z-ordering
        let bg = self.ui.theme().colors.background;

        // Get depth view before starting frame (avoids borrow conflicts)
        let depth_view = self.ui.depth_view();

        let mut frame = self.window.begin_drawing();

        // Create render pass with depth attachment
        {
            // SAFETY: We're creating a scope that ensures pass is dropped before we call
            // frame methods. The raw pointer usage is to work around borrow checker limitations.
            let surface_view = frame.surface().view() as *const wgpu::TextureView;
            let encoder = frame.encoder();

            // SAFETY: surface_view pointer is valid for the duration of this scope
            let surface_view = unsafe { &*surface_view };

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("UI Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(bg.to_wgpu()),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0), // Clear to 0.0 for reverse-Z
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.ui.render(&mut pass);
        }

        frame.increment_passes();
        frame.finish();
    }
}
