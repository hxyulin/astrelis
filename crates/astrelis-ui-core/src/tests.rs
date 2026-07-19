//! Unit tests for the retained UI core.

use super::*;
use astrelis_text::FontFamily;
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
struct MemoryClipboard(Mutex<Option<String>>);

impl astrelis_platform::backend::Clipboard for MemoryClipboard {
    fn capabilities(&self) -> astrelis_platform::ClipboardCapabilities {
        astrelis_platform::ClipboardCapabilities {
            read_text: true,
            write_text: true,
        }
    }

    fn read_text(&self) -> Result<Option<String>, PlatformError> {
        Ok(self.0.lock().unwrap().clone())
    }

    fn write_text(&self, text: String) -> Result<(), PlatformError> {
        *self.0.lock().unwrap() = Some(text);
        Ok(())
    }
}

#[derive(Debug)]
struct UnsupportedClipboard;

impl astrelis_platform::backend::Clipboard for UnsupportedClipboard {
    fn capabilities(&self) -> astrelis_platform::ClipboardCapabilities {
        astrelis_platform::ClipboardCapabilities::default()
    }

    fn read_text(&self) -> Result<Option<String>, PlatformError> {
        panic!("unsupported clipboard read should not be attempted")
    }

    fn write_text(&self, _text: String) -> Result<(), PlatformError> {
        panic!("unsupported clipboard write should not be attempted")
    }
}

fn key(logical_key: Key, text: Option<&str>) -> astrelis_platform::KeyboardInput {
    astrelis_platform::KeyboardInput {
        device_id: DeviceId(1),
        physical_key: astrelis_platform::PhysicalKey::Unidentified,
        logical_key,
        text: text.map(str::to_owned),
        location: astrelis_platform::KeyLocation::Standard,
        state: ElementState::Pressed,
        repeat: false,
        synthetic: false,
    }
}

fn ui() -> Ui {
    let mut ui = Ui::new(FontDatabase::default(), Theme::default());
    ui.set_viewport(Size::new(640.0, 480.0), 1.0);
    ui
}

#[test]
fn handles_are_generational_and_subtree_removal_is_recursive() {
    let mut ui = ui();
    let root = ui.root();
    let row = ui.add_row(root).unwrap();
    let label = ui.add_label(row, "old").unwrap();
    ui.remove(row).unwrap();
    assert!(ui.set_label_text(label, "stale").is_err());
    let replacement = ui.add_label(root, "new").unwrap();
    assert_ne!(label.id(), replacement.id());
}

#[test]
fn taffy_lays_out_rows_and_padding_without_leaking_types() {
    let mut ui = ui();
    let root = ui.root();
    let padding = ui.add_padding(root, Insets::all(20.0)).unwrap();
    let row = ui.add_row(padding).unwrap();
    ui.set_flex(row, 12.0, Alignment::Center).unwrap();
    let first = ui.add_button(row, "One").unwrap();
    let second = ui.add_button(row, "Two").unwrap();
    ui.ensure_layout().unwrap();
    let first_bounds = ui.node(first.id()).unwrap().bounds;
    let second_bounds = ui.node(second.id()).unwrap().bounds;
    assert!(first_bounds.origin.x >= 20.0);
    assert!(second_bounds.origin.x >= first_bounds.max_x() + 11.9);
}

#[test]
fn semantic_tree_contains_roles_values_and_selection() {
    let mut ui = ui();
    let root = ui.root();
    let field = ui.add_text_field(root, "hello").unwrap();
    ui.set_placeholder(field, "Name").unwrap();
    ui.set_focus(Some(field.id())).unwrap();
    let tree = ui.semantic_tree().unwrap();
    let field_node = &tree.children[0];
    assert_eq!(field_node.role, SemanticRole::TextField);
    assert_eq!(field_node.label, "Name");
    assert_eq!(field_node.value.as_deref(), Some("hello"));
    assert!(field_node.focused);
    assert!(
        field_node
            .actions
            .contains(&SemanticActionKind::SetSelection)
    );
}

#[test]
fn grapheme_deletion_does_not_split_unicode() {
    let mut ui = ui();
    let root = ui.root();
    let field = ui.add_text_field(root, "a👨‍👩‍👧‍👦").unwrap();
    let end = ui.text(field).unwrap().len();
    let previous = previous_grapheme(ui.text(field).unwrap(), end).unwrap();
    ui.replace_range(field.id(), previous, end, "").unwrap();
    assert_eq!(ui.text(field).unwrap(), "a");
    assert!(matches!(
        ui.drain_events().last().unwrap().kind,
        UiEventKind::TextChanged(ref value) if value == "a"
    ));
}

#[test]
fn focus_traversal_and_button_activation_queue_events() {
    let mut ui = ui();
    let root = ui.root();
    let first = ui.add_button(root, "First").unwrap();
    let second = ui.add_button(root, "Second").unwrap();
    ui.move_focus(true).unwrap();
    assert_eq!(ui.focus, Some(first.id()));
    ui.move_focus(true).unwrap();
    assert_eq!(ui.focus, Some(second.id()));
    assert!(
        ui.drain_events()
            .any(|event| event.is_from(second) && event.kind == UiEventKind::FocusChanged(true))
    );
}

#[test]
fn display_list_is_stable_when_read_repeatedly() {
    let mut ui = ui();
    let root = ui.root();
    ui.add_label(root, "Astrelis").unwrap();
    ui.add_button(root, "Save").unwrap();
    let first = ui.display_list().unwrap();
    assert!(!ui.needs_redraw());
    let second = ui.display_list().unwrap();
    assert_eq!(
        format!("{:?}", first.commands()),
        format!("{:?}", second.commands())
    );
}

#[test]
fn clipboard_shortcuts_and_ime_replace_selection() {
    let mut ui = ui();
    let root = ui.root();
    let field = ui.add_text_field(root, "alpha").unwrap();
    ui.set_focus(Some(field.id())).unwrap();
    let clipboard = Clipboard::from_backend(Arc::new(MemoryClipboard::default()));
    {
        let state = ui.text_field_mut(field.id()).unwrap();
        state.anchor.byte_index = 0;
        state.caret.byte_index = state.text.len();
    }
    ui.modifiers.control = true;
    ui.handle_text_key(
        field.id(),
        &key(Key::Character("c".into()), None),
        &clipboard,
    )
    .unwrap();
    assert_eq!(clipboard.read_text().unwrap().as_deref(), Some("alpha"));
    clipboard.write_text("βeta").unwrap();
    ui.handle_text_key(
        field.id(),
        &key(Key::Character("v".into()), None),
        &clipboard,
    )
    .unwrap();
    assert_eq!(ui.text(field).unwrap(), "βeta");
    ui.modifiers.control = false;
    ui.handle_ime(field.id(), &ImeEvent::Preedit("中".into(), None))
        .unwrap();
    assert_eq!(ui.text_field(field.id()).unwrap().preedit, "中");
    ui.handle_ime(field.id(), &ImeEvent::Commit("中文".into()))
        .unwrap();
    assert_eq!(ui.text(field).unwrap(), "βeta中文");
}

#[test]
fn unsupported_clipboard_shortcuts_are_noops() {
    let mut ui = ui();
    let root = ui.root();
    let field = ui.add_text_field(root, "alpha").unwrap();
    ui.set_focus(Some(field.id())).unwrap();
    {
        let state = ui.text_field_mut(field.id()).unwrap();
        state.anchor.byte_index = 0;
        state.caret.byte_index = state.text.len();
    }
    ui.modifiers.control = true;
    let clipboard = Clipboard::from_backend(Arc::new(UnsupportedClipboard));
    for shortcut in ["c", "x", "v"] {
        ui.handle_text_key(
            field.id(),
            &key(Key::Character(shortcut.into()), None),
            &clipboard,
        )
        .unwrap();
    }
    assert_eq!(ui.text(field).unwrap(), "alpha");
}

#[test]
fn theme_font_family_resolves_an_embedded_font() {
    let mut fonts = FontDatabase::empty();
    fonts
        .register_font(Arc::<[u8]>::from(
            &include_bytes!("../assets/NotoSans.ttf")[..],
        ))
        .unwrap();
    let theme = Theme {
        font_families: vec![FontFamily::Named("Noto Sans".into())],
        ..Default::default()
    };
    let mut ui: Ui = Ui::new(fonts, theme);
    let root = ui.root();
    ui.add_label(root, "Astrelis on WebGPU").unwrap();
    assert!(!ui.display_list().unwrap().texts().is_empty());
}

#[test]
fn bidi_caret_and_password_positions_round_trip() {
    let mut ui = ui();
    let root = ui.root();
    let field = ui.add_text_field(root, "hello אבג").unwrap();
    ui.set_focus(Some(field.id())).unwrap();
    ui.ensure_layout().unwrap();
    let before = ui.text_field(field.id()).unwrap().caret;
    ui.move_text_caret(field.id(), "ArrowLeft", false).unwrap();
    assert_ne!(ui.text_field(field.id()).unwrap().caret, before);
    ui.set_password(field, true).unwrap();
    ui.ensure_layout().unwrap();
    let state = ui.text_field(field.id()).unwrap();
    let layout_position = to_layout_position(state, state.caret);
    assert_eq!(from_layout_position(state, layout_position), state.caret);
}

#[derive(Debug, PartialEq)]
enum TestMessage {
    Activated,
    Checked(bool),
}

struct Compound {
    content: Option<ElementHandle<Column>>,
    unmounted: Arc<Mutex<bool>>,
}

impl Widget<TestMessage> for Compound {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn mounted(&mut self, context: &mut MountContext<'_, TestMessage>) -> Result<(), UiError> {
        context.add_label("Mounted")?;
        self.content = Some(context.add_column()?);
        Ok(())
    }
    fn unmounted(&mut self) {
        *self.unmounted.lock().unwrap() = true;
    }
    fn semantics(&self) -> Option<(SemanticRole, String, Option<String>)> {
        Some((SemanticRole::Group, "Compound".into(), None))
    }
}

#[test]
fn custom_widget_mounts_children_and_unmounts_with_subtree() {
    let mut ui = Ui::<TestMessage>::new(FontDatabase::default(), Theme::default());
    let root = ui.root();
    let flag = Arc::new(Mutex::new(false));
    let widget = ui
        .add_widget(
            root,
            Compound {
                content: None,
                unmounted: flag.clone(),
            },
        )
        .unwrap();
    assert!(ui.widget(widget).unwrap().content.is_some());
    assert_eq!(ui.semantic_tree().unwrap().children[0].label, "Compound");
    ui.remove(widget).unwrap();
    assert!(*flag.lock().unwrap());
}

struct StructuralSelfLayout;

impl Widget<TestMessage> for StructuralSelfLayout {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn mounted(&mut self, context: &mut MountContext<'_, TestMessage>) -> Result<(), UiError> {
        context.add_label("Structural child")?;
        Ok(())
    }

    fn container_style(&self, _theme: &Theme) -> WidgetContainerStyle {
        WidgetContainerStyle::structural()
    }

    fn event(&mut self, context: &mut EventContext<'_, TestMessage>, event: &RoutedEvent) {
        if matches!(event.kind, RoutedEventKind::PointerMoved { .. }) {
            context.set_current_layout(LayoutStyle {
                width: Length::Px(220.0),
                height: Length::Px(80.0),
                ..Default::default()
            });
        }
    }

    fn hit_testable(&self) -> bool {
        true
    }
}

#[test]
fn structural_widgets_can_update_their_own_layout_without_implicit_insets() {
    let mut ui = Ui::<TestMessage>::new(FontDatabase::default(), Theme::default());
    ui.set_viewport(Size::new(500.0, 300.0), 1.0);
    let root = ui.root();
    let widget = ui.add_widget(root, StructuralSelfLayout).unwrap();
    ui.set_layout(
        widget,
        LayoutStyle {
            width: Length::Px(100.0),
            height: Length::Px(40.0),
            ..Default::default()
        },
    )
    .unwrap();
    let child = ElementHandle::<Label> {
        id: ui.node(widget.id()).unwrap().children[0],
        marker: PhantomData,
    };
    ui.display_list().unwrap();
    let before = ui.layout_bounds(widget).unwrap();
    let child_bounds = ui.layout_bounds(child).unwrap();
    assert_eq!(child_bounds.origin, before.origin);

    ui.dispatch_routed(
        widget.id(),
        RoutedEventKind::PointerMoved {
            device_id: DeviceId(1),
            position: Point::new(10.0, 10.0),
        },
    )
    .unwrap();
    ui.display_list().unwrap();
    assert_eq!(ui.layout_bounds(widget).unwrap().size.width, 220.0);
}

#[test]
fn routed_listeners_emit_typed_messages_and_cancel_defaults() {
    let mut ui = Ui::<TestMessage>::new(FontDatabase::default(), Theme::default());
    let root = ui.root();
    let column = ui.add_column(root).unwrap();
    let checkbox = ui.add_checkbox(column, false).unwrap();
    ui.listen(
        column,
        Some(EventPhase::Capture),
        EventFilter::Activate,
        |context, _| {
            context.emit(TestMessage::Activated);
            context.prevent_default();
        },
    )
    .unwrap();
    assert!(
        ui.dispatch_routed(checkbox.id(), RoutedEventKind::Activate)
            .unwrap()
    );
    assert_eq!(
        ui.drain_messages().collect::<Vec<_>>(),
        vec![TestMessage::Activated]
    );
    assert!(!ui.checked(checkbox).unwrap());
    ui.listen(
        checkbox,
        None,
        EventFilter::ValueChanged,
        |context, event| {
            if let RoutedEventKind::CheckedChanged(value) = event.kind {
                context.emit(TestMessage::Checked(value));
            }
        },
    )
    .unwrap();
    ui.toggle_checkbox_id(checkbox.id()).unwrap();
    assert_eq!(
        ui.drain_messages().collect::<Vec<_>>(),
        vec![TestMessage::Checked(true)]
    );
}

#[test]
fn slider_snaps_and_scroll_view_clamps_and_reveals_focus() {
    let mut ui = Ui::<TestMessage>::new(FontDatabase::default(), Theme::default());
    ui.set_viewport(Size::new(320.0, 200.0), 1.0);
    let root = ui.root();
    let slider = ui.add_slider(root, 0.0, 1.0, 0.25, 0.6).unwrap();
    assert_eq!(ui.slider_value(slider).unwrap(), 0.5);
    let scroll = ui.add_scroll_view(root).unwrap();
    ui.set_layout(
        scroll,
        LayoutStyle {
            height: Length::Px(80.0),
            ..Default::default()
        },
    )
    .unwrap();
    let column = ui.add_column(scroll).unwrap();
    let mut last = None;
    for index in 0..8 {
        last = Some(ui.add_button(column, format!("Button {index}")).unwrap());
    }
    ui.ensure_layout().unwrap();
    assert!(
        matches!(ui.node(scroll.id()).unwrap().kind, Kind::ScrollView { content_height, .. } if content_height > 80.0)
    );
    ui.set_focus(last.map(|handle| handle.id())).unwrap();
    assert!(ui.scroll_offset(scroll).unwrap() > 0.0);
    ui.scroll_by_id(scroll.id(), f32::MAX).unwrap();
    let max = match ui.node(scroll.id()).unwrap().kind {
        Kind::ScrollView { content_height, .. } => content_height - 80.0,
        _ => unreachable!(),
    };
    assert!((ui.scroll_offset(scroll).unwrap() - max).abs() < 0.01);
    ui.set_scroll_offset(scroll, 0.0).unwrap();
    let bounds = ui.node(scroll.id()).unwrap().bounds;
    ui.set_scroll_from_point(
        scroll.id(),
        Point::new(bounds.origin.x + 20.0, bounds.origin.y + 60.0),
    )
    .unwrap();
    assert_eq!(ui.scroll_offset(scroll).unwrap(), 0.0);
    ui.set_scroll_from_point(
        scroll.id(),
        Point::new(bounds.max_x() - 2.0, bounds.origin.y + 60.0),
    )
    .unwrap();
    assert!(ui.scroll_offset(scroll).unwrap() > 0.0);
}

#[test]
fn flex_scroll_view_shrinks_tracks_nested_overflow_and_clips_input() {
    let mut ui = Ui::<TestMessage>::new(FontDatabase::default(), Theme::default());
    ui.set_viewport(Size::new(320.0, 180.0), 1.0);
    let root = ui.root();
    let padding = ui.add_padding(root, Insets::all(12.0)).unwrap();
    ui.set_layout(
        padding,
        LayoutStyle {
            grow: 1.0,
            ..Default::default()
        },
    )
    .unwrap();
    let scroll = ui.add_scroll_view(padding).unwrap();
    ui.set_layout(
        scroll,
        LayoutStyle {
            grow: 1.0,
            ..Default::default()
        },
    )
    .unwrap();
    let content = ui.add_column(scroll).unwrap();
    let mut last = None;
    for index in 0..12 {
        last = Some(ui.add_button(content, format!("Item {index}")).unwrap());
    }
    ui.ensure_layout().unwrap();
    let scroll_bounds = ui.node(scroll.id()).unwrap().bounds;
    let last = last.unwrap();
    let last_bounds = ui.node(last.id()).unwrap().bounds;
    let content_height = match ui.node(scroll.id()).unwrap().kind {
        Kind::ScrollView { content_height, .. } => content_height,
        _ => unreachable!(),
    };
    assert!(
        scroll_bounds.size.height <= 156.0 + f32::EPSILON,
        "{scroll_bounds:?}"
    );
    assert!(content_height > scroll_bounds.size.height);
    assert!(last_bounds.origin.y > scroll_bounds.max_y());
    assert_ne!(
        ui.hit_test(Point::new(
            last_bounds.origin.x + 2.0,
            last_bounds.origin.y + 2.0
        )),
        Some(last.id())
    );
    assert!(ui.scroll_by_id(scroll.id(), 40.0).unwrap());
    assert!(ui.scroll_offset(scroll).unwrap() > 0.0);
    ui.scroll_by_id(scroll.id(), -f32::MAX).unwrap();
    for _ in 0..13 {
        ui.move_focus(true).unwrap();
    }
    assert_eq!(ui.focus, Some(last.id()));
    assert!(ui.scroll_offset(scroll).unwrap() > 0.0);
    assert!(
        ui.hit_test(Point::new(
            scroll_bounds.origin.x + 2.0,
            scroll_bounds.max_y() - 2.0
        ))
        .is_some()
    );
}

#[test]
fn outer_scroll_extent_stops_at_nested_scroll_view_bounds() {
    let mut ui = ui();
    ui.set_viewport(Size::new(400.0, 260.0), 1.0);
    let root = ui.root();
    let outer = ui.add_scroll_view(root).unwrap();
    ui.set_layout(
        outer,
        LayoutStyle {
            height: Length::Px(200.0),
            ..Default::default()
        },
    )
    .unwrap();
    let column = ui.add_column(outer).unwrap();
    let inner = ui.add_scroll_view(column).unwrap();
    ui.set_layout(
        inner,
        LayoutStyle {
            height: Length::Px(100.0),
            shrink: 0.0,
            ..Default::default()
        },
    )
    .unwrap();
    let tall = ui.add_stack(inner).unwrap();
    ui.set_layout(
        tall,
        LayoutStyle {
            height: Length::Px(10_000.0),
            shrink: 0.0,
            ..Default::default()
        },
    )
    .unwrap();
    ui.add_label(column, "After nested scroll").unwrap();
    ui.ensure_layout().unwrap();
    let Kind::ScrollView { content_height, .. } = ui.node(outer.id()).unwrap().kind else {
        unreachable!()
    };
    assert!(
        content_height < 500.0,
        "outer extent leaked to {content_height}"
    );
}

#[test]
fn rich_layout_supports_percent_constraints_wrapping_and_absolute_stack_children() {
    let mut ui = ui();
    let root = ui.root();
    let row = ui.add_row(root).unwrap();
    ui.set_layout(
        row,
        LayoutStyle {
            width: Length::Px(300.0),
            height: Length::Px(120.0),
            ..Default::default()
        },
    )
    .unwrap();
    ui.set_flex_style(
        row,
        FlexStyle {
            wrap: FlexWrap::Wrap,
            column_gap: 8.0,
            row_gap: 8.0,
            ..Default::default()
        },
    )
    .unwrap();
    for label in ["One", "Two", "Three"] {
        let button = ui.add_button(row, label).unwrap();
        ui.set_layout(
            button,
            LayoutStyle {
                width: Length::Percent(0.48),
                min_height: Length::Px(40.0),
                ..Default::default()
            },
        )
        .unwrap();
    }
    let stack = ui.add_stack(root).unwrap();
    ui.set_layout(
        stack,
        LayoutStyle {
            width: Length::Px(100.0),
            height: Length::Px(60.0),
            ..Default::default()
        },
    )
    .unwrap();
    let back = ui.add_button(stack, "Back").unwrap();
    let front = ui.add_button(stack, "Front").unwrap();
    ui.set_z_index(front, 4).unwrap();
    ui.ensure_layout().unwrap();
    let third = ui.node(row.id()).unwrap().children[2];
    assert!(ui.node(third).unwrap().bounds.origin.y > ui.node(row.id()).unwrap().bounds.origin.y);
    let origin = ui.node(front.id()).unwrap().bounds.origin;
    let point = Point::new(origin.x + 5.0, origin.y + 5.0);
    assert_eq!(ui.hit_test(point), Some(front.id()));
    ui.set_visibility(front, Visibility::Hidden).unwrap();
    ui.ensure_layout().unwrap();
    assert_eq!(ui.hit_test(point), Some(back.id()));
}

#[test]
fn transformed_hit_testing_clipping_and_effective_enablement_match_painting() {
    let mut ui = ui();
    let root = ui.root();
    let stack = ui.add_stack(root).unwrap();
    ui.set_layout(
        stack,
        LayoutStyle {
            width: Length::Px(100.0),
            height: Length::Px(100.0),
            ..Default::default()
        },
    )
    .unwrap();
    ui.set_overflow(stack, Overflow::Clip).unwrap();
    let button = ui.add_button(stack, "Moved").unwrap();
    ui.set_transform(
        button,
        Affine2::from_translation(Vec2::new(30.0, 0.0)),
        LogicalPoint::ZERO,
    )
    .unwrap();
    ui.ensure_layout().unwrap();
    let bounds = ui.node(button.id()).unwrap().bounds;
    assert_eq!(
        ui.hit_test(Point::new(bounds.origin.x + 35.0, bounds.origin.y + 5.0)),
        Some(button.id())
    );
    assert_eq!(
        ui.hit_test(Point::new(bounds.max_x() + 20.0, bounds.origin.y + 5.0)),
        None
    );
    ui.set_enabled(stack, false).unwrap();
    assert_eq!(
        ui.hit_test(Point::new(bounds.origin.x + 35.0, bounds.origin.y + 5.0)),
        None
    );
}

#[test]
fn focus_scope_restores_focus_and_overlay_is_viewport_hosted() {
    let mut ui = ui();
    let root = ui.root();
    let owner = ui.add_button(root, "Owner").unwrap();
    ui.set_focus(Some(owner.id())).unwrap();
    let overlay = ui
        .add_overlay(
            owner,
            OverlayOptions {
                focus: FocusScopeOptions {
                    trapped: true,
                    autofocus: false,
                    restore_focus: true,
                },
                ..Default::default()
            },
        )
        .unwrap();
    ui.set_layout(
        overlay,
        LayoutStyle {
            width: Length::Px(140.0),
            height: Length::Px(80.0),
            ..Default::default()
        },
    )
    .unwrap();
    let action = ui.add_button(overlay, "Action").unwrap();
    ui.set_focus(Some(action.id())).unwrap();
    ui.ensure_layout().unwrap();
    assert!(
        ui.node(overlay.id()).unwrap().bounds.origin.y
            >= ui.node(owner.id()).unwrap().bounds.max_y()
    );
    let inspection = ui.inspect().unwrap();
    assert!(
        inspection
            .nodes
            .iter()
            .any(|node| node.id == overlay.id() && !node.focused)
    );
    ui.remove(overlay).unwrap();
    assert_eq!(ui.focus, Some(owner.id()));
}

#[test]
fn overlay_children_do_not_collapse_intrinsic_button_owners() {
    let mut ui = ui();
    let root = ui.root();
    let owner = ui.add_button(root, "A reasonably wide owner").unwrap();
    let before = ui.layout_bounds(owner).unwrap();
    let overlay = ui.add_overlay(owner, OverlayOptions::default()).unwrap();
    ui.add_label(overlay, "Overlay content").unwrap();
    let after = ui.layout_bounds(owner).unwrap();
    assert!(before.size.width > 100.0);
    assert!(after.size.width >= before.size.width);
    assert!(after.size.height >= before.size.height);
}

#[test]
fn inspection_and_public_hit_test_are_deterministic() {
    let mut ui = ui();
    let root = ui.root();
    let button = ui.add_button(root, "Inspect").unwrap();
    let first = ui.inspect().unwrap();
    let second = ui.inspect().unwrap();
    assert_eq!(first, second);
    let bounds = first
        .nodes
        .iter()
        .find(|node| node.id == button.id())
        .unwrap()
        .world_bounds;
    assert_eq!(
        ui.hit_test_at(Point::new(bounds.origin.x + 1.0, bounds.origin.y + 1.0))
            .unwrap(),
        Some(button.id())
    );
}

#[test]
fn drag_threshold_routes_drop_and_reports_outcome() {
    let mut ui = ui();
    ui.set_viewport(Size::new(500.0, 200.0), 1.0);
    let root = ui.root();
    let row = ui.add_row(root).unwrap();
    let source = ui.add_button(row, "source").unwrap();
    let target = ui.add_button(row, "target").unwrap();
    for handle in [source, target] {
        ui.set_layout(
            handle,
            LayoutStyle {
                width: Length::Px(180.0),
                height: Length::Px(80.0),
                ..Default::default()
            },
        )
        .unwrap();
    }
    let device = DeviceId(9);
    ui.listen(source, None, EventFilter::Pointer, move |context, event| {
        if let RoutedEventKind::PointerButton {
            position,
            state: ElementState::Pressed,
            ..
        } = event.kind
        {
            context.begin_drag(
                device,
                position,
                DragPayload::new(42_u32),
                DragOptions {
                    threshold: 5.0,
                    allowed: DragOperations::MOVE,
                },
            );
        }
    })
    .unwrap();
    let dropped = Arc::new(Mutex::new(Vec::new()));
    let dropped_listener = dropped.clone();
    ui.listen(
        target,
        None,
        EventFilter::Drag,
        move |context, event| match &event.kind {
            RoutedEventKind::DragOver {
                device_id, payload, ..
            } if payload.downcast_ref::<u32>() == Some(&42) => {
                context.accept_drop(*device_id, DropOperation::Move);
            }
            RoutedEventKind::Dropped {
                payload, operation, ..
            } => dropped_listener
                .lock()
                .unwrap()
                .push((*payload.downcast_ref::<u32>().unwrap(), *operation)),
            _ => {}
        },
    )
    .unwrap();
    let outcomes = Arc::new(Mutex::new(Vec::new()));
    let outcome_listener = outcomes.clone();
    ui.listen(source, None, EventFilter::Drag, move |_, event| {
        if let RoutedEventKind::DragEnded { outcome, .. } = event.kind {
            outcome_listener.lock().unwrap().push(outcome);
        }
    })
    .unwrap();

    ui.ensure_layout().unwrap();
    let source_point = ui.node(source.id()).unwrap().bounds.origin;
    let target_bounds = ui.node(target.id()).unwrap().bounds;
    let target_point = Point::new(target_bounds.origin.x + 20.0, target_bounds.origin.y + 20.0);
    ui.dispatch_routed(
        source.id(),
        RoutedEventKind::PointerButton {
            device_id: device,
            position: source_point,
            button: PointerButton::Primary,
            state: ElementState::Pressed,
        },
    )
    .unwrap();
    ui.update_drag(device, Point::new(source_point.x + 2.0, source_point.y))
        .unwrap();
    assert!(
        ui.drag_sessions
            .get(&device)
            .is_some_and(|drag| !drag.active)
    );
    ui.update_drag(device, target_point).unwrap();
    assert!(
        ui.drag_sessions
            .get(&device)
            .is_some_and(|drag| drag.active)
    );
    assert!(ui.finish_drag(device, target_point).unwrap());
    assert_eq!(*dropped.lock().unwrap(), vec![(42, DropOperation::Move)]);
    assert_eq!(
        *outcomes.lock().unwrap(),
        vec![DragOutcome::Dropped(DropOperation::Move)]
    );
}

#[test]
fn default_root_button_targets_window_origin() {
    let mut ui = ui();
    let button = ui.add_button(ui.root(), "Save").unwrap();
    ui.ensure_layout().unwrap();
    assert_eq!(ui.hit_test(Point::new(5.0, 5.0)), Some(button.id()));
}

#[test]
fn hover_paths_route_enter_leave_and_retarget_after_layout() {
    let mut ui = Ui::<TestMessage>::new(FontDatabase::default(), Theme::default());
    ui.set_viewport(Size::new(300.0, 200.0), 1.0);
    let root = ui.root();
    let parent = ui.add_column(root).unwrap();
    let button = ui.add_button(parent, "Hover").unwrap();
    let transitions = Arc::new(Mutex::new(Vec::new()));
    let observed = transitions.clone();
    ui.listen(button, None, EventFilter::Pointer, move |_, event| {
        if matches!(event.kind, RoutedEventKind::PointerEntered { .. }) {
            observed.lock().unwrap().push("enter");
        }
        if matches!(event.kind, RoutedEventKind::PointerLeft { .. }) {
            observed.lock().unwrap().push("leave");
        }
    })
    .unwrap();
    ui.ensure_layout().unwrap();
    let bounds = ui.node(button.id()).unwrap().bounds;
    let point = Point::new(bounds.origin.x + 2.0, bounds.origin.y + 2.0);
    let device = DeviceId(9);
    ui.pointer_positions.insert(device, point);
    ui.set_hover(device, point, Some(button.id())).unwrap();
    assert!(ui.is_hovered(parent).unwrap());
    ui.set_transform(
        button,
        Affine2::from_translation(Vec2::new(500.0, 0.0)),
        LogicalPoint::ZERO,
    )
    .unwrap();
    ui.invalidate_layout();
    ui.ensure_layout().unwrap();
    assert_eq!(&*transitions.lock().unwrap(), &["enter", "leave"]);
    assert!(!ui.is_hovered(parent).unwrap());
}

#[test]
fn set_theme_restyles_existing_typed_controls() {
    use astrelis_paint::Command;

    // The checkbox box is the only filled rounded rect in this tree, so its
    // brush color is an unambiguous probe for the resolved background.
    fn checkbox_fill(list: &DisplayList) -> Color {
        for command in list.commands() {
            if let Command::FillRoundedRect {
                brush: Brush::Solid(color),
                ..
            } = command
            {
                return *color;
            }
        }
        panic!("expected a filled checkbox box in the display list");
    }

    let mut ui = ui();
    let root = ui.root();
    ui.add_checkbox(root, false).unwrap();

    let base = Theme::default();
    assert_eq!(
        checkbox_fill(&ui.display_list().unwrap()),
        base.button.normal
    );

    // Regression: typed styles used to snapshot theme colors at creation, so
    // set_theme left already-created checkboxes stale. Resolving at paint time
    // means an unset override tracks the live theme.
    let restyled = Theme {
        button: ControlColors {
            normal: Color::new(0.9, 0.1, 0.1, 1.0),
            ..base.button
        },
        ..Theme::default()
    };
    ui.set_theme(restyled);

    assert_eq!(
        checkbox_fill(&ui.display_list().unwrap()),
        Color::new(0.9, 0.1, 0.1, 1.0),
        "set_theme must restyle an existing checkbox, not keep a snapshot"
    );
}

#[test]
fn checkbox_override_wins_over_theme() {
    use astrelis_paint::Command;

    fn checkbox_fill(list: &DisplayList) -> Color {
        for command in list.commands() {
            if let Command::FillRoundedRect {
                brush: Brush::Solid(color),
                ..
            } = command
            {
                return *color;
            }
        }
        panic!("expected a filled checkbox box in the display list");
    }

    let mut ui = ui();
    let root = ui.root();
    let checkbox = ui.add_checkbox(root, false).unwrap();
    let override_color = Color::new(0.2, 0.8, 0.4, 1.0);
    ui.set_checkbox_style(
        checkbox,
        CheckboxStyle {
            background: Some(override_color),
            ..Default::default()
        },
    )
    .unwrap();

    // An explicit override is honored, and it survives a theme change because
    // only the unset fields fall back to the theme.
    assert_eq!(checkbox_fill(&ui.display_list().unwrap()), override_color);
    ui.set_theme(Theme {
        button: ControlColors {
            normal: Color::new(0.9, 0.1, 0.1, 1.0),
            ..Theme::default().button
        },
        ..Theme::default()
    });
    assert_eq!(checkbox_fill(&ui.display_list().unwrap()), override_color);
}

#[test]
fn light_and_dark_themes_both_render() {
    use astrelis_paint::Command;

    // The first command is the viewport background fill, so its brush color is
    // the active theme's `background` token.
    fn background_fill(list: &DisplayList) -> Color {
        for command in list.commands() {
            if let Command::FillRect {
                brush: Brush::Solid(color),
                ..
            } = command
            {
                return *color;
            }
        }
        panic!("expected a viewport background fill");
    }

    let mut ui = ui();
    let root = ui.root();
    ui.add_button(root, "ok").unwrap();
    ui.add_checkbox(root, true).unwrap();

    ui.set_theme(Theme::dark());
    let dark = ui.display_list().unwrap();
    assert_eq!(background_fill(&dark), Theme::dark().background);

    ui.set_theme(Theme::light());
    let light = ui.display_list().unwrap();
    assert_eq!(background_fill(&light), Theme::light().background);

    assert_ne!(
        Theme::light().background,
        Theme::dark().background,
        "the two themes must be visually distinct"
    );
}

#[test]
fn metric_token_drives_slider_thumb() {
    use astrelis_paint::Command;

    // The slider thumb is the only ellipse the tree paints; its width reports
    // the resolved thumb diameter.
    fn thumb_diameter(list: &DisplayList) -> f32 {
        for command in list.commands() {
            if let Command::FillEllipse { rect, .. } = command {
                return rect.size.width;
            }
        }
        panic!("expected a slider thumb ellipse");
    }

    let mut ui = ui();
    let root = ui.root();
    ui.add_slider(root, 0.0, 1.0, 0.1, 0.5).unwrap();

    assert_eq!(
        thumb_diameter(&ui.display_list().unwrap()),
        Theme::default().metrics.slider_thumb
    );

    // A metric change must flow to paint rather than a hardcoded literal.
    let mut theme = Theme::default();
    theme.metrics.slider_thumb = 40.0;
    ui.set_theme(theme);
    assert_eq!(thumb_diameter(&ui.display_list().unwrap()), 40.0);
}

#[test]
fn disabling_a_checkbox_changes_its_box_fill() {
    use astrelis_paint::Command;

    // The checkbox box is the first filled rounded rect in the tree.
    fn checkbox_fill(list: &DisplayList) -> Color {
        for command in list.commands() {
            if let Command::FillRoundedRect {
                brush: Brush::Solid(color),
                ..
            } = command
            {
                return *color;
            }
        }
        panic!("expected a filled checkbox box in the display list");
    }

    let mut ui = ui();
    let root = ui.root();
    let checkbox = ui.add_checkbox(root, false).unwrap();

    let theme = Theme::default();
    assert_eq!(
        checkbox_fill(&ui.display_list().unwrap()),
        theme.button.normal
    );

    // Disabling now resolves the box through the shared state ladder, so it
    // paints the disabled color instead of staying at the normal one.
    ui.set_enabled(checkbox, false).unwrap();
    assert_eq!(
        checkbox_fill(&ui.display_list().unwrap()),
        theme.button.disabled,
        "a disabled checkbox must show the disabled color"
    );
}

#[test]
fn wrapping_label_respects_max_width_and_grows_taller() {
    let mut ui = ui();
    let root = ui.root();
    let text = "The quick brown fox jumps over the lazy dog repeatedly";

    let single_line = ui.add_label(root, text).unwrap();
    let wrapped = ui.add_label(root, text).unwrap();
    ui.set_layout(
        wrapped,
        LayoutStyle {
            max_width: Length::Px(120.0),
            ..Default::default()
        },
    )
    .unwrap();
    ui.set_wrap(wrapped, true).unwrap();

    let single = ui.layout_bounds(single_line).unwrap();
    let multi = ui.layout_bounds(wrapped).unwrap();

    assert!(
        single.size.width > 120.5,
        "the un-wrapped label is a single wide line"
    );
    assert!(
        multi.size.width <= 120.5,
        "a wrapping label stays within its max width"
    );
    assert!(
        multi.size.height > single.size.height,
        "wrapping breaks the text into more lines, growing the height"
    );
}

#[test]
fn retained_layout_reflows_siblings_when_a_style_changes() {
    let mut ui = ui();
    let root = ui.root();
    let column = ui.add_column(root).unwrap();
    let top = ui.add_column(column).unwrap();
    ui.set_layout(
        top,
        LayoutStyle {
            height: Length::Px(40.0),
            ..Default::default()
        },
    )
    .unwrap();
    let bottom = ui.add_column(column).unwrap();
    ui.set_layout(
        bottom,
        LayoutStyle {
            height: Length::Px(40.0),
            ..Default::default()
        },
    )
    .unwrap();
    let before = ui.layout_bounds(bottom).unwrap().origin.y;

    // Growing the box above must reflow the sibling below through the retained
    // Taffy tree: the style diff re-pushes only `top`, and Taffy re-solves the
    // column. A stale cache would leave `bottom` where it was.
    ui.set_layout(
        top,
        LayoutStyle {
            height: Length::Px(100.0),
            ..Default::default()
        },
    )
    .unwrap();
    let after = ui.layout_bounds(bottom).unwrap().origin.y;

    assert!(
        (after - before - 60.0).abs() < 0.5,
        "sibling should shift down by the 60px growth, moved {}",
        after - before
    );
}

#[test]
fn retained_layout_remeasures_text_when_font_size_changes() {
    let mut ui = ui();
    let root = ui.root();
    let column = ui.add_column(root).unwrap();
    let first = ui.add_label(column, "First").unwrap();
    let second = ui.add_label(column, "Second").unwrap();
    let before = ui.layout_bounds(second).unwrap().origin.y;

    // A larger font enlarges the first label's shaped height. Its Taffy style is
    // unchanged (labels size by measure, not style), so only the measured-size
    // diff can catch it and mark the node dirty; otherwise Taffy reuses the
    // cached measure and the label below never moves.
    ui.set_widget_style(
        first,
        WidgetStyle {
            font_size: Some(48.0),
            ..Default::default()
        },
    )
    .unwrap();
    let after = ui.layout_bounds(second).unwrap().origin.y;

    assert!(
        after > before + 10.0,
        "larger text should push the label below it down, moved from {before} to {after}"
    );
}

#[test]
fn a_single_text_change_enqueues_one_node_not_a_resweep() {
    let mut ui: Ui = Ui::new(FontDatabase::default(), Theme::default());
    ui.set_viewport(LogicalSize::new(640.0, 480.0), 1.0);
    let root = ui.root();
    let column = ui.add_column(root).unwrap();
    let mut labels = Vec::new();
    for index in 0..8 {
        labels.push(ui.add_label(column, format!("Item {index}")).unwrap());
    }
    // Settle the tree, clearing the construction-time dirty state.
    ui.layout_bounds(root).unwrap();
    assert!(ui.dirty_nodes.is_empty());
    assert!(!ui.measure_resweep);

    // Changing one label enqueues exactly that node, without a resweep.
    let target = labels[4];
    ui.set_label_text(target, "Changed").unwrap();
    assert!(
        !ui.measure_resweep,
        "a single label change must not resweep"
    );
    assert_eq!(
        ui.dirty_nodes.iter().copied().collect::<Vec<_>>(),
        vec![target.id()],
        "only the changed node should be enqueued"
    );

    // The next layout consumes and clears the queue.
    ui.layout_bounds(root).unwrap();
    assert!(ui.dirty_nodes.is_empty());
    assert!(!ui.measure_resweep);
}

#[test]
fn coarse_changes_request_a_full_resweep() {
    let mut ui: Ui = Ui::new(FontDatabase::default(), Theme::default());
    ui.set_viewport(LogicalSize::new(640.0, 480.0), 1.0);
    let root = ui.root();
    ui.add_label(root, "Item").unwrap();
    ui.layout_bounds(root).unwrap();
    assert!(!ui.measure_resweep);

    // A theme swap can restyle every node, so it must fall back to a resweep
    // rather than trusting a per-node queue that never saw the change.
    let mut theme = Theme::default();
    theme.type_scale.body += 2.0;
    ui.set_theme(theme);
    assert!(ui.measure_resweep, "a theme change must resweep");
}

/// The text-shaping job is `(request) -> layout` over owned data, so it can run
/// on a worker thread for background reshaping. This locks that boundary open;
/// if a future change makes either type thread-bound, offloading breaks here
/// rather than silently.
#[test]
fn shaping_job_types_are_send() {
    fn assert_send<T: Send>() {}
    assert_send::<astrelis_text::TextLayoutRequest>();
    assert_send::<astrelis_text::TextLayout>();
}

/// A deterministic font database for the async-worker tests. Both the main
/// thread and the worker build one of these; because they register the same
/// blob into an empty collection, they shape byte-identically — the invariant
/// the sync/async parity assertions below rest on.
fn async_test_fonts() -> FontDatabase {
    let mut fonts = FontDatabase::empty();
    fonts
        .register_font(Arc::<[u8]>::from(
            &include_bytes!("../assets/NotoSans.ttf")[..],
        ))
        .unwrap();
    fonts
}

fn async_test_theme() -> Theme {
    Theme {
        font_families: vec![FontFamily::Named("Noto Sans".into())],
        ..Default::default()
    }
}

/// The core async-worker contract: a reshape of an already-shaped node is
/// offloaded, the *previous* extent stays on screen until the result is drained
/// (so nothing reflows mid-flight), and once drained the layout matches exactly
/// what a synchronous shape of the same string produces.
///
/// The "old extent until flush" half is deterministic despite the worker
/// running concurrently: results are only ever applied by `poll_async` /
/// `flush_async`, and the layout pass that enqueues the reshape polls *before*
/// it enqueues — so within that pass the node still measures with its old
/// layout no matter how fast the worker is.
#[test]
fn async_worker_keeps_old_extent_then_matches_sync() {
    let sync_bounds = {
        let mut ui: Ui = Ui::new(async_test_fonts(), async_test_theme());
        ui.set_viewport(LogicalSize::new(640.0, 480.0), 1.0);
        let root = ui.root();
        let label = ui.add_label(root, "short").unwrap();
        ui.layout_bounds(label).unwrap();
        ui.set_label_text(label, "a considerably longer label string")
            .unwrap();
        ui.layout_bounds(label).unwrap()
    };

    let mut ui: Ui = Ui::new(async_test_fonts(), async_test_theme());
    ui.enable_async_shaping(async_test_fonts, || {});
    ui.set_viewport(LogicalSize::new(640.0, 480.0), 1.0);
    let root = ui.root();
    let label = ui.add_label(root, "short").unwrap();
    // First shape is force-synced (no previous layout to keep on screen).
    let initial = ui.layout_bounds(label).unwrap();

    // Growing the text is eligible for the worker: the old extent must stay.
    ui.set_label_text(label, "a considerably longer label string")
        .unwrap();
    let before_flush = ui.layout_bounds(label).unwrap();
    assert_eq!(
        before_flush, initial,
        "the previous extent must stay on screen while the worker reshapes"
    );
    assert!(
        ui.node(label.id()).unwrap().pending.is_some(),
        "a reshape must be in flight before it is drained"
    );

    assert!(ui.flush_async(), "draining the worker changed the layout");
    let after_flush = ui.layout_bounds(label).unwrap();
    assert_eq!(
        after_flush, sync_bounds,
        "the worker result must match a synchronous shape of the same string"
    );
    assert!(
        ui.node(label.id()).unwrap().pending.is_none(),
        "pending must settle once the result is applied"
    );
}

/// When a node is edited again while a reshape is still in flight, the stale
/// result must be dropped (matched out by `RequestId`) and only the latest
/// string may win — regardless of the order the worker returns them.
#[test]
fn async_worker_drops_superseded_reshape() {
    let mut ui: Ui = Ui::new(async_test_fonts(), async_test_theme());
    ui.enable_async_shaping(async_test_fonts, || {});
    ui.set_viewport(LogicalSize::new(640.0, 480.0), 1.0);
    let root = ui.root();
    let label = ui.add_label(root, "one").unwrap();
    ui.layout_bounds(label).unwrap();

    ui.set_label_text(label, "two").unwrap();
    ui.layout_bounds(label).unwrap(); // enqueue reshape for "two"
    ui.set_label_text(label, "three three three three").unwrap();
    ui.layout_bounds(label).unwrap(); // enqueue reshape for the final string
    ui.flush_async();
    let async_bounds = ui.layout_bounds(label).unwrap();

    let sync_bounds = {
        let mut ui: Ui = Ui::new(async_test_fonts(), async_test_theme());
        ui.set_viewport(LogicalSize::new(640.0, 480.0), 1.0);
        let root = ui.root();
        let label = ui.add_label(root, "three three three three").unwrap();
        ui.layout_bounds(label).unwrap()
    };

    assert_eq!(
        async_bounds, sync_bounds,
        "only the latest string may win after superseded reshapes"
    );
    assert!(ui.node(label.id()).unwrap().pending.is_none());
}
