//! Text shaping, caret placement, editing, and IME composition.

use super::*;

/// Whether text reshaping runs inline on the layout pass or is offloaded to a
/// background worker.
///
/// `Sync` shapes on the main thread during layout. It is deterministic and the
/// default, so tests and headless runs are unaffected. `Async` is the opt-in
/// path selected by [`Ui::enable_async_shaping`](crate::Ui::enable_async_shaping):
/// eligible reshapes are offloaded to a background worker while the previous
/// `TextLayout` stays on screen, and the result is applied by `poll_async` /
/// `flush_async` once it arrives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ShapePolicy {
    /// Shape inline during the layout pass (default, deterministic).
    #[default]
    Sync,
    /// Offload eligible reshapes to the background worker, keeping the old
    /// layout on screen until the result is ready. Force-synced for nodes that
    /// must be correct immediately (never-shaped, focused, or during a
    /// resweep); see `shape_node`.
    ///
    /// Selected only off-wasm: `enable_async_shaping` stays synchronous on wasm
    /// until the web-worker backend lands, so this variant is dead there.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    Async,
}

/// Identifies one reshape request so a completed shape can be matched against
/// the node's current in-flight request and dropped if it was superseded.
/// Monotonic and globally unique per `Ui`, so a recycled slot can never collide
/// with an older request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RequestId(pub(crate) u64);

/// Shapes a request into a `TextLayout` against the given contexts. This is the
/// offloadable unit of work: it touches only the parley scratch context, the
/// font database, and the request — never the `Ui` tree — so a background
/// worker calls it with its own contexts (see `worker.rs`) while the main
/// thread calls it inline for synchronous shaping.
pub(crate) fn shape_request(
    text_context: &mut TextLayoutContext,
    fonts: &mut FontDatabase,
    request: &TextLayoutRequest,
) -> Result<TextLayout, UiError> {
    text_context
        .layout(fonts, request.clone())
        .map_err(|error| UiError::new(error.to_string()))
}

impl<Message: 'static> Ui<Message> {
    pub(crate) fn prepare_text_layouts(&mut self) -> Result<(), UiError> {
        astrelis_profiling::profile_scope!("ui.shape");
        // Reshape only the nodes whose text or resolved style changed since the
        // last pass; a resweep (theme/viewport, or a self-resizing custom
        // widget) revisits everything. Snapshot the ids so the loop can take
        // `&mut self` per node.
        let ids: Vec<ElementId> = if self.measure_resweep {
            (0..self.slots.len())
                .filter_map(|index| self.id_at(index))
                .collect()
        } else {
            self.dirty_nodes
                .iter()
                .copied()
                .filter(|id| self.node(*id).is_ok())
                .collect()
        };
        for id in ids {
            self.shape_node(id)?;
        }
        Ok(())
    }

    /// Resolves one node's text layout for the current pass.
    ///
    /// Builds the node's request and reconciles it against what the node is
    /// currently showing and any reshape already in flight (`pending`):
    ///
    /// - showing layout matches and nothing is in flight — nothing to do;
    /// - the exact request is already in flight — leave the old layout up;
    /// - the request matches the showing layout but a *different* reshape is in
    ///   flight (edited then reverted) — drop the stale in-flight request;
    /// - otherwise (re)shape.
    ///
    /// Under `Sync` the reshape happens inline. Under `Async` it is recorded as
    /// `pending` first; the worker phase (Milestone 20) will move the shaping
    /// itself onto a background thread at that point, leaving the old layout on
    /// screen until the result arrives.
    fn shape_node(&mut self, id: ElementId) -> Result<(), UiError> {
        let Some(request) = self.build_text_request(id)? else {
            // A node that no longer shapes text (e.g. kind changed) must not
            // keep a stale cache entry that would suppress future shaping.
            let node = self.node_mut(id)?;
            node.text_layout = None;
            node.text_request = None;
            node.pending = None;
            return Ok(());
        };
        let node = self.node(id)?;
        let showing = node.text_layout.is_some() && node.text_request.as_ref() == Some(&request);
        // Shaping (BiDi, itemization, fallback, kerning) is the costly half of
        // text layout, so skip it when neither the string nor any resolved
        // style input has changed since it last ran. The dirty flags are
        // global, so without this every label reshapes on any mutation
        // anywhere in the tree.
        if showing && node.pending.is_none() {
            return Ok(());
        }
        // The exact reshape is already in flight: leave the showing layout up
        // and wait for its result rather than requeuing the same work.
        if node.pending.as_ref().map(|(_, req)| req) == Some(&request) {
            return Ok(());
        }
        // The showing layout already matches, but a *different* reshape is
        // still in flight (edited, then reverted). The screen is already
        // correct, so drop the stale in-flight request.
        if showing {
            self.node_mut(id)?.pending = None;
            return Ok(());
        }
        match self.shape_policy {
            ShapePolicy::Sync => self.shape_inline(id, request)?,
            ShapePolicy::Async => {
                // Force synchronous shaping whenever the layout must be correct
                // within this pass and cannot fall back to a stale extent:
                // - no worker is attached (e.g. wasm, or async not enabled);
                // - a resweep touched every node, and funnelling hundreds of
                //   jobs through one serial worker would reflow over many
                //   frames instead of one;
                // - the node has never been shaped, so there is no previous
                //   layout to keep on screen while the worker runs;
                // - the node is focused: caret hit-testing and movement read
                //   its layout on every keystroke, so it must stay current.
                let force_sync = self.worker.is_none()
                    || self.measure_resweep
                    || self.node(id)?.text_layout.is_none()
                    || self.focus == Some(id);
                if force_sync {
                    self.shape_inline(id, request)?;
                } else {
                    let request_id = self.allocate_request_id();
                    self.node_mut(id)?.pending = Some((request_id, request.clone()));
                    self.async_outstanding += 1;
                    if let Some(worker) = &self.worker {
                        worker.send(WorkerJob::Shape {
                            id,
                            request_id,
                            request,
                        });
                    }
                    // Leave the previous `text_layout` on screen; `poll_async`
                    // applies the worker's result when it arrives.
                }
            }
        }
        Ok(())
    }

    /// Shapes a node's request on the calling thread and installs the result,
    /// clearing any reshape in flight. A synchronous shape always supersedes a
    /// pending async one, which is what keeps a late worker result from
    /// clobbering a freshly focused or force-synced node.
    fn shape_inline(&mut self, id: ElementId, request: TextLayoutRequest) -> Result<(), UiError> {
        let layout = shape_request(&mut self.text_context, &mut self.fonts, &request)?;
        let node = self.node_mut(id)?;
        node.text_layout = Some(layout);
        node.text_request = Some(request);
        node.pending = None;
        Ok(())
    }

    /// Allocates the next globally-unique reshape request id.
    fn allocate_request_id(&mut self) -> RequestId {
        let id = RequestId(self.request_id_counter);
        self.request_id_counter += 1;
        id
    }

    /// Builds the text-shaping request for a node from its kind, resolved
    /// visual style, and the current theme and viewport, or `None` if the node
    /// kind carries no text. A pure read of `&self`: the shaping *input*,
    /// separated from the shaping itself (`shape_request`) so the two halves
    /// can later run on different threads.
    fn build_text_request(&self, id: ElementId) -> Result<Option<TextLayoutRequest>, UiError> {
        let node = self.node(id)?;
        let request = match &node.kind {
            Kind::Label { text } | Kind::Button { text } => {
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
                let visual = node.visual;
                let enabled = node.enabled;
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
        Ok(request)
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
                self.invalidate_node(id, Dirty::all());
            }
            ImeEvent::Commit(value) => {
                self.text_field_mut(id)?.preedit.clear();
                self.replace_selection(id, value)?;
            }
            ImeEvent::Disabled => {
                self.text_field_mut(id)?.preedit.clear();
                self.invalidate_node(id, Dirty::all());
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
        self.invalidate_node(id, Dirty::all());
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
