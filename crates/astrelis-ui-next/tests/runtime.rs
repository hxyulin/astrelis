//! Incremental retained runtime behavior.

use astrelis_core::{
    color::Color,
    geometry::{LogicalPoint, LogicalSize},
};
use astrelis_platform::{
    DeviceId, ElementState, Key, KeyLocation, KeyboardInput, Modifiers, NamedKey, PhysicalKey,
};
use astrelis_ui_next::{
    Axis, BoxElement, Button, Checkbox, Flex, Invalidation, Label, Scroll, ScrollAxis,
    SemanticAction, SemanticData, SemanticRole, Stack, TextField, UiInput, UiRoot,
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
