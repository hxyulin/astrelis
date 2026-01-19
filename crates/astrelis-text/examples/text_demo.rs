//! Text Rendering Demo - Basic Text Features
//!
//! Demonstrates the core text rendering capabilities:
//! - Various font sizes and colors
//! - Bold and italic styles
//! - Text alignment
//! - Line height control
//! - Monospace fonts
//!
//! For more advanced features, see:
//! - rich_text_demo.rs - Inline formatting with mixed styles
//! - text_effects.rs - Shadows, outlines, glows
//! - text_decoration.rs - Underlines, strikethrough, backgrounds
//! - text_editor_demo.rs - Text editing with cursor and selection

use std::sync::Arc;
use astrelis_core::logging;
use astrelis_core::math::Vec2;
use astrelis_render::{
    Color, GraphicsContext, RenderTarget, RenderableWindow,
    WindowContextDescriptor, wgpu,
};
use astrelis_text::{FontRenderer, FontSystem, FontWeight, Text, TextAlign};
use astrelis_winit::{
    WindowId,
    app::{App, AppCtx, run_app},
    event::EventBatch,
    window::{PhysicalSize, WindowBackend, WindowDescriptor},
};

struct TextDemo {
    context: Arc<GraphicsContext>,
    window: RenderableWindow,
    window_id: WindowId,
    font_renderer: FontRenderer,
}

fn main() {
    logging::init();

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_owned_sync();

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Text Rendering Demo".to_string(),
                size: Some(PhysicalSize::new(1024.0, 768.0)),
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
        );

        let window_id = window.id();

        // Create font system with system fonts
        let font_system = FontSystem::with_system_fonts();
        let font_renderer = FontRenderer::new(graphics_ctx.clone(), font_system);

        tracing::info!("Text demo initialized");

        Box::new(TextDemo {
            context: graphics_ctx,
            window,
            window_id,
            font_renderer,
        })
    });
}

impl App for TextDemo {
    fn update(&mut self, _ctx: &mut AppCtx) {
        // Global logic (none needed for this demo)
    }

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        if window_id != self.window_id {
            return;
        }

        // Handle resize events
        events.dispatch(|event| {
            if let astrelis_winit::event::Event::WindowResized(size) = event {
                self.window.resized(*size);
                astrelis_winit::event::HandleStatus::consumed()
            } else {
                astrelis_winit::event::HandleStatus::ignored()
            }
        });

        // set the viewport
        self.font_renderer.set_viewport(self.window.viewport());

        // Prepare text examples
        let title = Text::new("Astrelis Text Rendering")
            .size(24.0)
            .color(Color::WHITE)
            .weight(FontWeight::Bold);

        let subtitle = Text::new("Powered by cosmic-text")
            .size(18.0)
            .color(Color::from_rgb_u8(150, 150, 255))
            .align(TextAlign::Left);

        let body_text = Text::new(
            "This is a demonstration of text rendering with various styles and properties. \
             The text system supports different sizes, colors, weights, and alignments.",
        )
        .size(12.0)
        .color(Color::from_rgb_u8(200, 200, 200))
        .max_width(self.window.size_f32().width - 50.0)
        .line_height(1.5);

        let bold_text = Text::new("Bold text example")
            .size(14.0)
            .color(Color::YELLOW)
            .bold();

        let italic_text = Text::new("Italic text example")
            .size(14.0)
            .color(Color::CYAN)
            .italic();

        let colored_samples = [
            ("Red text", Color::RED),
            ("Green text", Color::GREEN),
            ("Blue text", Color::BLUE),
            ("Yellow text", Color::YELLOW),
            ("Magenta text", Color::MAGENTA),
            ("Cyan text", Color::CYAN),
        ];

        let monospace_text = Text::new("Monospace font: fn main() { println!(\"Hello\"); }")
            .size(18.0)
            .color(Color::from_rgb_u8(100, 255, 100))
            .font("monospace");

        // Prepare text buffers
        let mut title_buffer = self.font_renderer.prepare(&title);
        let mut subtitle_buffer = self.font_renderer.prepare(&subtitle);
        let mut body_buffer = self.font_renderer.prepare(&body_text);
        let mut bold_buffer = self.font_renderer.prepare(&bold_text);
        let mut italic_buffer = self.font_renderer.prepare(&italic_text);
        let mut monospace_buffer = self.font_renderer.prepare(&monospace_text);

        let mut color_buffers: Vec<_> = colored_samples
            .iter()
            .map(|(text, color)| {
                let t = Text::new(*text).size(12.0).color(*color);
                self.font_renderer.prepare(&t)
            })
            .collect();

        // Draw text
        let mut y = 50.0;
        self.font_renderer
            .draw_text(&mut title_buffer, Vec2::new(50.0, y));
        y += 60.0;

        self.font_renderer
            .draw_text(&mut subtitle_buffer, Vec2::new(50.0, y));
        y += 40.0;

        self.font_renderer
            .draw_text(&mut body_buffer, Vec2::new(50.0, y));
        y += 120.0;

        self.font_renderer
            .draw_text(&mut bold_buffer, Vec2::new(50.0, y));
        y += 35.0;

        self.font_renderer
            .draw_text(&mut italic_buffer, Vec2::new(50.0, y));
        y += 50.0;

        for buffer in &mut color_buffers {
            self.font_renderer.draw_text(buffer, Vec2::new(50.0, y));
            y += 30.0;
        }

        y += 20.0;
        self.font_renderer
            .draw_text(&mut monospace_buffer, Vec2::new(50.0, y));

        // Begin frame
        let mut frame = self.window.begin_drawing();

        // Render with automatic scoping (no manual {} block needed)
        frame.clear_and_render(
            RenderTarget::Surface,
            Color::from_rgb_u8(20, 20, 30),
            |pass| {
                self.font_renderer.render(pass.descriptor());
            },
        );

        frame.finish();
    }
}
