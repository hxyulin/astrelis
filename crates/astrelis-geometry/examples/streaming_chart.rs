//! Advanced Streaming Chart Example with GPU Acceleration
//!
//! This example demonstrates:
//! - **GPU-accelerated line rendering** for large datasets
//! - Real-time data streaming with automatic downsampling (LTTB)
//! - Auto-scrolling time axis with manual override
//! - Interactive pan/zoom with gesture support
//! - Efficient ring buffer storage
//! - Multiple sensor streams
//! - Per-series dirty tracking for efficient updates
//! - FPS and performance statistics
//!
//! The GPU rendering path automatically activates for charts with >500 points
//! per series, providing 60x-300x performance improvement for large datasets.
//!
//! Controls:
//! - **SPACE**: Pause/resume streaming
//! - **R**: Reset view and re-enable auto-scroll
//! - **A**: Toggle auto-scroll on/off
//! - **Mouse drag**: Pan the chart (disables auto-scroll)
//! - **Scroll**: Zoom in/out
//! - **Pinch gesture**: Zoom (on trackpads)
//! - **+/-**: Zoom in/out
//! - **ESC**: Exit
//!
//! Run with: cargo run -p astrelis-geometry --features ui-integration --example streaming_chart

use astrelis_core::logging;
use astrelis_core::profiling::{init_profiling, new_frame, ProfilingBackend};
use astrelis_geometry::{
    chart::{
        AxisId, ChartBuilder, DataPoint, GpuStreamingChart, InteractiveChartController,
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
use std::time::Instant;

/// Maximum points per series (ring buffer size).
const MAX_POINTS_PER_SERIES: usize = 10_000;

/// Points added per frame (simulates high-frequency sensor data).
const POINTS_PER_FRAME: usize = 10;

/// Time window to display (seconds).
const DISPLAY_WINDOW: f64 = 30.0;

struct StreamingApp {
    #[allow(dead_code)]
    graphics: Arc<GraphicsContext>,
    window: RenderableWindow,
    window_id: WindowId,
    geometry: GeometryRenderer,

    // GPU-accelerated streaming chart with auto-scroll
    streaming: GpuStreamingChart,

    // Interactive controller for pan/zoom
    controller: InteractiveChartController,

    // Bounds for interaction
    chart_bounds: Rect,

    // Simulation state
    time: f64,
    paused: bool,
    #[allow(dead_code)]
    start_instant: Instant,

    // Whether user has manually panned (disables auto-scroll temporarily)
    manual_pan_active: bool,

    // Performance tracking
    frame_count: u64,
    last_fps_time: Instant,
    fps: f64,
    last_update_ms: f64,
}

fn main() {
    logging::init();
    init_profiling(ProfilingBackend::PuffinHttp);

    run_app(|ctx| {
        let graphics = GraphicsContext::new_owned_sync().expect("Failed to create graphics context");

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Streaming Chart - GPU Accelerated".to_string(),
                size: Some(WinitPhysicalSize::new(1400.0, 800.0)),
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

        // Create the GPU-accelerated streaming chart
        let chart = create_streaming_chart();
        let mut streaming = GpuStreamingChart::new(chart, graphics.clone(), surface_format)
            .force_gpu_rendering(true); // Force GPU for this demo

        // Configure auto-scroll on X axis
        streaming.auto_scroll(AxisId::X_PRIMARY, DISPLAY_WINDOW);

        let chart_bounds = Rect::new(30.0, 30.0, 1340.0, 740.0);
        let controller = InteractiveChartController::new();

        Box::new(StreamingApp {
            graphics,
            window,
            window_id,
            geometry,
            streaming,
            controller,
            chart_bounds,
            time: 0.0,
            paused: false,
            start_instant: Instant::now(),
            manual_pan_active: false,
            frame_count: 0,
            last_fps_time: Instant::now(),
            fps: 0.0,
            last_update_ms: 0.0,
        })
    });
}

fn create_streaming_chart() -> astrelis_geometry::chart::Chart {
    ChartBuilder::line()
        .title("Real-Time Sensor Monitoring")
        .subtitle("SPACE: pause | Drag: pan | Scroll: zoom | R: reset | A: toggle auto-scroll")
        .x_label("Time (s)")
        .y_label("Sensor Value")
        .y_range(-3.0, 3.0)
        // Create empty series that will be filled with streaming data
        .add_series("Accelerometer X", &[] as &[(f64, f64)])
        .add_series("Accelerometer Y", &[] as &[(f64, f64)])
        .add_series("Accelerometer Z", &[] as &[(f64, f64)])
        .add_series("Gyroscope", &[] as &[(f64, f64)])
        // Grid and styling
        .with_grid()
        .with_legend(LegendPosition::TopRight)
        // Interactivity
        .interactive(true)
        .with_crosshair()
        .with_tooltips()
        // Reference lines
        .add_horizontal_line(0.0, Color::rgba(0.5, 0.5, 0.5, 0.3))
        .add_horizontal_band(-1.0, 1.0, Color::rgba(0.0, 1.0, 0.0, 0.05))
        .padding(60.0)
        .build()
}

impl StreamingApp {
    /// Generate simulated sensor data and push to the streaming chart.
    fn update_sensor_data(&mut self, _dt: f64) {
        if self.paused {
            return;
        }

        let update_start = Instant::now();

        // Generate multiple points per frame for high-frequency simulation
        for _ in 0..POINTS_PER_FRAME {
            self.time += 0.002; // 500 Hz sample rate

            // Accelerometer X - primary vibration signal
            let accel_x = (self.time * 10.0).sin()
                + 0.5 * (self.time * 25.0).sin()
                + 0.2 * (self.time * 67.0).sin()
                + 0.1 * noise(self.time * 100.0);

            // Accelerometer Y - phase-shifted vibration
            let accel_y = (self.time * 10.0 + 2.0).sin()
                + 0.4 * (self.time * 30.0).sin()
                + 0.15 * (self.time * 50.0).cos()
                + 0.1 * noise(self.time * 101.0);

            // Accelerometer Z - gravity component with vibration overlay
            let accel_z = 0.2
                + 0.3 * (self.time * 8.0).sin()
                + 0.1 * (self.time * 40.0).sin()
                + 0.05 * noise(self.time * 102.0);

            // Gyroscope - rotation rate
            let gyro = 0.5 * (self.time * 3.0).sin()
                + 0.3 * (self.time * 7.0).cos()
                + 0.2 * (self.time * 15.0).sin()
                + 0.15 * noise(self.time * 50.0);

            // Push with sliding window (keeps last MAX_POINTS_PER_SERIES)
            self.streaming.push_point(
                0,
                DataPoint::new(self.time, accel_x),
                Some(MAX_POINTS_PER_SERIES),
            );
            self.streaming.push_point(
                1,
                DataPoint::new(self.time, accel_y),
                Some(MAX_POINTS_PER_SERIES),
            );
            self.streaming.push_point(
                2,
                DataPoint::new(self.time, accel_z),
                Some(MAX_POINTS_PER_SERIES),
            );
            self.streaming.push_point(
                3,
                DataPoint::new(self.time, gyro),
                Some(MAX_POINTS_PER_SERIES),
            );
        }

        // Only auto-scroll if user hasn't manually panned
        if !self.manual_pan_active {
            // Apply auto-scroll to keep latest data visible
            // (This is handled internally by the streaming chart's auto_scroll config)
        }

        self.last_update_ms = update_start.elapsed().as_secs_f64() * 1000.0;
    }

    fn draw_chart(&mut self, size: (u32, u32)) {
        let margin = 30.0;
        self.chart_bounds = Rect::new(
            margin,
            margin,
            size.0 as f32 - margin * 2.0,
            size.1 as f32 - margin * 2.0,
        );

        // Update controller bounds for interaction
        self.controller.set_bounds(self.chart_bounds);

        // Conditionally enable/disable auto-scroll based on manual pan state
        if self.manual_pan_active {
            self.streaming.disable_auto_scroll(AxisId::X_PRIMARY);
        } else {
            self.streaming.auto_scroll(AxisId::X_PRIMARY, DISPLAY_WINDOW);
        }

        // Prepare for GPU rendering (rebuilds buffers if data changed)
        self.streaming.prepare_render(&self.chart_bounds);

        // Draw stats overlay
        self.draw_stats_overlay(size);
    }

    fn draw_stats_overlay(&mut self, size: (u32, u32)) {
        // Stats background
        let stats_width = 300.0;
        let stats_height = 160.0;
        let stats_x = size.0 as f32 - stats_width - 15.0;
        let stats_y = size.1 as f32 - stats_height - 15.0;

        self.geometry.draw_rect(
            glam::Vec2::new(stats_x, stats_y),
            glam::Vec2::new(stats_width, stats_height),
            Color::rgba(0.0, 0.0, 0.0, 0.75),
        );

        // Get statistics
        let stats = self.streaming.statistics();

        // Log stats (in a real app, you'd render text here)
        tracing::debug!(
            "FPS: {:.1} | Points: {} | Update: {:.2}ms | GPU: {} | Segments: {} | AutoScroll: {}",
            self.fps,
            stats.total_points,
            self.last_update_ms,
            stats.gpu_enabled,
            stats.gpu_segment_count,
            !self.manual_pan_active,
        );
    }

    fn update_fps(&mut self) {
        self.frame_count += 1;
        let now = Instant::now();
        let elapsed = (now - self.last_fps_time).as_secs_f64();

        if elapsed >= 1.0 {
            self.fps = self.frame_count as f64 / elapsed;
            self.frame_count = 0;
            self.last_fps_time = now;

            let stats = self.streaming.statistics();
            tracing::info!(
                "FPS: {:.1} | Total Points: {} | Series counts: {:?} | Paused: {}",
                self.fps,
                stats.total_points,
                stats.series_counts,
                self.paused
            );
        }
    }
}

/// Simple deterministic noise function.
fn noise(x: f64) -> f64 {
    let x = x.fract();
    (x * 12.9898 + 78.233).sin() * 43758.5453 % 1.0 * 2.0 - 1.0
}

impl App for StreamingApp {
    fn update(&mut self, _ctx: &mut AppCtx, time: &FrameTime) {
        new_frame();

        // Generate sensor data
        self.update_sensor_data(time.delta_seconds() as f64);

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
                                if self.paused { "PAUSED" } else { "RESUMED" }
                            );
                            return HandleStatus::consumed();
                        }
                        Key::Character(c) if c == "r" || c == "R" => {
                            // Reset view and re-enable auto-scroll
                            self.streaming.chart_mut().interactive.reset();
                            self.streaming.mark_view_changed();
                            self.manual_pan_active = false;
                            tracing::info!("View reset, auto-scroll re-enabled");
                            return HandleStatus::consumed();
                        }
                        Key::Character(c) if c == "a" || c == "A" => {
                            // Toggle auto-scroll
                            self.manual_pan_active = !self.manual_pan_active;
                            tracing::info!(
                                "Auto-scroll {}",
                                if self.manual_pan_active {
                                    "DISABLED (manual pan mode)"
                                } else {
                                    "ENABLED"
                                }
                            );
                            return HandleStatus::consumed();
                        }
                        _ => {}
                    }
                }
            }
            HandleStatus::ignored()
        });

        // Handle chart interaction events (pan, zoom, hover)
        events.dispatch(|event| {
            if self
                .controller
                .handle_event(self.streaming.chart_mut(), event)
            {
                // If user is actively panning, temporarily disable auto-scroll
                if self.streaming.chart().interactive.is_dragging {
                    self.manual_pan_active = true;
                }
                self.streaming.mark_view_changed();
                HandleStatus::consumed()
            } else {
                HandleStatus::ignored()
            }
        });

        // Prepare geometry and chart
        let size = self.window.logical_size();
        self.geometry.clear();
        self.draw_chart((size.width, size.height));

        // Begin frame and render
        let mut frame = self.window.begin_drawing();
        let viewport = self.window.viewport();

        frame.clear_and_render(
            RenderTarget::Surface,
            Color::from_rgb_u8(12, 12, 16),
            |pass| {
                // Render GPU-accelerated chart (includes geometry rendering)
                self.streaming.render(
                    pass.wgpu_pass(),
                    viewport,
                    &mut self.geometry,
                    &self.chart_bounds,
                );
            },
        );

        frame.finish();
    }
}
