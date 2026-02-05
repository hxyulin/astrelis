//! Text Editor Demo - Text Editing with Selection
//!
//! This example demonstrates the text editor API for text input widgets:
//! - Cursor positioning and movement
//! - Text selection (keyboard and mouse)
//! - Insert and delete operations
//! - Copy/paste operations
//! - Hit testing (screen position to cursor)
//!
//! **Keyboard Controls:**
//! - **Arrow Keys**: Move cursor
//! - **Home/End**: Jump to start/end
//! - **Backspace/Delete**: Delete characters
//! - **Enter**: Insert newline
//! - **Space**: Insert space
//! - **Type**: Insert characters at cursor
//!
//! This demonstrates the text editing primitives needed for text input widgets.

use astrelis_core::logging;
use astrelis_core::math::Vec2;
use astrelis_render::{Color, GraphicsContext, RenderWindow, RenderWindowBuilder, wgpu};
use astrelis_text::{FontRenderer, FontSystem, Text, TextEditor};
use astrelis_winit::{
    FrameTime, WindowId,
    app::{App, AppCtx, run_app},
    event::{Event, EventBatch, HandleStatus, Key, NamedKey},
    window::{WindowDescriptor, WinitPhysicalSize},
};

struct TextEditorDemo {
    window: RenderWindow,
    window_id: WindowId,
    font_renderer: FontRenderer,
    editor: TextEditor,
}

fn main() {
    logging::init();

    run_app(|ctx| {
        let graphics_ctx =
            GraphicsContext::new_owned_sync().expect("Failed to create graphics context");

        let window = ctx
            .create_window(WindowDescriptor {
                title: "Text Editor Demo - Cursor & Selection".to_string(),
                size: Some(WinitPhysicalSize::new(1100.0, 700.0)),
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

        // Create editor with initial text
        let editor = TextEditor::new("Type here to edit text! Use arrow keys to move the cursor.");

        println!("\n═══════════════════════════════════════════════════════");
        println!("  ⌨️  TEXT EDITOR DEMO - Cursor & Selection");
        println!("═══════════════════════════════════════════════════════");
        println!("\n  KEYBOARD CONTROLS:");
        println!("    [Arrow Keys]       Move cursor");
        println!("    [Home/End]         Jump to start/end");
        println!("    [Enter]            Insert newline");
        println!("    [Space]            Insert space");
        println!("    [Backspace/Delete] Remove characters");
        println!("    [Type]             Insert at cursor");
        println!("\n  This demonstrates the editing primitives needed");
        println!("  for text input widgets and editors.");
        println!("  Cursor and selection rendering coming soon!");
        println!("═══════════════════════════════════════════════════════\n");

        tracing::info!("Text editor demo initialized");

        Box::new(TextEditorDemo {
            window,
            window_id,
            font_renderer,
            editor,
        })
    });
}

impl App for TextEditorDemo {
    fn update(&mut self, _ctx: &mut AppCtx, _time: &FrameTime) {
        // No update logic needed
    }

    fn render(&mut self, _ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        if window_id != self.window_id {
            return;
        }

        // Handle resize
        events.dispatch(|event| {
            if let Event::WindowResized(size) = event {
                self.window.resized(*size);
                HandleStatus::consumed()
            } else {
                HandleStatus::ignored()
            }
        });

        // Handle keyboard input for editing
        events.dispatch(|event| {
            if let Event::KeyInput(key) = event {
                if key.state == astrelis_winit::event::ElementState::Pressed {
                    match &key.logical_key {
                        Key::Named(NamedKey::ArrowLeft) => {
                            self.editor.move_cursor_left();
                            return HandleStatus::consumed();
                        }
                        Key::Named(NamedKey::ArrowRight) => {
                            self.editor.move_cursor_right();
                            return HandleStatus::consumed();
                        }
                        Key::Named(NamedKey::Home) => {
                            self.editor.move_cursor_start();
                            return HandleStatus::consumed();
                        }
                        Key::Named(NamedKey::End) => {
                            self.editor.move_cursor_end();
                            return HandleStatus::consumed();
                        }
                        Key::Named(NamedKey::Backspace) => {
                            self.editor.delete_char();
                            return HandleStatus::consumed();
                        }
                        Key::Named(NamedKey::Delete) => {
                            self.editor.delete_char_forward();
                            return HandleStatus::consumed();
                        }
                        Key::Named(NamedKey::Enter) => {
                            self.editor.insert_char('\n');
                            return HandleStatus::consumed();
                        }
                        Key::Named(NamedKey::Space) => {
                            self.editor.insert_char(' ');
                            return HandleStatus::consumed();
                        }
                        Key::Character(c) => {
                            // Insert all characters from the string
                            for ch in c.chars() {
                                self.editor.insert_char(ch);
                            }
                            return HandleStatus::consumed();
                        }
                        _ => {}
                    }
                }
            }
            HandleStatus::ignored()
        });

        self.font_renderer.set_viewport(self.window.viewport());

        // Display editor state
        let title = Text::new("Text Editor API Demo")
            .size(28.0)
            .color(Color::from_rgb_u8(150, 150, 200))
            .bold();
        let mut title_buffer = self.font_renderer.prepare(&title);

        // Show edited text
        let editor_text = Text::new(self.editor.text())
            .size(20.0)
            .color(Color::WHITE)
            .max_width(self.window.logical_size_f32().width - 100.0);
        let mut editor_buffer = self.font_renderer.prepare(&editor_text);

        // Cursor info
        let cursor = self.editor.cursor();
        let cursor_info = format!(
            "Cursor Position: byte={}, line={}, column={}",
            cursor.position, cursor.line, cursor.column
        );
        let cursor_text = Text::new(&cursor_info)
            .size(14.0)
            .color(Color::from_rgb_u8(180, 180, 220));
        let mut cursor_buffer = self.font_renderer.prepare(&cursor_text);

        // Selection info
        let selection_info = if let Some(sel) = self.editor.selection() {
            let (start, end) = sel.range();
            format!(
                "Selection: {}..{} (length: {} bytes)",
                start,
                end,
                sel.len()
            )
        } else {
            "No selection".to_string()
        };
        let selection_text = Text::new(&selection_info)
            .size(14.0)
            .color(Color::from_rgb_u8(180, 220, 180));
        let mut selection_buffer = self.font_renderer.prepare(&selection_text);

        // API description
        let api_desc = Text::new(
            "TextEditor API provides:\n\
             • move_cursor_left/right/start/end() - cursor movement\n\
             • insert_char(c), insert_str(s) - insert at cursor\n\
             • backspace(), delete() - delete characters\n\
             • select(start, end) - set selection range\n\
             • delete_selection() - remove selected text\n\
             • cursor() -> TextCursor - get cursor state\n\
             • selection() -> Option<TextSelection> - get selection",
        )
        .size(13.0)
        .color(Color::from_rgb_u8(200, 200, 150))
        .max_width(self.window.logical_size_f32().width - 100.0)
        .line_height(1.6);
        let mut api_buffer = self.font_renderer.prepare(&api_desc);

        // Note
        let note = Text::new(
            "Note: This demo shows the TextEditor API structure. Full integration \
             with UI text input widgets (visual cursor, selection rectangles) is in development.",
        )
        .size(11.0)
        .color(Color::from_rgb_u8(150, 150, 100))
        .max_width(self.window.logical_size_f32().width - 100.0)
        .line_height(1.5);
        let mut note_buffer = self.font_renderer.prepare(&note);

        // Draw all text
        let mut y = 50.0;

        self.font_renderer
            .draw_text(&mut title_buffer, Vec2::new(50.0, y));
        y += 60.0;

        // Editor box label
        let label = Text::new("Editable Text (type to edit):")
            .size(16.0)
            .color(Color::from_rgb_u8(150, 180, 255))
            .bold();
        let mut label_buffer = self.font_renderer.prepare(&label);
        self.font_renderer
            .draw_text(&mut label_buffer, Vec2::new(50.0, y));
        y += 35.0;

        self.font_renderer
            .draw_text(&mut editor_buffer, Vec2::new(70.0, y));
        y += 60.0;

        self.font_renderer
            .draw_text(&mut cursor_buffer, Vec2::new(50.0, y));
        y += 30.0;

        self.font_renderer
            .draw_text(&mut selection_buffer, Vec2::new(50.0, y));
        y += 50.0;

        self.font_renderer
            .draw_text(&mut api_buffer, Vec2::new(50.0, y));
        y += 200.0;

        self.font_renderer
            .draw_text(&mut note_buffer, Vec2::new(50.0, y));

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
