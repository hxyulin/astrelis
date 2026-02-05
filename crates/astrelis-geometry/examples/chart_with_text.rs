//! Chart with Text Rendering Example
//!
//! This example demonstrates using the `chart-text` feature for GPU-accelerated
//! text rendering in charts with astrelis-text integration.
//!
//! Features demonstrated:
//! - Chart title and subtitle (centered above chart)
//! - Axis tick labels with smart number formatting (1.5K, 2.3M, etc.)
//! - Axis labels (X centered below, Y horizontal above)
//! - Legend with color swatches and series names
//! - Multiple series with different data
//! - Reference lines and shaded bands
//!
//! Controls:
//! - ESC to exit
//!
//! Run with: cargo run -p astrelis-geometry --features chart-text --example chart_with_text

use astrelis_core::logging;
use astrelis_core::profiling::{ProfilingBackend, init_profiling, new_frame};
use astrelis_geometry::{
    GeometryRenderer,
    chart::{ChartBuilder, ChartRenderer, ChartTextRenderer, LegendPosition, Rect},
};
use astrelis_render::{Color, GraphicsContext, RenderWindow, RenderWindowBuilder};
use astrelis_text::FontSystem;
use astrelis_winit::{
    FrameTime, WindowId,
    app::{App, AppCtx, run_app},
    event::{ElementState, Event, EventBatch, HandleStatus, Key, NamedKey},
    window::{WindowBackend, WindowDescriptor, WinitPhysicalSize},
};
use std::sync::Arc;

struct ChartTextApp {
    #[allow(dead_code)]
    graphics: Arc<GraphicsContext>,
    window: RenderWindow,
    window_id: WindowId,
    geometry: GeometryRenderer,
    text_renderer: ChartTextRenderer,
    chart: astrelis_geometry::chart::Chart,
}

fn main() {
    logging::init();
    init_profiling(ProfilingBackend::PuffinHttp);

    run_app(|ctx| {
        let graphics =
            GraphicsContext::new_owned_sync().expect("Failed to create graphics context");

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Chart with Text Rendering".to_string(),
                size: Some(WinitPhysicalSize::new(1100.0, 750.0)),
                ..Default::default()
            })
            .expect("Failed to create window");

        let window = RenderWindowBuilder::new()
            .with_depth_default()
            .build(window, graphics.clone())
            .expect("Failed to create render window");

        let window_id = window.id();
        let geometry = GeometryRenderer::new(graphics.clone());

        // Create font system for text rendering
        let font_system = FontSystem::with_system_fonts();
        let text_renderer = ChartTextRenderer::new(graphics.clone(), font_system);

        // Create the chart
        let chart = create_chart();

        Box::new(ChartTextApp {
            graphics,
            window,
            window_id,
            geometry,
            text_renderer,
            chart,
        })
    });
}

fn create_chart() -> astrelis_geometry::chart::Chart {
    // Generate sample data - simulating stock prices
    let num_points = 200;
    let days = 30.0;

    // Stock price with trend and noise
    let stock_a: Vec<(f64, f64)> = (0..num_points)
        .map(|i| {
            let t = (i as f64 / (num_points - 1) as f64) * days;
            let trend = 100.0 + t * 0.5;
            let volatility = 5.0 * (t * 0.3).sin() + 3.0 * (t * 0.7).cos();
            let noise = 2.0 * ((t * 10.0).sin() + (t * 17.0).cos() * 0.5);
            (t, trend + volatility + noise)
        })
        .collect();

    // Another stock with different behavior
    let stock_b: Vec<(f64, f64)> = (0..num_points)
        .map(|i| {
            let t = (i as f64 / (num_points - 1) as f64) * days;
            let trend = 80.0 + t * 0.3;
            let volatility = 8.0 * (t * 0.2 + 1.0).sin() + 4.0 * (t * 0.5).cos();
            let noise = 1.5 * ((t * 12.0).sin() + (t * 23.0).cos() * 0.3);
            (t, trend + volatility + noise)
        })
        .collect();

    // Market average
    let market_avg: Vec<(f64, f64)> = (0..num_points)
        .map(|i| {
            let t = (i as f64 / (num_points - 1) as f64) * days;
            let base = 90.0 + t * 0.4;
            let wave = 3.0 * (t * 0.15).sin();
            (t, base + wave)
        })
        .collect();

    ChartBuilder::line()
        // Title and subtitle
        .title("Stock Price Analysis")
        .subtitle("30-day performance comparison with market benchmark")
        // Axis labels
        .x_label("Trading Day")
        .y_label("Price ($)")
        // Axis ranges
        .x_range(0.0, days)
        .y_range(70.0, 130.0)
        // Data series
        .add_series("ACME Corp (ACME)", &stock_a)
        .add_series("TechStart Inc (TSI)", &stock_b)
        .add_series("Market Index", &market_avg)
        // Visual features
        .with_grid()
        .with_legend(LegendPosition::TopLeft)
        // Reference annotations
        .add_horizontal_line(100.0, Color::rgba(0.5, 0.5, 0.5, 0.4)) // $100 reference
        .add_horizontal_band(95.0, 105.0, Color::rgba(0.0, 1.0, 0.0, 0.05)) // "Stable" zone
        // Layout - extra padding for text elements
        .padding(70.0)
        .build()
}

impl App for ChartTextApp {
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

        // Calculate bounds using logical size (the geometry renderer handles scaling)
        let size = self.window.logical_size();
        let margin = 30.0;
        let bounds = Rect::new(
            margin,
            margin,
            size.width as f32 - margin * 2.0,
            size.height as f32 - margin * 2.0,
        );

        // Set up text renderer first (needed for margin calculation)
        let viewport = self.window.viewport();
        self.text_renderer.set_viewport(viewport);

        // Calculate text margins to adjust the plot area
        // The chart padding already provides space for margins, so we use
        // the padded bounds as the plot area where data is drawn.
        // Text elements are positioned relative to this same plot area.
        let text_margins = self.text_renderer.calculate_margins(&self.chart);

        // Create adjusted bounds that account for text margins
        // This ensures ChartRenderer draws data in the same area as text labels expect
        let adjusted_bounds = Rect::new(
            bounds.x + text_margins.left,
            bounds.y + text_margins.top,
            (bounds.width - text_margins.left - text_margins.right).max(1.0),
            (bounds.height - text_margins.top - text_margins.bottom).max(1.0),
        );

        // The plot area is where the actual chart data is drawn
        // Both ChartRenderer and ChartTextRenderer need to use the same plot area
        let plot_area = adjusted_bounds.inset(self.chart.padding);

        // Clear geometry and prepare for rendering
        self.geometry.clear();

        // Draw chart geometry (grid, axes, data series) using adjusted bounds
        // ChartRenderer internally computes plot_area = adjusted_bounds.inset(padding)
        // which matches our plot_area calculation above
        {
            let mut chart_renderer = ChartRenderer::new(&mut self.geometry);
            chart_renderer.draw(&self.chart, adjusted_bounds);
        }

        // Draw text elements using the same plot_area
        self.text_renderer.draw_title(&self.chart, &bounds);
        self.text_renderer.draw_tick_labels(&self.chart, &plot_area);
        self.text_renderer.draw_axis_labels(&self.chart, &plot_area);
        self.text_renderer
            .draw_legend(&self.chart, &plot_area, &mut self.geometry);

        // Begin frame and render
        let Some(frame) = self.window.begin_frame() else {
            return; // Surface not available
        };

        {
            let mut pass = frame
                .render_pass()
                .clear_color(Color::from_rgb_u8(18, 18, 22))
                .label("chart_with_text_pass")
                .build();

            // Render chart geometry
            self.geometry.render(pass.wgpu_pass(), viewport);

            // Render text
            self.text_renderer.render(pass.wgpu_pass());
        }
        // Frame auto-submits on drop
    }
}
