//! Text Decoration API Demo - Underlines, Strikethrough, Backgrounds
//!
//! **⚠️  IMPORTANT**: This demo shows the TextDecoration API structure ONLY.
//! The text displayed here does NOT visually show the decorations because
//! full rendering integration is currently in development.
//!
//! TextDecoration API demonstrates:
//! - Underline styles (solid, dashed, dotted, wavy)
//! - Strikethrough
//! - Background highlighting
//! - Color and thickness customization
//!
//! This is an API reference example - visual rendering coming soon!

use std::sync::Arc;
use astrelis_core::logging;
use astrelis_core::math::Vec2;
use astrelis_render::{
    Color, GraphicsContext, RenderTarget, RenderableWindow,
    WindowContextDescriptor, wgpu,
};
use astrelis_text::{
    FontRenderer, FontSystem, Text, TextDecoration, UnderlineStyle,
    StrikethroughStyle,
};
use astrelis_winit::{
    WindowId,
    app::{App, AppCtx, run_app},
    event::EventBatch,
    window::{PhysicalSize, WindowBackend, WindowDescriptor},
};

struct TextDecorationDemo {
    _context: Arc<GraphicsContext>,
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
                title: "Text Decoration Demo - Underlines & Strikethrough".to_string(),
                size: Some(PhysicalSize::new(1100.0, 750.0)),
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

        let font_system = FontSystem::with_system_fonts();
        let font_renderer = FontRenderer::new(graphics_ctx.clone(), font_system);

        println!("\n═══════════════════════════════════════════════════════");
        println!("  ⚠️  TEXT DECORATION API DEMO (API Reference Only)");
        println!("═══════════════════════════════════════════════════════");
        println!("\n  ⚠️  IMPORTANT: Decorations are NOT visually rendered!");
        println!("  This demo shows the TextDecoration API structure only.");
        println!("  Visual rendering is in development.");
        println!("\n  DEMONSTRATED API:");
        println!("    • UnderlineStyle::solid() - Solid underlines");
        println!("    • UnderlineStyle::dashed() - Dashed underlines");
        println!("    • UnderlineStyle::dotted() - Dotted underlines");
        println!("    • UnderlineStyle::wavy() - Wavy underlines");
        println!("    • StrikethroughStyle::new() - Strikethrough text");
        println!("    • TextDecoration::background() - Highlighting");
        println!("\n  The text you see has NO visual decorations applied.");
        println!("  This is purely an API structure demonstration.");
        println!("═══════════════════════════════════════════════════════\n");

        tracing::info!("Text decoration demo initialized");

        Box::new(TextDecorationDemo {
            _context: graphics_ctx,
            window,
            window_id,
            font_renderer,
        })
    });
}

impl App for TextDecorationDemo {
    fn update(&mut self, _ctx: &mut AppCtx) {
        // No update logic needed
    }

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        if window_id != self.window_id {
            return;
        }

        // Handle resize
        events.dispatch(|event| {
            if let astrelis_winit::event::Event::WindowResized(size) = event {
                self.window.resized(*size);
                astrelis_winit::event::HandleStatus::consumed()
            } else {
                astrelis_winit::event::HandleStatus::ignored()
            }
        });

        self.font_renderer.set_viewport(self.window.viewport());

        // Create various decoration styles

        // Solid underline
        let solid_underline = UnderlineStyle::solid(
            Color::from_rgb_u8(100, 150, 255),
            2.0
        );

        // Dashed underline
        let dashed_underline = UnderlineStyle::dashed(
            Color::from_rgb_u8(255, 150, 100),
            2.0
        );

        // Dotted underline
        let dotted_underline = UnderlineStyle::dotted(
            Color::from_rgb_u8(150, 255, 150),
            2.0
        );

        // Wavy underline (like spell-check)
        let wavy_underline = UnderlineStyle::wavy(
            Color::RED,
            1.5
        );

        // Strikethrough
        let strikethrough = StrikethroughStyle::solid(
            Color::from_rgb_u8(180, 180, 180),
            1.5
        );

        // Create decoration configurations
        let _decoration_solid = TextDecoration::new()
            .underline(solid_underline);

        let _decoration_dashed = TextDecoration::new()
            .underline(dashed_underline);

        let _decoration_dotted = TextDecoration::new()
            .underline(dotted_underline);

        let _decoration_wavy = TextDecoration::new()
            .underline(wavy_underline);

        let _decoration_strike = TextDecoration::new()
            .strikethrough(strikethrough);

        let _decoration_highlight = TextDecoration::new()
            .background(Color::from_rgb_u8(80, 80, 100));

        let _decoration_combined = TextDecoration::new()
            .underline(solid_underline)
            .background(Color::from_rgb_u8(60, 60, 80));

        // Create text examples
        let examples = [
            ("Solid Underline", "Blue solid line below text"),
            ("Dashed Underline", "Orange dashed line style"),
            ("Dotted Underline", "Green dotted line style"),
            ("Wavy Underline", "Red wavy line (spell-check style)"),
            ("Strikethrough Text", "Line through the middle"),
            ("Highlighted Background", "Yellow background highlight"),
            ("Combined Decoration", "Underline + background"),
        ];

        // Prepare all text
        let title = Text::new("Text Decoration Styles:")
            .size(24.0)
            .color(Color::from_rgb_u8(150, 150, 200))
            .bold();
        let mut title_buffer = self.font_renderer.prepare(&title);

        let mut example_buffers = Vec::new();
        let mut desc_buffers = Vec::new();

        for (example_text, desc_text) in &examples {
            let text = Text::new(*example_text)
                .size(28.0)
                .color(Color::WHITE);
            let desc = Text::new(*desc_text)
                .size(12.0)
                .color(Color::from_rgb_u8(180, 180, 180));

            example_buffers.push(self.font_renderer.prepare(&text));
            desc_buffers.push(self.font_renderer.prepare(&desc));
        }

        // Info about API
        let api_info = Text::new(
            "The decoration API allows you to add:\n\
             • underline(style) - various line styles below text\n\
             • strikethrough(style) - line through text\n\
             • background(color) - highlighting behind text"
        )
        .size(13.0)
        .color(Color::from_rgb_u8(200, 200, 150))
        .max_width(self.window.size_f32().width - 100.0)
        .line_height(1.6);
        let mut api_buffer = self.font_renderer.prepare(&api_info);

        // Note
        let note = Text::new(
            "Note: Full decoration rendering is in development. This demo shows the API structure."
        )
        .size(11.0)
        .color(Color::from_rgb_u8(150, 150, 100))
        .max_width(self.window.size_f32().width - 100.0);
        let mut note_buffer = self.font_renderer.prepare(&note);

        // Draw all text
        let mut y = 50.0;

        self.font_renderer.draw_text(&mut title_buffer, Vec2::new(50.0, y));
        y += 60.0;

        for i in 0..examples.len() {
            self.font_renderer.draw_text(&mut example_buffers[i], Vec2::new(50.0, y));
            y += 40.0;
            self.font_renderer.draw_text(&mut desc_buffers[i], Vec2::new(70.0, y));
            y += 35.0;
        }

        y += 20.0;
        self.font_renderer.draw_text(&mut api_buffer, Vec2::new(50.0, y));
        y += 110.0;
        self.font_renderer.draw_text(&mut note_buffer, Vec2::new(50.0, y));

        // Begin frame
        let mut frame = self.window.begin_drawing();

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
