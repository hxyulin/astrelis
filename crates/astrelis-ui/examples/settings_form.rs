//! Headless demonstration of the `astrelis-ui` facade.
//!
//! Builds a settings-style screen entirely through the facade — infallible
//! chained builders, fluent layout, intent-named listeners, and the
//! `widget_any!` macro — then lays it out and prints a summary. Run with:
//!
//! ```text
//! cargo run -p astrelis-ui --example settings_form
//! ```
//!
//! Contrast the construction here with `facade_settings_window.rs` in
//! `astrelis-ui-core`, where the equivalent tree threads a `map_err` through
//! every call and configures each node in a separate struct-literal statement.

use astrelis_core::color::Color;
use astrelis_core::geometry::LogicalRect;
use astrelis_paint::{Brush, Painter};
use astrelis_text::FontDatabase;
use astrelis_ui::prelude::*;
use astrelis_ui_core::SemanticRole;

/// Application messages the listeners emit.
#[derive(Debug)]
enum Message {
    ReduceMotion(bool),
    Volume(f32),
    Name(String),
    Save,
}

/// A minimal custom widget: a colored status dot. `widget_any!` supplies the
/// identity casts so only the painting behaviour is written by hand.
struct StatusDot {
    color: Color,
}

impl Widget<Message> for StatusDot {
    widget_any!();

    fn intrinsic_size(&self, _theme: &Theme) -> LogicalSize {
        LogicalSize::new(14.0, 14.0)
    }

    fn paint(
        &self,
        painter: &mut Painter,
        bounds: LogicalRect,
        _theme: &Theme,
    ) -> Result<(), UiError> {
        painter.fill_ellipse(bounds, Brush::Solid(self.color))?;
        Ok(())
    }

    fn semantics(&self) -> Option<(SemanticRole, String, Option<String>)> {
        Some((SemanticRole::Group, "status".into(), None))
    }
}

fn main() {
    let mut ui = Ui::<Message>::new(FontDatabase::default(), Theme::default());
    ui.set_viewport(LogicalSize::new(480.0, 360.0), 1.0);
    let root = ui.root();

    // padding -> scroll view -> content column, in one chain.
    let content = ui
        .padding(root, Insets::all(24.0))
        .grow(1.0)
        .scroll_view()
        .grow(1.0)
        .column()
        .finish();

    ui.label(content, "Settings")
        .style(WidgetStyle {
            font_size: Some(24.0),
            ..Default::default()
        })
        .finish();

    // A labelled toggle row.
    let motion_row = ui.row(content).finish();
    ui.mount(
        motion_row,
        StatusDot {
            color: Color::new(0.32, 0.78, 0.48, 1.0),
        },
    )
    .finish();
    ui.label(motion_row, "Reduce motion").grow(1.0).finish();
    let reduce_motion = ui.checkbox(motion_row, false).finish();
    ui.on_checked(reduce_motion, |ctx, on| ctx.emit(Message::ReduceMotion(on)));

    // A labelled slider row.
    let volume_row = ui.row(content).finish();
    ui.label(volume_row, "Volume").grow(1.0).finish();
    let volume = ui
        .slider(volume_row, 0.0, 1.0, 0.05, 0.6)
        .width(px(200.0))
        .finish();
    ui.on_slider(volume, |ctx, value| ctx.emit(Message::Volume(value)));

    // A labelled text field row.
    let name_row = ui.row(content).finish();
    ui.label(name_row, "Display name").grow(1.0).finish();
    let name = ui.text_field(name_row, "Ada").width(px(200.0)).finish();
    ui.on_text_changed(name, |ctx, text| ctx.emit(Message::Name(text.to_owned())));

    // A right-aligned save button.
    let actions = ui
        .row(content)
        .flex(FlexStyle {
            justify_content: Justification::End,
            ..Default::default()
        })
        .finish();
    let save = ui.button(actions, "Save").min_width(px(120.0)).finish();
    ui.on_click(save, |ctx| ctx.emit(Message::Save));

    let inspection = ui.inspect().expect("layout");
    println!(
        "Built a settings form: {} elements laid out in a {}x{} viewport.",
        inspection.nodes.len(),
        inspection.viewport.width as u32,
        inspection.viewport.height as u32,
    );
    let save_bounds = ui.layout_bounds(save).expect("save bounds");
    println!(
        "Save button rests at x={:.0}, y={:.0} ({:.0}x{:.0}).",
        save_bounds.origin.x, save_bounds.origin.y, save_bounds.size.width, save_bounds.size.height,
    );

    // Under a real event loop the listeners above emit these as the user
    // interacts. Headless, the queue is empty, but draining it shows the wiring.
    for message in ui.drain_messages() {
        match message {
            Message::ReduceMotion(on) => println!("reduce motion -> {on}"),
            Message::Volume(value) => println!("volume -> {value:.2}"),
            Message::Name(name) => println!("name -> {name}"),
            Message::Save => println!("save"),
        }
    }
}
