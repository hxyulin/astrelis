//! Text Decoration Demo - Underlines, Strikethrough, Backgrounds
//!
//! This demo shows the TextDecoration rendering capabilities:
//! - Underline styles (solid only for MVP)
//! - Strikethrough
//! - Background highlighting
//! - Color and thickness customization

use astrelis_core::logging;
use astrelis_core::math::Vec2;
use astrelis_render::{Color, GraphicsContext, RenderWindow, RenderWindowBuilder, wgpu};
use astrelis_text::{
    FontRenderer, FontSystem, StrikethroughStyle, Text, TextDecoration, UnderlineStyle,
};
use astrelis_winit::{
    FrameTime, WindowId,
    app::{App, AppCtx, run_app},
    event::EventBatch,
    window::{WindowDescriptor, WinitPhysicalSize},
};

struct TextDecorationDemo {
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
                title: "Text Decoration Demo - Underlines & Strikethrough".to_string(),
                size: Some(WinitPhysicalSize::new(900.0, 700.0)),
                ..Default::default()
            })
            .expect("Failed to create window");

        let window = RenderWindowBuilder::new()
            .color_format(wgpu::TextureFormat::Bgra8UnormSrgb)
            .build(window, graphics_ctx.clone())
            .expect("Failed to create render window");

        let window_id = window.id();

        let font_system = FontSystem::with_system_fonts();
        let font_renderer = FontRenderer::new(graphics_ctx.clone(), font_system);

        println!("\n========================================");
        println!("  Text Decoration Demo");
        println!("========================================");
        println!("\n  Demonstrating:");
        println!("    - Solid underlines");
        println!("    - Strikethrough");
        println!("    - Background highlighting");
        println!("    - Combined decorations");
        println!("========================================\n");

        tracing::info!("Text decoration demo initialized");

        Box::new(TextDecorationDemo {
            window,
            window_id,
            font_renderer,
        })
    });
}

impl App for TextDecorationDemo {
    fn update(&mut self, _ctx: &mut AppCtx, _time: &FrameTime) {
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

        // === Title ===
        let title = Text::new("Text Decoration Styles")
            .size(28.0)
            .color(Color::from_rgb_u8(150, 180, 255))
            .bold();
        let mut title_buffer = self.font_renderer.prepare(&title);

        // === Underline Examples ===

        // Simple blue underline using convenience method
        let underlined_text = Text::new("Underlined Text")
            .size(24.0)
            .color(Color::WHITE)
            .underline(Color::from_rgb_u8(100, 150, 255));
        let mut underlined_buffer = self.font_renderer.prepare(&underlined_text);

        // Custom underline with thickness
        let thick_underline = Text::new("Thick Underline")
            .size(24.0)
            .color(Color::WHITE)
            .with_decoration(TextDecoration::new().underline(UnderlineStyle::solid(
                Color::from_rgb_u8(255, 150, 100),
                3.0,
            )));
        let mut thick_underline_buffer = self.font_renderer.prepare(&thick_underline);

        // === Strikethrough Examples ===

        // Simple strikethrough
        let strikethrough_text = Text::new("Deleted Text")
            .size(24.0)
            .color(Color::from_rgb_u8(180, 180, 180))
            .strikethrough(Color::RED);
        let mut strikethrough_buffer = self.font_renderer.prepare(&strikethrough_text);

        // Custom strikethrough
        let custom_strike = Text::new("Custom Strikethrough")
            .size(24.0)
            .color(Color::WHITE)
            .with_decoration(
                TextDecoration::new().strikethrough(StrikethroughStyle::solid(
                    Color::from_rgb_u8(255, 200, 50),
                    2.0,
                )),
            );
        let mut custom_strike_buffer = self.font_renderer.prepare(&custom_strike);

        // === Background Examples ===

        // Simple background highlight
        let highlighted_text = Text::new("Highlighted Text")
            .size(24.0)
            .color(Color::BLACK)
            .background_color(Color::from_rgb_u8(255, 255, 100));
        let mut highlighted_buffer = self.font_renderer.prepare(&highlighted_text);

        // Dark background
        let dark_highlight = Text::new("Dark Background")
            .size(24.0)
            .color(Color::WHITE)
            .with_decoration(TextDecoration::new().background(Color::from_rgb_u8(50, 50, 80)));
        let mut dark_highlight_buffer = self.font_renderer.prepare(&dark_highlight);

        // === Combined Decorations ===

        // Underline + background
        let combined1 = Text::new("Underline + Background")
            .size(24.0)
            .color(Color::WHITE)
            .with_decoration(
                TextDecoration::new()
                    .underline(UnderlineStyle::solid(
                        Color::from_rgb_u8(100, 200, 100),
                        2.0,
                    ))
                    .background(Color::from_rgb_u8(30, 60, 30)),
            );
        let mut combined1_buffer = self.font_renderer.prepare(&combined1);

        // Strikethrough + background (for "removed" effect)
        let combined2 = Text::new("Removed Item")
            .size(24.0)
            .color(Color::from_rgb_u8(150, 150, 150))
            .with_decoration(
                TextDecoration::new()
                    .strikethrough(StrikethroughStyle::solid(Color::RED, 1.5))
                    .background(Color::from_rgb_u8(60, 30, 30)),
            );
        let mut combined2_buffer = self.font_renderer.prepare(&combined2);

        // All three decorations
        let all_decorations = Text::new("All Decorations")
            .size(24.0)
            .color(Color::WHITE)
            .with_decoration(
                TextDecoration::new()
                    .underline(UnderlineStyle::solid(Color::CYAN, 2.0))
                    .strikethrough(StrikethroughStyle::solid(Color::MAGENTA, 1.5))
                    .background(Color::from_rgb_u8(40, 40, 60)),
            );
        let mut all_decorations_buffer = self.font_renderer.prepare(&all_decorations);

        // === Info text ===
        let info = Text::new(
            "MVP supports solid underlines, strikethrough, and backgrounds.\n\
             Dashed, dotted, and wavy styles coming in future updates.",
        )
        .size(14.0)
        .color(Color::from_rgb_u8(180, 180, 180))
        .max_width(self.window.logical_size_f32().width - 100.0)
        .line_height(1.6);
        let mut info_buffer = self.font_renderer.prepare(&info);

        // === Draw all text ===
        let mut y = 40.0;
        let x = 50.0;
        let line_spacing = 50.0;

        self.font_renderer
            .draw_text(&mut title_buffer, Vec2::new(x, y));
        y += 60.0;

        // Underline section
        self.font_renderer.draw_text_with_decoration(
            &mut underlined_buffer,
            Vec2::new(x, y),
            &underlined_text,
        );
        y += line_spacing;

        self.font_renderer.draw_text_with_decoration(
            &mut thick_underline_buffer,
            Vec2::new(x, y),
            &thick_underline,
        );
        y += line_spacing + 10.0;

        // Strikethrough section
        self.font_renderer.draw_text_with_decoration(
            &mut strikethrough_buffer,
            Vec2::new(x, y),
            &strikethrough_text,
        );
        y += line_spacing;

        self.font_renderer.draw_text_with_decoration(
            &mut custom_strike_buffer,
            Vec2::new(x, y),
            &custom_strike,
        );
        y += line_spacing + 10.0;

        // Background section
        self.font_renderer.draw_text_with_decoration(
            &mut highlighted_buffer,
            Vec2::new(x, y),
            &highlighted_text,
        );
        y += line_spacing;

        self.font_renderer.draw_text_with_decoration(
            &mut dark_highlight_buffer,
            Vec2::new(x, y),
            &dark_highlight,
        );
        y += line_spacing + 10.0;

        // Combined section
        self.font_renderer.draw_text_with_decoration(
            &mut combined1_buffer,
            Vec2::new(x, y),
            &combined1,
        );
        y += line_spacing;

        self.font_renderer.draw_text_with_decoration(
            &mut combined2_buffer,
            Vec2::new(x, y),
            &combined2,
        );
        y += line_spacing;

        self.font_renderer.draw_text_with_decoration(
            &mut all_decorations_buffer,
            Vec2::new(x, y),
            &all_decorations,
        );
        y += line_spacing + 20.0;

        // Info
        self.font_renderer
            .draw_text(&mut info_buffer, Vec2::new(x, y));

        // Begin frame
        let Some(frame) = self.window.begin_frame() else {
            return; // Surface not available (minimized, etc.)
        };

        {
            let mut pass = frame
                .render_pass()
                .clear_color(Color::from_rgb_u8(20, 20, 30))
                .build();

            self.font_renderer.render(pass.wgpu_pass());
        }
        // Frame auto-submits on drop
    }
}
