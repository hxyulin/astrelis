//! Type-safe widget handles with generational safety.
//!
//! This module provides typed handles to widgets that prevent use-after-free
//! and provide compile-time type checking.

use super::capability::*;
use std::marker::PhantomData;

/// Type-safe handle to a widget.
///
/// # Borrow Checking Pattern: Phantom Types
///
/// This handle doesn't actually contain a `T`, but uses `PhantomData<*const T>`
/// to track the type at compile time.
///
/// ## Why `PhantomData<*const T>` not `PhantomData<T>`?
///
/// - `PhantomData<T>` implies ownership (not Copy if T is not Copy)
/// - `PhantomData<*const T>` implies borrowed pointer (always Copy)
/// - Correct variance (covariant in T)
/// - No drop glue
/// - Makes it clear: this is an ID, not ownership
///
/// ## Benefits
///
/// 1. **Zero-cost**: Same size as `(WidgetId, u32)`
/// 2. **Type safety**: Can't pass wrong handle type
/// 3. **Copy**: No borrowing issues
/// 4. **Generational**: Prevents use-after-free
///
/// # Generational Safety
///
/// The generation counter prevents use-after-free:
/// - Widget removed → generation increments
/// - Old handle → generation mismatch → returns None
///
/// Similar to generational-arena or slotmap crates.
///
/// # Example
///
/// ```rust,ignore
/// use astrelis_ui::widget::{WidgetHandle, Button, TextWidget};
///
/// // Create a button - returns typed handle
/// let button: WidgetHandle<Button> = ui.add(Button::new("Click me"));
///
/// // Can pass handle by value (it's Copy)
/// update_button(button);
///
/// // Can upcast to trait
/// let text_handle: WidgetHandle<dyn TextWidget> = button.upcast();
///
/// // Compile-time type safety!
/// let rect: WidgetHandle<Rect> = ui.add(Rect::new());
/// // update_button(rect);  // ERROR: expected Button, found Rect
/// ```
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct WidgetHandle<T: ?Sized> {
    id: WidgetId,
    generation: u32,
    _phantom: PhantomData<*const T>,
}

// Manually implement Clone and Copy since PhantomData<*const T> is Copy
impl<T: ?Sized> Clone for WidgetHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for WidgetHandle<T> {}

// SAFETY: WidgetHandle is just an ID + generation, no actual T pointer
unsafe impl<T: ?Sized> Send for WidgetHandle<T> {}
unsafe impl<T: ?Sized> Sync for WidgetHandle<T> {}

impl<T: ?Sized> WidgetHandle<T> {
    /// Create a new handle (internal use only).
    ///
    /// This should only be called by WidgetStorage.
    pub(crate) fn new(id: WidgetId, generation: u32) -> Self {
        Self {
            id,
            generation,
            _phantom: PhantomData,
        }
    }

    /// Get the widget ID.
    pub fn id(&self) -> WidgetId {
        self.id
    }

    /// Get the generation.
    pub fn generation(&self) -> u32 {
        self.generation
    }

    // TODO: Implement upcast/downcast once Unsize trait is stabilized
    // or use a different approach (e.g., manual trait object coercion)
    //
    // /// Upcast to a trait object handle.
    // pub fn upcast<U: ?Sized>(self) -> WidgetHandle<U>
    // where
    //     T: std::marker::Unsize<U>,
    // {
    //     WidgetHandle {
    //         id: self.id,
    //         generation: self.generation,
    //         _phantom: PhantomData,
    //     }
    // }
}

/// Type alias for a handle to any widget.
pub type AnyWidgetHandle = WidgetHandle<dyn Widget>;

/// Type alias for a handle to a text widget.
pub type TextWidgetHandle = WidgetHandle<dyn TextWidget>;

/// Type alias for a handle to a parent widget.
pub type ParentWidgetHandle = WidgetHandle<dyn ParentWidget>;

/// Type alias for a handle to a color widget.
pub type ColorWidgetHandle = WidgetHandle<dyn ColorWidget>;

#[cfg(test)]
mod tests {
    use super::*;

    // Mock widget types for testing
    #[derive(Debug, PartialEq)]
    struct Button {
        id: WidgetId,
    }

    impl Widget for Button {
        fn id(&self) -> WidgetId {
            self.id
        }

        fn layout_node(&self) -> taffy::NodeId {
            taffy::NodeId::from(0u64)
        }

        fn as_text_widget(&self) -> Option<&dyn TextWidget> {
            Some(self)
        }
    }

    impl TextWidget for Button {
        fn text(&self) -> &str {
            "button"
        }

        fn set_text(&mut self, _text: &str) {}

        fn build_text_style(&self) -> astrelis_text::Text {
            astrelis_text::Text::new("button")
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
    fn test_handle_copy() {
        let handle = WidgetHandle::<Button>::new(WidgetId(1), 0);
        let handle2 = handle; // Copy, not move
        let handle3 = handle; // handle still valid!

        assert_eq!(handle.id(), handle2.id());
        assert_eq!(handle.id(), handle3.id());
    }

    #[test]
    fn test_handle_generation() {
        let handle1 = WidgetHandle::<Button>::new(WidgetId(1), 0);
        let handle2 = WidgetHandle::<Button>::new(WidgetId(1), 1);

        assert_eq!(handle1.id(), handle2.id());
        assert_ne!(handle1.generation(), handle2.generation());
        assert_ne!(handle1, handle2);
    }

    // TODO: Re-enable once upcast is implemented
    // #[test]
    // fn test_handle_upcast() {
    //     let button_handle = WidgetHandle::<Button>::new(WidgetId(1), 0);
    //
    //     // Upcast to trait
    //     let text_handle: WidgetHandle<dyn TextWidget> = button_handle.upcast();
    //     let widget_handle: WidgetHandle<dyn Widget> = button_handle.upcast();
    //
    //     // All have same ID
    //     assert_eq!(button_handle.id(), text_handle.id());
    //     assert_eq!(button_handle.id(), widget_handle.id());
    // }

    // TODO: Re-enable once upcast is implemented
    // #[test]
    // fn test_type_aliases() {
    //     let handle = WidgetHandle::<Button>::new(WidgetId(1), 0);
    //
    //     let _any: AnyWidgetHandle = handle.upcast();
    //     let _text: TextWidgetHandle = handle.upcast();
    //
    //     // Type aliases work as expected
    // }
}
