//! Intent-named event listeners.
//!
//! `astrelis-ui-core`'s `listen` is one call taking a phase, a filter, and a
//! closure that receives every routed event and must match the payload out by
//! hand. Most call sites want one intent — a click, a new checkbox value, a new
//! slider value — so [`On`] wraps `listen` per intent: it picks the phase and
//! filter, unwraps the payload, and hands the callback exactly what it needs.

use astrelis_ui_core::{
    Checkbox, ElementHandle, EventContext, EventFilter, EventPhase, ListenerId, RoutedEventKind,
    Slider, TextField, Ui,
};

/// Intent-named listener registration on [`Ui`].
///
/// Each method targets one routed-event intent at the target phase and calls
/// back with the decoded payload. Panics if the handle is stale, matching the
/// rest of the facade's infallible construction path.
pub trait On<Message: 'static> {
    /// Runs `callback` when the control is activated (clicked or key-activated).
    fn on_click<T>(
        &mut self,
        handle: ElementHandle<T>,
        callback: impl FnMut(&mut EventContext<'_, Message>) + 'static,
    ) -> ListenerId;

    /// Runs `callback` with the new value whenever the checkbox toggles.
    fn on_checked(
        &mut self,
        handle: ElementHandle<Checkbox>,
        callback: impl FnMut(&mut EventContext<'_, Message>, bool) + 'static,
    ) -> ListenerId;

    /// Runs `callback` with the new value whenever the slider changes.
    fn on_slider(
        &mut self,
        handle: ElementHandle<Slider>,
        callback: impl FnMut(&mut EventContext<'_, Message>, f32) + 'static,
    ) -> ListenerId;

    /// Runs `callback` with the new text whenever the field's content changes.
    fn on_text_changed(
        &mut self,
        handle: ElementHandle<TextField>,
        callback: impl FnMut(&mut EventContext<'_, Message>, &str) + 'static,
    ) -> ListenerId;

    /// Runs `callback` with the text when the field is submitted (Enter).
    fn on_text_submitted(
        &mut self,
        handle: ElementHandle<TextField>,
        callback: impl FnMut(&mut EventContext<'_, Message>, &str) + 'static,
    ) -> ListenerId;
}

impl<Message: 'static> On<Message> for Ui<Message> {
    fn on_click<T>(
        &mut self,
        handle: ElementHandle<T>,
        mut callback: impl FnMut(&mut EventContext<'_, Message>) + 'static,
    ) -> ListenerId {
        self.listen(
            handle,
            Some(EventPhase::Target),
            EventFilter::Activate,
            move |context, event| {
                if matches!(event.kind, RoutedEventKind::Activate) {
                    callback(context);
                }
            },
        )
        .expect("on_click on a live handle")
    }

    fn on_checked(
        &mut self,
        handle: ElementHandle<Checkbox>,
        mut callback: impl FnMut(&mut EventContext<'_, Message>, bool) + 'static,
    ) -> ListenerId {
        self.listen(
            handle,
            Some(EventPhase::Target),
            EventFilter::ValueChanged,
            move |context, event| {
                if let RoutedEventKind::CheckedChanged(value) = event.kind {
                    callback(context, value);
                }
            },
        )
        .expect("on_checked on a live handle")
    }

    fn on_slider(
        &mut self,
        handle: ElementHandle<Slider>,
        mut callback: impl FnMut(&mut EventContext<'_, Message>, f32) + 'static,
    ) -> ListenerId {
        self.listen(
            handle,
            Some(EventPhase::Target),
            EventFilter::ValueChanged,
            move |context, event| {
                if let RoutedEventKind::SliderChanged(value) = event.kind {
                    callback(context, value);
                }
            },
        )
        .expect("on_slider on a live handle")
    }

    fn on_text_changed(
        &mut self,
        handle: ElementHandle<TextField>,
        mut callback: impl FnMut(&mut EventContext<'_, Message>, &str) + 'static,
    ) -> ListenerId {
        self.listen(
            handle,
            Some(EventPhase::Target),
            EventFilter::ValueChanged,
            move |context, event| {
                if let RoutedEventKind::TextChanged(value) = &event.kind {
                    callback(context, value);
                }
            },
        )
        .expect("on_text_changed on a live handle")
    }

    fn on_text_submitted(
        &mut self,
        handle: ElementHandle<TextField>,
        mut callback: impl FnMut(&mut EventContext<'_, Message>, &str) + 'static,
    ) -> ListenerId {
        // TextSubmitted is not mapped to a dedicated EventFilter category, so
        // Any is the only filter that reaches it; the closure narrows to it.
        self.listen(
            handle,
            Some(EventPhase::Target),
            EventFilter::Any,
            move |context, event| {
                if let RoutedEventKind::TextSubmitted(value) = &event.kind {
                    callback(context, value);
                }
            },
        )
        .expect("on_text_submitted on a live handle")
    }
}
