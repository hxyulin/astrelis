//! Shapes Example - Basic shape rendering with astrelis-geometry
//!
//! This example demonstrates:
//! - Drawing basic shapes (rectangles, circles, lines)
//! - Using the GeometryRenderer with different styles
//! - Stroke and fill combinations
//!
//! Controls:
//! - ESC to exit

use astrelis_core::logging;
use astrelis_core::profiling::{init_profiling, new_frame, ProfilingBackend};
use astrelis_geometry::{GeometryRenderer, PathBuilder, Shape, Stroke, Style};
use astrelis_render::{
    Color, GraphicsContext, RenderTarget, RenderableWindow, WindowContextDescriptor,
};
use astrelis_winit::{
    app::{run_app, App, AppCtx},
    event::{ElementState, Event, EventBatch, HandleStatus, Key, NamedKey},
    window::{WindowBackend, WindowDescriptor, WinitPhysicalSize},
    FrameTime, WindowId,
};
use glam::Vec2;
use std::sync::Arc;

struct ShapesApp {
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
                title: "Geometry Shapes Example".to_string(),
                size: Some(WinitPhysicalSize::new(800.0, 600.0)),
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

        Box::new(ShapesApp {
            graphics,
            window,
            window_id,
            geometry,
        })
    });
}

impl ShapesApp {
    fn draw_shapes(&mut self) {
        // Draw a filled rectangle
        self.geometry
            .draw_rect(Vec2::new(50.0, 50.0), Vec2::new(100.0, 80.0), Color::RED);

        // Draw a filled circle
        self.geometry
            .draw_circle(Vec2::new(250.0, 90.0), 40.0, Color::GREEN);

        // Draw a line
        self.geometry.draw_line(
            Vec2::new(350.0, 50.0),
            Vec2::new(450.0, 130.0),
            3.0,
            Color::BLUE,
        );

        // Draw a stroked rectangle using Path
        let mut builder = PathBuilder::new();
        builder.rect(Vec2::new(500.0, 50.0), Vec2::new(120.0, 80.0));
        let path = builder.build();
        let stroke = Stroke::solid(Color::YELLOW, 2.0);
        self.geometry.draw_path_stroke(&path, &stroke);

        // Draw a rounded rectangle
        let shape = Shape::rounded_rect(Vec2::new(50.0, 180.0), Vec2::new(150.0, 100.0), 15.0);
        let style = Style::fill_color(Color::rgba(0.5, 0.2, 0.8, 1.0));
        self.geometry.draw_shape(&shape, &style);

        // Draw a star
        let star = Shape::star(Vec2::new(300.0, 230.0), 50.0, 25.0, 5);
        let star_style = Style::fill_color(Color::rgba(1.0, 0.8, 0.0, 1.0));
        self.geometry.draw_shape(&star, &star_style);

        // Draw a regular polygon (hexagon)
        let hexagon = Shape::regular_polygon(Vec2::new(450.0, 230.0), 45.0, 6);
        let hex_style = Style::fill_and_stroke(Color::rgba(0.2, 0.6, 0.8, 1.0), Color::WHITE, 2.0);
        self.geometry.draw_shape(&hexagon, &hex_style);

        // Draw a path with curves
        let mut builder = PathBuilder::new();
        builder.move_to(Vec2::new(550.0, 180.0));
        builder.cubic_to(
            Vec2::new(600.0, 150.0),
            Vec2::new(650.0, 250.0),
            Vec2::new(700.0, 200.0),
        );
        builder.cubic_to(
            Vec2::new(750.0, 150.0),
            Vec2::new(700.0, 280.0),
            Vec2::new(550.0, 280.0),
        );
        builder.close();
        let curve_path = builder.build();
        let curve_style = Style::fill_color(Color::rgba(0.8, 0.3, 0.5, 1.0));
        self.geometry.draw_path(&curve_path, &curve_style);

        // Draw an arc/pie
        let pie = Shape::pie(
            Vec2::new(150.0, 400.0),
            60.0,
            0.0,
            std::f32::consts::FRAC_PI_2 * 1.5,
        );
        let pie_style = Style::fill_color(Color::rgba(0.3, 0.7, 0.4, 1.0));
        self.geometry.draw_shape(&pie, &pie_style);

        // Draw multiple overlapping circles with transparency
        for i in 0..5 {
            let x = 320.0 + i as f32 * 30.0;
            let alpha = 0.4;
            let color = match i % 3 {
                0 => Color::rgba(1.0, 0.0, 0.0, alpha),
                1 => Color::rgba(0.0, 1.0, 0.0, alpha),
                _ => Color::rgba(0.0, 0.0, 1.0, alpha),
            };
            self.geometry.draw_circle(Vec2::new(x, 400.0), 35.0, color);
        }

        // Draw a polyline (open path)
        let points = vec![
            Vec2::new(550.0, 350.0),
            Vec2::new(580.0, 380.0),
            Vec2::new(620.0, 360.0),
            Vec2::new(660.0, 400.0),
            Vec2::new(700.0, 370.0),
        ];
        let polyline = Shape::polyline(points, false);
        let polyline_style = Style::stroke_color(Color::CYAN, 3.0);
        self.geometry.draw_shape(&polyline, &polyline_style);

        // Draw an ellipse
        let ellipse = Shape::ellipse(Vec2::new(150.0, 520.0), Vec2::new(80.0, 40.0));
        let ellipse_style =
            Style::fill_and_stroke(Color::rgba(0.9, 0.5, 0.1, 1.0), Color::WHITE, 2.0);
        self.geometry.draw_shape(&ellipse, &ellipse_style);
    }
}

impl App for ShapesApp {
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
                    && let Key::Named(NamedKey::Escape) = &key.logical_key {
                        std::process::exit(0);
                    }
            HandleStatus::ignored()
        });

        // Prepare geometry
        self.geometry.clear();
        self.draw_shapes();

        // Begin frame and render
        let mut frame = self.window.begin_drawing();
        let viewport = self.window.viewport();

        frame.clear_and_render(RenderTarget::Surface, Color::from_rgb_u8(30, 30, 40), |pass| {
            self.geometry.render(pass.wgpu_pass(), viewport);
        });

        frame.finish();
    }
}
