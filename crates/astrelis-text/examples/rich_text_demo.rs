//! Rich Text Demo - Inline Text Formatting
//!
//! This example demonstrates the RichText API for creating text with mixed inline styles:
//! - Bold, italic, and regular text within the same block
//! - Different colors for different spans
//! - Font size variations and scaling
//! - Underline and strike through styles
//! - Background colors for text spans
//! - Markdown-like markup parsing (**bold**, *italic*)
//!
//! Rich text allows you to create sophisticated formatted text without needing
//! multiple separate text objects.

use astrelis_core::logging;
use astrelis_core::math::Vec2;
use astrelis_render::{Color, GraphicsContext, RenderWindow, RenderWindowBuilder, wgpu};
use astrelis_text::{
    FontRenderer, FontSystem, FontWeight, RichText, RichTextBuilder, TextSpanStyle,
};
use astrelis_winit::{
    FrameTime, WindowId,
    app::{App, AppCtx, run_app},
    event::EventBatch,
    window::{WindowDescriptor, WinitPhysicalSize},
};

struct RichTextDemo {
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
                title: "Rich Text Demo - Inline Formatting".to_string(),
                size: Some(WinitPhysicalSize::new(1100.0, 800.0)),
                ..Default::default()
            })
            .expect("Failed to create window");

        let window = RenderWindowBuilder::new()
            .color_format(wgpu::TextureFormat::Bgra8UnormSrgb)
            .build(window, graphics_ctx.clone())
            .expect("Failed to create render window");

        let window_id = window.id();

        // Create font system with system fonts
        let font_system = FontSystem::with_system_fonts();
        let font_renderer = FontRenderer::new(graphics_ctx.clone(), font_system);

        println!("\n═══════════════════════════════════════════════════════");
        println!("  📝 RICH TEXT DEMO - Inline Text Formatting");
        println!("═══════════════════════════════════════════════════════");
        println!("\n  DEMONSTRATED FEATURES:");
        println!("    • Bold, italic, and mixed inline styles");
        println!("    • Multiple colors within the same text");
        println!("    • Font size variations and scaling");
        println!("    • Underline and strikethrough decoration");
        println!("    • Background colors for spans");
        println!("    • Markdown-like markup parsing");
        println!("\n  Rich text enables sophisticated formatting without");
        println!("  managing multiple separate text objects!");
        println!("═══════════════════════════════════════════════════════\n");

        tracing::info!("Rich text demo initialized");

        Box::new(RichTextDemo {
            window,
            window_id,
            font_renderer,
        })
    });
}

impl App for RichTextDemo {
    fn update(&mut self, _ctx: &mut AppCtx, _time: &FrameTime) {
        // No update logic needed for this static demo
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

        // Set the viewport
        self.font_renderer.set_viewport(self.window.viewport());

        let width = self.window.logical_size_f32().width;

        // Example 1: Builder pattern with inline styles
        let mut intro = RichTextBuilder::new()
            .text("Rich Text Formatting: ")
            .bold("Bold")
            .text(", ")
            .italic("Italic")
            .text(", ")
            .colored("Colored", Color::from_rgb_u8(100, 200, 255))
            .text(", and ")
            .span(
                "custom styles",
                TextSpanStyle::default()
                    .with_color(Color::from_rgb_u8(255, 180, 100))
                    .bold()
                    .italic(),
            )
            .text(".")
            .build();

        intro.set_max_width(Some(width - 100.0));

        // Example 2: Manual span building
        let mut manual = RichText::new();
        manual.push_str("You can mix ");
        manual.push_bold("bold");
        manual.push_str(" and ");
        manual.push_italic("italic");
        manual.push_str(" text, add ");
        manual.push_colored("color", Color::YELLOW);
        manual.push_str(", or even ");
        manual.push(
            "scale text",
            TextSpanStyle::default()
                .with_scale(1.5)
                .with_color(Color::from_rgb_u8(150, 255, 150)),
        );
        manual.push_str(" within the same block!");
        manual.set_max_width(Some(width - 100.0));

        // Example 3: Code highlighting style
        let mut code_example = RichTextBuilder::new()
            .text("Code example: ")
            .span(
                "fn main()",
                TextSpanStyle::default()
                    .with_color(Color::from_rgb_u8(255, 150, 100))
                    .with_weight(FontWeight::Bold),
            )
            .text(" ")
            .span(
                "{",
                TextSpanStyle::default().with_color(Color::from_rgb_u8(200, 200, 200)),
            )
            .text("\n    ")
            .span(
                "println!",
                TextSpanStyle::default().with_color(Color::from_rgb_u8(100, 200, 255)),
            )
            .span(
                "(\"Hello, World!\")",
                TextSpanStyle::default().with_color(Color::from_rgb_u8(150, 255, 150)),
            )
            .span(
                ";",
                TextSpanStyle::default().with_color(Color::from_rgb_u8(200, 200, 200)),
            )
            .text("\n")
            .span(
                "}",
                TextSpanStyle::default().with_color(Color::from_rgb_u8(200, 200, 200)),
            )
            .build();

        code_example.set_max_width(Some(width - 100.0));

        // Example 4: Markdown parsing
        let mut markdown = RichText::from_markup(
            "Markdown-like syntax: **bold text**, *italic text*, __underlined__, and ~~strikethrough~~.",
        );
        markdown.set_max_width(Some(width - 100.0));

        // Example 5: Complex formatting with backgrounds
        let mut highlighted = RichTextBuilder::new()
            .text("Text can have ")
            .span(
                " highlighted backgrounds ",
                TextSpanStyle::default()
                    .with_background(Color::from_rgb_u8(80, 80, 100))
                    .with_color(Color::WHITE),
            )
            .text(" or ")
            .span(
                " warning styles ",
                TextSpanStyle::default()
                    .with_background(Color::from_rgb_u8(150, 50, 50))
                    .with_color(Color::WHITE)
                    .with_weight(FontWeight::Bold),
            )
            .text(" inline!")
            .build();

        highlighted.set_max_width(Some(width - 100.0));

        // Example 6: Size variations
        let mut sizes = RichTextBuilder::new()
            .text("Mix different ")
            .span("SIZES", TextSpanStyle::default().with_scale(1.8).bold())
            .text(" in ")
            .span("the same", TextSpanStyle::default().with_scale(0.8))
            .text(" text flow.")
            .build();

        sizes.set_max_width(Some(width - 100.0));

        // Convert all rich text to segments and prepare buffers
        let examples = vec![
            ("Builder Pattern:", intro),
            ("Manual Span Building:", manual),
            ("Code Highlighting:", code_example),
            ("Markdown Parsing:", markdown),
            ("Background Colors:", highlighted),
            ("Size Variations:", sizes),
        ];

        let mut all_buffers = Vec::new();
        for (title, rich_text) in examples {
            // Prepare title
            let title_text = astrelis_text::Text::new(title)
                .size(18.0)
                .color(Color::from_rgb_u8(100, 180, 255))
                .bold();
            let title_buffer = self.font_renderer.prepare(&title_text);
            all_buffers.push((title_buffer, Vec::new()));

            // Prepare rich text segments
            let segments = rich_text.to_text_segments();
            let mut segment_buffers = Vec::new();
            for (text, _style) in segments {
                let buffer = self.font_renderer.prepare(&text);
                segment_buffers.push(buffer);
            }
            all_buffers.last_mut().unwrap().1 = segment_buffers;
        }

        // Draw all text with proper inline layout
        let mut y = 50.0;
        let left_margin = 70.0;
        let max_line_width = width - 120.0; // Leave margin on both sides
        let line_height = 25.0;

        for (mut title_buffer, mut segment_buffers) in all_buffers {
            // Draw title
            self.font_renderer
                .draw_text(&mut title_buffer, Vec2::new(50.0, y));
            y += 35.0;

            // Draw segments with proper inline layout
            let mut x = left_margin;
            for buffer in &mut segment_buffers {
                // Get logical (unscaled) bounds for layout
                let (seg_width, _seg_height) = self.font_renderer.buffer_bounds(buffer);

                // Check if we need to wrap to next line
                if x > left_margin && x + seg_width > left_margin + max_line_width {
                    // Move to next line
                    x = left_margin;
                    y += line_height;
                }

                // Draw the segment at current position
                self.font_renderer.draw_text(buffer, Vec2::new(x, y));

                // Advance x position for next segment
                x += seg_width;
            }

            // Move to next line after the rich text block
            y += line_height + 20.0; // Line height plus extra spacing
        }

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
