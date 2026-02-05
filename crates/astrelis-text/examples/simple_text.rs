//! Simple Text Rendering Example
//!
//! Demonstrates basic text rendering with astrelis-text:
//! - Loading system fonts with FontSystem
//! - Creating a FontRenderer
//! - Rendering text to screen
//! - Text positioning and alignment
//! - Font size and color customization
//!
//! ## Features Showcased
//! - `FontSystem` initialization with system fonts
//! - `FontRenderer` setup and viewport configuration
//! - `Text` builder API for styling
//! - GPU-accelerated text rendering
//! - Glyph atlas caching
//!
//! ## Usage
//! ```bash
//! cargo run -p astrelis-text --example simple_text
//! ```

use astrelis_core::logging;
use astrelis_core::math::Vec2;
use astrelis_render::{Color, GraphicsContext, RenderWindow, RenderWindowBuilder, wgpu};
use astrelis_text::{FontRenderer, FontSystem, Text};
use astrelis_winit::{
    FrameTime, WindowId,
    app::{App, AppCtx, run_app},
    event::EventBatch,
    window::{WindowDescriptor, WinitPhysicalSize},
};

struct SimpleTextApp {
    window: RenderWindow,
    window_id: WindowId,
    font_renderer: FontRenderer,
}

fn main() {
    logging::init();

    run_app(|ctx| {
        let graphics_ctx =
            GraphicsContext::new_owned_sync().expect("Failed to create graphics context");

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Simple Text Drawing".to_string(),
                size: Some(WinitPhysicalSize::new(800.0, 600.0)),
                ..Default::default()
            })
            .expect("Failed to create window");

        let window = RenderWindowBuilder::new()
            .color_format(wgpu::TextureFormat::Bgra8UnormSrgb)
            .build(window, graphics_ctx.clone())
            .expect("Failed to create render window");

        let window_id = window.id();

        // Initialize font system with system fonts
        let font_system = FontSystem::with_system_fonts();
        let mut font_renderer = FontRenderer::new(graphics_ctx.clone(), font_system);
        font_renderer.set_viewport(window.viewport());

        tracing::info!("Simple text example initialized");

        Box::new(SimpleTextApp {
            window,
            window_id,
            font_renderer,
        })
    });
}

impl App for SimpleTextApp {
    fn update(&mut self, _ctx: &mut AppCtx, _time: &FrameTime) {
        // No global logic needed for this example
    }

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        if window_id != self.window_id {
            return;
        }

        // Handle window resize events
        events.dispatch(|event| {
            if let astrelis_winit::event::Event::WindowResized(size) = event {
                self.window.resized(*size);
                self.font_renderer.set_viewport(self.window.viewport());
                astrelis_winit::event::HandleStatus::consumed()
            } else {
                astrelis_winit::event::HandleStatus::ignored()
            }
        });

        // Create some simple text
        let hello = Text::new("Hello, World!").size(24.0).color(Color::WHITE);

        let subtitle = Text::new("This is simple text rendering with Astrelis")
            .size(16.0)
            .color(Color::from_rgb_u8(150, 200, 255));

        let info = Text::new("Press Ctrl+C to exit")
            .size(12.0)
            .color(Color::from_rgb_u8(150, 150, 150));

        // Prepare text buffers
        let mut hello_buffer = self.font_renderer.prepare(&hello);
        let mut subtitle_buffer = self.font_renderer.prepare(&subtitle);
        let mut info_buffer = self.font_renderer.prepare(&info);

        // Draw text at different position
        self.font_renderer
            .draw_text(&mut hello_buffer, Vec2::new(50.0, 100.0));
        self.font_renderer
            .draw_text(&mut subtitle_buffer, Vec2::new(50.0, 150.0));
        self.font_renderer
            .draw_text(&mut info_buffer, Vec2::new(50.0, 500.0));

        // Begin frame and render
        let Some(frame) = self.window.begin_frame() else {
            return; // Surface not available (minimized, etc.)
        };

        {
            let mut pass = frame
                .render_pass()
                .clear_color(Color::from_rgb_u8(25, 25, 35))
                .build();

            // Render all text
            self.font_renderer.render(pass.wgpu_pass());
        }
        // Frame auto-submits on drop
    }
}
