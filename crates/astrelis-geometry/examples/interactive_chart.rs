//! Interactive Chart with Fast Line Rendering
//!
//! This example demonstrates:
//! - Fast instanced line rendering for 10k+ points
//! - Using InteractiveChartController for pan, zoom, and hover
//! - Touch and gesture support (pinch to zoom, pan gestures)
//!
//! Controls:
//! - Drag to pan the chart (mouse or touch)
//! - Scroll/pinch to zoom in/out
//! - Two-finger pan gesture for panning
//! - Press R or Home to reset view
//! - Press +/- to zoom in/out
//! - Press ESC to exit
//!
//! Run with: cargo run -p astrelis-geometry --features ui-integration --example interactive_chart

use astrelis_core::logging;
use astrelis_core::profiling::{init_profiling, new_frame, profile_scope, ProfilingBackend};
use astrelis_geometry::chart::{
    AxisId, AxisOrientation, AxisPosition, ChartBuilder, InteractiveChartController,
    LegendPosition, Rect,
};
use astrelis_geometry::GeometryRenderer;
use astrelis_render::{
    Color, GraphicsContext, LineRenderer, RenderTarget, RenderableWindow, WindowContextDescriptor,
};
use astrelis_winit::{
    app::{run_app, App, AppCtx},
    event::{ElementState, Event, EventBatch, HandleStatus, Key, NamedKey},
    window::{WindowBackend, WindowDescriptor, WinitPhysicalSize},
    FrameTime, WindowId,
};
use glam::Vec2;
use std::sync::Arc;

struct InteractiveChartApp {
    #[allow(dead_code)]
    graphics: Arc<GraphicsContext>,
    window: RenderableWindow,
    window_id: WindowId,

    // Renderers
    geometry: GeometryRenderer, // For background only
    ui_lines: LineRenderer,     // Fast line rendering for grid/axes (screen coords)
    data_lines: LineRenderer,   // Fast line rendering for data series (data coords)

    // Chart data and state
    chart: astrelis_geometry::chart::Chart,
    controller: InteractiveChartController,

    // Track whether line data needs rebuilding (only when data changes, not on pan/zoom)
    data_dirty: bool,
}

fn main() {
    logging::init();
    init_profiling(ProfilingBackend::PuffinHttp);

    run_app(|ctx| {
        let graphics = GraphicsContext::new_owned_sync().expect("Failed to create graphics context");

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Fast Interactive Chart - 20k Points".to_string(),
                size: Some(WinitPhysicalSize::new(1200.0, 800.0)),
                ..Default::default()
            })
            .expect("Failed to create window");

        let window = RenderableWindow::new_with_descriptor(
            window,
            graphics.clone(),
            WindowContextDescriptor::default(),
        )
        .expect("Failed to create renderable window");

        let window_id = window.id();
        let surface_format = window.surface_format();
        let geometry = GeometryRenderer::new(graphics.clone());
        let ui_lines = LineRenderer::new(graphics.clone(), surface_format);
        let data_lines = LineRenderer::new(graphics.clone(), surface_format);

        // Create the chart
        let chart = create_demo_chart();
        let controller = InteractiveChartController::new();

        Box::new(InteractiveChartApp {
            graphics,
            window,
            window_id,
            geometry,
            ui_lines,
            data_lines,
            chart,
            controller,
            data_dirty: true, // Build line data on first frame
        })
    });
}

fn create_demo_chart() -> astrelis_geometry::chart::Chart {
    // Generate sine wave with 100k+ points for performance testing
    let num_points = 100_000;
    let x_range = 200.0 * std::f64::consts::PI; // 100 full cycles

    let sine_data: Vec<(f64, f64)> = (0..num_points)
        .map(|i| {
            let x = (i as f64 / (num_points - 1) as f64) * x_range;
            let y = x.sin() + 0.3 * (3.0 * x).sin() + 0.1 * (5.0 * x).sin();
            (x, y)
        })
        .collect();

    let cosine_data: Vec<(f64, f64)> = (0..num_points)
        .map(|i| {
            let x = (i as f64 / (num_points - 1) as f64) * x_range;
            let y = x.cos() + 0.2 * (2.0 * x).cos();
            (x, y)
        })
        .collect();

    tracing::info!("Created chart with {} total points", num_points * 2);

    ChartBuilder::line()
        .title("Fast Interactive Chart - 200k Points")
        .subtitle("Drag to pan, scroll/pinch to zoom")
        .x_label("Time (radians)")
        .y_label("Amplitude")
        .x_range(0.0, x_range)
        .y_range(-2.0, 2.0)
        .add_series("sin(x) + harmonics", &sine_data)
        .add_series("cos(x) + harmonics", &cosine_data)
        .with_grid()
        .with_legend(LegendPosition::TopRight)
        .interactive(true)
        .padding(60.0)
        .build()
}

impl InteractiveChartApp {
    fn draw_chart(&mut self, size: (u32, u32)) {
        profile_scope!("draw_chart");

        let margin = 30.0;
        let bounds = Rect::new(
            margin,
            margin,
            size.0 as f32 - margin * 2.0,
            size.1 as f32 - margin * 2.0,
        );
        let plot_area = bounds.inset(self.chart.padding);

        // Update controller bounds
        self.controller.set_bounds(bounds);

        // Draw background
        self.geometry
            .draw_rect(bounds.position(), bounds.size(), self.chart.background_color);

        // Draw grid
        {
            profile_scope!("draw_grid");
            self.draw_grid(&plot_area);
        }

        // Draw axes
        {
            profile_scope!("draw_axes");
            self.draw_axes(&plot_area);
        }

        // Note: Line series are rendered separately using render_with_data_transform
        // which lets the GPU handle the coordinate transformation
    }

    fn draw_grid(&mut self, plot_area: &Rect) {
        for axis in &self.chart.axes {
            if !axis.grid_lines || !axis.visible {
                continue;
            }

            let style = &axis.style;

            match axis.orientation {
                AxisOrientation::Horizontal => {
                    for i in 0..=axis.tick_count {
                        let t = i as f32 / axis.tick_count as f32;
                        let x = plot_area.x + t * plot_area.width;
                        self.ui_lines.add_line(
                            Vec2::new(x, plot_area.y),
                            Vec2::new(x, plot_area.bottom()),
                            style.grid_width,
                            style.grid_color,
                        );
                    }
                }
                AxisOrientation::Vertical => {
                    for i in 0..=axis.tick_count {
                        let t = i as f32 / axis.tick_count as f32;
                        let y = plot_area.y + t * plot_area.height;
                        self.ui_lines.add_line(
                            Vec2::new(plot_area.x, y),
                            Vec2::new(plot_area.right(), y),
                            style.grid_width,
                            style.grid_color,
                        );
                    }
                }
            }
        }
    }

    fn draw_axes(&mut self, plot_area: &Rect) {
        for axis in &self.chart.axes {
            if !axis.visible {
                continue;
            }

            let style = &axis.style;

            match (axis.orientation, axis.position) {
                (AxisOrientation::Horizontal, AxisPosition::Bottom) => {
                    self.ui_lines.add_line(
                        Vec2::new(plot_area.x, plot_area.bottom()),
                        Vec2::new(plot_area.right(), plot_area.bottom()),
                        style.line_width,
                        style.line_color,
                    );

                    for i in 0..=axis.tick_count {
                        let t = i as f32 / axis.tick_count as f32;
                        let x = plot_area.x + t * plot_area.width;
                        let y = plot_area.bottom();
                        self.ui_lines.add_line(
                            Vec2::new(x, y),
                            Vec2::new(x, y + style.tick_length),
                            style.line_width,
                            style.tick_color,
                        );
                    }
                }
                (AxisOrientation::Horizontal, AxisPosition::Top) => {
                    self.ui_lines.add_line(
                        Vec2::new(plot_area.x, plot_area.y),
                        Vec2::new(plot_area.right(), plot_area.y),
                        style.line_width,
                        style.line_color,
                    );

                    for i in 0..=axis.tick_count {
                        let t = i as f32 / axis.tick_count as f32;
                        let x = plot_area.x + t * plot_area.width;
                        let y = plot_area.y;
                        self.ui_lines.add_line(
                            Vec2::new(x, y - style.tick_length),
                            Vec2::new(x, y),
                            style.line_width,
                            style.tick_color,
                        );
                    }
                }
                (AxisOrientation::Vertical, AxisPosition::Left) => {
                    self.ui_lines.add_line(
                        Vec2::new(plot_area.x, plot_area.y),
                        Vec2::new(plot_area.x, plot_area.bottom()),
                        style.line_width,
                        style.line_color,
                    );

                    for i in 0..=axis.tick_count {
                        let t = i as f32 / axis.tick_count as f32;
                        let x = plot_area.x;
                        let y = plot_area.y + t * plot_area.height;
                        self.ui_lines.add_line(
                            Vec2::new(x - style.tick_length, y),
                            Vec2::new(x, y),
                            style.line_width,
                            style.tick_color,
                        );
                    }
                }
                (AxisOrientation::Vertical, AxisPosition::Right) => {
                    self.ui_lines.add_line(
                        Vec2::new(plot_area.right(), plot_area.y),
                        Vec2::new(plot_area.right(), plot_area.bottom()),
                        style.line_width,
                        style.line_color,
                    );

                    for i in 0..=axis.tick_count {
                        let t = i as f32 / axis.tick_count as f32;
                        let x = plot_area.right();
                        let y = plot_area.y + t * plot_area.height;
                        self.ui_lines.add_line(
                            Vec2::new(x, y),
                            Vec2::new(x + style.tick_length, y),
                            style.line_width,
                            style.tick_color,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    /// Build line data in DATA coordinates (not screen coordinates).
    /// The GPU will transform these to screen coordinates using the current view transform.
    /// This only needs to be called when the underlying data changes, not on pan/zoom.
    fn build_data_lines(&mut self) {
        profile_scope!("build_data_lines");

        self.data_lines.clear();

        for series in &self.chart.series {
            if series.data.len() < 2 {
                continue;
            }

            let color = series.style.color;
            let width = series.style.line_width;

            tracing::debug!(
                "Building line data for series '{}': {} points",
                series.name,
                series.data.len()
            );

            // Add ALL line segments in DATA coordinates
            // The GPU shader will transform these to screen coordinates
            let mut prev = Vec2::new(series.data[0].x as f32, series.data[0].y as f32);

            for point in &series.data[1..] {
                let curr = Vec2::new(point.x as f32, point.y as f32);
                self.data_lines.add_line(prev, curr, width, color);
                prev = curr;
            }
        }

        self.data_dirty = false;
    }
}


impl App for InteractiveChartApp {
    fn update(&mut self, _ctx: &mut AppCtx, _time: &FrameTime) {
        new_frame();
    }

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        profile_scope!("app_render");

        if window_id != self.window_id {
            return;
        }

        // Handle window resize
        {
            profile_scope!("handle_resize");
            events.dispatch(|event| {
                if let Event::WindowResized(size) = event {
                    self.window.resized(*size);
                    return HandleStatus::consumed();
                }
                HandleStatus::ignored()
            });
        }

        // Handle keyboard events
        {
            profile_scope!("handle_keyboard");
            events.dispatch(|event| {
                if let Event::KeyInput(key) = event
                    && key.state == ElementState::Pressed
                    && let Key::Named(NamedKey::Escape) = &key.logical_key
                {
                    std::process::exit(0);
                }
                HandleStatus::ignored()
            });
        }

        // Handle chart interaction events
        {
            profile_scope!("handle_interaction");
            events.dispatch(|event| {
                if self.controller.handle_event(&mut self.chart, event) {
                    HandleStatus::consumed()
                } else {
                    HandleStatus::ignored()
                }
            });
        }

        // Build data line segments only when data changes (not on pan/zoom)
        // This is the key optimization: pan/zoom only updates the transform uniform
        if self.data_dirty {
            profile_scope!("build_data_lines");
            self.build_data_lines();
            self.data_lines.prepare();
        }

        // Get window size and calculate plot area
        let size = self.window.logical_size();
        let margin = 30.0;
        let bounds = Rect::new(
            margin,
            margin,
            size.width as f32 - margin * 2.0,
            size.height as f32 - margin * 2.0,
        );
        let plot_area = bounds.inset(self.chart.padding);

        // Get current axis ranges (these change on pan/zoom)
        let (x_min, x_max) = self.chart.axis_range(AxisId(0)); // X axis
        let (y_min, y_max) = self.chart.axis_range(AxisId(1)); // Y axis

        // Clear and prepare UI lines (grid, axes) - these use screen coordinates
        // and are rebuilt every frame (but fast because no tessellation)
        {
            profile_scope!("prepare_ui");
            self.geometry.clear();
            self.ui_lines.clear();
            self.draw_chart((size.width, size.height));
            self.ui_lines.prepare();
        }

        // Begin frame and render
        {
            profile_scope!("begin_frame");
            let mut frame = self.window.begin_drawing();
            let viewport = self.window.viewport();

            {
                profile_scope!("clear_and_render");
                frame.clear_and_render(
                    RenderTarget::Surface,
                    Color::from_rgb_u8(18, 18, 22),
                    |pass| {
                        profile_scope!("render_pass");
                        // Draw background
                        {
                            profile_scope!("geometry_render");
                            self.geometry.render(pass.wgpu_pass(), viewport);
                        }
                        // Draw grid and axes (screen coordinates)
                        {
                            profile_scope!("ui_lines_render");
                            self.ui_lines.render(pass.wgpu_pass(), viewport);
                        }
                        // Draw data series (data coordinates, GPU transforms)
                        // Use scissor rect to clip to the plot area
                        {
                            profile_scope!("data_lines_render");
                            let scale = viewport.scale_factor.0 as f32;
                            let pass = pass.wgpu_pass();

                            // Set scissor rect to clip data lines to plot area
                            pass.set_scissor_rect(
                                (plot_area.x * scale) as u32,
                                (plot_area.y * scale) as u32,
                                (plot_area.width * scale) as u32,
                                (plot_area.height * scale) as u32,
                            );

                            self.data_lines.render_with_data_transform(
                                pass,
                                viewport,
                                plot_area.x,
                                plot_area.y,
                                plot_area.width,
                                plot_area.height,
                                x_min,
                                x_max,
                                y_min,
                                y_max,
                            );

                            // Reset scissor rect to full viewport
                            let physical = viewport.size;
                            pass.set_scissor_rect(0, 0, physical.width as u32, physical.height as u32);
                        }
                    },
                );
            }

            {
                profile_scope!("frame_finish");
                frame.finish();
            }
        }
    }
}
