//! Line Chart Example - Data visualization with astrelis-geometry
//!
//! This example demonstrates:
//! - Creating a line chart using ChartBuilder
//! - Chart title and subtitle
//! - Axis labels (X and Y)
//! - Legend with multiple series
//! - Reference lines and shaded bands
//! - Plotting mathematical functions with many data points
//! - Modern, minimal styling
//!
//! Controls:
//! - ESC to exit

use astrelis_core::logging;
use astrelis_core::profiling::{init_profiling, new_frame, ProfilingBackend};
use astrelis_geometry::{
    chart::{ChartBuilder, ChartRenderer, LegendPosition, Rect},
    GeometryRenderer,
};
use astrelis_render::{
    Color, GraphicsContext, RenderTarget, RenderableWindow, WindowContextDescriptor,
};
use astrelis_winit::{
    app::{run_app, App, AppCtx},
    event::{ElementState, Event, EventBatch, HandleStatus, Key, NamedKey},
    window::{WindowBackend, WindowDescriptor, WinitPhysicalSize},
    FrameTime, WindowId,
};
use std::sync::Arc;

struct LineChartApp {
    #[allow(dead_code)]
    graphics: Arc<GraphicsContext>,
    window: RenderableWindow,
    window_id: WindowId,
    geometry: GeometryRenderer,
}

fn main() {
    logging::init();
    init_profiling(ProfilingBackend::PuffinHttp);

    run_app(|ctx| {
        let graphics = GraphicsContext::new_owned_sync().expect("Failed to create graphics context");

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Line Chart - Mathematical Functions".to_string(),
                size: Some(WinitPhysicalSize::new(1000.0, 700.0)),
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
        let geometry = GeometryRenderer::new(graphics.clone());

        Box::new(LineChartApp {
            graphics,
            window,
            window_id,
            geometry,
        })
    });
}

impl LineChartApp {
    fn draw_chart(&mut self, size: (u32, u32)) {
        // Generate sine wave with 500 data points
        let num_points = 500;
        let x_range = 4.0 * std::f64::consts::PI; // 0 to 4π

        let sine_wave: Vec<(f64, f64)> = (0..num_points)
            .map(|i| {
                let x = (i as f64 / (num_points - 1) as f64) * x_range;
                let y = x.sin();
                (x, y)
            })
            .collect();

        // Cosine wave
        let cosine_wave: Vec<(f64, f64)> = (0..num_points)
            .map(|i| {
                let x = (i as f64 / (num_points - 1) as f64) * x_range;
                let y = x.cos();
                (x, y)
            })
            .collect();

        // Damped sine wave
        let damped_sine: Vec<(f64, f64)> = (0..num_points)
            .map(|i| {
                let x = (i as f64 / (num_points - 1) as f64) * x_range;
                let decay = (-x * 0.15).exp();
                let y = x.sin() * decay;
                (x, y)
            })
            .collect();

        let chart = ChartBuilder::line()
            // Title and subtitle
            .title("Mathematical Functions")
            .subtitle("Trigonometric waves with exponential damping")
            // Axis labels
            .x_label("x (radians)")
            .y_label("Amplitude")
            // Axis ranges
            .x_range(0.0, x_range)
            .y_range(-1.2, 1.2)
            // Data series
            .add_series("sin(x)", &sine_wave)
            .add_series("cos(x)", &cosine_wave)
            .add_series("sin(x)·e^(-0.15x)", &damped_sine)
            // Visual features
            .with_grid()
            .with_legend(LegendPosition::TopRight)
            // Reference annotations
            .add_horizontal_line(0.0, Color::rgba(0.5, 0.5, 0.5, 0.4)) // Zero line
            .add_horizontal_band(-0.5, 0.5, Color::rgba(0.0, 1.0, 0.0, 0.08)) // "Normal" range
            .add_vertical_line(std::f64::consts::PI, Color::rgba(1.0, 0.5, 0.0, 0.3)) // π marker
            .add_vertical_line(2.0 * std::f64::consts::PI, Color::rgba(1.0, 0.5, 0.0, 0.3)) // 2π marker
            // Layout
            .padding(60.0)
            .build();

        // Calculate bounds with proper margins
        let margin = 30.0;
        let bounds = Rect::new(
            margin,
            margin,
            size.0 as f32 - margin * 2.0,
            size.1 as f32 - margin * 2.0,
        );

        let mut chart_renderer = ChartRenderer::new(&mut self.geometry);
        chart_renderer.draw(&chart, bounds);
    }
}

impl App for LineChartApp {
    fn update(&mut self, _ctx: &mut AppCtx, _time: &FrameTime) {
        new_frame();
    }

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        if window_id != self.window_id {
            return;
        }

        // Handle window resize
        events.dispatch(|event| {
            if let Event::WindowResized(size) = event {
                self.window.resized(*size);
                return HandleStatus::consumed();
            }
            HandleStatus::ignored()
        });

        // Handle keyboard events
        events.dispatch(|event| {
            if let Event::KeyInput(key) = event
                && key.state == ElementState::Pressed
                && let Key::Named(NamedKey::Escape) = &key.logical_key
            {
                std::process::exit(0);
            }
            HandleStatus::ignored()
        });

        // Prepare geometry (use logical size - geometry renderer handles scaling)
        let size = self.window.logical_size();
        self.geometry.clear();
        self.draw_chart((size.width, size.height));

        // Begin frame and render
        let mut frame = self.window.begin_drawing();
        let viewport = self.window.viewport();

        frame.clear_and_render(
            RenderTarget::Surface,
            Color::from_rgb_u8(18, 18, 22),
            |pass| {
                self.geometry.render(pass.wgpu_pass(), viewport);
            },
        );

        frame.finish();
    }
}
