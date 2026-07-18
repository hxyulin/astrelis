//! Text shaping, caret placement, editing, and IME composition.

use super::*;

impl<Message: 'static> Ui<Message> {
    pub(crate) fn prepare_text_layouts(&mut self) -> Result<(), UiError> {
        astrelis_profiling::profile_scope!("ui.shape");
        for index in 0..self.slots.len() {
            let Some(id) = self.id_at(index) else {
                continue;
            };
            let request = match &self.node(id)?.kind {
                Kind::Label { text } | Kind::Button { text } => {
                    let node = self.node(id)?;
                    let visual = node.visual;
                    let enabled = node.enabled;
                    let wrap_width = node.wrap.then(|| match node.style.max_width {
                        Length::Px(px) => px.max(0.0),
                        _ => self.viewport.width.max(0.0),
                    });
                    let mut request = TextLayoutRequest::new(text);
                    request.style.size = visual.font_size.unwrap_or(self.theme.type_scale.body);
                    request.style.families = self.theme.font_families.clone();
                    request.style.color = visual.foreground.unwrap_or(if enabled {
                        self.theme.foreground
                    } else {
                        self.theme.disabled_foreground
                    });
                    request.paragraph = ParagraphStyle {
                        wrap: if wrap_width.is_some() {
                            TextWrap::Wrap
                        } else {
                            TextWrap::NoWrap
                        },
                        max_width: wrap_width,
                        ..Default::default()
                    };
                    Some(request)
                }
                Kind::TextField(field) => {
                    let display = if field.password {
                        "•".repeat(field.text.graphemes(true).count())
                    } else {
                        field.text.clone()
                    };
                    let mut shown = display;
                    if !field.preedit.is_empty() && !field.password {
                        let insert = field.caret.byte_index.min(shown.len());
                        if shown.is_char_boundary(insert) {
                            shown.insert_str(insert, &field.preedit);
                        }
                    }
                    if shown.is_empty() {
                        shown = field.placeholder.clone();
                    }
                    let visual = self.node(id)?.visual;
                    let enabled = self.node(id)?.enabled;
                    let mut request = TextLayoutRequest::new(shown);
                    request.style.size = visual.font_size.unwrap_or(self.theme.type_scale.body);
                    request.style.families = self.theme.font_families.clone();
                    request.style.color = visual.foreground.unwrap_or(if !enabled {
                        self.theme.disabled_foreground
                    } else if field.text.is_empty() && field.preedit.is_empty() {
                        self.theme.muted_foreground
                    } else {
                        self.theme.foreground
                    });
                    request.paragraph = ParagraphStyle {
                        wrap: TextWrap::NoWrap,
                        ..Default::default()
                    };
                    Some(request)
                }
                _ => None,
            };
            if let Some(request) = request {
                let node = self.node(id)?;
                // Shaping (BiDi, itemization, fallback, kerning) is the costly
                // half of text layout, so skip it when neither the string nor
                // any resolved style input has changed since it last ran. The
                // dirty flags are global, so without this every label reshapes
                // on any mutation anywhere in the tree.
                if node.text_layout.is_some() && node.text_request.as_ref() == Some(&request) {
                    continue;
                }
                let layout = self
                    .text_context
                    .layout(&mut self.fonts, request.clone())
                    .map_err(|error| UiError::new(error.to_string()))?;
                let node = self.node_mut(id)?;
                node.text_layout = Some(layout);
                node.text_request = Some(request);
            } else {
                // A node that no longer shapes text (e.g. kind changed) must not
                // keep a stale cache entry that would suppress future shaping.
                let node = self.node_mut(id)?;
                node.text_layout = None;
                node.text_request = None;
            }
        }
        Ok(())
    }

    pub(crate) fn place_text_caret(
        &mut self,
        id: ElementId,
        point: LogicalPoint,
        extend: bool,
    ) -> Result<(), UiError> {
        let node = self.node(id)?;
        let layout = node
            .text_layout
            .clone()
            .ok_or_else(|| UiError::new("text field has not been measured"))?;
        let Kind::TextField(field) = &node.kind else {
            return Ok(());
        };
        let local = Point::new(
            point.x - node.bounds.origin.x - self.theme.control_padding.left
                + field.horizontal_offset,
            point.y - node.bounds.origin.y - self.theme.control_padding.top,
        );
        let position = from_layout_position(field, layout.hit_test(local).position);
        let node = self.node_mut(id)?;
        let Kind::TextField(field) = &mut node.kind else {
            return Ok(());
        };
        field.caret = position;
        if !extend {
            field.anchor = position;
        }
        self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
        Ok(())
    }

    pub(crate) fn handle_text_key(
        &mut self,
        id: ElementId,
        input: &astrelis_platform::KeyboardInput,
        clipboard: &Clipboard,
    ) -> Result<(), UiError> {
        let command = self.modifiers.control || self.modifiers.super_key;
        let character = match &input.logical_key {
            Key::Character(value) => Some(value.to_lowercase()),
            _ => None,
        };
        if command && character.as_deref() == Some("a") {
            let length = self.text_field(id)?.text.len();
            let field = self.text_field_mut(id)?;
            field.anchor.byte_index = 0;
            field.caret.byte_index = length;
            self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
            return Ok(());
        }
        if command && character.as_deref() == Some("c") {
            let field = self.text_field(id)?;
            if !field.password && clipboard.capabilities().write_text {
                let (start, end) = field.selection();
                if start != end {
                    clipboard
                        .write_text(field.text[start..end].to_owned())
                        .map_err(platform_error)?;
                }
            }
            return Ok(());
        }
        if command && character.as_deref() == Some("x") {
            let field = self.text_field(id)?.clone();
            if !field.password && clipboard.capabilities().write_text {
                let (start, end) = field.selection();
                if start != end {
                    clipboard
                        .write_text(field.text[start..end].to_owned())
                        .map_err(platform_error)?;
                    self.replace_selection(id, "")?;
                }
            }
            return Ok(());
        }
        if command && character.as_deref() == Some("v") {
            if clipboard.capabilities().read_text
                && let Some(text) = clipboard.read_text().map_err(platform_error)?
            {
                let text = text.replace(['\r', '\n'], " ");
                self.replace_selection(id, &text)?;
            }
            return Ok(());
        }

        match &input.logical_key {
            Key::Named(NamedKey::Backspace) => {
                let field = self.text_field(id)?.clone();
                let (start, end) = field.selection();
                if start != end {
                    self.replace_selection(id, "")?;
                } else if let Some(previous) = previous_grapheme(&field.text, start) {
                    self.replace_range(id, previous, start, "")?;
                }
            }
            Key::Named(NamedKey::Enter) => {
                let text = self.text_field(id)?.text.clone();
                self.dispatch_routed(id, RoutedEventKind::TextSubmitted(text.clone()))?;
                self.events.push_back(UiEvent {
                    target: id,
                    kind: UiEventKind::TextSubmitted(text),
                });
            }
            Key::Named(NamedKey::Other(name)) if name == "Delete" => {
                let field = self.text_field(id)?.clone();
                let (start, end) = field.selection();
                if start != end {
                    self.replace_selection(id, "")?;
                } else if let Some(next) = next_grapheme(&field.text, end) {
                    self.replace_range(id, end, next, "")?;
                }
            }
            Key::Named(NamedKey::Other(name))
                if matches!(name.as_str(), "ArrowLeft" | "ArrowRight" | "Home" | "End") =>
            {
                self.move_text_caret(id, name, self.modifiers.shift)?;
            }
            _ if !command && !self.modifiers.alt => {
                if let Some(text) = input.text.as_deref()
                    && !text.chars().any(char::is_control)
                {
                    self.replace_selection(id, text)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn move_text_caret(
        &mut self,
        id: ElementId,
        key: &str,
        extend: bool,
    ) -> Result<(), UiError> {
        self.ensure_layout()?;
        let node = self.node(id)?;
        let layout = node
            .text_layout
            .clone()
            .ok_or_else(|| UiError::new("text field has not been measured"))?;
        let field = self.text_field(id)?.clone();
        let movement = match key {
            "ArrowLeft" => CaretMovement::VisualLeft,
            "ArrowRight" => CaretMovement::VisualRight,
            "Home" => CaretMovement::LineStart,
            "End" => CaretMovement::LineEnd,
            _ => return Ok(()),
        };
        let position = from_layout_position(
            &field,
            layout.move_caret(to_layout_position(&field, field.caret), movement),
        );
        let field = self.text_field_mut(id)?;
        field.caret = position;
        if !extend {
            field.anchor = position;
        }
        self.dirty |= Dirty::PAINT | Dirty::SEMANTICS;
        Ok(())
    }

    pub(crate) fn handle_ime(&mut self, id: ElementId, event: &ImeEvent) -> Result<(), UiError> {
        match event {
            ImeEvent::Preedit(value, _) => {
                self.text_field_mut(id)?.preedit = value.clone();
                self.invalidate_layout();
            }
            ImeEvent::Commit(value) => {
                self.text_field_mut(id)?.preedit.clear();
                self.replace_selection(id, value)?;
            }
            ImeEvent::Disabled => {
                self.text_field_mut(id)?.preedit.clear();
                self.invalidate_layout();
            }
            ImeEvent::Enabled => {}
        }
        Ok(())
    }

    pub(crate) fn replace_selection(&mut self, id: ElementId, value: &str) -> Result<(), UiError> {
        let (start, end) = self.text_field(id)?.selection();
        self.replace_range(id, start, end, value)
    }

    pub(crate) fn replace_range(
        &mut self,
        id: ElementId,
        start: usize,
        end: usize,
        value: &str,
    ) -> Result<(), UiError> {
        let field = self.text_field_mut(id)?;
        field.text.replace_range(start..end, value);
        field.caret = TextPosition {
            byte_index: start + value.len(),
            ..Default::default()
        };
        field.anchor = field.caret;
        let text = field.text.clone();
        self.events.push_back(UiEvent {
            target: id,
            kind: UiEventKind::TextChanged(text.clone()),
        });
        self.dispatch_routed(id, RoutedEventKind::TextChanged(text))?;
        self.invalidate_layout();
        Ok(())
    }

    pub(crate) fn text_field(&self, id: ElementId) -> Result<&TextFieldState, UiError> {
        let Kind::TextField(field) = &self.node(id)?.kind else {
            return Err(UiError::new("element is not a text field"));
        };
        Ok(field)
    }

    pub(crate) fn text_field_mut(&mut self, id: ElementId) -> Result<&mut TextFieldState, UiError> {
        let Kind::TextField(field) = &mut self.node_mut(id)?.kind else {
            return Err(UiError::new("element is not a text field"));
        };
        Ok(field)
    }
}
