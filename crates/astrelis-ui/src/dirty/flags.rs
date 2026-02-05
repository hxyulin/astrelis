//! Fine-grained dirty flag tracking for UI nodes.

use bitflags::bitflags;

bitflags! {
    /// Fine-grained dirty flags for UI node updates.
    ///
    /// These flags allow selective recomputation based on what actually changed.
    /// For example, a color-only change doesn't need layout recomputation.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct DirtyFlags: u16 {
        /// No changes
        const NONE              = 0;

        // Layout-affecting (bits 0-2)

        /// Layout-affecting properties changed (size, position, flex, padding, etc.)
        /// Requires Taffy recomputation.
        const LAYOUT            = 1 << 0;

        /// Text content or font properties changed.
        /// Requires text reshaping and possibly layout if wrapping changes.
        const TEXT_SHAPING      = 1 << 1;

        /// Children were added, removed, or reordered.
        /// Requires layout recomputation for parent and children.
        const CHILDREN_ORDER    = 1 << 2;

        // Paint-only (bits 3-7)

        /// Only colors changed (background, border color).
        /// Can skip layout and text shaping, only update paint data.
        const COLOR             = 1 << 3;

        /// Only opacity changed.
        /// Can skip layout and text shaping, only update alpha channel.
        const OPACITY           = 1 << 4;

        /// Geometry changed (border width, border radius).
        /// Requires geometry rebuild but not necessarily layout.
        const GEOMETRY          = 1 << 5;

        /// Image source or UV coordinates changed.
        const IMAGE             = 1 << 6;

        /// Focus state changed.
        const FOCUS             = 1 << 7;

        // Structural (bits 8-10)

        /// Transform properties changed (position, rotation, scale).
        const TRANSFORM         = 1 << 8;

        /// Clip bounds changed (overflow property or layout affecting clip rect).
        /// Requires recalculation of scissor rects for rendering.
        const CLIP              = 1 << 9;

        /// Visible/hidden toggle changed.
        const VISIBILITY        = 1 << 10;

        // Scroll (bit 11)

        /// Scroll offset changed.
        const SCROLL            = 1 << 11;

        // Reserved bits 12-15 for future use
    }
}

impl DirtyFlags {
    /// Layout-affecting flags group.
    pub const LAYOUT_GROUP: Self = Self::LAYOUT
        .union(Self::TEXT_SHAPING)
        .union(Self::CHILDREN_ORDER);

    /// Paint-only flags group.
    pub const PAINT_GROUP: Self = Self::COLOR
        .union(Self::OPACITY)
        .union(Self::GEOMETRY)
        .union(Self::IMAGE)
        .union(Self::FOCUS);

    /// Flags that should propagate to parent nodes.
    pub const PROPAGATE_GROUP: Self = Self::LAYOUT
        .union(Self::TEXT_SHAPING)
        .union(Self::CHILDREN_ORDER);

    /// Returns true if any layout-affecting flags are set.
    #[inline]
    pub fn needs_layout(&self) -> bool {
        self.intersects(Self::LAYOUT_GROUP)
    }

    /// Returns true if text needs to be reshaped.
    #[inline]
    pub fn needs_text_shaping(&self) -> bool {
        self.contains(Self::TEXT_SHAPING)
    }

    /// Returns true if only visual properties changed (no layout needed).
    #[inline]
    pub fn is_paint_only(&self) -> bool {
        !self.is_empty()
            && !self.intersects(
                Self::LAYOUT | Self::TEXT_SHAPING | Self::CHILDREN_ORDER | Self::TRANSFORM,
            )
    }

    /// Returns true if geometry needs to be rebuilt.
    #[inline]
    pub fn needs_geometry_rebuild(&self) -> bool {
        self.intersects(
            Self::LAYOUT
                | Self::GEOMETRY
                | Self::TEXT_SHAPING
                | Self::CHILDREN_ORDER
                | Self::TRANSFORM
                | Self::VISIBILITY,
        )
    }

    /// Returns true if clip rects need to be recalculated.
    #[inline]
    pub fn needs_clip_update(&self) -> bool {
        self.intersects(Self::CLIP | Self::LAYOUT | Self::CHILDREN_ORDER | Self::TRANSFORM)
    }

    /// Returns true if the node should propagate dirty flags to ancestors.
    #[inline]
    pub fn should_propagate_to_parent(&self) -> bool {
        self.intersects(Self::PROPAGATE_GROUP)
    }

    /// Get flags that should be propagated to parent nodes.
    #[inline]
    pub fn propagation_flags(&self) -> Self {
        *self & (Self::LAYOUT | Self::CHILDREN_ORDER)
    }
}

impl Default for DirtyFlags {
    fn default() -> Self {
        Self::NONE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_flags() {
        assert!(DirtyFlags::LAYOUT.needs_layout());
        assert!(DirtyFlags::TEXT_SHAPING.needs_layout());
        assert!(DirtyFlags::CHILDREN_ORDER.needs_layout());
        assert!(!DirtyFlags::COLOR.needs_layout());
        assert!(!DirtyFlags::OPACITY.needs_layout());
    }

    #[test]
    fn test_paint_only() {
        assert!(DirtyFlags::COLOR.is_paint_only());
        assert!(DirtyFlags::OPACITY.is_paint_only());
        assert!((DirtyFlags::COLOR | DirtyFlags::OPACITY).is_paint_only());
        assert!(DirtyFlags::GEOMETRY.is_paint_only());
        assert!(DirtyFlags::IMAGE.is_paint_only());
        assert!(DirtyFlags::FOCUS.is_paint_only());
        assert!(!DirtyFlags::LAYOUT.is_paint_only());
        assert!(!DirtyFlags::TEXT_SHAPING.is_paint_only());
    }

    #[test]
    fn test_propagation() {
        assert!(DirtyFlags::LAYOUT.should_propagate_to_parent());
        assert!(DirtyFlags::TEXT_SHAPING.should_propagate_to_parent());
        assert!(DirtyFlags::CHILDREN_ORDER.should_propagate_to_parent());
        assert!(!DirtyFlags::COLOR.should_propagate_to_parent());
        assert!(!DirtyFlags::OPACITY.should_propagate_to_parent());
        assert!(!DirtyFlags::GEOMETRY.should_propagate_to_parent());
    }

    #[test]
    fn test_propagation_flags() {
        let flags = DirtyFlags::LAYOUT | DirtyFlags::COLOR;
        let propagated = flags.propagation_flags();
        assert_eq!(propagated, DirtyFlags::LAYOUT);
        assert!(!propagated.contains(DirtyFlags::COLOR));
    }

    #[test]
    fn test_text_shaping() {
        assert!(DirtyFlags::TEXT_SHAPING.needs_text_shaping());
        assert!(!DirtyFlags::COLOR.needs_text_shaping());
        assert!(!DirtyFlags::LAYOUT.needs_text_shaping());
    }

    #[test]
    fn test_geometry_rebuild() {
        assert!(DirtyFlags::LAYOUT.needs_geometry_rebuild());
        assert!(DirtyFlags::GEOMETRY.needs_geometry_rebuild());
        assert!(DirtyFlags::TEXT_SHAPING.needs_geometry_rebuild());
        assert!(DirtyFlags::TRANSFORM.needs_geometry_rebuild());
        assert!(DirtyFlags::VISIBILITY.needs_geometry_rebuild());
        assert!(!DirtyFlags::COLOR.needs_geometry_rebuild());
        assert!(!DirtyFlags::OPACITY.needs_geometry_rebuild());
    }

    #[test]
    fn test_clip_update() {
        assert!(DirtyFlags::CLIP.needs_clip_update());
        assert!(DirtyFlags::LAYOUT.needs_clip_update());
        assert!(DirtyFlags::CHILDREN_ORDER.needs_clip_update());
        assert!(DirtyFlags::TRANSFORM.needs_clip_update());
        assert!(!DirtyFlags::COLOR.needs_clip_update());
        assert!(!DirtyFlags::OPACITY.needs_clip_update());
        assert!(!DirtyFlags::GEOMETRY.needs_clip_update());
    }

    #[test]
    fn test_new_flags() {
        assert!(DirtyFlags::IMAGE.is_paint_only());
        assert!(DirtyFlags::FOCUS.is_paint_only());
        assert!(DirtyFlags::VISIBILITY.is_paint_only());
        assert!(DirtyFlags::SCROLL.is_paint_only());
        assert!(!DirtyFlags::IMAGE.needs_layout());
        assert!(!DirtyFlags::FOCUS.needs_layout());
        assert!(!DirtyFlags::VISIBILITY.needs_layout());
        assert!(!DirtyFlags::SCROLL.needs_layout());
    }
}
