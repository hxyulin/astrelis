//! Live Chart with Streaming Data
//!
//! This example demonstrates:
//! - Real-time data streaming with sliding window
//! - Efficient cache-based rendering for large datasets
//! - Pinch gesture and touch support for zoom/pan
//! - Performance optimizations for 10k+ points
//!
//! Controls:
//! - Drag to pan the chart (mouse or touch)
//! - Scroll/pinch to zoom in/out
//! - Hover over points to see tooltips
//! - Press R or Home to reset view
//! - Press SPACE to pause/resume data streaming
//! - Press +/- to zoom in/out
//! - Press ESC to exit
//!
//! Run with: cargo run -p astrelis-geometry --features ui-integration --example live_chart

use astrelis_core::logging;
use astrelis_core::profiling::{ProfilingBackend, init_profiling, new_frame};
use astrelis_geometry::{
    GeometryRenderer,
    chart::{
        ChartBuilder, ChartRenderer, DataPoint, InteractiveChartController, LegendPosition, Rect,
        StreamingChart,
    },
};
use astrelis_render::{Color, GraphicsContext, RenderWindow, RenderWindowBuilder};
use astrelis_winit::{
    FrameTime, WindowId,
    app::{App, AppCtx, run_app},
    event::{ElementState, Event, EventBatch, HandleStatus, Key, NamedKey},
    window::{WindowBackend, WindowDescriptor, WinitPhysicalSize},
};
use std::sync::Arc;
use std::time::Instant;

/// Maximum number of points to keep in the sliding window.
const MAX_POINTS: usize = 1000;

/// Number of points to add per update.
const POINTS_PER_UPDATE: usize = 5;

struct LiveChartApp {
    #[allow(dead_code)]
    graphics: Arc<GraphicsContext>,
    window: RenderWindow,
    window_id: WindowId,
    geometry: GeometryRenderer,

    // Chart with streaming support
    streaming: StreamingChart,
    controller: InteractiveChartController,

    // Simulation state
    time: f64,
    paused: bool,
    frame_count: u64,
    last_fps_update: Instant,
    fps: f64,

    // Tooltip state
    tooltip_text: Option<String>,
}

fn main() {
    logging::init();
    init_profiling(ProfilingBackend::PuffinHttp);

    run_app(|ctx| {
        let graphics =
            GraphicsContext::new_owned_sync().expect("Failed to create graphics context");

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Live Chart - Streaming Data Demo".to_string(),
                size: Some(WinitPhysicalSize::new(1200.0, 800.0)),
                ..Default::default()
            })
            .expect("Failed to create window");

        let window = RenderWindowBuilder::new()
            .with_depth_default()
            .build(window, graphics.clone())
            .expect("Failed to create render window");

        let window_id = window.id();
        let geometry = GeometryRenderer::new(graphics.clone());

        // Create the chart
        let chart = create_streaming_chart();
        let streaming = StreamingChart::new(chart);
        let controller = InteractiveChartController::new();

        Box::new(LiveChartApp {
            graphics,
            window,
            window_id,
            geometry,
            streaming,
            controller,
            time: 0.0,
            paused: false,
            frame_count: 0,
            last_fps_update: Instant::now(),
            fps: 0.0,
            tooltip_text: None,
        })
    });
}

fn create_streaming_chart() -> astrelis_geometry::chart::Chart {
    ChartBuilder::line()
        .title("Live Streaming Data")
        .subtitle("Real-time sensor simulation (SPACE to pause)")
        .x_label("Time (s)")
        .y_label("Value")
        .y_range(-2.0, 2.0)
        .add_series("Signal A", &[] as &[(f64, f64)])
        .add_series("Signal B", &[] as &[(f64, f64)])
        .add_series("Noise", &[] as &[(f64, f64)])
        .with_grid()
        .with_legend(LegendPosition::TopRight)
        .interactive(true)
        .with_crosshair()
        .with_tooltips()
        // Add bands to show acceptable range
        .add_horizontal_band(-1.0, 1.0, Color::rgba(0.0, 1.0, 0.0, 0.08))
        .padding(60.0)
        .build()
}

impl LiveChartApp {
    fn update_data(&mut self, _dt: f64) {
        if self.paused {
            return;
        }

        // Add multiple points per update for faster data rate
        for _ in 0..POINTS_PER_UPDATE {
            self.time += 0.02; // 50 samples per second

            // Signal A: Primary sine wave with harmonics
            let signal_a = (self.time * 2.0).sin()
                + 0.3 * (self.time * 7.0).sin()
                + 0.15 * (self.time * 13.0).sin();

            // Signal B: Phase-shifted and amplitude-modulated
            let envelope = 0.5 + 0.5 * (self.time * 0.3).sin();
            let signal_b = envelope * (self.time * 2.0 + 1.5).sin();

            // Noise: Random walk with mean reversion
            use std::f64::consts::PI;
            let noise_val = 0.3
                * ((self.time * 0.7).sin()
                    + (self.time * 1.3).cos()
                    + (self.time * 2.7 + PI).sin());

            // Use streaming API with sliding window
            self.streaming
                .push_point(0, DataPoint::new(self.time, signal_a), Some(MAX_POINTS));
            self.streaming
                .push_point(1, DataPoint::new(self.time, signal_b), Some(MAX_POINTS));
            self.streaming
                .push_point(2, DataPoint::new(self.time, noise_val), Some(MAX_POINTS));
        }

        // Auto-scroll X axis to keep latest data visible
        if self.streaming.chart().series_len(0) > 0 {
            let latest_x = self.time;
            let window_size = 20.0; // Show last 20 seconds

            // Update X axis range
            if let Some(x_axis) = self
                .streaming
                .chart_mut()
                .get_axis_mut(astrelis_geometry::chart::AxisId::X_PRIMARY)
            {
                x_axis.min = Some((latest_x - window_size).max(0.0));
                x_axis.max = Some(latest_x);
            }

            // Mark view as changed
            self.streaming.mark_view_changed();
        }
    }

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

        // Prepare cache for rendering
        self.streaming.prepare_render(&bounds);

        // Draw the chart
        let mut chart_renderer = ChartRenderer::new(&mut self.geometry);
        chart_renderer.draw(self.streaming.chart(), bounds);

        // Draw FPS and stats overlay
        self.draw_stats(size);
    }

    fn draw_stats(&mut self, size: (u32, u32)) {
        // Draw stats background
        let stats_width = 200.0;
        let stats_height = 80.0;
        let stats_x = size.0 as f32 - stats_width - 10.0;
        let stats_y = size.1 as f32 - stats_height - 10.0;

        self.geometry.draw_rect(
            glam::Vec2::new(stats_x, stats_y),
            glam::Vec2::new(stats_width, stats_height),
            Color::rgba(0.0, 0.0, 0.0, 0.7),
        );

        // Stats would be drawn here with text rendering
        // For now, we log them
        let total_points = self.streaming.chart().total_points();
        let dirty = self.streaming.dirty_flags();

        tracing::trace!(
            "FPS: {:.1} | Points: {} | Dirty: {:?}",
            self.fps,
            total_points,
            dirty
        );
    }

    fn update_fps(&mut self) {
        self.frame_count += 1;
        let now = Instant::now();
        let elapsed = (now - self.last_fps_update).as_secs_f64();

        if elapsed >= 1.0 {
            self.fps = self.frame_count as f64 / elapsed;
            self.frame_count = 0;
            self.last_fps_update = now;

            let total_points = self.streaming.chart().total_points();
            tracing::info!(
                "FPS: {:.1} | Total Points: {} | Paused: {}",
                self.fps,
                total_points,
                self.paused
            );
        }
    }
}

impl App for LiveChartApp {
    fn update(&mut self, _ctx: &mut AppCtx, time: &FrameTime) {
        new_frame();

        // Update streaming data
        self.update_data(time.delta_seconds() as f64);

        // Update FPS counter
        self.update_fps();
    }

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        if window_id != self.window_id {
            return;
        }

        // Handle window resize
        events.dispatch(|event| {
            if let Event::WindowResized(size) = event {
                self.window.resized(*size);
                self.streaming.mark_bounds_changed();
                return HandleStatus::consumed();
            }
            HandleStatus::ignored()
        });

        // Handle keyboard events
        events.dispatch(|event| {
            if let Event::KeyInput(key) = event {
                if key.state == ElementState::Pressed {
                    match &key.logical_key {
                        Key::Named(NamedKey::Escape) => {
                            std::process::exit(0);
                        }
                        Key::Named(NamedKey::Space) => {
                            self.paused = !self.paused;
                            tracing::info!(
                                "Streaming {}",
                                if self.paused { "paused" } else { "resumed" }
                            );
                            return HandleStatus::consumed();
                        }
                        _ => {}
                    }
                }
            }
            HandleStatus::ignored()
        });

        // Handle chart interaction events (including gestures)
        events.dispatch(|event| {
            if self
                .controller
                .handle_event(self.streaming.chart_mut(), event)
            {
                // If view was changed by interaction, mark it
                self.streaming.mark_view_changed();
                HandleStatus::consumed()
            } else {
                HandleStatus::ignored()
            }
        });

        // Update tooltip text
        self.tooltip_text = self.controller.tooltip_text(self.streaming.chart());

        // Prepare geometry
        let size = self.window.logical_size();
        self.geometry.clear();
        self.draw_chart((size.width, size.height));

        // Begin frame and render
        let viewport = self.window.viewport();
        let Some(frame) = self.window.begin_frame() else {
            return; // Surface not available
        };

        {
            let mut pass = frame
                .render_pass()
                .clear_color(Color::from_rgb_u8(18, 18, 22))
                .label("live_chart_pass")
                .build();
            self.geometry.render(pass.wgpu_pass(), viewport);
        }
        // Frame auto-submits on drop
    }
}
