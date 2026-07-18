//! One-chain node construction over the retained core.
//!
//! `astrelis-ui-core` splits authoring into a create call plus a series of
//! configure calls, each returning a `Result` that construction code threads
//! through `?` or `map_err`. On the hot construction path a stale parent handle
//! is a programmer error, not a runtime condition, so the facade panics with a
//! clear message instead of returning `Result`, and folds the configure calls
//! into a single chain committed once.
//!
//! ```ignore
//! let scroll = ui.padding(root, Insets::all(28.0)).grow(1.0)
//!     .scroll_view().grow(1.0).finish();
//! ```
//!
//! Callers that need the fallible API keep using `astrelis-ui-core` directly.

use astrelis_core::{geometry::LogicalPoint, math::Affine2};
use astrelis_platform::CursorIcon;
use astrelis_ui_core::{
    Button, Checkbox, Column, ElementHandle, FlexStyle, Insets, LayoutStyle, Length, Overflow,
    Overlay, OverlayOptions, Padding, Row, ScrollView, Slider, Stack, TextField, Ui, Visibility,
    Widget, WidgetStyle,
};

use crate::layout::LayoutExt;

/// A just-created node whose configuration is applied when the chain finishes.
///
/// Layout, flex, visual style, wrapping, and enablement accumulate on the
/// builder and commit exactly once — when [`Node::finish`] returns the handle,
/// or when a child method descends into a new node. Dropping a builder without
/// finishing leaves its pending configuration unapplied; the `must_use` lint
/// flags that so it is hard to do by accident.
#[must_use = "a node builder applies its configuration only when finished; call `.finish()` or a child method"]
pub struct Node<'ui, Message: 'static, T> {
    ui: &'ui mut Ui<Message>,
    handle: ElementHandle<T>,
    layout: LayoutStyle,
    layout_dirty: bool,
    flex: Option<FlexStyle>,
    style: Option<WidgetStyle>,
    wrap: Option<bool>,
    enabled: Option<bool>,
    overflow: Option<Overflow>,
    z_index: Option<i32>,
    visibility: Option<Visibility>,
    cursor: Option<Option<CursorIcon>>,
    transform: Option<(Affine2, LogicalPoint)>,
}

impl<'ui, Message: 'static, T> Node<'ui, Message, T> {
    fn new(ui: &'ui mut Ui<Message>, handle: ElementHandle<T>) -> Self {
        Self {
            ui,
            handle,
            layout: LayoutStyle::default(),
            layout_dirty: false,
            flex: None,
            style: None,
            wrap: None,
            enabled: None,
            overflow: None,
            z_index: None,
            visibility: None,
            cursor: None,
            transform: None,
        }
    }

    /// Replaces the whole layout style, e.g. from [`crate::layout`].
    pub fn layout(mut self, layout: LayoutStyle) -> Self {
        self.layout = layout;
        self.layout_dirty = true;
        self
    }

    /// Sets the preferred width.
    pub fn width(self, width: Length) -> Self {
        self.map_layout(|layout| layout.width(width))
    }

    /// Sets the preferred height.
    pub fn height(self, height: Length) -> Self {
        self.map_layout(|layout| layout.height(height))
    }

    /// Sets the minimum width.
    pub fn min_width(self, width: Length) -> Self {
        self.map_layout(|layout| layout.min_width(width))
    }

    /// Sets the minimum height.
    pub fn min_height(self, height: Length) -> Self {
        self.map_layout(|layout| layout.min_height(height))
    }

    /// Sets the maximum width.
    pub fn max_width(self, width: Length) -> Self {
        self.map_layout(|layout| layout.max_width(width))
    }

    /// Sets the maximum height.
    pub fn max_height(self, height: Length) -> Self {
        self.map_layout(|layout| layout.max_height(height))
    }

    /// Sets the flex growth factor.
    pub fn grow(self, factor: f32) -> Self {
        self.map_layout(|layout| layout.grow(factor))
    }

    /// Sets the flex shrink factor.
    pub fn shrink(self, factor: f32) -> Self {
        self.map_layout(|layout| layout.shrink(factor))
    }

    /// Sets a uniform margin on every edge.
    pub fn margin(self, margin: Length) -> Self {
        self.map_layout(|layout| layout.margin(margin))
    }

    /// Configures this container's flex behaviour.
    pub fn flex(mut self, flex: FlexStyle) -> Self {
        self.flex = Some(flex);
        self
    }

    /// Overrides this element's visual style.
    pub fn style(mut self, style: WidgetStyle) -> Self {
        self.style = Some(style);
        self
    }

    /// Enables or disables text wrapping within the element's max width.
    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = Some(wrap);
        self
    }

    /// Enables or disables the element and its subtree.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = Some(enabled);
        self
    }

    /// Sets whether descendants are clipped to this element's bounds.
    pub fn overflow(mut self, overflow: Overflow) -> Self {
        self.overflow = Some(overflow);
        self
    }

    /// Sets the stable paint and hit-test order among siblings.
    pub fn z_index(mut self, z_index: i32) -> Self {
        self.z_index = Some(z_index);
        self
    }

    /// Sets layout/paint visibility for this element and its subtree.
    pub fn visibility(mut self, visibility: Visibility) -> Self {
        self.visibility = Some(visibility);
        self
    }

    /// Overrides the cursor shown while this element is hovered.
    pub fn cursor_icon(mut self, cursor: Option<CursorIcon>) -> Self {
        self.cursor = Some(cursor);
        self
    }

    /// Applies a paint-only transform about a logical origin, without
    /// disturbing layout.
    pub fn transform(mut self, transform: Affine2, origin: LogicalPoint) -> Self {
        self.transform = Some((transform, origin));
        self
    }

    fn map_layout(mut self, edit: impl FnOnce(LayoutStyle) -> LayoutStyle) -> Self {
        self.layout = edit(self.layout);
        self.layout_dirty = true;
        self
    }

    fn commit(&mut self) {
        if self.layout_dirty {
            self.ui
                .set_layout(self.handle, self.layout)
                .expect("set_layout on a live handle");
            self.layout_dirty = false;
        }
        if let Some(flex) = self.flex.take() {
            self.ui
                .set_flex_style(self.handle, flex)
                .expect("set_flex_style on a live handle");
        }
        if let Some(style) = self.style.take() {
            self.ui
                .set_widget_style(self.handle, style)
                .expect("set_widget_style on a live handle");
        }
        if let Some(wrap) = self.wrap.take() {
            self.ui
                .set_wrap(self.handle, wrap)
                .expect("set_wrap on a live handle");
        }
        if let Some(enabled) = self.enabled.take() {
            self.ui
                .set_enabled(self.handle, enabled)
                .expect("set_enabled on a live handle");
        }
        if let Some(overflow) = self.overflow.take() {
            self.ui
                .set_overflow(self.handle, overflow)
                .expect("set_overflow on a live handle");
        }
        if let Some(z_index) = self.z_index.take() {
            self.ui
                .set_z_index(self.handle, z_index)
                .expect("set_z_index on a live handle");
        }
        if let Some(visibility) = self.visibility.take() {
            self.ui
                .set_visibility(self.handle, visibility)
                .expect("set_visibility on a live handle");
        }
        if let Some(cursor) = self.cursor.take() {
            self.ui
                .set_cursor_icon(self.handle, cursor)
                .expect("set_cursor_icon on a live handle");
        }
        if let Some((transform, origin)) = self.transform.take() {
            self.ui
                .set_transform(self.handle, transform, origin)
                .expect("set_transform on a live handle");
        }
    }

    /// Commits the accumulated configuration and returns the element handle.
    pub fn finish(mut self) -> ElementHandle<T> {
        self.commit();
        self.handle
    }

    /// Commits and returns the handle plus a borrow of the UI, so children can
    /// be added to this node while keeping a reference to it.
    pub fn build(mut self) -> (ElementHandle<T>, &'ui mut Ui<Message>) {
        self.commit();
        (self.handle, self.ui)
    }
}

/// Child methods: commit this node, add a child under it, and descend.
impl<'ui, Message: 'static, T> Node<'ui, Message, T> {
    fn descend<C>(
        mut self,
        add: impl FnOnce(&mut Ui<Message>, ElementHandle<T>) -> ElementHandle<C>,
    ) -> Node<'ui, Message, C> {
        self.commit();
        let child = add(self.ui, self.handle);
        Node::new(self.ui, child)
    }

    /// Adds a child column and descends into it.
    pub fn column(self) -> Node<'ui, Message, Column> {
        self.descend(|ui, parent| ui.add_column(parent).expect("add_column on a live handle"))
    }

    /// Adds a child row and descends into it.
    pub fn row(self) -> Node<'ui, Message, Row> {
        self.descend(|ui, parent| ui.add_row(parent).expect("add_row on a live handle"))
    }

    /// Adds a child overlaying stack and descends into it.
    pub fn stack(self) -> Node<'ui, Message, Stack> {
        self.descend(|ui, parent| ui.add_stack(parent).expect("add_stack on a live handle"))
    }

    /// Adds a child padding container and descends into it.
    pub fn padding(self, insets: Insets) -> Node<'ui, Message, Padding> {
        self.descend(move |ui, parent| {
            ui.add_padding(parent, insets)
                .expect("add_padding on a live handle")
        })
    }

    /// Adds a child scroll view and descends into it.
    pub fn scroll_view(self) -> Node<'ui, Message, ScrollView> {
        self.descend(|ui, parent| {
            ui.add_scroll_view(parent)
                .expect("add_scroll_view on a live handle")
        })
    }

    /// Adds a child label and descends into it.
    pub fn label(self, text: impl Into<String>) -> Node<'ui, Message, astrelis_ui_core::Label> {
        let text = text.into();
        self.descend(move |ui, parent| {
            ui.add_label(parent, text)
                .expect("add_label on a live handle")
        })
    }

    /// Adds a child button and descends into it.
    pub fn button(self, text: impl Into<String>) -> Node<'ui, Message, Button> {
        let text = text.into();
        self.descend(move |ui, parent| {
            ui.add_button(parent, text)
                .expect("add_button on a live handle")
        })
    }

    /// Adds a viewport-hosted overlay owned by this element and descends into
    /// it. The overlay is logically owned by this node but painted at the
    /// viewport root; anchor it with [`OverlayOptions`].
    pub fn overlay(self, options: OverlayOptions) -> Node<'ui, Message, Overlay> {
        self.descend(move |ui, owner| {
            ui.add_overlay(owner, options)
                .expect("add_overlay on a live handle")
        })
    }
}

/// Text-field-only configuration, applied eagerly since these are independent
/// core mutations rather than part of the batched layout/style commit.
impl<Message: 'static> Node<'_, Message, TextField> {
    /// Sets the placeholder shown while the field is empty.
    pub fn placeholder(self, placeholder: impl Into<String>) -> Self {
        self.ui
            .set_placeholder(self.handle, placeholder)
            .expect("set_placeholder on a live handle");
        self
    }

    /// Selects password-purpose display and IME behaviour.
    pub fn password(self, password: bool) -> Self {
        self.ui
            .set_password(self.handle, password)
            .expect("set_password on a live handle");
        self
    }
}

/// Infallible, chainable node creation on [`Ui`].
///
/// Each method creates a child of `parent` and returns a [`Node`] builder for
/// it. Panics if `parent` is stale — an impossible-in-practice error on the
/// construction path; use `astrelis-ui-core`'s `add_*` for the fallible API.
pub trait Build<Message: 'static> {
    /// Starts a builder chain from an existing handle without creating a node.
    fn at<T>(&mut self, handle: ElementHandle<T>) -> Node<'_, Message, T>;
    /// Adds a column.
    fn column<T>(&mut self, parent: ElementHandle<T>) -> Node<'_, Message, Column>;
    /// Adds a row.
    fn row<T>(&mut self, parent: ElementHandle<T>) -> Node<'_, Message, Row>;
    /// Adds an overlaying stack.
    fn stack<T>(&mut self, parent: ElementHandle<T>) -> Node<'_, Message, Stack>;
    /// Adds a padding container.
    fn padding<T>(
        &mut self,
        parent: ElementHandle<T>,
        insets: Insets,
    ) -> Node<'_, Message, Padding>;
    /// Adds a scroll view.
    fn scroll_view<T>(&mut self, parent: ElementHandle<T>) -> Node<'_, Message, ScrollView>;
    /// Adds a label.
    fn label<T>(
        &mut self,
        parent: ElementHandle<T>,
        text: impl Into<String>,
    ) -> Node<'_, Message, astrelis_ui_core::Label>;
    /// Adds a button.
    fn button<T>(
        &mut self,
        parent: ElementHandle<T>,
        text: impl Into<String>,
    ) -> Node<'_, Message, Button>;
    /// Adds a single-line text field.
    fn text_field<T>(
        &mut self,
        parent: ElementHandle<T>,
        text: impl Into<String>,
    ) -> Node<'_, Message, TextField>;
    /// Adds a checkbox.
    fn checkbox<T>(
        &mut self,
        parent: ElementHandle<T>,
        checked: bool,
    ) -> Node<'_, Message, Checkbox>;
    /// Adds a horizontal slider.
    fn slider<T>(
        &mut self,
        parent: ElementHandle<T>,
        min: f32,
        max: f32,
        step: f32,
        value: f32,
    ) -> Node<'_, Message, Slider>;
    /// Mounts an application-defined widget.
    ///
    /// Named `mount` rather than `widget` because `astrelis-ui-core`'s
    /// inherent `Ui::widget` reader would shadow a trait method of that name.
    fn mount<T, W: Widget<Message>>(
        &mut self,
        parent: ElementHandle<T>,
        widget: W,
    ) -> Node<'_, Message, W>;
    /// Adds a viewport-hosted overlay owned by `owner`.
    fn overlay<T>(
        &mut self,
        owner: ElementHandle<T>,
        options: OverlayOptions,
    ) -> Node<'_, Message, Overlay>;
}

impl<Message: 'static> Build<Message> for Ui<Message> {
    fn at<T>(&mut self, handle: ElementHandle<T>) -> Node<'_, Message, T> {
        Node::new(self, handle)
    }

    fn column<T>(&mut self, parent: ElementHandle<T>) -> Node<'_, Message, Column> {
        let handle = self
            .add_column(parent)
            .expect("add_column on a live handle");
        Node::new(self, handle)
    }

    fn row<T>(&mut self, parent: ElementHandle<T>) -> Node<'_, Message, Row> {
        let handle = self.add_row(parent).expect("add_row on a live handle");
        Node::new(self, handle)
    }

    fn stack<T>(&mut self, parent: ElementHandle<T>) -> Node<'_, Message, Stack> {
        let handle = self.add_stack(parent).expect("add_stack on a live handle");
        Node::new(self, handle)
    }

    fn padding<T>(
        &mut self,
        parent: ElementHandle<T>,
        insets: Insets,
    ) -> Node<'_, Message, Padding> {
        let handle = self
            .add_padding(parent, insets)
            .expect("add_padding on a live handle");
        Node::new(self, handle)
    }

    fn scroll_view<T>(&mut self, parent: ElementHandle<T>) -> Node<'_, Message, ScrollView> {
        let handle = self
            .add_scroll_view(parent)
            .expect("add_scroll_view on a live handle");
        Node::new(self, handle)
    }

    fn label<T>(
        &mut self,
        parent: ElementHandle<T>,
        text: impl Into<String>,
    ) -> Node<'_, Message, astrelis_ui_core::Label> {
        let handle = self
            .add_label(parent, text)
            .expect("add_label on a live handle");
        Node::new(self, handle)
    }

    fn button<T>(
        &mut self,
        parent: ElementHandle<T>,
        text: impl Into<String>,
    ) -> Node<'_, Message, Button> {
        let handle = self
            .add_button(parent, text)
            .expect("add_button on a live handle");
        Node::new(self, handle)
    }

    fn text_field<T>(
        &mut self,
        parent: ElementHandle<T>,
        text: impl Into<String>,
    ) -> Node<'_, Message, TextField> {
        let handle = self
            .add_text_field(parent, text)
            .expect("add_text_field on a live handle");
        Node::new(self, handle)
    }

    fn checkbox<T>(
        &mut self,
        parent: ElementHandle<T>,
        checked: bool,
    ) -> Node<'_, Message, Checkbox> {
        let handle = self
            .add_checkbox(parent, checked)
            .expect("add_checkbox on a live handle");
        Node::new(self, handle)
    }

    fn slider<T>(
        &mut self,
        parent: ElementHandle<T>,
        min: f32,
        max: f32,
        step: f32,
        value: f32,
    ) -> Node<'_, Message, Slider> {
        let handle = self
            .add_slider(parent, min, max, step, value)
            .expect("add_slider on a live handle");
        Node::new(self, handle)
    }

    fn mount<T, W: Widget<Message>>(
        &mut self,
        parent: ElementHandle<T>,
        widget: W,
    ) -> Node<'_, Message, W> {
        let handle = self
            .add_widget(parent, widget)
            .expect("add_widget on a live handle");
        Node::new(self, handle)
    }

    fn overlay<T>(
        &mut self,
        owner: ElementHandle<T>,
        options: OverlayOptions,
    ) -> Node<'_, Message, Overlay> {
        let handle = self
            .add_overlay(owner, options)
            .expect("add_overlay on a live handle");
        Node::new(self, handle)
    }
}
