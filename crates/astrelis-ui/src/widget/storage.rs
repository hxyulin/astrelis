//! Widget storage with generational safety.
//!
//! This module provides efficient storage for widgets with type-safe handles
//! and generational safety to prevent use-after-free.

use super::{capability::*, handle::*};
use ahash::HashMap;

/// Entry in widget storage with generation tracking.
struct WidgetEntry {
    /// The widget (trait object)
    widget: Box<dyn Widget>,
    /// Current generation (incremented on remove)
    generation: u32,
}

/// Storage for widgets with generational safety.
///
/// # Borrow Checking Pattern: Generational Arena
///
/// This is similar to generational-arena or slotmap crates.
///
/// ## Key Insight
///
/// We can safely give out handles because:
/// 1. Handles are Copy (just IDs)
/// 2. Storage checks generation before access
/// 3. Mutable access requires `&mut self` (exclusive borrow)
///
/// ## Prevents
///
/// - **Use-after-free**: Stale handles return None
/// - **Aliasing violations**: Can't get multiple `&mut`
///
/// # Example
///
/// ```rust,ignore
/// let mut storage = WidgetStorage::new();
///
/// // Add widgets
/// let button = storage.add(Button::new("Click"));
/// let text = storage.add(Text::new("Hello"));
///
/// // Get references
/// if let Some(widget) = storage.get(button) {
///     println!("Widget: {}", widget.id());
/// }
///
/// // Mutable access
/// if let Some(widget) = storage.get_mut(button) {
///     if let Some(text_widget) = widget.as_text_widget_mut() {
///         text_widget.set_text("Updated");
///     }
/// }
///
/// // Remove widget
/// storage.remove(button);
///
/// // Old handle now invalid
/// assert!(storage.get(button).is_none());
/// ```
pub struct WidgetStorage {
    /// Map from widget ID to entry
    widgets: HashMap<WidgetId, WidgetEntry>,

    /// Next ID to allocate
    next_id: u64,
}

impl WidgetStorage {
    /// Create a new widget storage.
    pub fn new() -> Self {
        Self {
            widgets: HashMap::default(),
            next_id: 0,
        }
    }

    /// Add a widget and return a typed handle.
    ///
    /// # Borrow Checking
    ///
    /// Takes `&mut self` (exclusive borrow) to modify storage.
    /// Returns handle (Copy) so no borrow conflicts.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let button_handle = storage.add(Button::new("Click"));
    /// let text_handle = storage.add(Text::new("Hello"));
    /// ```
    pub fn add<T: Widget + 'static>(&mut self, widget: T) -> WidgetHandle<T> {
        let id = WidgetId(self.next_id);
        self.next_id += 1;

        let entry = WidgetEntry {
            widget: Box::new(widget),
            generation: 0,
        };

        self.widgets.insert(id, entry);

        WidgetHandle::new(id, 0)
    }

    /// Get immutable reference to widget.
    ///
    /// Returns None if:
    /// - Widget doesn't exist
    /// - Generation mismatch (widget was removed and slot reused)
    ///
    /// # Borrow Checking
    ///
    /// Takes `&self` (shared borrow).
    /// Returns `Option<&dyn Widget>` with same lifetime as `&self`.
    /// Multiple `get()` calls can coexist (all shared borrows).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if let Some(widget) = storage.get(handle) {
    ///     println!("Widget ID: {:?}", widget.id());
    /// }
    /// ```
    pub fn get<T: ?Sized>(&self, handle: WidgetHandle<T>) -> Option<&dyn Widget> {
        let entry = self.widgets.get(&handle.id())?;

        // Check generation
        if entry.generation != handle.generation() {
            return None; // Stale handle
        }

        Some(&*entry.widget)
    }

    /// Get mutable reference to widget.
    ///
    /// # Borrow Checking
    ///
    /// Takes `&mut self` (exclusive borrow).
    /// Returns `Option<&mut dyn Widget>` with same lifetime as `&mut self`.
    ///
    /// Only one `get_mut()` can exist at a time (exclusive borrow).
    /// This prevents aliasing.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if let Some(widget) = storage.get_mut(handle) {
    ///     if let Some(text) = widget.as_text_widget_mut() {
    ///         text.set_text("Updated");
    ///     }
    /// }
    /// ```
    pub fn get_mut<T: ?Sized>(&mut self, handle: WidgetHandle<T>) -> Option<&mut dyn Widget> {
        let entry = self.widgets.get_mut(&handle.id())?;

        // Check generation
        if entry.generation != handle.generation() {
            return None; // Stale handle
        }

        Some(&mut *entry.widget)
    }

    /// Remove a widget.
    ///
    /// Increments generation so existing handles become invalid.
    ///
    /// # Borrow Checking
    ///
    /// Takes `&mut self` (exclusive borrow).
    /// Returns owned `Box<dyn Widget>` (no lifetime issues).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let removed = storage.remove(handle);
    /// assert!(removed.is_some());
    ///
    /// // Old handle now invalid
    /// assert!(storage.get(handle).is_none());
    /// ```
    pub fn remove<T: ?Sized>(&mut self, handle: WidgetHandle<T>) -> Option<Box<dyn Widget>> {
        let mut entry = self.widgets.remove(&handle.id())?;

        // Check generation
        if entry.generation != handle.generation() {
            // Put it back (wrong generation)
            self.widgets.insert(handle.id(), entry);
            return None;
        }

        // Increment generation for this ID
        // (If we reuse this ID later, old handles won't match)
        entry.generation += 1;

        Some(entry.widget)
    }

    /// Check if a handle is valid.
    ///
    /// Returns true if the widget exists and generation matches.
    pub fn contains<T: ?Sized>(&self, handle: WidgetHandle<T>) -> bool {
        if let Some(entry) = self.widgets.get(&handle.id()) {
            entry.generation == handle.generation()
        } else {
            false
        }
    }

    /// Get the number of widgets in storage.
    pub fn len(&self) -> usize {
        self.widgets.len()
    }

    /// Check if storage is empty.
    pub fn is_empty(&self) -> bool {
        self.widgets.is_empty()
    }

    /// Iterate over all widgets.
    ///
    /// # Borrow Checking
    ///
    /// Takes `&self` (shared borrow).
    /// Returns iterator yielding `(WidgetId, &dyn Widget)`.
    pub fn iter(&self) -> impl Iterator<Item = (WidgetId, &dyn Widget)> {
        self.widgets
            .iter()
            .map(|(id, entry)| (*id, &*entry.widget))
    }

    /// Iterate over all widgets mutably.
    ///
    /// # Borrow Checking
    ///
    /// Takes `&mut self` (exclusive borrow).
    /// Returns iterator yielding `(WidgetId, &mut dyn Widget)`.
    ///
    /// Only one `iter_mut()` can exist at a time.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (WidgetId, &mut (dyn Widget + '_))> + '_ {
        self.widgets
            .iter_mut()
            .map(|(id, entry)| (*id, &mut *entry.widget as &mut (dyn Widget + '_)))
    }

    /// Clear all widgets from storage.
    pub fn clear(&mut self) {
        self.widgets.clear();
    }
}

impl Default for WidgetStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use astrelis_core::math::Vec2;

    // Mock widget for testing
    struct MockButton {
        id: WidgetId,
        node: taffy::NodeId,
        text: String,
    }

    impl Widget for MockButton {
        fn id(&self) -> WidgetId {
            self.id
        }

        fn layout_node(&self) -> taffy::NodeId {
            self.node
        }

        fn as_text_widget(&self) -> Option<&dyn TextWidget> {
            Some(self)
        }

        fn as_text_widget_mut(&mut self) -> Option<&mut dyn TextWidget> {
            Some(self)
        }
    }

    impl TextWidget for MockButton {
        fn text(&self) -> &str {
            &self.text
        }

        fn set_text(&mut self, text: &str) {
            self.text = text.to_string();
        }

        fn build_text_style(&self) -> astrelis_text::Text {
            astrelis_text::Text::new(&self.text)
        }

        fn text_align(&self) -> astrelis_text::TextAlign {
            astrelis_text::TextAlign::Left
        }

        fn set_text_align(&mut self, _align: astrelis_text::TextAlign) {}

        fn vertical_align(&self) -> astrelis_text::VerticalAlign {
            astrelis_text::VerticalAlign::Top
        }

        fn set_vertical_align(&mut self, _align: astrelis_text::VerticalAlign) {}
    }

    #[test]
    fn test_add_and_get() {
        let mut storage = WidgetStorage::new();

        let button = MockButton {
            id: WidgetId(0),
            node: taffy::NodeId::from(0u64),
            text: "Click".to_string(),
        };

        let handle = storage.add(button);

        // Should be able to get it
        assert!(storage.get(handle).is_some());
        assert_eq!(storage.len(), 1);
    }

    #[test]
    fn test_get_mut() {
        let mut storage = WidgetStorage::new();

        let button = MockButton {
            id: WidgetId(0),
            node: taffy::NodeId::from(0u64),
            text: "Original".to_string(),
        };

        let handle = storage.add(button);

        // Modify via mutable reference
        if let Some(widget) = storage.get_mut(handle) {
            if let Some(text_widget) = widget.as_text_widget_mut() {
                text_widget.set_text("Modified");
            }
        }

        // Verify modification
        if let Some(widget) = storage.get(handle) {
            if let Some(text_widget) = widget.as_text_widget() {
                assert_eq!(text_widget.text(), "Modified");
            }
        }
    }

    #[test]
    fn test_remove() {
        let mut storage = WidgetStorage::new();

        let button = MockButton {
            id: WidgetId(0),
            node: taffy::NodeId::from(0u64),
            text: "Click".to_string(),
        };

        let handle = storage.add(button);

        assert!(storage.contains(handle));

        // Remove widget
        let removed = storage.remove(handle);
        assert!(removed.is_some());
        assert_eq!(storage.len(), 0);

        // Handle should now be invalid
        assert!(!storage.contains(handle));
        assert!(storage.get(handle).is_none());
    }

    #[test]
    fn test_generational_safety() {
        let mut storage = WidgetStorage::new();

        let button1 = MockButton {
            id: WidgetId(0),
            node: taffy::NodeId::from(0u64),
            text: "Button 1".to_string(),
        };

        let handle1 = storage.add(button1);

        // Remove first widget
        storage.remove(handle1);

        // Old handle should be invalid
        assert!(storage.get(handle1).is_none());

        // Add another widget (might reuse ID in real implementation)
        let button2 = MockButton {
            id: WidgetId(1),
            node: taffy::NodeId::from(1u64),
            text: "Button 2".to_string(),
        };

        let handle2 = storage.add(button2);

        // Old handle should still be invalid
        assert!(storage.get(handle1).is_none());
        // New handle should work
        assert!(storage.get(handle2).is_some());
    }

    #[test]
    fn test_iteration() {
        let mut storage = WidgetStorage::new();

        // Add multiple widgets
        for i in 0..3 {
            let button = MockButton {
                id: WidgetId(i),
                node: taffy::NodeId::from(i),
                text: format!("Button {}", i),
            };
            storage.add(button);
        }

        assert_eq!(storage.len(), 3);

        // Iterate immutably
        let count = storage.iter().count();
        assert_eq!(count, 3);

        // Iterate mutably
        for (_id, widget) in storage.iter_mut() {
            if let Some(text_widget) = widget.as_text_widget_mut() {
                text_widget.set_text("Updated");
            }
        }

        // Verify all were updated
        for (_id, widget) in storage.iter() {
            if let Some(text_widget) = widget.as_text_widget() {
                assert_eq!(text_widget.text(), "Updated");
            }
        }
    }
}
