//! Automatic dirty marking through guard patterns.
//!
//! The `StyleGuard` snapshots style state on creation and automatically
//! marks the appropriate dirty flags when dropped if properties changed.

use crate::dirty::DirtyFlags;
use crate::style::Overflow;
use crate::tree::{NodeId, UiTree};
use astrelis_render::Color;
use std::hash::{Hash, Hasher};

/// Hash of layout-affecting style fields.
///
/// Used to detect if a style change requires layout recomputation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LayoutHash(u64);

impl LayoutHash {
    /// Hash a Dimension value manually.
    fn hash_dimension(hasher: &mut impl Hasher, dim: &taffy::Dimension) {
        match dim {
            taffy::Dimension::Length(v) => {
                0u8.hash(hasher);
                v.to_bits().hash(hasher);
            }
            taffy::Dimension::Percent(v) => {
                1u8.hash(hasher);
                v.to_bits().hash(hasher);
            }
            taffy::Dimension::Auto => {
                2u8.hash(hasher);
            }
        }
    }

    /// Hash a LengthPercentage value manually.
    fn hash_length_percentage(hasher: &mut impl Hasher, lp: &taffy::LengthPercentage) {
        match lp {
            taffy::LengthPercentage::Length(v) => {
                0u8.hash(hasher);
                v.to_bits().hash(hasher);
            }
            taffy::LengthPercentage::Percent(v) => {
                1u8.hash(hasher);
                v.to_bits().hash(hasher);
            }
        }
    }

    /// Hash a LengthPercentageAuto value manually.
    fn hash_length_percentage_auto(hasher: &mut impl Hasher, lpa: &taffy::LengthPercentageAuto) {
        match lpa {
            taffy::LengthPercentageAuto::Length(v) => {
                0u8.hash(hasher);
                v.to_bits().hash(hasher);
            }
            taffy::LengthPercentageAuto::Percent(v) => {
                1u8.hash(hasher);
                v.to_bits().hash(hasher);
            }
            taffy::LengthPercentageAuto::Auto => {
                2u8.hash(hasher);
            }
        }
    }

    /// Compute hash from layout-affecting style fields using ahash.
    fn from_style(style: &taffy::Style) -> Self {
        let mut hasher = ahash::AHasher::default();

        // Size properties
        Self::hash_dimension(&mut hasher, &style.size.width);
        Self::hash_dimension(&mut hasher, &style.size.height);
        Self::hash_dimension(&mut hasher, &style.min_size.width);
        Self::hash_dimension(&mut hasher, &style.min_size.height);
        Self::hash_dimension(&mut hasher, &style.max_size.width);
        Self::hash_dimension(&mut hasher, &style.max_size.height);

        // Spacing
        Self::hash_length_percentage(&mut hasher, &style.padding.left);
        Self::hash_length_percentage(&mut hasher, &style.padding.right);
        Self::hash_length_percentage(&mut hasher, &style.padding.top);
        Self::hash_length_percentage(&mut hasher, &style.padding.bottom);
        Self::hash_length_percentage_auto(&mut hasher, &style.margin.left);
        Self::hash_length_percentage_auto(&mut hasher, &style.margin.right);
        Self::hash_length_percentage_auto(&mut hasher, &style.margin.top);
        Self::hash_length_percentage_auto(&mut hasher, &style.margin.bottom);
        Self::hash_length_percentage(&mut hasher, &style.border.left);
        Self::hash_length_percentage(&mut hasher, &style.border.right);
        Self::hash_length_percentage(&mut hasher, &style.border.top);
        Self::hash_length_percentage(&mut hasher, &style.border.bottom);

        // Flex properties
        std::mem::discriminant(&style.flex_direction).hash(&mut hasher);
        std::mem::discriminant(&style.flex_wrap).hash(&mut hasher);
        style
            .align_items
            .map(|v| std::mem::discriminant(&v))
            .hash(&mut hasher);
        style
            .align_content
            .map(|v| std::mem::discriminant(&v))
            .hash(&mut hasher);
        style
            .align_self
            .map(|v| std::mem::discriminant(&v))
            .hash(&mut hasher);
        style
            .justify_content
            .map(|v| std::mem::discriminant(&v))
            .hash(&mut hasher);
        Self::hash_length_percentage(&mut hasher, &style.gap.width);
        Self::hash_length_percentage(&mut hasher, &style.gap.height);

        // Flex sizing (previously missing)
        style.flex_grow.to_bits().hash(&mut hasher);
        style.flex_shrink.to_bits().hash(&mut hasher);
        Self::hash_dimension(&mut hasher, &style.flex_basis);
        style.aspect_ratio.map(|v| v.to_bits()).hash(&mut hasher);

        // Position
        std::mem::discriminant(&style.position).hash(&mut hasher);
        Self::hash_length_percentage_auto(&mut hasher, &style.inset.left);
        Self::hash_length_percentage_auto(&mut hasher, &style.inset.right);
        Self::hash_length_percentage_auto(&mut hasher, &style.inset.top);
        Self::hash_length_percentage_auto(&mut hasher, &style.inset.bottom);

        // Display
        std::mem::discriminant(&style.display).hash(&mut hasher);

        LayoutHash(hasher.finish())
    }
}

/// Guard for automatic dirty marking on style changes.
///
/// On creation, snapshots the current style state. On drop, compares with
/// the new state and marks appropriate dirty flags for each category of change.
///
/// # Example
/// ```ignore
/// let mut guard = tree.style_guard_mut(node_id);
/// guard.layout.padding = Rect::all(length(10.0));
/// // On drop, automatically marks LAYOUT flag if padding changed
/// ```
pub struct StyleGuard<'a> {
    node_id: NodeId,
    tree: &'a mut UiTree,
    layout_hash: LayoutHash,
    bg_color: Option<Color>,
    border_color: Option<Color>,
    border_width: f32,
    border_radius: f32,
    overflow: (Overflow, Overflow),
}

impl<'a> StyleGuard<'a> {
    /// Create a new style guard.
    pub(crate) fn new(tree: &'a mut UiTree, node_id: NodeId) -> Self {
        let (layout_hash, bg_color, border_color, border_width, border_radius, overflow) =
            if let Some(node) = tree.get_node(node_id) {
                let style = node.widget.style();
                (
                    LayoutHash::from_style(&style.layout),
                    style.background_color,
                    style.border_color,
                    style.border_width,
                    style.border_radius,
                    (style.overflow_x, style.overflow_y),
                )
            } else {
                (
                    LayoutHash(0),
                    None,
                    None,
                    0.0,
                    0.0,
                    (Overflow::default(), Overflow::default()),
                )
            };

        Self {
            node_id,
            tree,
            layout_hash,
            bg_color,
            border_color,
            border_width,
            border_radius,
            overflow,
        }
    }

    /// Get mutable reference to the widget's style.
    pub fn style_mut(&mut self) -> Option<&mut crate::style::Style> {
        self.tree
            .get_node_mut(self.node_id)
            .map(|node| node.widget.style_mut())
    }

    /// Get mutable reference to the Taffy layout style.
    pub fn layout_mut(&mut self) -> Option<&mut taffy::Style> {
        self.tree
            .get_node_mut(self.node_id)
            .map(|node| &mut node.widget.style_mut().layout)
    }
}

impl<'a> Drop for StyleGuard<'a> {
    fn drop(&mut self) {
        let Some(node) = self.tree.get_node(self.node_id) else {
            return;
        };

        let style = node.widget.style();
        let mut flags = DirtyFlags::NONE;

        // Check layout hash
        let after_hash = LayoutHash::from_style(&style.layout);
        if after_hash != self.layout_hash {
            flags |= DirtyFlags::LAYOUT;
        }

        // Check color changes
        if style.background_color != self.bg_color || style.border_color != self.border_color {
            flags |= DirtyFlags::COLOR;
        }

        // Check geometry changes (border width/radius)
        if (style.border_width - self.border_width).abs() > f32::EPSILON
            || (style.border_radius - self.border_radius).abs() > f32::EPSILON
        {
            flags |= DirtyFlags::GEOMETRY;
        }

        // Check overflow changes
        if (style.overflow_x, style.overflow_y) != self.overflow {
            flags |= DirtyFlags::CLIP;
        }

        if !flags.is_empty() {
            self.tree.mark_dirty_flags(self.node_id, flags);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_hash() {
        let style1 = taffy::Style {
            size: taffy::Size {
                width: taffy::Dimension::Length(100.0),
                height: taffy::Dimension::Length(50.0),
            },
            ..Default::default()
        };

        let style2 = taffy::Style {
            size: taffy::Size {
                width: taffy::Dimension::Length(100.0),
                height: taffy::Dimension::Length(50.0),
            },
            ..Default::default()
        };

        let style3 = taffy::Style {
            size: taffy::Size {
                width: taffy::Dimension::Length(200.0),
                height: taffy::Dimension::Length(50.0),
            },
            ..Default::default()
        };

        let hash1 = LayoutHash::from_style(&style1);
        let hash2 = LayoutHash::from_style(&style2);
        let hash3 = LayoutHash::from_style(&style3);

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_layout_hash_flex_fields() {
        let style1 = taffy::Style {
            flex_grow: 1.0,
            ..Default::default()
        };

        let style2 = taffy::Style {
            flex_grow: 2.0,
            ..Default::default()
        };

        let hash1 = LayoutHash::from_style(&style1);
        let hash2 = LayoutHash::from_style(&style2);

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_layout_hash_aspect_ratio() {
        let style1 = taffy::Style {
            aspect_ratio: Some(16.0 / 9.0),
            ..Default::default()
        };

        let style2 = taffy::Style {
            aspect_ratio: None,
            ..Default::default()
        };

        let hash1 = LayoutHash::from_style(&style1);
        let hash2 = LayoutHash::from_style(&style2);

        assert_ne!(hash1, hash2);
    }
}
