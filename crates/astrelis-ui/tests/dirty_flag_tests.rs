//! Unit tests for dirty flag system (no GPU required).
//!
//! These tests verify that dirty flags are correctly set, combined, and propagated
//! to enable efficient incremental UI updates.

use astrelis_ui::dirty::DirtyFlags;

#[test]
fn test_dirty_flags_none() {
    let flags = DirtyFlags::NONE;

    assert!(flags.is_empty());
    assert!(!flags.needs_layout());
    assert!(!flags.needs_text_shaping());
    assert!(!flags.needs_geometry_rebuild());
    assert!(!flags.is_paint_only());
    assert!(!flags.should_propagate_to_parent());
}

#[test]
fn test_dirty_flags_layout() {
    let flags = DirtyFlags::LAYOUT;

    assert!(!flags.is_empty());
    assert!(flags.needs_layout());
    assert!(!flags.needs_text_shaping());
    assert!(flags.needs_geometry_rebuild());
    assert!(!flags.is_paint_only());
    assert!(flags.should_propagate_to_parent());
}

#[test]
fn test_dirty_flags_text_shaping() {
    let flags = DirtyFlags::TEXT_SHAPING;

    assert!(!flags.is_empty());
    assert!(flags.needs_layout());
    assert!(flags.needs_text_shaping());
    assert!(flags.needs_geometry_rebuild());
    assert!(!flags.is_paint_only());
    assert!(flags.should_propagate_to_parent());
}

#[test]
fn test_dirty_flags_color_only() {
    let flags = DirtyFlags::COLOR;

    assert!(!flags.is_empty());
    assert!(!flags.needs_layout());
    assert!(!flags.needs_text_shaping());
    assert!(!flags.needs_geometry_rebuild());
    assert!(flags.is_paint_only());
    assert!(!flags.should_propagate_to_parent());
}

#[test]
fn test_dirty_flags_opacity_only() {
    let flags = DirtyFlags::OPACITY;

    assert!(!flags.is_empty());
    assert!(!flags.needs_layout());
    assert!(!flags.needs_text_shaping());
    assert!(!flags.needs_geometry_rebuild());
    assert!(flags.is_paint_only());
    assert!(!flags.should_propagate_to_parent());
}

#[test]
fn test_dirty_flags_geometry() {
    let flags = DirtyFlags::GEOMETRY;

    assert!(!flags.is_empty());
    assert!(!flags.needs_layout());
    assert!(!flags.needs_text_shaping());
    assert!(flags.needs_geometry_rebuild());
    // GEOMETRY alone is considered paint-only (e.g., border radius change)
    assert!(flags.is_paint_only());
    assert!(!flags.should_propagate_to_parent());
}

#[test]
fn test_dirty_flags_transform() {
    let flags = DirtyFlags::TRANSFORM;

    assert!(!flags.is_empty());
    assert!(!flags.needs_layout());
    assert!(!flags.needs_text_shaping());
    assert!(flags.needs_geometry_rebuild());
    assert!(!flags.is_paint_only());
    assert!(!flags.should_propagate_to_parent());
}

#[test]
fn test_dirty_flags_children_order() {
    let flags = DirtyFlags::CHILDREN_ORDER;

    assert!(!flags.is_empty());
    assert!(flags.needs_layout());
    assert!(!flags.needs_text_shaping());
    assert!(flags.needs_geometry_rebuild());
    assert!(!flags.is_paint_only());
    assert!(flags.should_propagate_to_parent());
}

#[test]
fn test_dirty_flags_geometry_standalone() {
    let flags = DirtyFlags::GEOMETRY;

    assert!(!flags.is_empty());
    assert!(!flags.needs_layout());
    assert!(!flags.needs_text_shaping());
    assert!(flags.needs_geometry_rebuild());
    assert!(flags.is_paint_only());
    assert!(!flags.should_propagate_to_parent());
}

#[test]
fn test_dirty_flags_combination_layout_and_color() {
    let flags = DirtyFlags::LAYOUT | DirtyFlags::COLOR;

    // Layout flag takes precedence
    assert!(flags.needs_layout());
    assert!(!flags.is_paint_only());
    assert!(flags.should_propagate_to_parent());
}

#[test]
fn test_dirty_flags_combination_color_and_opacity() {
    let flags = DirtyFlags::COLOR | DirtyFlags::OPACITY;

    // Both are paint-only changes
    assert!(!flags.needs_layout());
    assert!(flags.is_paint_only());
    assert!(!flags.should_propagate_to_parent());
}

#[test]
fn test_dirty_flags_combination_text_and_geometry() {
    let flags = DirtyFlags::TEXT_SHAPING | DirtyFlags::GEOMETRY;

    // Text shaping triggers layout
    assert!(flags.needs_layout());
    assert!(flags.needs_text_shaping());
    assert!(flags.needs_geometry_rebuild());
    assert!(!flags.is_paint_only());
    assert!(flags.should_propagate_to_parent());
}

#[test]
fn test_dirty_flags_propagation() {
    let layout_flags = DirtyFlags::LAYOUT;
    let propagated = layout_flags.propagation_flags();

    // Layout should propagate
    assert!(propagated.contains(DirtyFlags::LAYOUT));
}

#[test]
fn test_dirty_flags_no_propagation_for_color() {
    let color_flags = DirtyFlags::COLOR;
    let propagated = color_flags.propagation_flags();

    // Color-only changes don't propagate to parent
    assert!(propagated.is_empty());
}

#[test]
fn test_dirty_flags_propagation_children_order() {
    let flags = DirtyFlags::CHILDREN_ORDER;
    let propagated = flags.propagation_flags();

    // Children order changes should propagate
    assert!(propagated.contains(DirtyFlags::CHILDREN_ORDER));
}

#[test]
fn test_dirty_flags_propagation_mixed() {
    let flags = DirtyFlags::LAYOUT | DirtyFlags::COLOR | DirtyFlags::GEOMETRY;
    let propagated = flags.propagation_flags();

    // Only layout propagates, not color or geometry
    assert!(propagated.contains(DirtyFlags::LAYOUT));
    assert!(!propagated.contains(DirtyFlags::COLOR));
    assert!(!propagated.contains(DirtyFlags::GEOMETRY));
}

#[test]
fn test_dirty_flags_bitwise_or() {
    let flags1 = DirtyFlags::LAYOUT;
    let flags2 = DirtyFlags::COLOR;
    let combined = flags1 | flags2;

    assert!(combined.contains(DirtyFlags::LAYOUT));
    assert!(combined.contains(DirtyFlags::COLOR));
}

#[test]
fn test_dirty_flags_bitwise_and() {
    let flags1 = DirtyFlags::LAYOUT | DirtyFlags::COLOR;
    let flags2 = DirtyFlags::LAYOUT | DirtyFlags::GEOMETRY;
    let intersection = flags1 & flags2;

    assert!(intersection.contains(DirtyFlags::LAYOUT));
    assert!(!intersection.contains(DirtyFlags::COLOR));
    assert!(!intersection.contains(DirtyFlags::GEOMETRY));
}

#[test]
fn test_dirty_flags_intersects() {
    let flags = DirtyFlags::LAYOUT | DirtyFlags::TEXT_SHAPING;

    assert!(flags.intersects(DirtyFlags::LAYOUT));
    assert!(flags.intersects(DirtyFlags::TEXT_SHAPING));
    assert!(flags.intersects(DirtyFlags::LAYOUT | DirtyFlags::TEXT_SHAPING));
    assert!(!flags.intersects(DirtyFlags::COLOR));
}

#[test]
fn test_dirty_flags_contains() {
    let flags = DirtyFlags::LAYOUT | DirtyFlags::COLOR;

    assert!(flags.contains(DirtyFlags::LAYOUT));
    assert!(flags.contains(DirtyFlags::COLOR));
    assert!(flags.contains(DirtyFlags::LAYOUT | DirtyFlags::COLOR));
    assert!(!flags.contains(DirtyFlags::GEOMETRY));
}

#[test]
fn test_dirty_flags_insert() {
    let mut flags = DirtyFlags::LAYOUT;
    flags.insert(DirtyFlags::COLOR);

    assert!(flags.contains(DirtyFlags::LAYOUT));
    assert!(flags.contains(DirtyFlags::COLOR));
}

#[test]
fn test_dirty_flags_remove() {
    let mut flags = DirtyFlags::LAYOUT | DirtyFlags::COLOR;
    flags.remove(DirtyFlags::COLOR);

    assert!(flags.contains(DirtyFlags::LAYOUT));
    assert!(!flags.contains(DirtyFlags::COLOR));
}

#[test]
fn test_dirty_flags_toggle() {
    let mut flags = DirtyFlags::LAYOUT;
    flags.toggle(DirtyFlags::COLOR);

    assert!(flags.contains(DirtyFlags::LAYOUT));
    assert!(flags.contains(DirtyFlags::COLOR));

    flags.toggle(DirtyFlags::LAYOUT);
    assert!(!flags.contains(DirtyFlags::LAYOUT));
    assert!(flags.contains(DirtyFlags::COLOR));
}

#[test]
fn test_dirty_flags_default() {
    let flags = DirtyFlags::default();

    assert_eq!(flags, DirtyFlags::NONE);
    assert!(flags.is_empty());
}

#[test]
fn test_paint_only_optimization() {
    // Test the key optimization: paint-only changes don't need layout
    let paint_only = DirtyFlags::COLOR | DirtyFlags::OPACITY;

    assert!(paint_only.is_paint_only());
    assert!(!paint_only.needs_layout());
    assert!(!paint_only.needs_text_shaping());
    assert!(!paint_only.needs_geometry_rebuild());

    // Adding any structural change breaks paint-only optimization
    let not_paint_only = paint_only | DirtyFlags::LAYOUT;
    assert!(!not_paint_only.is_paint_only());
}

#[test]
fn test_needs_layout_scenarios() {
    // These flags should trigger layout recomputation
    assert!(DirtyFlags::LAYOUT.needs_layout());
    assert!(DirtyFlags::TEXT_SHAPING.needs_layout());
    assert!(DirtyFlags::CHILDREN_ORDER.needs_layout());

    // These flags should NOT trigger layout recomputation
    assert!(!DirtyFlags::COLOR.needs_layout());
    assert!(!DirtyFlags::OPACITY.needs_layout());
    assert!(!DirtyFlags::GEOMETRY.needs_layout());
    assert!(!DirtyFlags::TRANSFORM.needs_layout());
    assert!(!DirtyFlags::IMAGE.needs_layout());
    assert!(!DirtyFlags::FOCUS.needs_layout());
    assert!(!DirtyFlags::VISIBILITY.needs_layout());
    assert!(!DirtyFlags::SCROLL.needs_layout());
}

#[test]
fn test_needs_geometry_rebuild_scenarios() {
    // These flags should trigger geometry rebuild
    assert!(DirtyFlags::LAYOUT.needs_geometry_rebuild());
    assert!(DirtyFlags::GEOMETRY.needs_geometry_rebuild());
    assert!(DirtyFlags::TEXT_SHAPING.needs_geometry_rebuild());
    assert!(DirtyFlags::CHILDREN_ORDER.needs_geometry_rebuild());
    assert!(DirtyFlags::TRANSFORM.needs_geometry_rebuild());
    assert!(DirtyFlags::VISIBILITY.needs_geometry_rebuild());

    // These flags should NOT trigger geometry rebuild
    assert!(!DirtyFlags::COLOR.needs_geometry_rebuild());
    assert!(!DirtyFlags::OPACITY.needs_geometry_rebuild());
    assert!(!DirtyFlags::IMAGE.needs_geometry_rebuild());
    assert!(!DirtyFlags::FOCUS.needs_geometry_rebuild());
    assert!(!DirtyFlags::SCROLL.needs_geometry_rebuild());
}

#[test]
fn test_propagation_scenarios() {
    // These flags should propagate to parent
    assert!(DirtyFlags::LAYOUT.should_propagate_to_parent());
    assert!(DirtyFlags::TEXT_SHAPING.should_propagate_to_parent());
    assert!(DirtyFlags::CHILDREN_ORDER.should_propagate_to_parent());

    // These flags should NOT propagate to parent
    assert!(!DirtyFlags::COLOR.should_propagate_to_parent());
    assert!(!DirtyFlags::OPACITY.should_propagate_to_parent());
    assert!(!DirtyFlags::GEOMETRY.should_propagate_to_parent());
    assert!(!DirtyFlags::TRANSFORM.should_propagate_to_parent());
    assert!(!DirtyFlags::IMAGE.should_propagate_to_parent());
    assert!(!DirtyFlags::FOCUS.should_propagate_to_parent());
    assert!(!DirtyFlags::VISIBILITY.should_propagate_to_parent());
    assert!(!DirtyFlags::SCROLL.should_propagate_to_parent());
}

#[test]
fn test_dirty_flags_image() {
    let flags = DirtyFlags::IMAGE;

    assert!(!flags.is_empty());
    assert!(!flags.needs_layout());
    assert!(!flags.needs_text_shaping());
    assert!(!flags.needs_geometry_rebuild());
    assert!(flags.is_paint_only());
    assert!(!flags.should_propagate_to_parent());
}

#[test]
fn test_dirty_flags_focus() {
    let flags = DirtyFlags::FOCUS;

    assert!(!flags.is_empty());
    assert!(!flags.needs_layout());
    assert!(!flags.needs_text_shaping());
    assert!(!flags.needs_geometry_rebuild());
    assert!(flags.is_paint_only());
    assert!(!flags.should_propagate_to_parent());
}

#[test]
fn test_dirty_flags_visibility() {
    let flags = DirtyFlags::VISIBILITY;

    assert!(!flags.is_empty());
    assert!(!flags.needs_layout());
    assert!(flags.needs_geometry_rebuild());
    assert!(flags.is_paint_only());
    assert!(!flags.should_propagate_to_parent());
}

#[test]
fn test_dirty_flags_scroll() {
    let flags = DirtyFlags::SCROLL;

    assert!(!flags.is_empty());
    assert!(!flags.needs_layout());
    assert!(!flags.needs_text_shaping());
    assert!(!flags.needs_geometry_rebuild());
    assert!(flags.is_paint_only());
    assert!(!flags.should_propagate_to_parent());
}

#[test]
fn test_dirty_flags_clip() {
    let flags = DirtyFlags::CLIP;

    assert!(!flags.is_empty());
    assert!(!flags.needs_layout());
    assert!(flags.needs_clip_update());
    assert!(flags.is_paint_only());
    assert!(!flags.should_propagate_to_parent());
}

#[test]
fn test_clip_update_scenarios() {
    assert!(DirtyFlags::CLIP.needs_clip_update());
    assert!(DirtyFlags::LAYOUT.needs_clip_update());
    assert!(DirtyFlags::CHILDREN_ORDER.needs_clip_update());
    assert!(DirtyFlags::TRANSFORM.needs_clip_update());

    assert!(!DirtyFlags::COLOR.needs_clip_update());
    assert!(!DirtyFlags::GEOMETRY.needs_clip_update());
    assert!(!DirtyFlags::IMAGE.needs_clip_update());
    assert!(!DirtyFlags::SCROLL.needs_clip_update());
}
