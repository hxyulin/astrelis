//! Unit tests for opacity, transforms, visibility, and pointer events (no GPU required).

use astrelis_core::math::Vec2;
use astrelis_ui::dirty::DirtyFlags;
use astrelis_ui::style::{PointerEvents, Style};

// ── Style field defaults ──────────────────────────────────────────────────────

#[test]
fn test_style_default_opacity() {
    let style = Style::new();
    assert_eq!(style.opacity, 1.0);
}

#[test]
fn test_style_default_translate() {
    let style = Style::new();
    assert_eq!(style.translate, Vec2::ZERO);
}

#[test]
fn test_style_default_scale() {
    let style = Style::new();
    assert_eq!(style.scale, Vec2::ONE);
}

#[test]
fn test_style_default_visible() {
    let style = Style::new();
    assert!(style.visible);
}

#[test]
fn test_style_default_pointer_events() {
    let style = Style::new();
    assert_eq!(style.pointer_events, PointerEvents::Auto);
}

// ── Style consuming builder methods ───────────────────────────────────────────

#[test]
fn test_style_opacity_builder() {
    let style = Style::new().opacity(0.5);
    assert_eq!(style.opacity, 0.5);
}

#[test]
fn test_style_opacity_clamped() {
    let low = Style::new().opacity(-0.5);
    assert_eq!(low.opacity, 0.0);

    let high = Style::new().opacity(2.0);
    assert_eq!(high.opacity, 1.0);
}

#[test]
fn test_style_translate_builder() {
    let style = Style::new().translate(Vec2::new(10.0, 20.0));
    assert_eq!(style.translate, Vec2::new(10.0, 20.0));
}

#[test]
fn test_style_translate_x_builder() {
    let style = Style::new().translate_x(15.0);
    assert_eq!(style.translate.x, 15.0);
    assert_eq!(style.translate.y, 0.0);
}

#[test]
fn test_style_translate_y_builder() {
    let style = Style::new().translate_y(-30.0);
    assert_eq!(style.translate.x, 0.0);
    assert_eq!(style.translate.y, -30.0);
}

#[test]
fn test_style_scale_builder() {
    let style = Style::new().scale(Vec2::new(2.0, 3.0));
    assert_eq!(style.scale, Vec2::new(2.0, 3.0));
}

#[test]
fn test_style_uniform_scale_builder() {
    let style = Style::new().uniform_scale(0.5);
    assert_eq!(style.scale, Vec2::new(0.5, 0.5));
}

#[test]
fn test_style_visible_builder() {
    let style = Style::new().visible(false);
    assert!(!style.visible);
}

#[test]
fn test_style_pointer_events_builder() {
    let style = Style::new().pointer_events(PointerEvents::None);
    assert_eq!(style.pointer_events, PointerEvents::None);
}

// ── Style in-place setter methods ─────────────────────────────────────────────

#[test]
fn test_style_set_opacity() {
    let mut style = Style::new();
    style.set_opacity(0.3);
    assert_eq!(style.opacity, 0.3);

    // Test clamping
    style.set_opacity(-1.0);
    assert_eq!(style.opacity, 0.0);
    style.set_opacity(5.0);
    assert_eq!(style.opacity, 1.0);
}

#[test]
fn test_style_set_translate() {
    let mut style = Style::new();
    style.set_translate(Vec2::new(5.0, 10.0));
    assert_eq!(style.translate, Vec2::new(5.0, 10.0));
}

#[test]
fn test_style_set_translate_x() {
    let mut style = Style::new();
    style.set_translate_x(42.0);
    assert_eq!(style.translate.x, 42.0);
    assert_eq!(style.translate.y, 0.0);
}

#[test]
fn test_style_set_translate_y() {
    let mut style = Style::new();
    style.set_translate_y(-10.0);
    assert_eq!(style.translate.x, 0.0);
    assert_eq!(style.translate.y, -10.0);
}

#[test]
fn test_style_set_scale() {
    let mut style = Style::new();
    style.set_scale(Vec2::new(2.0, 0.5));
    assert_eq!(style.scale, Vec2::new(2.0, 0.5));
}

#[test]
fn test_style_set_uniform_scale() {
    let mut style = Style::new();
    style.set_uniform_scale(3.0);
    assert_eq!(style.scale, Vec2::new(3.0, 3.0));
}

#[test]
fn test_style_set_visible() {
    let mut style = Style::new();
    style.set_visible(false);
    assert!(!style.visible);
    style.set_visible(true);
    assert!(style.visible);
}

#[test]
fn test_style_set_pointer_events() {
    let mut style = Style::new();
    style.set_pointer_events(PointerEvents::None);
    assert_eq!(style.pointer_events, PointerEvents::None);
    style.set_pointer_events(PointerEvents::Auto);
    assert_eq!(style.pointer_events, PointerEvents::Auto);
}

// ── Padding/margin convenience builder methods ────────────────────────────────

#[test]
fn test_style_padding_x_builder() {
    let style = Style::new().padding_x(10.0);
    let constraints = style.padding_constraints().unwrap();
    // padding_x sets left (index 0) and right (index 2)
    assert!(constraints[0].try_to_length_percentage().is_some());
    assert!(constraints[2].try_to_length_percentage().is_some());
}

#[test]
fn test_style_padding_y_builder() {
    let style = Style::new().padding_y(8.0);
    let constraints = style.padding_constraints().unwrap();
    // padding_y sets top (index 1) and bottom (index 3)
    assert!(constraints[1].try_to_length_percentage().is_some());
    assert!(constraints[3].try_to_length_percentage().is_some());
}

#[test]
fn test_style_margin_x_builder() {
    let style = Style::new().margin_x(5.0);
    let constraints = style.margin_constraints().unwrap();
    assert!(constraints[0].try_to_length_percentage_auto().is_some());
    assert!(constraints[2].try_to_length_percentage_auto().is_some());
}

#[test]
fn test_style_margin_y_builder() {
    let style = Style::new().margin_y(12.0);
    let constraints = style.margin_constraints().unwrap();
    assert!(constraints[1].try_to_length_percentage_auto().is_some());
    assert!(constraints[3].try_to_length_percentage_auto().is_some());
}

// ── Dirty flags for new properties ────────────────────────────────────────────

#[test]
fn test_dirty_flags_transform() {
    let flags = DirtyFlags::TRANSFORM;

    assert!(!flags.is_empty());
    assert!(!flags.needs_layout());
    assert!(flags.needs_geometry_rebuild());
    assert!(flags.needs_clip_update());
    assert!(!flags.should_propagate_to_parent());
}

#[test]
fn test_dirty_flags_visibility() {
    let flags = DirtyFlags::VISIBILITY;

    assert!(!flags.is_empty());
    assert!(!flags.needs_layout());
    assert!(flags.needs_geometry_rebuild());
    assert!(!flags.should_propagate_to_parent());
}

// ── UiCore API integration ────────────────────────────────────────────────────

#[test]
fn test_uicore_opacity_update() {
    let mut core = astrelis_ui::UiCore::new();
    let wid = astrelis_ui::WidgetId::new("test");

    core.build(|root| {
        root.container().id(wid).build();
    });

    // Update opacity
    assert!(core.update_opacity(wid, 0.5));
    // Same value should return false
    assert!(!core.update_opacity(wid, 0.5));
    // Different value should return true
    assert!(core.update_opacity(wid, 1.0));
}

#[test]
fn test_uicore_translate_update() {
    let mut core = astrelis_ui::UiCore::new();
    let wid = astrelis_ui::WidgetId::new("test");

    core.build(|root| {
        root.container().id(wid).build();
    });

    assert!(core.update_translate(wid, Vec2::new(10.0, 20.0)));
    assert!(!core.update_translate(wid, Vec2::new(10.0, 20.0))); // Same value
    assert!(core.update_translate_x(wid, 15.0));
    assert!(core.update_translate_y(wid, 25.0));
}

#[test]
fn test_uicore_scale_update() {
    let mut core = astrelis_ui::UiCore::new();
    let wid = astrelis_ui::WidgetId::new("test");

    core.build(|root| {
        root.container().id(wid).build();
    });

    assert!(core.update_scale(wid, Vec2::new(2.0, 2.0)));
    assert!(!core.update_scale(wid, Vec2::new(2.0, 2.0))); // Same value
    assert!(core.update_scale_x(wid, 3.0));
    assert!(core.update_scale_y(wid, 0.5));
}

#[test]
fn test_uicore_visibility_toggle() {
    let mut core = astrelis_ui::UiCore::new();
    let wid = astrelis_ui::WidgetId::new("panel");

    core.build(|root| {
        root.container().id(wid).build();
    });

    // Initially visible
    assert!(core.set_visible(wid, false));
    assert!(!core.set_visible(wid, false)); // Same value

    // Toggle back
    assert!(core.toggle_visible(wid));
    assert!(core.toggle_visible(wid)); // Toggle again
}

#[test]
fn test_uicore_nonexistent_widget() {
    let mut core = astrelis_ui::UiCore::new();
    let wid = astrelis_ui::WidgetId::new("does_not_exist");

    assert!(!core.update_opacity(wid, 0.5));
    assert!(!core.update_translate(wid, Vec2::new(1.0, 1.0)));
    assert!(!core.update_scale(wid, Vec2::new(2.0, 2.0)));
    assert!(!core.set_visible(wid, false));
    assert!(!core.toggle_visible(wid));
}

// ── PointerEvents enum ───────────────────────────────────────────────────────

#[test]
fn test_pointer_events_default_is_auto() {
    assert_eq!(PointerEvents::default(), PointerEvents::Auto);
}

#[test]
fn test_pointer_events_equality() {
    assert_eq!(PointerEvents::Auto, PointerEvents::Auto);
    assert_eq!(PointerEvents::None, PointerEvents::None);
    assert_ne!(PointerEvents::Auto, PointerEvents::None);
}
