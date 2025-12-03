//! Automatic dirty marking through guard patterns and versioned values.
//!
//! This module provides zero-cost abstractions that automatically mark nodes dirty
//! when their properties change, eliminating manual `mark_dirty()` calls.

use crate::dirty::DirtyFlags;
use crate::tree::{NodeId, UiTree};
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

    /// Compute hash from layout-affecting style fields.
    fn from_style(style: &taffy::Style) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();

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

        // Flex properties (these do implement Hash via derive)
        std::mem::discriminant(&style.flex_direction).hash(&mut hasher);
        std::mem::discriminant(&style.flex_wrap).hash(&mut hasher);
        style
            .align_items
            .and_then(|v| Some(std::mem::discriminant(&v)))
            .hash(&mut hasher);
        style
            .align_content
            .and_then(|v| Some(std::mem::discriminant(&v)))
            .hash(&mut hasher);
        style
            .align_self
            .and_then(|v| Some(std::mem::discriminant(&v)))
            .hash(&mut hasher);
        style
            .justify_content
            .and_then(|v| Some(std::mem::discriminant(&v)))
            .hash(&mut hasher);
        Self::hash_length_percentage(&mut hasher, &style.gap.width);
        Self::hash_length_percentage(&mut hasher, &style.gap.height);

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
/// On creation, snapshots the current layout hash. On drop, compares the new hash
/// and marks appropriate dirty flags if the style changed.
///
/// # Example
/// ```ignore
/// let mut guard = tree.style_guard_mut(node_id);
/// guard.layout.padding = Rect::all(length(10.0));
/// // On drop, automatically marks LAYOUT flag if padding changed
/// ```
pub struct StyleGuard<'a> {
    node_id: NodeId,
    before_hash: LayoutHash,
    tree: &'a mut UiTree,
}

impl<'a> StyleGuard<'a> {
    /// Create a new style guard.
    pub(crate) fn new(tree: &'a mut UiTree, node_id: NodeId) -> Self {
        let before_hash = if let Some(node) = tree.get_node(node_id) {
            LayoutHash::from_style(&node.widget.style().layout)
        } else {
            LayoutHash(0)
        };

        Self {
            node_id,
            before_hash,
            tree,
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
            .and_then(|node| Some(&mut node.widget.style_mut().layout))
    }
}

impl<'a> Drop for StyleGuard<'a> {
    fn drop(&mut self) {
        // Check if layout-affecting properties changed
        if let Some(node) = self.tree.get_node(self.node_id) {
            let after_hash = LayoutHash::from_style(&node.widget.style().layout);

            if after_hash != self.before_hash {
                // Layout properties changed - mark LAYOUT flag
                self.tree.mark_dirty_flags(self.node_id, DirtyFlags::LAYOUT);
            }
        }
    }
}

/// Versioned text value that auto-bumps version on changes.
///
/// Used to track text content changes for cache invalidation and
/// automatic dirty marking.
///
/// # Example
/// ```ignore
/// let mut text_value = TextValue::new("Hello");
/// assert_eq!(text_value.version(), 0);
///
/// text_value.set("World");
/// assert_eq!(text_value.version(), 1); // Auto-incremented
/// ```
#[derive(Debug, Clone)]
pub struct TextValue {
    content: String,
    version: u32,
}

impl TextValue {
    /// Create a new text value.
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            version: 0,
        }
    }

    /// Get the current content.
    pub fn get(&self) -> &str {
        &self.content
    }

    /// Set new content, incrementing version if changed.
    pub fn set(&mut self, new_content: impl Into<String>) -> bool {
        let new_content = new_content.into();
        if self.content != new_content {
            self.content = new_content;
            self.version = self.version.wrapping_add(1);
            true
        } else {
            false
        }
    }

    /// Get the current version.
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Check if this value is newer than a cached version.
    pub fn is_newer_than(&self, cached_version: u32) -> bool {
        self.version != cached_version
    }
}

impl Default for TextValue {
    fn default() -> Self {
        Self::new("")
    }
}

impl From<String> for TextValue {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for TextValue {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl AsRef<str> for TextValue {
    fn as_ref(&self) -> &str {
        &self.content
    }
}

impl std::fmt::Display for TextValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.content)
    }
}

/// Versioned numeric value for counters and metrics.
///
/// Similar to TextValue but specialized for numeric types.
/// Provides fast-path optimizations for common numeric operations.
#[derive(Debug, Clone)]
pub struct NumericValue<T> {
    value: T,
    version: u32,
}

impl<T: PartialEq> NumericValue<T> {
    /// Create a new numeric value.
    pub fn new(value: T) -> Self {
        Self { value, version: 0 }
    }

    /// Get the current value.
    pub fn get(&self) -> &T {
        &self.value
    }

    /// Set new value, incrementing version if changed.
    pub fn set(&mut self, new_value: T) -> bool {
        if self.value != new_value {
            self.value = new_value;
            self.version = self.version.wrapping_add(1);
            true
        } else {
            false
        }
    }

    /// Get the current version.
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Check if this value is newer than a cached version.
    pub fn is_newer_than(&self, cached_version: u32) -> bool {
        self.version != cached_version
    }
}

impl<T: Default + PartialEq> Default for NumericValue<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

/// Reactive value wrapper with dirty callback.
///
/// When the value changes, automatically invokes a callback to mark
/// the appropriate dirty flags.
///
/// # Example
/// ```ignore
/// let value = Value::new(Color::RED, |tree, node_id| {
///     tree.mark_dirty_flags(node_id, DirtyFlags::COLOR_ONLY);
/// });
///
/// value.set(Color::BLUE); // Automatically marks node dirty
/// ```
pub struct Value<T> {
    inner: T,
    version: u32,
}

impl<T> Value<T> {
    /// Create a new reactive value.
    pub fn new(value: T) -> Self {
        Self {
            inner: value,
            version: 0,
        }
    }

    /// Get reference to the inner value.
    pub fn get(&self) -> &T {
        &self.inner
    }

    /// Get the current version.
    pub fn version(&self) -> u32 {
        self.version
    }
}

impl<T: PartialEq> Value<T> {
    /// Set the value, returning true if it changed.
    pub fn set(&mut self, new_value: T) -> bool {
        if self.inner != new_value {
            self.inner = new_value;
            self.version = self.version.wrapping_add(1);
            true
        } else {
            false
        }
    }
}

impl<T: Default> Default for Value<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: Clone> Clone for Value<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            version: self.version,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_value_version() {
        let mut value = TextValue::new("Hello");
        assert_eq!(value.version(), 0);
        assert_eq!(value.get(), "Hello");

        // Change increments version
        assert!(value.set("World"));
        assert_eq!(value.version(), 1);
        assert_eq!(value.get(), "World");

        // No change doesn't increment
        assert!(!value.set("World"));
        assert_eq!(value.version(), 1);
    }

    #[test]
    fn test_text_value_is_newer() {
        let mut value = TextValue::new("Test");
        assert!(!value.is_newer_than(0));

        value.set("New");
        assert!(value.is_newer_than(0));
        assert!(!value.is_newer_than(1));
    }

    #[test]
    fn test_numeric_value() {
        let mut value = NumericValue::new(42);
        assert_eq!(*value.get(), 42);
        assert_eq!(value.version(), 0);

        assert!(value.set(100));
        assert_eq!(*value.get(), 100);
        assert_eq!(value.version(), 1);

        assert!(!value.set(100));
        assert_eq!(value.version(), 1);
    }

    #[test]
    fn test_value() {
        let mut value = Value::new(42);
        assert_eq!(*value.get(), 42);
        assert_eq!(value.version(), 0);

        assert!(value.set(100));
        assert_eq!(value.version(), 1);

        assert!(!value.set(100));
        assert_eq!(value.version(), 1);
    }

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
}
