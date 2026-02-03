//! Interactive Chart with egui Integration
//!
//! This example demonstrates:
//! - Using ChartWidget with egui for interactive charts
//! - Automatic text rendering (titles, labels, legend, tick values)
//! - Multiple axes (primary and secondary Y axes)
//! - Annotations (text, lines, fill regions)
//! - Pan and zoom controls
//! - Tooltips on hover
//!
//! Run with: cargo run -p astrelis-geometry --features egui-integration --example egui_chart

use astrelis_core::logging;
use astrelis_egui::Egui;
use astrelis_geometry::chart::{Axis, ChartBuilder, ChartWidget, LegendPosition};
use astrelis_render::{Color, GraphicsContext, RenderableWindow};
use astrelis_winit::{
    app::{run_app, App, AppCtx},
    event::EventBatch,
    window::{WindowBackend, WindowDescriptor, WinitPhysicalSize},
    FrameTime, WindowId,
};
use std::sync::Arc;

struct ChartApp {
    _context: Arc<GraphicsContext>,
    window: RenderableWindow,
    window_id: WindowId,
    egui: Egui,

    // Chart data
    chart: astrelis_geometry::chart::Chart,
    show_annotations: bool,
    show_fill_regions: bool,
}

fn main() {
    logging::init();

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_owned_sync().expect("Failed to create graphics context");

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Interactive Chart Demo (egui)".to_string(),
                size: Some(WinitPhysicalSize::new(1200.0, 800.0)),
                ..Default::default()
            })
            .expect("Failed to create window");

        let window =
            RenderableWindow::new(window, graphics_ctx.clone()).expect("Failed to create renderable window");
        let window_id = window.id();
        let egui = Egui::new(&window, &graphics_ctx);

        // Create initial chart with all features
        let chart = create_demo_chart(true, true);

        Box::new(ChartApp {
            _context: graphics_ctx,
            window,
            window_id,
            egui,
            chart,
            show_annotations: true,
            show_fill_regions: true,
        })
    });
}

fn create_demo_chart(
    with_annotations: bool,
    with_fill_regions: bool,
) -> astrelis_geometry::chart::Chart {
    // Generate sine wave data
    let num_points = 100;
    let x_range = 2.0 * std::f64::consts::PI;

    let sine_data: Vec<(f64, f64)> = (0..num_points)
        .map(|i| {
            let x = (i as f64 / (num_points - 1) as f64) * x_range;
            let y = x.sin();
            (x, y)
        })
        .collect();

    let cosine_data: Vec<(f64, f64)> = (0..num_points)
        .map(|i| {
            let x = (i as f64 / (num_points - 1) as f64) * x_range;
            let y = x.cos();
            (x, y)
        })
        .collect();

    // Temperature data for secondary axis (scaled differently)
    let temperature_data: Vec<(f64, f64)> = (0..num_points)
        .map(|i| {
            let x = (i as f64 / (num_points - 1) as f64) * x_range;
            let y = 20.0 + 5.0 * (x * 0.5).sin() + 2.0 * (x * 2.0).cos();
            (x, y)
        })
        .collect();

    let mut builder = ChartBuilder::line()
        .title("Interactive Chart Demo")
        .subtitle("Hover over points for details, drag to pan, scroll to zoom")
        .x_label("Time (radians)")
        .y_label("Amplitude")
        .x_range(0.0, x_range)
        .y_range(-1.5, 1.5)
        .add_series("sin(x)", &sine_data)
        .add_series("cos(x)", &cosine_data)
        .secondary_y_axis(Axis::y_secondary().with_label("Temperature (°C)"))
        .secondary_y_range(10.0, 30.0)
        .add_series_secondary_y("Temperature", &temperature_data)
        .with_grid()
        .with_legend(LegendPosition::TopRight)
        .interactive(true)
        .with_crosshair()
        .with_tooltips();

    // Add annotations if enabled
    if with_annotations {
        builder = builder
            .add_text_at("Peak", std::f64::consts::FRAC_PI_2, 1.0)
            .add_horizontal_line(0.0, Color::rgba(0.5, 0.5, 0.5, 0.5))
            .add_vertical_line(std::f64::consts::PI, Color::rgba(0.5, 0.5, 0.5, 0.5));
    }

    // Add fill regions if enabled
    if with_fill_regions {
        builder = builder
            .add_horizontal_band(-0.5, 0.5, Color::rgba(0.0, 1.0, 0.0, 0.1))
            .fill_below_series(0, -1.5, Color::rgba(0.0, 0.5, 1.0, 0.15));
    }

    builder.build()
}

impl App for ChartApp {
    fn update(&mut self, _ctx: &mut AppCtx, _time: &FrameTime) {}

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        if window_id != self.window_id {
            return;
        }

        // Handle window resize
        events.dispatch(|event| {
            if let astrelis_winit::event::Event::WindowResized(size) = event {
                self.window.resized(*size);
                astrelis_winit::event::HandleStatus::consumed()
            } else {
                astrelis_winit::event::HandleStatus::ignored()
            }
        });

        self.egui.handle_events(&self.window, events);

        // Track if we need to rebuild chart
        let mut rebuild_chart = false;
        let prev_annotations = self.show_annotations;
        let prev_fill = self.show_fill_regions;

        self.egui.ui(&self.window, |ctx| {
            // Top panel with controls
            egui::TopBottomPanel::top("controls").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Chart Options:");

                    if ui
                        .checkbox(&mut self.show_annotations, "Show Annotations")
                        .changed()
                    {
                        rebuild_chart = self.show_annotations != prev_annotations;
                    }

                    if ui
                        .checkbox(&mut self.show_fill_regions, "Show Fill Regions")
                        .changed()
                    {
                        rebuild_chart = self.show_fill_regions != prev_fill;
                    }

                    ui.separator();

                    if ui.button("Reset View").clicked() {
                        self.chart.interactive.reset();
                    }

                    ui.separator();

                    ui.label(format!(
                        "Zoom: {:.1}x | Pan: ({:.2}, {:.2})",
                        self.chart.interactive.zoom,
                        self.chart.interactive.pan_offset.x,
                        self.chart.interactive.pan_offset.y
                    ));
                });
            });

            // Side panel with info
            egui::SidePanel::left("info")
                .default_width(200.0)
                .show(ctx, |ui| {
                    ui.heading("Chart Info");
                    ui.separator();

                    ui.label("Series:");
                    for series in &self.chart.series {
                        ui.horizontal(|ui| {
                            let color = egui::Color32::from_rgba_unmultiplied(
                                (series.style.color.r * 255.0) as u8,
                                (series.style.color.g * 255.0) as u8,
                                (series.style.color.b * 255.0) as u8,
                                255,
                            );
                            ui.colored_label(color, &series.name);
                            ui.label(format!("({} points)", series.data.len()));
                        });
                    }

                    ui.separator();

                    ui.label("Controls:");
                    ui.label("• Drag to pan");
                    ui.label("• Scroll to zoom");
                    ui.label("• Hover for tooltips");
                    ui.label("• Press 'R' to reset");

                    ui.separator();

                    // Show text rendering info
                    ui.label("Text Rendering:");
                    ui.label("• Title & subtitle");
                    ui.label("• Tick labels (auto-formatted)");
                    ui.label("• Axis labels");
                    ui.label("• Legend with swatches");

                    ui.separator();

                    if let Some((series_idx, point_idx)) = self.chart.interactive.hovered_point {
                        ui.label("Hovered Point:");
                        if let Some(series) = self.chart.series.get(series_idx) {
                            if let Some(point) = series.data.get(point_idx) {
                                ui.label(format!("Series: {}", series.name));
                                ui.label(format!("x: {:.4}", point.x));
                                ui.label(format!("y: {:.4}", point.y));
                            }
                        }
                    } else {
                        ui.label("Hover over a point for details");
                    }
                });

            // Central panel with chart
            egui::CentralPanel::default().show(ctx, |ui| {
                // The ChartWidget now automatically renders:
                // - Title and subtitle
                // - Tick labels on all axes
                // - Axis labels
                // - Legend with color swatches
                ui.add(ChartWidget::new(&mut self.chart).min_size(egui::vec2(600.0, 400.0)));
            });
        });

        // Rebuild chart if options changed
        if rebuild_chart {
            self.chart = create_demo_chart(self.show_annotations, self.show_fill_regions);
        }

        let mut frame = self.window.begin_drawing();

        // Clear to dark background
        {
            use astrelis_render::{RenderPassBuilder, RenderTarget};
            let render_pass = RenderPassBuilder::new()
                .label("Clear Pass")
                .target(RenderTarget::Surface)
                .clear_color(Color::rgb(0.1, 0.1, 0.12))
                .build(&mut frame);
            drop(render_pass);
        }

        self.egui.render(&self.window, &mut frame);
        frame.finish();
    }
}
