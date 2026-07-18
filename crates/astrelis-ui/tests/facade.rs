//! Integration tests for the ergonomic facade.

use astrelis_text::FontDatabase;
use astrelis_ui::prelude::*;

fn ui() -> Ui {
    let mut ui = Ui::new(FontDatabase::default(), Theme::default());
    ui.set_viewport(LogicalSize::new(640.0, 480.0), 1.0);
    ui
}

#[test]
fn builder_commits_layout_in_one_chain() {
    let mut ui = ui();
    let root = ui.root();
    let box_handle = ui.column(root).width(px(200.0)).height(px(80.0)).finish();

    let bounds = ui.layout_bounds(box_handle).unwrap();
    assert!((bounds.size.width - 200.0).abs() < 0.5);
    assert!((bounds.size.height - 80.0).abs() < 0.5);
}

#[test]
fn descend_chain_nests_children_under_their_parents() {
    let mut ui = ui();
    let root = ui.root();
    // padding -> scroll view -> column, configured and committed in one chain.
    let column = ui
        .padding(root, Insets::all(20.0))
        .grow(1.0)
        .scroll_view()
        .grow(1.0)
        .column()
        .finish();
    let label = ui.label(column, "Nested").finish();

    // The label sits inside the 20px padding the outer chain established.
    let bounds = ui.layout_bounds(label).unwrap();
    assert!(
        bounds.origin.x >= 20.0,
        "descend chain should have applied the padding, x = {}",
        bounds.origin.x
    );
    assert!(bounds.origin.y >= 20.0);
}

#[test]
fn fluent_layout_matches_the_struct_literal() {
    let built = layout().grow(1.0).width(px(120.0)).min_height(px(30.0));
    let literal = LayoutStyle {
        grow: 1.0,
        width: Length::Px(120.0),
        min_height: Length::Px(30.0),
        ..Default::default()
    };
    assert_eq!(built, literal);
}

#[test]
fn intent_listeners_register_without_panicking() {
    #[derive(Debug, PartialEq)]
    enum Message {
        Toggled(bool),
        Slid(f32),
        Clicked,
    }

    let mut ui = Ui::<Message>::new(FontDatabase::default(), Theme::default());
    let root = ui.root();
    let button = ui.button(root, "Go").finish();
    let checkbox = ui.checkbox(root, false).finish();
    let slider = ui.slider(root, 0.0, 1.0, 0.1, 0.5).finish();

    ui.on_click(button, |ctx| ctx.emit(Message::Clicked));
    ui.on_checked(checkbox, |ctx, value| ctx.emit(Message::Toggled(value)));
    ui.on_slider(slider, |ctx, value| ctx.emit(Message::Slid(value)));

    // Building and registering must leave the tree renderable.
    ui.display_list().unwrap();
}
