//! Incremental retained runtime behavior.

use astrelis_core::{
    color::Color,
    geometry::{LogicalPoint, LogicalSize},
};
use astrelis_platform::{
    CursorIcon, DeviceId, ElementState, Key, KeyLocation, KeyboardInput, Modifiers, NamedKey,
    PhysicalKey,
};
use astrelis_ui_next::{
    Align, Alignment, Axis, BoxElement, Button, Checkbox, Flex, Frame, Invalidation, KeyListener,
    Label, Scroll, ScrollAxis, SemanticAction, SemanticData, SemanticRole, SplitPane, Stack,
    TextField, UiInput, UiRoot,
};

#[test]
fn paint_only_update_rebuilds_one_fragment_and_skips_layout() {
    let mut ui = UiRoot::new(Flex::default(), LogicalSize::new(400.0, 300.0));
    let child = ui
        .append(
            ui.root(),
            BoxElement::new(LogicalSize::new(40.0, 20.0), Color::WHITE),
        )
        .unwrap();
    ui.update_passes().unwrap();

    ui.update(child, Invalidation::PAINT, |child| {
        child.color = Color::BLACK
    })
    .unwrap();
    let update = ui.update_passes().unwrap();
    assert_eq!(update.stats.layout_elements, 0);
    assert_eq!(update.stats.rebuilt_fragments, 1);
    assert!(update.scene.rebuilt(child.id()));
}

#[test]
fn unchanged_update_does_no_retained_work() {
    let mut ui = UiRoot::new(Flex::default(), LogicalSize::new(400.0, 300.0));
    ui.append(ui.root(), Label::new("unchanged")).unwrap();
    ui.update_passes().unwrap();
    let update = ui.update_passes().unwrap();
    assert_eq!(update.stats.layout_elements, 0);
    assert_eq!(update.stats.rebuilt_fragments, 0);
    assert_eq!(update.accessibility.changed.len(), 0);
}

#[test]
fn keyed_reorder_primitive_preserves_identity_and_cached_fragments() {
    let mut ui = UiRoot::new(
        Flex {
            axis: Axis::Horizontal,
            ..Flex::default()
        },
        LogicalSize::new(400.0, 300.0),
    );
    let root = ui.root();
    let a = ui
        .append(
            root,
            BoxElement::new(LogicalSize::new(20.0, 20.0), Color::WHITE),
        )
        .unwrap();
    let b = ui
        .append(
            root,
            BoxElement::new(LogicalSize::new(20.0, 20.0), Color::BLACK),
        )
        .unwrap();
    ui.update_passes().unwrap();
    ui.set_children(root, &[b.id(), a.id()]).unwrap();
    let stats = ui.update_passes().unwrap().stats;
    assert!(ui.contains(a.id()) && ui.contains(b.id()));
    assert_eq!(
        stats.rebuilt_fragments, 1,
        "only the changed parent repaints"
    );
}

#[test]
fn stack_overlays_children_and_targets_the_topmost_control() {
    let mut ui = UiRoot::new(Stack::default(), LogicalSize::new(200.0, 100.0));
    let first = ui
        .append(
            ui.root(),
            Button::new(
                "First",
                LogicalSize::new(100.0, 30.0),
                Color::WHITE,
                Color::BLACK,
                Action::Activate,
            ),
        )
        .unwrap();
    let second = ui
        .append(
            ui.root(),
            Button::new(
                "Second",
                LogicalSize::new(100.0, 30.0),
                Color::BLACK,
                Color::WHITE,
                Action::Activate,
            ),
        )
        .unwrap();
    ui.update_passes().unwrap();

    assert_eq!(ui.hit_test(LogicalPoint::new(5.0, 5.0)), Some(second.id()));
    assert!(ui.contains(first.id()));
}

#[test]
fn wheel_input_bubbles_to_a_clipped_scroll_ancestor() {
    let mut ui = UiRoot::new(
        Scroll::new(ScrollAxis::Vertical),
        LogicalSize::new(200.0, 50.0),
    );
    let content = ui
        .append(
            ui.root(),
            Flex {
                axis: Axis::Vertical,
                gap: 4.0,
                ..Flex::default()
            },
        )
        .unwrap();
    for index in 0..8 {
        ui.append(content.id(), Label::new(format!("Row {index}")))
            .unwrap();
    }
    ui.update_passes().unwrap();
    let before = ui
        .semantic_snapshot()
        .into_iter()
        .find(|node| node.data.label == "Row 0")
        .unwrap()
        .bounds
        .origin
        .y;

    ui.dispatch(UiInput::PointerWheel {
        position: LogicalPoint::new(10.0, 10.0),
        delta: LogicalPoint::new(0.0, 30.0),
    })
    .unwrap();
    ui.update_passes().unwrap();
    let after = ui
        .semantic_snapshot()
        .into_iter()
        .find(|node| node.data.label == "Row 0")
        .unwrap()
        .bounds
        .origin
        .y;
    assert_eq!(after, before - 30.0);
}

#[derive(Clone, Debug, PartialEq)]
enum Action {
    Activate,
    Edit(String),
    Checked(bool),
}

fn key(text: &str) -> KeyboardInput {
    KeyboardInput {
        device_id: DeviceId(1),
        physical_key: PhysicalKey::Unidentified,
        logical_key: Key::Character(text.into()),
        text: Some(text.into()),
        location: KeyLocation::Standard,
        state: ElementState::Pressed,
        repeat: false,
        synthetic: false,
    }
}

fn named_key(key: NamedKey) -> KeyboardInput {
    KeyboardInput {
        device_id: DeviceId(1),
        physical_key: PhysicalKey::Unidentified,
        logical_key: Key::Named(key),
        text: None,
        location: KeyLocation::Standard,
        state: ElementState::Pressed,
        repeat: false,
        synthetic: false,
    }
}

#[test]
fn hit_testing_prunes_subtrees_and_dispatches_typed_actions() {
    let mut ui = UiRoot::new(Flex::default(), LogicalSize::new(400.0, 300.0));
    let button = ui
        .append(
            ui.root(),
            Button::new(
                "Run",
                LogicalSize::new(100.0, 30.0),
                Color::WHITE,
                Color::BLACK,
                Action::Activate,
            ),
        )
        .unwrap();
    ui.update_passes().unwrap();
    let point = LogicalPoint::new(10.0, 10.0);
    assert_eq!(ui.hit_test(point), Some(button.id()));
    ui.dispatch(UiInput::PointerPressed(point)).unwrap();
    let action = ui
        .dispatch(UiInput::PointerReleased(point))
        .unwrap()
        .unwrap();
    assert_eq!(*action.downcast::<Action>().unwrap(), Action::Activate);
}

#[test]
fn flex_growth_allocates_exact_bounded_shares() {
    let mut ui = UiRoot::new(
        Flex {
            axis: Axis::Horizontal,
            ..Flex::default()
        },
        LogicalSize::new(300.0, 100.0),
    );
    let first = ui
        .append(
            ui.root(),
            Frame {
                grow: 1.0,
                ..Frame::default()
            },
        )
        .unwrap();
    ui.append(first.id(), Label::new("First")).unwrap();
    let second = ui
        .append(
            ui.root(),
            Frame {
                grow: 2.0,
                ..Frame::default()
            },
        )
        .unwrap();
    ui.append(second.id(), Label::new("Second")).unwrap();
    ui.update_passes().unwrap();

    let semantics = ui.semantic_snapshot();
    let first = semantics
        .iter()
        .find(|node| node.data.label == "First")
        .unwrap();
    let second = semantics
        .iter()
        .find(|node| node.data.label == "Second")
        .unwrap();
    assert_eq!(first.bounds.size.width, 100.0);
    assert_eq!(second.bounds.origin.x, 100.0);
    assert_eq!(second.bounds.size.width, 200.0);
}

#[test]
fn alignment_centers_intrinsic_content_in_the_viewport() {
    let mut ui = UiRoot::new(
        Align {
            alignment: Alignment::Center,
            padding: 0.0,
        },
        LogicalSize::new(200.0, 100.0),
    );
    ui.append(ui.root(), Label::new("Centered")).unwrap();
    ui.update_passes().unwrap();
    let label = ui
        .semantic_snapshot()
        .into_iter()
        .find(|node| node.data.label == "Centered")
        .unwrap();
    assert!((label.bounds.origin.x + label.bounds.size.width * 0.5 - 100.0).abs() < 0.01);
    assert!((label.bounds.origin.y + label.bounds.size.height * 0.5 - 50.0).abs() < 0.01);
}

#[test]
fn pointer_capture_finishes_a_press_released_outside() {
    let mut ui = UiRoot::new(Flex::default(), LogicalSize::new(200.0, 100.0));
    ui.append(
        ui.root(),
        Button::new(
            "Run",
            LogicalSize::new(100.0, 30.0),
            Color::WHITE,
            Color::BLACK,
            Action::Activate,
        ),
    )
    .unwrap();
    ui.update_passes().unwrap();

    ui.dispatch(UiInput::PointerPressed(LogicalPoint::new(10.0, 10.0)))
        .unwrap();
    assert!(
        ui.dispatch(UiInput::PointerReleased(LogicalPoint::new(180.0, 80.0)))
            .unwrap()
            .is_none()
    );
    ui.dispatch(UiInput::PointerPressed(LogicalPoint::new(10.0, 10.0)))
        .unwrap();
    assert!(
        ui.dispatch(UiInput::PointerReleased(LogicalPoint::new(10.0, 10.0)))
            .unwrap()
            .is_some()
    );
}

#[test]
fn hover_transitions_repaint_only_the_entered_and_exited_controls() {
    let mut ui = UiRoot::new(
        Flex {
            axis: Axis::Horizontal,
            ..Flex::default()
        },
        LogicalSize::new(240.0, 60.0),
    );
    ui.append(
        ui.root(),
        Button::new(
            "First",
            LogicalSize::new(100.0, 30.0),
            Color::WHITE,
            Color::BLACK,
            Action::Activate,
        ),
    )
    .unwrap();
    ui.append(
        ui.root(),
        Button::new(
            "Second",
            LogicalSize::new(100.0, 30.0),
            Color::WHITE,
            Color::BLACK,
            Action::Activate,
        ),
    )
    .unwrap();
    ui.update_passes().unwrap();

    ui.dispatch(UiInput::PointerMoved(LogicalPoint::new(10.0, 10.0)))
        .unwrap();
    assert_eq!(ui.update_passes().unwrap().stats.rebuilt_fragments, 1);

    ui.dispatch(UiInput::PointerMoved(LogicalPoint::new(20.0, 10.0)))
        .unwrap();
    assert_eq!(ui.update_passes().unwrap().stats.rebuilt_fragments, 0);

    ui.dispatch(UiInput::PointerMoved(LogicalPoint::new(110.0, 10.0)))
        .unwrap();
    assert_eq!(ui.update_passes().unwrap().stats.rebuilt_fragments, 2);

    ui.dispatch(UiInput::PointerLeft).unwrap();
    assert_eq!(ui.update_passes().unwrap().stats.rebuilt_fragments, 1);
}

#[test]
fn hovered_and_captured_elements_select_native_cursors() {
    let mut ui = UiRoot::new(Flex::default(), LogicalSize::new(200.0, 100.0));
    ui.append(
        ui.root(),
        Button::new(
            "Run",
            LogicalSize::new(100.0, 30.0),
            Color::WHITE,
            Color::BLACK,
            Action::Activate,
        ),
    )
    .unwrap();
    ui.update_passes().unwrap();
    assert_eq!(ui.cursor_icon(), CursorIcon::Default);
    ui.dispatch(UiInput::PointerMoved(LogicalPoint::new(10.0, 10.0)))
        .unwrap();
    assert_eq!(ui.cursor_icon(), CursorIcon::Pointer);
    ui.dispatch(UiInput::PointerLeft).unwrap();
    assert_eq!(ui.cursor_icon(), CursorIcon::Default);

    let mut split = UiRoot::new(
        SplitPane::new(Axis::Horizontal, 0.5, |ratio| Box::new(ratio)),
        LogicalSize::new(200.0, 100.0),
    );
    split.update_passes().unwrap();
    split
        .dispatch(UiInput::PointerPressed(LogicalPoint::new(100.0, 50.0)))
        .unwrap();
    assert_eq!(split.cursor_icon(), CursorIcon::EwResize);
    split
        .dispatch(UiInput::PointerMoved(LogicalPoint::new(20.0, 50.0)))
        .unwrap();
    assert_eq!(split.cursor_icon(), CursorIcon::EwResize);
}

#[test]
fn keyboard_events_bubble_to_overlay_boundaries() {
    let mut ui = UiRoot::new(
        KeyListener::on_escape(|| Box::new(Action::Activate)),
        LogicalSize::new(200.0, 100.0),
    );
    ui.append(
        ui.root(),
        Button::new(
            "Focused",
            LogicalSize::new(100.0, 30.0),
            Color::WHITE,
            Color::BLACK,
            Action::Activate,
        ),
    )
    .unwrap();
    ui.update_passes().unwrap();
    ui.focus_first_in_subtree(ui.root()).unwrap();

    let action = ui
        .dispatch(UiInput::Keyboard {
            input: named_key(NamedKey::Escape),
            modifiers: Modifiers::default(),
        })
        .unwrap()
        .unwrap();
    assert_eq!(*action.downcast::<Action>().unwrap(), Action::Activate);
}

#[test]
fn split_pane_routes_drag_outside_the_divider_and_reflows_children() {
    let mut ui = UiRoot::new(
        SplitPane::new(Axis::Horizontal, 0.25, |ratio| Box::new(ratio)),
        LogicalSize::new(200.0, 100.0),
    );
    ui.append(ui.root(), Label::new("First")).unwrap();
    ui.append(ui.root(), Label::new("Second")).unwrap();
    ui.update_passes().unwrap();

    ui.dispatch(UiInput::PointerPressed(LogicalPoint::new(50.0, 50.0)))
        .unwrap();
    let ratio = ui
        .dispatch(UiInput::PointerMoved(LogicalPoint::new(100.0, 50.0)))
        .unwrap()
        .unwrap();
    assert!((*ratio.downcast::<f32>().unwrap() - 0.5).abs() < 0.02);
    ui.dispatch(UiInput::PointerReleased(LogicalPoint::new(150.0, 50.0)))
        .unwrap();
    ui.update_passes().unwrap();

    let second = ui
        .semantic_snapshot()
        .into_iter()
        .find(|node| node.data.label == "Second")
        .unwrap();
    assert!(second.bounds.origin.x > 145.0);
}

#[test]
fn clicking_a_splitter_does_not_move_it() {
    let mut ui = UiRoot::new(
        SplitPane::new(Axis::Horizontal, 0.25, |ratio| Box::new(ratio)),
        LogicalSize::new(200.0, 100.0),
    );
    ui.append(ui.root(), Label::new("First")).unwrap();
    ui.append(ui.root(), Label::new("Second")).unwrap();
    ui.update_passes().unwrap();

    let point = LogicalPoint::new(50.0, 50.0);
    assert!(
        ui.dispatch(UiInput::PointerPressed(point))
            .unwrap()
            .is_none()
    );
    let ratio = ui
        .dispatch(UiInput::PointerReleased(point))
        .unwrap()
        .unwrap();
    assert_eq!(*ratio.downcast::<f32>().unwrap(), 0.25);
    ui.update_passes().unwrap();
    let second = ui
        .semantic_snapshot()
        .into_iter()
        .find(|node| node.data.label == "Second")
        .unwrap();
    assert_eq!(second.bounds.origin.x, 54.5);
}

#[test]
fn shaped_text_field_routes_focus_editing_and_incremental_repaint() {
    let mut ui = UiRoot::new(Flex::default(), LogicalSize::new(400.0, 300.0));
    let field = ui
        .append(
            ui.root(),
            TextField::new("Name", "Astrelis").on_changed(Action::Edit),
        )
        .unwrap();
    ui.update_passes().unwrap();

    ui.dispatch(UiInput::PointerPressed(LogicalPoint::new(20.0, 10.0)))
        .unwrap();
    let action = ui
        .dispatch(UiInput::Keyboard {
            input: key("!"),
            modifiers: Modifiers::default(),
        })
        .unwrap()
        .unwrap();
    assert_eq!(
        *action.downcast::<Action>().unwrap(),
        Action::Edit("A!strelis".into())
    );

    let update = ui.update_passes().unwrap();
    assert!(update.scene.rebuilt(field.id()));
    assert_eq!(ui.element(field).unwrap().selection(), (2, 2));
}

#[test]
fn accessibility_reports_deltas_and_removals() {
    let mut ui = UiRoot::new(Flex::default(), LogicalSize::new(400.0, 300.0));
    let child = ui
        .append(
            ui.root(),
            BoxElement {
                size: LogicalSize::new(40.0, 20.0),
                color: Color::WHITE,
                semantics: Some(SemanticData {
                    role: SemanticRole::Field,
                    label: "Width".into(),
                    value: Some("40".into()),
                    ..SemanticData::default()
                }),
                interactive: true,
            },
        )
        .unwrap();
    let first = ui.update_passes().unwrap();
    assert!(
        first
            .accessibility
            .changed
            .iter()
            .any(|node| node.id == child.id())
    );
    ui.update(child, Invalidation::ACCESSIBILITY, |child| {
        child.semantics.as_mut().unwrap().value = Some("41".into());
    })
    .unwrap();
    let changed = ui.update_passes().unwrap();
    assert_eq!(changed.accessibility.changed.len(), 1);
    ui.remove(child.id()).unwrap();
    let removed = ui.update_passes().unwrap();
    assert!(removed.accessibility.removed.contains(&child.id()));
}

#[test]
fn semantic_actions_focus_and_activate_control_values() {
    let mut ui = UiRoot::new(Flex::default(), LogicalSize::new(400.0, 300.0));
    let checkbox = ui
        .append(
            ui.root(),
            Checkbox::new("Visible", false, |checked| {
                Box::new(Action::Checked(checked))
            }),
        )
        .unwrap();
    ui.update_passes().unwrap();

    ui.perform_semantic_action(checkbox.id(), SemanticAction::Focus)
        .unwrap();
    let action = ui
        .perform_semantic_action(checkbox.id(), SemanticAction::Activate)
        .unwrap()
        .unwrap();
    assert_eq!(*action.downcast::<Action>().unwrap(), Action::Checked(true));
    let update = ui.update_passes().unwrap();
    assert!(
        update
            .accessibility
            .changed
            .iter()
            .any(|node| node.id == checkbox.id() && node.focused)
    );
}

#[test]
fn tab_focus_traversal_and_keyboard_activation_follow_tree_order() {
    let mut ui = UiRoot::new(Flex::default(), LogicalSize::new(400.0, 300.0));
    let first = ui
        .append(
            ui.root(),
            Button::new(
                "First",
                LogicalSize::new(100.0, 30.0),
                Color::WHITE,
                Color::BLACK,
                Action::Activate,
            ),
        )
        .unwrap();
    let second = ui
        .append(
            ui.root(),
            Checkbox::new("Second", false, |checked| {
                Box::new(Action::Checked(checked))
            }),
        )
        .unwrap();
    ui.update_passes().unwrap();

    ui.dispatch(UiInput::Keyboard {
        input: named_key(NamedKey::Tab),
        modifiers: Modifiers::default(),
    })
    .unwrap();
    ui.update_passes().unwrap();
    assert_eq!(
        ui.semantic_snapshot()
            .into_iter()
            .find(|node| node.focused)
            .map(|node| node.id),
        Some(first.id())
    );
    let action = ui
        .dispatch(UiInput::Keyboard {
            input: named_key(NamedKey::Enter),
            modifiers: Modifiers::default(),
        })
        .unwrap()
        .unwrap();
    assert_eq!(*action.downcast::<Action>().unwrap(), Action::Activate);

    ui.dispatch(UiInput::Keyboard {
        input: named_key(NamedKey::Tab),
        modifiers: Modifiers {
            shift: true,
            ..Modifiers::default()
        },
    })
    .unwrap();
    ui.update_passes().unwrap();
    assert_eq!(
        ui.semantic_snapshot()
            .into_iter()
            .find(|node| node.focused)
            .map(|node| node.id),
        Some(second.id())
    );
}
