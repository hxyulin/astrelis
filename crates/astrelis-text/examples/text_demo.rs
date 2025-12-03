use astrelis_core::logging;
use astrelis_render::{
    Color, GraphicsContext, RenderPassBuilder, RenderableWindow, WindowContextDescriptor, wgpu,
};
use astrelis_text::{FontRenderer, FontSystem, FontWeight, Text, TextAlign};
use astrelis_winit::{
    WindowId,
    app::{App, AppCtx, run_app},
    event::EventBatch,
    window::{PhysicalSize, WindowBackend, WindowDescriptor},
};
use glam::Vec2;

struct TextDemo {
    context: &'static GraphicsContext,
    window: RenderableWindow,
    window_id: WindowId,
    font_renderer: FontRenderer,
}

fn main() {
    logging::init();

    run_app(|ctx| {
        let graphics_ctx = GraphicsContext::new_sync();

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Text Rendering Demo".to_string(),
                size: Some(PhysicalSize::new(1024.0, 768.0)),
                ..Default::default()
            })
            .expect("Failed to create window");

        let window = RenderableWindow::new_with_descriptor(
            window,
            graphics_ctx,
            WindowContextDescriptor {
                format: Some(wgpu::TextureFormat::Bgra8UnormSrgb),
                ..Default::default()
            },
        );

        let window_id = window.id();

        // Create font system with system fonts
        let font_system = FontSystem::with_system_fonts();
        let font_renderer = FontRenderer::new(graphics_ctx, font_system);

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

        // Prepare text examples
        let title = Text::new("Astrelis Text Rendering")
            .size(48.0)
            .color(Color::WHITE)
            .weight(FontWeight::Bold);

        let subtitle = Text::new("Powered by cosmic-text")
            .size(24.0)
            .color(Color::from_rgb_u8(150, 150, 255))
            .align(TextAlign::Left);

        let body_text = Text::new(
            "This is a demonstration of text rendering with various styles and properties. \
             The text system supports different sizes, colors, weights, and alignments.",
        )
        .size(16.0)
        .color(Color::from_rgb_u8(200, 200, 200))
        .max_width(700.0)
        .line_height(1.5);

        let bold_text = Text::new("Bold text example")
            .size(20.0)
            .color(Color::YELLOW)
            .bold();

        let italic_text = Text::new("Italic text example")
            .size(20.0)
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
            .size(14.0)
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
                let t = Text::new(*text).size(18.0).color(*color);
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
        y += 80.0;

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

        {
            let mut render_pass = RenderPassBuilder::new()
                .label("Clear Pass")
                .color_attachment(
                    None,
                    None,
                    wgpu::Operations {
                        load: wgpu::LoadOp::Clear(Color::from_rgb_u8(20, 20, 30).to_wgpu()),
                        store: wgpu::StoreOp::Store,
                    },
                )
                .build(&mut frame);

            // Render text
            let size = self.window.window().window.inner_size();
            self.font_renderer.render(
                render_pass.descriptor(),
                Vec2::new(size.width as f32, size.height as f32),
            );
        }

        frame.finish();
    }
}
