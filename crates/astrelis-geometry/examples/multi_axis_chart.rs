//! Multi-Axis Chart Example
//!
//! This example demonstrates:
//! - Multiple Y axes with different scales
//! - Custom named axes
//! - Configurable grid lines (major/minor)
//! - Series assigned to different axes
//! - Legend with multi-axis support
//!
//! Controls:
//! - ESC to exit
//!
//! Run with: cargo run -p astrelis-geometry --features ui-integration --example multi_axis_chart

use astrelis_core::logging;
use astrelis_core::profiling::{init_profiling, new_frame, ProfilingBackend};
use astrelis_geometry::{
    chart::{
        Axis, AxisId, AxisPosition, ChartBuilder, ChartRenderer, InteractiveChartController,
        LegendPosition, Rect,
    },
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

struct MultiAxisApp {
    #[allow(dead_code)]
    graphics: Arc<GraphicsContext>,
    window: RenderableWindow,
    window_id: WindowId,
    geometry: GeometryRenderer,
    chart: astrelis_geometry::chart::Chart,
    controller: InteractiveChartController,
}

fn main() {
    logging::init();
    init_profiling(ProfilingBackend::PuffinHttp);

    run_app(|ctx| {
        let graphics = GraphicsContext::new_owned_sync().expect("Failed to create graphics context");

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Multi-Axis Chart - Temperature, Pressure & Humidity".to_string(),
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
        let geometry = GeometryRenderer::new(graphics.clone());
        let chart = create_multi_axis_chart();
        let controller = InteractiveChartController::new();

        Box::new(MultiAxisApp {
            graphics,
            window,
            window_id,
            geometry,
            chart,
            controller,
        })
    });
}

fn create_multi_axis_chart() -> astrelis_geometry::chart::Chart {
    // Generate time series data (24 hours of simulated sensor data)
    let num_points = 288; // One reading every 5 minutes for 24 hours
    let hours = 24.0;

    // Temperature data (varies sinusoidally with some noise)
    let temperature_data: Vec<(f64, f64)> = (0..num_points)
        .map(|i| {
            let t = (i as f64 / (num_points - 1) as f64) * hours;
            // Temperature varies from 15°C to 30°C with daily cycle
            let temp = 22.5 + 7.5 * ((t - 6.0) * std::f64::consts::PI / 12.0).sin();
            // Add some noise
            let noise = (t * 10.0).sin() * 0.5 + (t * 23.0).sin() * 0.3;
            (t, temp + noise)
        })
        .collect();

    // Pressure data (higher values, different scale)
    let pressure_data: Vec<(f64, f64)> = (0..num_points)
        .map(|i| {
            let t = (i as f64 / (num_points - 1) as f64) * hours;
            // Atmospheric pressure in hPa (typical range: 980-1030)
            let base_pressure = 1013.25;
            let variation = 10.0 * (t * std::f64::consts::PI / 12.0).sin();
            let noise = (t * 5.0).sin() * 2.0;
            (t, base_pressure + variation + noise)
        })
        .collect();

    // Humidity data (percentage)
    let humidity_data: Vec<(f64, f64)> = (0..num_points)
        .map(|i| {
            let t = (i as f64 / (num_points - 1) as f64) * hours;
            // Humidity inversely related to temperature
            let base_humidity = 60.0;
            let temp_effect = -15.0 * ((t - 6.0) * std::f64::consts::PI / 12.0).sin();
            let noise = (t * 7.0).sin() * 5.0;
            (t, (base_humidity + temp_effect + noise).clamp(20.0, 95.0))
        })
        .collect();

    // Create the pressure axis ID using a name
    let pressure_axis = AxisId::from_name("pressure");
    let humidity_axis = AxisId::from_name("humidity");

    ChartBuilder::line()
        .title("Environmental Monitoring - 24 Hour Data")
        .subtitle("Temperature, Pressure, and Humidity sensors")
        .x_label("Time (hours)")
        .y_label("Temperature (°C)")
        .x_range(0.0, hours)
        .y_range(10.0, 35.0) // Temperature range
        // Primary Y axis (Temperature) - already set up by default
        // Add secondary Y axis for Pressure (right side)
        .secondary_y_axis(
            Axis::y_secondary()
                .with_label("Pressure (hPa)")
                .with_range(990.0, 1040.0)
                .with_id(pressure_axis),
        )
        .secondary_y_range(990.0, 1040.0)
        // Add third Y axis for Humidity (far right, offset)
        .add_axis(
            Axis::y_secondary()
                .with_id(humidity_axis)
                .with_label("Humidity (%)")
                .with_range(0.0, 100.0)
                .with_position(AxisPosition::Right),
        )
        // Temperature series (uses primary Y axis - default)
        .add_series("Temperature", &temperature_data)
        // Pressure series (uses secondary Y axis)
        .add_series_secondary_y("Pressure", &pressure_data)
        // Humidity series (uses custom humidity axis)
        .add_series_with_axes("Humidity", &humidity_data, AxisId::X_PRIMARY, humidity_axis)
        // Configure grid
        .with_grid()
        // Legend
        .with_legend(LegendPosition::TopLeft)
        // Interactivity
        .interactive(true)
        .with_crosshair()
        .with_tooltips()
        // Add annotation lines for reference values
        .add_horizontal_line(25.0, Color::rgba(1.0, 0.5, 0.0, 0.3)) // Comfortable temp
        // Add shaded region for "comfortable" temperature zone
        .add_horizontal_band(20.0, 26.0, Color::rgba(0.0, 1.0, 0.0, 0.08))
        .padding(80.0) // More padding for multiple axis labels
        .build()
}

impl MultiAxisApp {
    fn draw_chart(&mut self, size: (u32, u32)) {
        let margin = 30.0;
        let bounds = Rect::new(
            margin,
            margin,
            size.0 as f32 - margin * 2.0,
            size.1 as f32 - margin * 2.0,
        );

        // Update controller bounds
        self.controller.set_bounds(bounds);

        let mut chart_renderer = ChartRenderer::new(&mut self.geometry);
        chart_renderer.draw(&self.chart, bounds);
    }
}

impl App for MultiAxisApp {
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

        // Handle chart interaction events
        events.dispatch(|event| {
            if self.controller.handle_event(&mut self.chart, event) {
                HandleStatus::consumed()
            } else {
                HandleStatus::ignored()
            }
        });

        // Prepare geometry
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
