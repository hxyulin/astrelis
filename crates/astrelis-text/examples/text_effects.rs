//! Text Effects API Demo - Shadows, Outlines, and Glows
//!
//! **⚠️  IMPORTANT**: This demo shows the TextEffect API structure ONLY.
//! The text displayed here does NOT visually show the effects because
//! GPU-accelerated effect rendering requires SDF (Signed Distance Field)
//! text rendering, which is currently in development.
//!
//! TextEffect API demonstrates:
//! - Drop shadows (hard and blurred)
//! - Outlines for readability
//! - Glow effects for highlights
//! - Inner shadows for depth
//! - Combining multiple effects
//!
//! This is an API reference example - visual rendering coming soon!

use std::sync::Arc;
use astrelis_core::logging;
use astrelis_core::math::Vec2;
use astrelis_render::{
    Color, GraphicsContext, RenderTarget, RenderableWindow,
    WindowContextDescriptor, wgpu,
};
use astrelis_text::{FontRenderer, FontSystem, Text, TextEffect, TextEffects};
use astrelis_winit::{
    WindowId,
    app::{App, AppCtx, run_app},
    event::EventBatch,
    window::{PhysicalSize, WindowBackend, WindowDescriptor},
};

struct TextEffectsDemo {
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
                title: "Text Effects Demo - Shadows, Outlines, Glows".to_string(),
                size: Some(PhysicalSize::new(1100.0, 800.0)),
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
        println!("  ⚠️  TEXT EFFECTS API DEMO (API Reference Only)");
        println!("═══════════════════════════════════════════════════════");
        println!("\n  ⚠️  IMPORTANT: Effects are NOT visually rendered!");
        println!("  This demo shows the TextEffect API structure only.");
        println!("  Visual rendering requires SDF text (in development).");
        println!("\n  DEMONSTRATED API:");
        println!("    • TextEffect::shadow() - Drop shadows");
        println!("    • TextEffect::shadow_blurred() - Soft shadows");
        println!("    • TextEffect::outline() - Text outlines");
        println!("    • TextEffect::glow() - Glow effects");
        println!("    • TextEffect::inner_shadow() - Inner shadows");
        println!("    • TextEffects::add() - Stack multiple effects");
        println!("\n  The text you see has NO visual effects applied.");
        println!("  This is purely an API structure demonstration.");
        println!("═══════════════════════════════════════════════════════\n");

        tracing::info!("Text effects demo initialized");

        Box::new(TextEffectsDemo {
            _context: graphics_ctx,
            window,
            window_id,
            font_renderer,
        })
    });
}

impl App for TextEffectsDemo {
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

        // Example 1: Drop shadow effect
        let _shadow_effect = TextEffect::shadow(
            Vec2::new(2.0, 2.0),
            Color::from_rgba_u8(0, 0, 0, 180)
        );

        let shadow_text = Text::new("Drop Shadow Text")
            .size(32.0)
            .color(Color::WHITE)
            .bold();

        // Example 2: Blurred shadow
        let _blurred_shadow = TextEffect::shadow_blurred(
            Vec2::new(3.0, 3.0),
            5.0,
            Color::from_rgba_u8(0, 0, 0, 150)
        );

        let blurred_text = Text::new("Blurred Shadow")
            .size(32.0)
            .color(Color::from_rgb_u8(100, 200, 255))
            .bold();

        // Example 3: Outline effect
        let _outline_effect = TextEffect::outline(
            2.0,
            Color::BLACK
        );

        let outline_text = Text::new("Outlined Text")
            .size(32.0)
            .color(Color::YELLOW)
            .bold();

        // Example 4: Glow effect
        let _glow_effect = TextEffect::glow(
            8.0,
            Color::from_rgb_u8(0, 150, 255),
            0.8
        );

        let glow_text = Text::new("Glowing Text")
            .size(32.0)
            .color(Color::from_rgb_u8(150, 220, 255))
            .bold();

        // Example 5: Inner shadow
        let _inner_shadow = TextEffect::inner_shadow(
            Vec2::new(0.0, 2.0),
            3.0,
            Color::from_rgba_u8(0, 0, 0, 100)
        );

        let inner_text = Text::new("Inner Shadow")
            .size(32.0)
            .color(Color::from_rgb_u8(200, 200, 200))
            .bold();

        // Example 6: Multiple effects combined
        let mut _combined_effects = TextEffects::new();
        _combined_effects.add(TextEffect::shadow(
            Vec2::new(3.0, 3.0),
            Color::from_rgba_u8(0, 0, 0, 200)
        ));
        _combined_effects.add(TextEffect::outline(
            2.0,
            Color::BLACK
        ));
        _combined_effects.add(TextEffect::glow(
            6.0,
            Color::from_rgb_u8(255, 200, 0),
            0.6
        ));

        let combined_text = Text::new("Combined Effects!")
            .size(36.0)
            .color(Color::YELLOW)
            .bold();

        // Prepare all text buffers
        let mut shadow_buffer = self.font_renderer.prepare(&shadow_text);
        let mut blurred_buffer = self.font_renderer.prepare(&blurred_text);
        let mut outline_buffer = self.font_renderer.prepare(&outline_text);
        let mut glow_buffer = self.font_renderer.prepare(&glow_text);
        let mut inner_buffer = self.font_renderer.prepare(&inner_text);
        let mut combined_buffer = self.font_renderer.prepare(&combined_text);

        // Info text
        let info_text = Text::new("Text Effect Types:")
            .size(20.0)
            .color(Color::from_rgb_u8(150, 150, 200))
            .bold();
        let mut info_buffer = self.font_renderer.prepare(&info_text);

        // Description texts
        let descriptions = [
            ("Shadow: offset=(2,2), hard edge", Color::from_rgb_u8(180, 180, 180)),
            ("Shadow: offset=(3,3), blur=5px", Color::from_rgb_u8(180, 180, 180)),
            ("Outline: width=2px, black", Color::from_rgb_u8(180, 180, 180)),
            ("Glow: radius=8px, intensity=0.8", Color::from_rgb_u8(180, 180, 180)),
            ("Inner Shadow: offset=(0,2), blur=3px", Color::from_rgb_u8(180, 180, 180)),
            ("Multiple: shadow + outline + glow", Color::from_rgb_u8(180, 180, 180)),
        ];

        let mut desc_buffers: Vec<_> = descriptions
            .iter()
            .map(|(text, color)| {
                let t = Text::new(*text).size(12.0).color(*color);
                self.font_renderer.prepare(&t)
            })
            .collect();

        // Draw all text
        let mut y = 50.0;

        self.font_renderer.draw_text(&mut info_buffer, Vec2::new(50.0, y));
        y += 50.0;

        // Draw effects with descriptions
        let mut text_buffers = vec![
            shadow_buffer,
            blurred_buffer,
            outline_buffer,
            glow_buffer,
            inner_buffer,
            combined_buffer,
        ];

        for i in 0..text_buffers.len() {
            self.font_renderer.draw_text(&mut text_buffers[i], Vec2::new(50.0, y));
            y += 45.0;
            self.font_renderer.draw_text(&mut desc_buffers[i], Vec2::new(70.0, y));
            y += 35.0;
        }

        // Render note
        let note = Text::new(
            "Note: Full effect rendering requires SDF text (in development). \
             This demo showcases the effects API structure."
        )
        .size(11.0)
        .color(Color::from_rgb_u8(150, 150, 100))
        .max_width(self.window.size_f32().width - 100.0);

        let mut note_buffer = self.font_renderer.prepare(&note);
        self.font_renderer.draw_text(&mut note_buffer, Vec2::new(50.0, y + 30.0));

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
