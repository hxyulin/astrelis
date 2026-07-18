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

#[test]
fn facade_builds_an_identical_tree_to_hand_written_core() {
    // The facade is a construction-time convenience: it must emit the exact
    // same retained tree as the fallible core calls, so the per-frame render
    // path is untouched and adds no allocation over hand-written code.
    fn with_facade() -> Ui {
        let mut ui = ui();
        let root = ui.root();
        let column = ui
            .padding(root, Insets::all(28.0))
            .grow(1.0)
            .scroll_view()
            .grow(1.0)
            .column()
            .finish();
        ui.label(column, "Title").finish();
        let row = ui.row(column).finish();
        ui.button(row, "One").width(px(120.0)).finish();
        ui.button(row, "Two").width(px(120.0)).finish();
        ui
    }

    fn with_core() -> Ui {
        let mut ui = ui();
        let root = ui.root();
        let grow = LayoutStyle {
            grow: 1.0,
            ..Default::default()
        };
        let padding = ui.add_padding(root, Insets::all(28.0)).unwrap();
        ui.set_layout(padding, grow).unwrap();
        let scroll = ui.add_scroll_view(padding).unwrap();
        ui.set_layout(scroll, grow).unwrap();
        let column = ui.add_column(scroll).unwrap();
        ui.add_label(column, "Title").unwrap();
        let row = ui.add_row(column).unwrap();
        let one = ui.add_button(row, "One").unwrap();
        ui.set_layout(
            one,
            LayoutStyle {
                width: Length::Px(120.0),
                ..Default::default()
            },
        )
        .unwrap();
        let two = ui.add_button(row, "Two").unwrap();
        ui.set_layout(
            two,
            LayoutStyle {
                width: Length::Px(120.0),
                ..Default::default()
            },
        )
        .unwrap();
        ui
    }

    let facade_nodes = with_facade().inspect().unwrap().nodes;
    let core_nodes = with_core().inspect().unwrap().nodes;
    assert_eq!(facade_nodes.len(), core_nodes.len());
    for (a, b) in facade_nodes.iter().zip(&core_nodes) {
        assert_eq!(a.layout_bounds, b.layout_bounds);
    }
}

#[test]
fn builder_config_methods_match_hand_written_core() {
    // The overflow/z-index/visibility/transform config methods must produce the
    // same retained state as the core setters they wrap, so a facade port
    // renders identically to the original.
    fn with_facade() -> Ui {
        let mut ui = ui();
        let root = ui.root();
        let clip = ui
            .stack(root)
            .width(px(400.0))
            .height(px(200.0))
            .overflow(Overflow::Clip)
            .finish();
        ui.button(clip, "A")
            .layout(
                layout()
                    .width(px(120.0))
                    .height(px(40.0))
                    .positioning(Positioning::Absolute)
                    .inset(Edges {
                        left: px(10.0),
                        top: px(10.0),
                        ..Default::default()
                    }),
            )
            .z_index(3)
            .transform(Affine2::from_angle(-0.05), Point::new(20.0, 10.0))
            .finish();
        ui.button(clip, "B")
            .layout(
                layout()
                    .width(px(120.0))
                    .height(px(40.0))
                    .positioning(Positioning::Absolute)
                    .inset(Edges {
                        left: px(40.0),
                        top: px(20.0),
                        ..Default::default()
                    }),
            )
            .z_index(1)
            .visibility(Visibility::Hidden)
            .finish();
        ui
    }

    fn with_core() -> Ui {
        let mut ui = ui();
        let root = ui.root();
        let clip = ui.add_stack(root).unwrap();
        ui.set_layout(
            clip,
            LayoutStyle {
                width: Length::Px(400.0),
                height: Length::Px(200.0),
                ..Default::default()
            },
        )
        .unwrap();
        ui.set_overflow(clip, Overflow::Clip).unwrap();
        let a = ui.add_button(clip, "A").unwrap();
        ui.set_layout(
            a,
            LayoutStyle {
                width: Length::Px(120.0),
                height: Length::Px(40.0),
                positioning: Positioning::Absolute,
                inset: Edges {
                    left: Length::Px(10.0),
                    top: Length::Px(10.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .unwrap();
        ui.set_z_index(a, 3).unwrap();
        ui.set_transform(a, Affine2::from_angle(-0.05), Point::new(20.0, 10.0))
            .unwrap();
        let b = ui.add_button(clip, "B").unwrap();
        ui.set_layout(
            b,
            LayoutStyle {
                width: Length::Px(120.0),
                height: Length::Px(40.0),
                positioning: Positioning::Absolute,
                inset: Edges {
                    left: Length::Px(40.0),
                    top: Length::Px(20.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .unwrap();
        ui.set_z_index(b, 1).unwrap();
        ui.set_visibility(b, Visibility::Hidden).unwrap();
        ui
    }

    let facade_nodes = with_facade().inspect().unwrap().nodes;
    let core_nodes = with_core().inspect().unwrap().nodes;
    assert_eq!(facade_nodes.len(), core_nodes.len());
    for (a, b) in facade_nodes.iter().zip(&core_nodes) {
        assert_eq!(a.world_bounds, b.world_bounds);
        assert_eq!(a.world_transform, b.world_transform);
        assert_eq!(a.visibility, b.visibility);
        assert_eq!(a.clip, b.clip);
        assert_eq!(a.paint_rank, b.paint_rank);
    }
}
