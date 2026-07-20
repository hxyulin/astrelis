//! Retained element property accessors and mutators.

use super::*;

impl<Message: 'static> Ui<Message> {
    /// Moves keyboard focus to an eligible retained element.
    pub fn focus<T>(&mut self, handle: ElementHandle<T>) -> Result<(), UiError> {
        self.set_focus(Some(handle.id))
    }

    /// Changes the gap and cross-axis alignment of a row or column.
    pub fn set_flex<T>(
        &mut self,
        handle: ElementHandle<T>,
        gap: f32,
        alignment: Alignment,
    ) -> Result<(), UiError> {
        let node = self.node_mut(handle.id)?;
        match &mut node.kind {
            Kind::Row { flex } | Kind::Column { flex } => {
                flex.column_gap = gap.max(0.0);
                flex.row_gap = gap.max(0.0);
                flex.align_items = alignment;
                self.invalidate_node(handle.id, Dirty::all());
                Ok(())
            }
            _ => Err(UiError::new("element is not a row or column")),
        }
    }

    /// Replaces a row or column's complete flex-container configuration.
    pub fn set_flex_style<T>(
        &mut self,
        handle: ElementHandle<T>,
        style: FlexStyle,
    ) -> Result<(), UiError> {
        let node = self.node_mut(handle.id)?;
        match &mut node.kind {
            Kind::Row { flex } | Kind::Column { flex } => *flex = style,
            _ => return Err(UiError::new("element is not a row or column")),
        }
        self.invalidate_node(handle.id, Dirty::all());
        Ok(())
    }

    pub(crate) fn remove_subtree(&mut self, id: ElementId) {
        let children = self
            .node(id)
            .map(|node| node.children.clone())
            .unwrap_or_default();
        for child in children {
            self.remove_subtree(child);
        }
        if self.focus == Some(id) {
            self.focus = None;
        }
        if self.hover == Some(id) {
            self.hover = None;
        }
        for path in self.hover_paths.values_mut() {
            path.retain(|hovered| *hovered != id);
        }
        self.capture.retain(|_, captured| *captured != id);
        self.listeners.remove(&id);
        self.semantic_roles.remove(&id);
        self.semantic_descriptions.remove(&id);
        self.semantic_invalid.remove(&id);
        self.semantic_live.remove(&id);
        self.semantic_selected.remove(&id);
        self.semantic_expanded.remove(&id);
        if let Some(mut widget) = self.custom_widgets.remove(&id) {
            widget.unmounted();
        }
        let slot = &mut self.slots[id.index as usize];
        slot.node = None;
        self.free.push(id.index);
    }

    /// Changes an element's sizing constraints.
    pub fn set_layout<T>(
        &mut self,
        handle: ElementHandle<T>,
        style: LayoutStyle,
    ) -> Result<(), UiError> {
        let lengths = [
            style.width,
            style.height,
            style.min_width,
            style.min_height,
            style.max_width,
            style.max_height,
            style.margin.left,
            style.margin.top,
            style.margin.right,
            style.margin.bottom,
            style.basis,
            style.inset.left,
            style.inset.top,
            style.inset.right,
            style.inset.bottom,
        ];
        if lengths.into_iter().any(|value| match value {
            Length::Auto => false,
            Length::Px(value) | Length::Percent(value) => !value.is_finite(),
        }) || !style.grow.is_finite()
            || !style.shrink.is_finite()
            || style
                .aspect_ratio
                .is_some_and(|value| !value.is_finite() || value <= 0.0)
        {
            return Err(UiError::new(
                "layout values must be finite and aspect ratios positive",
            ));
        }
        let node = self.node_mut(handle.id)?;
        if node.style != style {
            node.style = style;
            self.invalidate_node(handle.id, Dirty::all());
        }
        Ok(())
    }

    /// Returns an element's current sizing constraints.
    pub fn layout<T>(&self, handle: ElementHandle<T>) -> Result<LayoutStyle, UiError> {
        Ok(self.node(handle.id)?.style)
    }

    /// Returns an element's declared visual overrides.
    pub fn widget_style<T>(&self, handle: ElementHandle<T>) -> Result<WidgetStyle, UiError> {
        Ok(self.node(handle.id)?.visual)
    }

    /// Replaces a padding container's insets.
    pub fn set_padding_insets(
        &mut self,
        handle: ElementHandle<Padding>,
        insets: Insets,
    ) -> Result<(), UiError> {
        if !insets.left.is_finite()
            || !insets.top.is_finite()
            || !insets.right.is_finite()
            || !insets.bottom.is_finite()
        {
            return Err(UiError::new("padding insets must be finite"));
        }
        let node = self.node_mut(handle.id)?;
        match &mut node.kind {
            Kind::Padding { insets: current } if *current != insets => *current = insets,
            Kind::Padding { .. } => return Ok(()),
            _ => return Err(UiError::new("element is not a padding container")),
        }
        self.invalidate_node(handle.id, Dirty::all());
        Ok(())
    }

    /// Replaces an overlay's placement options.
    pub fn set_overlay_options(
        &mut self,
        handle: ElementHandle<Overlay>,
        options: OverlayOptions,
    ) -> Result<(), UiError> {
        let node = self.node_mut(handle.id)?;
        let Kind::Overlay {
            options: current, ..
        } = &mut node.kind
        else {
            return Err(UiError::new("element is not an overlay"));
        };
        if *current != options {
            *current = options;
            node.z_index = options.z_index;
            self.invalidate_layout();
        }
        Ok(())
    }

    /// Applies direct foreground and background overrides to one widget.
    pub fn set_widget_style<T>(
        &mut self,
        handle: ElementHandle<T>,
        style: WidgetStyle,
    ) -> Result<(), UiError> {
        let node = self.node_mut(handle.id)?;
        if node.visual != style {
            node.visual = style;
            self.invalidate_node(handle.id, Dirty::MEASURE | Dirty::PAINT);
        }
        Ok(())
    }

    /// Sets whether an element's text wraps within its max width.
    ///
    /// When enabled, text is broken to fit the node's configured maximum width
    /// (`LayoutStyle::max_width`), falling back to the viewport width when none
    /// is set. Wrapping is resolved at shape time, so it tracks a fixed maximum
    /// rather than the width a flex parent happens to hand out.
    pub fn set_wrap<T>(&mut self, handle: ElementHandle<T>, wrap: bool) -> Result<(), UiError> {
        let node = self.node_mut(handle.id)?;
        if node.wrap != wrap {
            node.wrap = wrap;
            self.invalidate_node(handle.id, Dirty::MEASURE | Dirty::LAYOUT | Dirty::PAINT);
        }
        Ok(())
    }

    /// Enables or disables an element.
    pub fn set_enabled<T>(
        &mut self,
        handle: ElementHandle<T>,
        enabled: bool,
    ) -> Result<(), UiError> {
        let changed = self.node(handle.id)?.enabled != enabled;
        if changed {
            self.node_mut(handle.id)?.enabled = enabled;
            // Enabled state participates in the cached text request's color,
            // so text-bearing nodes must reshape as well as repaint.
            self.invalidate_node(handle.id, Dirty::MEASURE | Dirty::PAINT | Dirty::SEMANTICS);
            if !enabled
                && self
                    .focus
                    .is_some_and(|focus| self.is_descendant_of(focus, handle.id))
            {
                self.set_focus(None)?;
            }
        }
        Ok(())
    }

    /// Changes layout/paint visibility for an element and its subtree.
    pub fn set_visibility<T>(
        &mut self,
        handle: ElementHandle<T>,
        visibility: Visibility,
    ) -> Result<(), UiError> {
        let changed = self.node(handle.id)?.visibility != visibility;
        if changed {
            if visibility == Visibility::Visible {
                let current_focus = self.focus;
                match &mut self.node_mut(handle.id)?.kind {
                    Kind::FocusScope { options, restore } if options.restore_focus => {
                        *restore = current_focus;
                    }
                    Kind::Overlay {
                        options, restore, ..
                    } if options.focus.restore_focus => {
                        *restore = current_focus;
                    }
                    _ => {}
                }
            }
            let restore = match self.node(handle.id)?.kind {
                Kind::FocusScope { restore, .. } | Kind::Overlay { restore, .. } => restore,
                _ => None,
            };
            let restore_focus = (visibility == Visibility::Hidden
                && self
                    .focus
                    .is_some_and(|focus| self.is_descendant_of(focus, handle.id)))
            .then_some(restore)
            .flatten();
            self.node_mut(handle.id)?.visibility = visibility;
            self.invalidate_layout();
            if visibility == Visibility::Hidden {
                self.set_focus(restore_focus)?;
            }
        }
        Ok(())
    }

    /// Changes whether descendants are clipped to an element's bounds.
    pub fn set_overflow<T>(
        &mut self,
        handle: ElementHandle<T>,
        overflow: Overflow,
    ) -> Result<(), UiError> {
        let node = self.node_mut(handle.id)?;
        if node.overflow != overflow {
            node.overflow = overflow;
            self.dirty |= Dirty::PAINT;
        }
        Ok(())
    }

    /// Changes stable paint and targeting order among siblings.
    pub fn set_z_index<T>(
        &mut self,
        handle: ElementHandle<T>,
        z_index: i32,
    ) -> Result<(), UiError> {
        let node = self.node_mut(handle.id)?;
        if node.z_index != z_index {
            node.z_index = z_index;
            self.dirty |= Dirty::PAINT;
        }
        Ok(())
    }

    /// Applies a visual transform around a logical origin without affecting layout.
    pub fn set_transform<T>(
        &mut self,
        handle: ElementHandle<T>,
        transform: Affine2,
        origin: LogicalPoint,
    ) -> Result<(), UiError> {
        let node = self.node_mut(handle.id)?;
        if node.transform != transform || node.transform_origin != origin {
            node.transform = transform;
            node.transform_origin = origin;
            self.dirty |= Dirty::PAINT;
        }
        Ok(())
    }

    /// Overrides the cursor selected while this element is hovered.
    pub fn set_cursor_icon<T>(
        &mut self,
        handle: ElementHandle<T>,
        cursor: Option<CursorIcon>,
    ) -> Result<(), UiError> {
        self.node_mut(handle.id)?.cursor = cursor;
        Ok(())
    }

    /// Replaces label text.
    pub fn set_label_text(
        &mut self,
        handle: ElementHandle<Label>,
        text: impl Into<String>,
    ) -> Result<(), UiError> {
        self.set_static_text(handle.id, text.into(), false)
    }

    /// Replaces button text.
    pub fn set_button_text(
        &mut self,
        handle: ElementHandle<Button>,
        text: impl Into<String>,
    ) -> Result<(), UiError> {
        self.set_static_text(handle.id, text.into(), true)
    }

    pub(crate) fn set_static_text(
        &mut self,
        id: ElementId,
        value: String,
        button: bool,
    ) -> Result<(), UiError> {
        let node = self.node_mut(id)?;
        let text = match &mut node.kind {
            Kind::Label { text } if !button => text,
            Kind::Button { text } if button => text,
            _ => return Err(UiError::new("handle has the wrong widget type")),
        };
        if *text != value {
            *text = value;
            self.invalidate_node(id, Dirty::all());
        }
        Ok(())
    }

    /// Replaces text-field content and collapses selection at the end.
    pub fn set_text(
        &mut self,
        handle: ElementHandle<TextField>,
        text: impl Into<String>,
    ) -> Result<(), UiError> {
        let text = text.into();
        let node = self.node_mut(handle.id)?;
        let Kind::TextField(field) = &mut node.kind else {
            return Err(UiError::new("handle has the wrong widget type"));
        };
        if field.text != text {
            field.text = text;
            field.caret.byte_index = field.text.len();
            field.anchor = field.caret;
            self.invalidate_node(handle.id, Dirty::all());
        }
        Ok(())
    }

    /// Returns text-field content.
    pub fn text(&self, handle: ElementHandle<TextField>) -> Result<&str, UiError> {
        match &self.node(handle.id)?.kind {
            Kind::TextField(field) => Ok(&field.text),
            _ => Err(UiError::new("handle has the wrong widget type")),
        }
    }

    /// Changes text-field placeholder text.
    pub fn set_placeholder(
        &mut self,
        handle: ElementHandle<TextField>,
        placeholder: impl Into<String>,
    ) -> Result<(), UiError> {
        let placeholder = placeholder.into();
        let node = self.node_mut(handle.id)?;
        let Kind::TextField(field) = &mut node.kind else {
            return Err(UiError::new("handle has the wrong widget type"));
        };
        if field.placeholder != placeholder {
            field.placeholder = placeholder;
            self.invalidate_node(handle.id, Dirty::all());
        }
        Ok(())
    }

    /// Selects password-purpose display and IME behavior.
    pub fn set_password(
        &mut self,
        handle: ElementHandle<TextField>,
        password: bool,
    ) -> Result<(), UiError> {
        let node = self.node_mut(handle.id)?;
        let Kind::TextField(field) = &mut node.kind else {
            return Err(UiError::new("handle has the wrong widget type"));
        };
        if field.password != password {
            field.password = password;
            self.invalidate_node(handle.id, Dirty::all());
        }
        Ok(())
    }

    /// Returns a checkbox's retained value.
    pub fn checked(&self, handle: ElementHandle<Checkbox>) -> Result<bool, UiError> {
        match self.node(handle.id)?.kind {
            Kind::Checkbox { checked, .. } => Ok(checked),
            _ => Err(UiError::new("handle has the wrong widget type")),
        }
    }

    /// Sets a checkbox's retained value.
    pub fn set_checked(
        &mut self,
        handle: ElementHandle<Checkbox>,
        checked: bool,
    ) -> Result<(), UiError> {
        let Kind::Checkbox {
            checked: current, ..
        } = &mut self.node_mut(handle.id)?.kind
        else {
            return Err(UiError::new("handle has the wrong widget type"));
        };
        if *current != checked {
            *current = checked;
            self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
        }
        Ok(())
    }

    /// Returns a slider's retained value.
    pub fn slider_value(&self, handle: ElementHandle<Slider>) -> Result<f32, UiError> {
        match self.node(handle.id)?.kind {
            Kind::Slider { value, .. } => Ok(value),
            _ => Err(UiError::new("handle has the wrong widget type")),
        }
    }

    /// Sets and snaps a slider's retained value.
    pub fn set_slider_value(
        &mut self,
        handle: ElementHandle<Slider>,
        value: f32,
    ) -> Result<(), UiError> {
        let Kind::Slider {
            min,
            max,
            step,
            value: current,
            ..
        } = &mut self.node_mut(handle.id)?.kind
        else {
            return Err(UiError::new("handle has the wrong widget type"));
        };
        let value = snap_slider(value, *min, *max, *step);
        if *current != value {
            *current = value;
            self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
        }
        Ok(())
    }

    /// Returns a scroll view's vertical offset.
    pub fn scroll_offset(&self, handle: ElementHandle<ScrollView>) -> Result<f32, UiError> {
        match self.node(handle.id)?.kind {
            Kind::ScrollView { offset, .. } => Ok(offset),
            _ => Err(UiError::new("handle has the wrong widget type")),
        }
    }

    /// Sets a vertical scroll view's logical offset, clamped to its content.
    pub fn set_scroll_offset(
        &mut self,
        handle: ElementHandle<ScrollView>,
        offset: f32,
    ) -> Result<(), UiError> {
        self.set_scroll_offset_id(handle.id, offset)
    }

    pub(crate) fn set_scroll_offset_id(
        &mut self,
        id: ElementId,
        offset: f32,
    ) -> Result<(), UiError> {
        let (height, content_height) = {
            let node = self.node(id)?;
            let Kind::ScrollView { content_height, .. } = node.kind else {
                return Err(UiError::new("element is not a scroll view"));
            };
            (node.bounds.size.height, content_height)
        };
        let offset = offset.clamp(0.0, (content_height - height).max(0.0));
        let Kind::ScrollView {
            offset: current, ..
        } = &mut self.node_mut(id)?.kind
        else {
            unreachable!("kind was checked above")
        };
        if *current != offset {
            *current = offset;
            self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
        }
        Ok(())
    }

    /// Replaces a checkbox's visual overrides.
    ///
    /// Unset (`None`) fields continue to track the active theme; see
    /// [`CheckboxStyle`].
    pub fn set_checkbox_style(
        &mut self,
        handle: ElementHandle<Checkbox>,
        style: CheckboxStyle,
    ) -> Result<(), UiError> {
        match &mut self.node_mut(handle.id)?.kind {
            Kind::Checkbox { style: current, .. } => *current = style,
            _ => return Err(UiError::new("element is not a checkbox")),
        }
        self.dirty |= Dirty::PAINT;
        Ok(())
    }

    /// Replaces a slider's visual overrides.
    ///
    /// Unset (`None`) fields continue to track the active theme; see
    /// [`SliderStyle`].
    pub fn set_slider_style(
        &mut self,
        handle: ElementHandle<Slider>,
        style: SliderStyle,
    ) -> Result<(), UiError> {
        match &mut self.node_mut(handle.id)?.kind {
            Kind::Slider { style: current, .. } => *current = style,
            _ => return Err(UiError::new("element is not a slider")),
        }
        self.dirty |= Dirty::PAINT;
        Ok(())
    }

    /// Replaces a scroll view's visual overrides.
    ///
    /// Unset (`None`) fields continue to track the active theme; see
    /// [`ScrollViewStyle`].
    pub fn set_scroll_view_style(
        &mut self,
        handle: ElementHandle<ScrollView>,
        style: ScrollViewStyle,
    ) -> Result<(), UiError> {
        match &mut self.node_mut(handle.id)?.kind {
            Kind::ScrollView { style: current, .. } => *current = style,
            _ => return Err(UiError::new("element is not a scroll view")),
        }
        self.dirty |= Dirty::PAINT;
        Ok(())
    }

    /// Overrides the semantic role reported for one retained element.
    pub fn set_semantic_role<T>(
        &mut self,
        handle: ElementHandle<T>,
        role: SemanticRole,
    ) -> Result<(), UiError> {
        self.node(handle.id)?;
        self.semantic_roles.insert(handle.id, role);
        self.dirty |= Dirty::SEMANTICS;
        Ok(())
    }

    /// Sets or clears the accessible description for one retained element.
    pub fn set_semantic_description<T>(
        &mut self,
        handle: ElementHandle<T>,
        description: Option<String>,
    ) -> Result<(), UiError> {
        self.node(handle.id)?;
        if let Some(description) = description {
            self.semantic_descriptions.insert(handle.id, description);
        } else {
            self.semantic_descriptions.remove(&handle.id);
        }
        self.dirty |= Dirty::SEMANTICS;
        Ok(())
    }

    /// Sets whether one retained element exposes an invalid value.
    pub fn set_semantic_invalid<T>(
        &mut self,
        handle: ElementHandle<T>,
        invalid: bool,
    ) -> Result<(), UiError> {
        self.node(handle.id)?;
        if invalid {
            self.semantic_invalid.insert(handle.id);
        } else {
            self.semantic_invalid.remove(&handle.id);
        }
        self.dirty |= Dirty::SEMANTICS;
        Ok(())
    }

    /// Sets live-region announcement behavior for one retained element.
    pub fn set_semantic_live<T>(
        &mut self,
        handle: ElementHandle<T>,
        live: SemanticLive,
    ) -> Result<(), UiError> {
        self.node(handle.id)?;
        if live == SemanticLive::Off {
            self.semantic_live.remove(&handle.id);
        } else {
            self.semantic_live.insert(handle.id, live);
        }
        self.dirty |= Dirty::SEMANTICS;
        Ok(())
    }

    /// Sets or clears selection state for one semantic element.
    pub fn set_semantic_selected<T>(
        &mut self,
        handle: ElementHandle<T>,
        selected: Option<bool>,
    ) -> Result<(), UiError> {
        self.node(handle.id)?;
        if let Some(selected) = selected {
            self.semantic_selected.insert(handle.id, selected);
        } else {
            self.semantic_selected.remove(&handle.id);
        }
        self.dirty |= Dirty::SEMANTICS;
        Ok(())
    }

    /// Sets or clears expansion state for one semantic element.
    pub fn set_semantic_expanded<T>(
        &mut self,
        handle: ElementHandle<T>,
        expanded: Option<bool>,
    ) -> Result<(), UiError> {
        self.node(handle.id)?;
        if let Some(expanded) = expanded {
            self.semantic_expanded.insert(handle.id, expanded);
        } else {
            self.semantic_expanded.remove(&handle.id);
        }
        self.dirty |= Dirty::SEMANTICS;
        Ok(())
    }

    /// Returns whether `element` belongs to `ancestor`'s retained subtree.
    pub fn is_descendant<T, A>(
        &self,
        element: ElementHandle<T>,
        ancestor: ElementHandle<A>,
    ) -> Result<bool, UiError> {
        self.node(element.id)?;
        self.node(ancestor.id)?;
        Ok(self.is_descendant_of(element.id, ancestor.id))
    }

    /// Returns the active drag session associated with a pointer, when any.
    pub fn drag_session(&self, device_id: DeviceId) -> Option<DragSessionId> {
        self.drag_sessions.get(&device_id).map(|session| session.id)
    }

    /// Returns whether an element belongs to any active pointer hover path.
    pub fn is_hovered<T>(&self, handle: ElementHandle<T>) -> Result<bool, UiError> {
        Ok(self.node(handle.id)?.hovered)
    }

    /// Returns whether an element currently owns keyboard focus.
    pub fn is_focused<T>(&self, handle: ElementHandle<T>) -> Result<bool, UiError> {
        self.node(handle.id)?;
        Ok(self.focus == Some(handle.id))
    }
}
