use crate::{
    Window,
    event::{ElementState, Event, MouseButton},
};

#[derive(Clone, Copy, Debug, Default)]
pub struct EventResponse {
    /// If true, egui consumed this event, i.e. wants exclusive use of this event
    /// (e.g. a mouse click on an egui window, or entering text into a text field).
    ///
    /// For instance, if you use egui for a game, you should only
    /// pass on the events to your game when [`Self::consumed`] is `false`.
    ///
    /// Note that egui uses `tab` to move focus between elements, so this will always be `true` for tabs.
    pub consumed: bool,

    /// Do we need an egui refresh because of this event?
    pub repaint: bool,
}

pub struct State {
    context: egui::Context,
    input: egui::RawInput,
    viewport_id: egui::ViewportId,
    pointer_pos_in_points: Option<egui::Pos2>,
    any_pointer_button_down: bool,
    current_cursor_icon: Option<egui::CursorIcon>,
}

impl State {
    pub fn new(
        context: egui::Context,
        viewport_id: egui::ViewportId,
        native_pixels_per_point: Option<f32>,
        max_texture_side: Option<usize>,
    ) -> Self {
        let input = egui::RawInput {
            focused: false, // winit will tell us when we have focus
            ..Default::default()
        };

        let mut slf = Self {
            context,
            viewport_id,
            input,
            pointer_pos_in_points: None,
            any_pointer_button_down: false,
            current_cursor_icon: None,
        };

        slf.input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = native_pixels_per_point;

        if let Some(max_texture_side) = max_texture_side {
            slf.input.max_texture_side = Some(max_texture_side);
        }
        slf
    }

    pub fn take_input(&mut self, window: &Window) -> egui::RawInput {
        // On Windows, a minimized window will have 0 width and height.
        // See: https://github.com/rust-windowing/winit/issues/208
        // This solves an issue where egui window positions would be changed when minimizing on Windows.
        let screen_size_in_pixels = screen_size_in_pixels(window);
        let screen_size_in_points = screen_size_in_pixels / pixels_per_point(&self.context, window);

        self.input.screen_rect = (screen_size_in_points.x > 0.0 && screen_size_in_points.y > 0.0)
            .then(|| egui::Rect::from_min_size(egui::Pos2::ZERO, screen_size_in_points));

        // Tell egui which viewport is now active:
        self.input.viewport_id = self.viewport_id;

        self.input
            .viewports
            .entry(self.viewport_id)
            .or_default()
            .native_pixels_per_point = Some(window.window.scale_factor() as f32);

        self.input.take()
    }
    pub fn handle_platform_output(&mut self, _window: &Window, _output: egui::PlatformOutput) {
        // TODO: Handle cursor changes, clipboard copies, opening links in browsers
    }

    pub fn on_event(&mut self, window: &Window, event: &Event) -> EventResponse {
        match event {
            Event::ScaleFactorChanged(scale_factor) => {
                let native_pixels_per_point = *scale_factor as f32;

                self.input
                    .viewports
                    .entry(self.viewport_id)
                    .or_default()
                    .native_pixels_per_point = Some(native_pixels_per_point);

                EventResponse {
                    repaint: true,
                    consumed: false,
                }
            }

            Event::Focused(focused) => {
                self.input.focused = *focused;
                self.input.events.push(egui::Event::WindowFocused(*focused));
                EventResponse {
                    repaint: true,
                    consumed: false,
                }
            }
            Event::MouseButtonDown(button) => {
                self.on_mouse_button_input(ElementState::Pressed, *button);
                EventResponse {
                    repaint: true,
                    consumed: self.context.wants_pointer_input(),
                }
            }
            Event::MouseButtonUp(button) => {
                self.on_mouse_button_input(ElementState::Released, *button);
                EventResponse {
                    repaint: true,
                    consumed: self.context.wants_pointer_input(),
                }
            }
            Event::MouseScrolled(delta) => {
                self.on_mouse_wheel(window, *delta);
                EventResponse {
                    repaint: true,
                    consumed: self.context.wants_pointer_input(),
                }
            }
            Event::MouseMoved(pos) => {
                self.on_cursor_moved(window, *pos);
                EventResponse {
                    repaint: true,
                    consumed: self.context.is_using_pointer(),
                }
            }
            Event::MouseLeft => {
                self.pointer_pos_in_points = None;
                self.input.events.push(egui::Event::PointerGone);
                EventResponse {
                    repaint: true,
                    consumed: false,
                }
            }

            Event::KeyInput(event) => {
                // Winit generates fake "synthetic" KeyboardInput events when the focus
                // is changed to the window, or away from it. Synthetic key presses
                // represent no real key presses and should be ignored.
                // See https://github.com/rust-windowing/winit/issues/3543
                if event.is_synthetic && event.state == ElementState::Pressed {
                    EventResponse {
                        repaint: true,
                        consumed: false,
                    }
                } else {
                    self.on_keyboard_input(event);

                    // When pressing the Tab key, egui focuses the first focusable element, hence Tab always consumes.
                    let consumed = self.context.wants_keyboard_input()
                        || event.logical_key
                            == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Tab);
                    EventResponse {
                        repaint: true,
                        consumed,
                    }
                }
            }

            _ => EventResponse {
                repaint: false,
                consumed: false,
            },
        }
    }

    fn on_mouse_button_input(&mut self, state: ElementState, button: MouseButton) {
        if let Some(pos) = self.pointer_pos_in_points {
            if let Some(button) = translate_mouse_button(button) {
                let pressed = state == winit::event::ElementState::Pressed;

                self.input.events.push(egui::Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers: self.input.modifiers,
                });
            }
        }
    }

    fn on_cursor_moved(
        &mut self,
        window: &Window,
        pos_in_pixels: winit::dpi::PhysicalPosition<f64>,
    ) {
        let pixels_per_point = pixels_per_point(&self.context, window);

        let pos_in_points = egui::pos2(
            pos_in_pixels.x as f32 / pixels_per_point,
            pos_in_pixels.y as f32 / pixels_per_point,
        );
        self.pointer_pos_in_points = Some(pos_in_points);

        self.input
            .events
            .push(egui::Event::PointerMoved(pos_in_points));
    }

    fn on_mouse_wheel(&mut self, window: &Window, delta: winit::event::MouseScrollDelta) {
        let pixels_per_point = pixels_per_point(&self.context, window);

        {
            let (unit, delta) = match delta {
                winit::event::MouseScrollDelta::LineDelta(x, y) => {
                    (egui::MouseWheelUnit::Line, egui::vec2(x, y))
                }
                winit::event::MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition {
                    x,
                    y,
                }) => (
                    egui::MouseWheelUnit::Point,
                    egui::vec2(x as f32, y as f32) / pixels_per_point,
                ),
            };
            let modifiers = self.input.modifiers;
            self.input.events.push(egui::Event::MouseWheel {
                unit,
                delta,
                modifiers,
            });
        }
    }

    fn on_keyboard_input(&mut self, event: &crate::event::KeyEvent) {
        let crate::event::KeyEvent {
            // Represents the position of a key independent of the currently active layout.
            //
            // It also uniquely identifies the physical key (i.e. it's mostly synonymous with a scancode).
            // The most prevalent use case for this is games. For example the default keys for the player
            // to move around might be the W, A, S, and D keys on a US layout. The position of these keys
            // is more important than their label, so they should map to Z, Q, S, and D on an "AZERTY"
            // layout. (This value is `KeyCode::KeyW` for the Z key on an AZERTY layout.)
            physical_key,

            // Represents the results of a keymap, i.e. what character a certain key press represents.
            // When telling users "Press Ctrl-F to find", this is where we should
            // look for the "F" key, because they may have a dvorak layout on
            // a qwerty keyboard, and so the logical "F" character may not be located on the physical `KeyCode::KeyF` position.
            logical_key,

            text,

            state,

            location: _, // e.g. is it on the numpad?
            repeat: _,   // egui will figure this out for us
            ..
        } = event;

        let pressed = *state == winit::event::ElementState::Pressed;

        let physical_key = if let winit::keyboard::PhysicalKey::Code(keycode) = *physical_key {
            key_from_key_code(keycode)
        } else {
            None
        };

        let logical_key = key_from_winit_key(logical_key);

        // "Logical OR physical key" is a fallback mechanism for keyboard layouts without Latin characters: it lets them
        // emit events as if the corresponding keys from the Latin layout were pressed. In this case, clipboard shortcuts
        // are mapped to the physical keys that normally contain C, X, V, etc.
        // See also: https://github.com/emilk/egui/issues/3653
        if let Some(active_key) = logical_key.or(physical_key) {
            if pressed {
                if is_cut_command(self.input.modifiers, active_key) {
                    self.input.events.push(egui::Event::Cut);
                    return;
                } else if is_copy_command(self.input.modifiers, active_key) {
                    self.input.events.push(egui::Event::Copy);
                    return;
                } else if is_paste_command(self.input.modifiers, active_key) {
                    // TODO: Support keyboard
                    /*
                    if let Some(contents) = self.clipboard.get() {
                        let contents = contents.replace("\r\n", "\n");
                        if !contents.is_empty() {
                            self.input.events.push(egui::Event::Paste(contents));
                        }
                    }
                    */
                    return;
                }
            }

            self.input.events.push(egui::Event::Key {
                key: active_key,
                physical_key,
                pressed,
                repeat: false, // egui will fill this in for us!
                modifiers: self.input.modifiers,
            });
        }

        if let Some(text) = &text {
            // Make sure there is text, and that it is not control characters
            // (e.g. delete is sent as "\u{f728}" on macOS).
            if !text.is_empty() && text.chars().all(is_printable_char) {
                // On some platforms we get here when the user presses Cmd-C (copy), ctrl-W, etc.
                // We need to ignore these characters that are side-effects of commands.
                // Also make sure the key is pressed (not released). On Linux, text might
                // contain some data even when the key is released.
                let is_cmd = self.input.modifiers.ctrl
                    || self.input.modifiers.command
                    || self.input.modifiers.mac_cmd;
                if pressed && !is_cmd {
                    self.input.events.push(egui::Event::Text(text.to_string()));
                }
            }
        }
    }
}

pub fn screen_size_in_pixels(window: &Window) -> egui::Vec2 {
    let size = window.window.inner_size();
    egui::vec2(size.width as f32, size.height as f32)
}

/// Calculate the `pixels_per_point` for a given window, given the current egui zoom factor
pub fn pixels_per_point(context: &egui::Context, window: &Window) -> f32 {
    let native_pixels_per_point = window.window.scale_factor() as f32;
    let egui_zoom_factor = context.zoom_factor();
    egui_zoom_factor * native_pixels_per_point
}

/// Winit sends special keys (backspace, delete, F1, …) as characters.
/// Ignore those.
/// We also ignore '\r', '\n', '\t'.
/// Newlines are handled by the `Key::Enter` event.
fn is_printable_char(chr: char) -> bool {
    let is_in_private_use_area = '\u{e000}' <= chr && chr <= '\u{f8ff}'
        || '\u{f0000}' <= chr && chr <= '\u{ffffd}'
        || '\u{100000}' <= chr && chr <= '\u{10fffd}';

    !is_in_private_use_area && !chr.is_ascii_control()
}

fn is_cut_command(modifiers: egui::Modifiers, keycode: egui::Key) -> bool {
    keycode == egui::Key::Cut
        || (modifiers.command && keycode == egui::Key::X)
        || (cfg!(target_os = "windows") && modifiers.shift && keycode == egui::Key::Delete)
}

fn is_copy_command(modifiers: egui::Modifiers, keycode: egui::Key) -> bool {
    keycode == egui::Key::Copy
        || (modifiers.command && keycode == egui::Key::C)
        || (cfg!(target_os = "windows") && modifiers.ctrl && keycode == egui::Key::Insert)
}

fn is_paste_command(modifiers: egui::Modifiers, keycode: egui::Key) -> bool {
    keycode == egui::Key::Paste
        || (modifiers.command && keycode == egui::Key::V)
        || (cfg!(target_os = "windows") && modifiers.shift && keycode == egui::Key::Insert)
}

fn translate_mouse_button(button: winit::event::MouseButton) -> Option<egui::PointerButton> {
    match button {
        winit::event::MouseButton::Left => Some(egui::PointerButton::Primary),
        winit::event::MouseButton::Right => Some(egui::PointerButton::Secondary),
        winit::event::MouseButton::Middle => Some(egui::PointerButton::Middle),
        winit::event::MouseButton::Back => Some(egui::PointerButton::Extra1),
        winit::event::MouseButton::Forward => Some(egui::PointerButton::Extra2),
        winit::event::MouseButton::Other(_) => None,
    }
}

fn key_from_winit_key(key: &winit::keyboard::Key) -> Option<egui::Key> {
    match key {
        winit::keyboard::Key::Named(named_key) => key_from_named_key(*named_key),
        winit::keyboard::Key::Character(str) => egui::Key::from_name(str.as_str()),
        winit::keyboard::Key::Unidentified(_) | winit::keyboard::Key::Dead(_) => None,
    }
}

fn key_from_named_key(named_key: winit::keyboard::NamedKey) -> Option<egui::Key> {
    use egui::Key;
    use winit::keyboard::NamedKey;

    Some(match named_key {
        NamedKey::Enter => Key::Enter,
        NamedKey::Tab => Key::Tab,
        NamedKey::ArrowDown => Key::ArrowDown,
        NamedKey::ArrowLeft => Key::ArrowLeft,
        NamedKey::ArrowRight => Key::ArrowRight,
        NamedKey::ArrowUp => Key::ArrowUp,
        NamedKey::End => Key::End,
        NamedKey::Home => Key::Home,
        NamedKey::PageDown => Key::PageDown,
        NamedKey::PageUp => Key::PageUp,
        NamedKey::Backspace => Key::Backspace,
        NamedKey::Delete => Key::Delete,
        NamedKey::Insert => Key::Insert,
        NamedKey::Escape => Key::Escape,
        NamedKey::Cut => Key::Cut,
        NamedKey::Copy => Key::Copy,
        NamedKey::Paste => Key::Paste,

        NamedKey::Space => Key::Space,

        NamedKey::F1 => Key::F1,
        NamedKey::F2 => Key::F2,
        NamedKey::F3 => Key::F3,
        NamedKey::F4 => Key::F4,
        NamedKey::F5 => Key::F5,
        NamedKey::F6 => Key::F6,
        NamedKey::F7 => Key::F7,
        NamedKey::F8 => Key::F8,
        NamedKey::F9 => Key::F9,
        NamedKey::F10 => Key::F10,
        NamedKey::F11 => Key::F11,
        NamedKey::F12 => Key::F12,
        NamedKey::F13 => Key::F13,
        NamedKey::F14 => Key::F14,
        NamedKey::F15 => Key::F15,
        NamedKey::F16 => Key::F16,
        NamedKey::F17 => Key::F17,
        NamedKey::F18 => Key::F18,
        NamedKey::F19 => Key::F19,
        NamedKey::F20 => Key::F20,
        NamedKey::F21 => Key::F21,
        NamedKey::F22 => Key::F22,
        NamedKey::F23 => Key::F23,
        NamedKey::F24 => Key::F24,
        NamedKey::F25 => Key::F25,
        NamedKey::F26 => Key::F26,
        NamedKey::F27 => Key::F27,
        NamedKey::F28 => Key::F28,
        NamedKey::F29 => Key::F29,
        NamedKey::F30 => Key::F30,
        NamedKey::F31 => Key::F31,
        NamedKey::F32 => Key::F32,
        NamedKey::F33 => Key::F33,
        NamedKey::F34 => Key::F34,
        NamedKey::F35 => Key::F35,
        _ => {
            log::trace!("Unknown key: {named_key:?}");
            return None;
        }
    })
}

fn key_from_key_code(key: winit::keyboard::KeyCode) -> Option<egui::Key> {
    use egui::Key;
    use winit::keyboard::KeyCode;

    Some(match key {
        KeyCode::ArrowDown => Key::ArrowDown,
        KeyCode::ArrowLeft => Key::ArrowLeft,
        KeyCode::ArrowRight => Key::ArrowRight,
        KeyCode::ArrowUp => Key::ArrowUp,

        KeyCode::Escape => Key::Escape,
        KeyCode::Tab => Key::Tab,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Enter | KeyCode::NumpadEnter => Key::Enter,

        KeyCode::Insert => Key::Insert,
        KeyCode::Delete => Key::Delete,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,

        // Punctuation
        KeyCode::Space => Key::Space,
        KeyCode::Comma => Key::Comma,
        KeyCode::Period => Key::Period,
        // KeyCode::Colon => Key::Colon, // NOTE: there is no physical colon key on an american keyboard
        KeyCode::Semicolon => Key::Semicolon,
        KeyCode::Backslash => Key::Backslash,
        KeyCode::Slash | KeyCode::NumpadDivide => Key::Slash,
        KeyCode::BracketLeft => Key::OpenBracket,
        KeyCode::BracketRight => Key::CloseBracket,
        KeyCode::Backquote => Key::Backtick,
        KeyCode::Quote => Key::Quote,

        KeyCode::Cut => Key::Cut,
        KeyCode::Copy => Key::Copy,
        KeyCode::Paste => Key::Paste,
        KeyCode::Minus | KeyCode::NumpadSubtract => Key::Minus,
        KeyCode::NumpadAdd => Key::Plus,
        KeyCode::Equal => Key::Equals,

        KeyCode::Digit0 | KeyCode::Numpad0 => Key::Num0,
        KeyCode::Digit1 | KeyCode::Numpad1 => Key::Num1,
        KeyCode::Digit2 | KeyCode::Numpad2 => Key::Num2,
        KeyCode::Digit3 | KeyCode::Numpad3 => Key::Num3,
        KeyCode::Digit4 | KeyCode::Numpad4 => Key::Num4,
        KeyCode::Digit5 | KeyCode::Numpad5 => Key::Num5,
        KeyCode::Digit6 | KeyCode::Numpad6 => Key::Num6,
        KeyCode::Digit7 | KeyCode::Numpad7 => Key::Num7,
        KeyCode::Digit8 | KeyCode::Numpad8 => Key::Num8,
        KeyCode::Digit9 | KeyCode::Numpad9 => Key::Num9,

        KeyCode::KeyA => Key::A,
        KeyCode::KeyB => Key::B,
        KeyCode::KeyC => Key::C,
        KeyCode::KeyD => Key::D,
        KeyCode::KeyE => Key::E,
        KeyCode::KeyF => Key::F,
        KeyCode::KeyG => Key::G,
        KeyCode::KeyH => Key::H,
        KeyCode::KeyI => Key::I,
        KeyCode::KeyJ => Key::J,
        KeyCode::KeyK => Key::K,
        KeyCode::KeyL => Key::L,
        KeyCode::KeyM => Key::M,
        KeyCode::KeyN => Key::N,
        KeyCode::KeyO => Key::O,
        KeyCode::KeyP => Key::P,
        KeyCode::KeyQ => Key::Q,
        KeyCode::KeyR => Key::R,
        KeyCode::KeyS => Key::S,
        KeyCode::KeyT => Key::T,
        KeyCode::KeyU => Key::U,
        KeyCode::KeyV => Key::V,
        KeyCode::KeyW => Key::W,
        KeyCode::KeyX => Key::X,
        KeyCode::KeyY => Key::Y,
        KeyCode::KeyZ => Key::Z,

        KeyCode::F1 => Key::F1,
        KeyCode::F2 => Key::F2,
        KeyCode::F3 => Key::F3,
        KeyCode::F4 => Key::F4,
        KeyCode::F5 => Key::F5,
        KeyCode::F6 => Key::F6,
        KeyCode::F7 => Key::F7,
        KeyCode::F8 => Key::F8,
        KeyCode::F9 => Key::F9,
        KeyCode::F10 => Key::F10,
        KeyCode::F11 => Key::F11,
        KeyCode::F12 => Key::F12,
        KeyCode::F13 => Key::F13,
        KeyCode::F14 => Key::F14,
        KeyCode::F15 => Key::F15,
        KeyCode::F16 => Key::F16,
        KeyCode::F17 => Key::F17,
        KeyCode::F18 => Key::F18,
        KeyCode::F19 => Key::F19,
        KeyCode::F20 => Key::F20,
        KeyCode::F21 => Key::F21,
        KeyCode::F22 => Key::F22,
        KeyCode::F23 => Key::F23,
        KeyCode::F24 => Key::F24,
        KeyCode::F25 => Key::F25,
        KeyCode::F26 => Key::F26,
        KeyCode::F27 => Key::F27,
        KeyCode::F28 => Key::F28,
        KeyCode::F29 => Key::F29,
        KeyCode::F30 => Key::F30,
        KeyCode::F31 => Key::F31,
        KeyCode::F32 => Key::F32,
        KeyCode::F33 => Key::F33,
        KeyCode::F34 => Key::F34,
        KeyCode::F35 => Key::F35,

        _ => {
            return None;
        }
    })
}

fn translate_cursor(cursor_icon: egui::CursorIcon) -> Option<winit::window::CursorIcon> {
    match cursor_icon {
        egui::CursorIcon::None => None,

        egui::CursorIcon::Alias => Some(winit::window::CursorIcon::Alias),
        egui::CursorIcon::AllScroll => Some(winit::window::CursorIcon::AllScroll),
        egui::CursorIcon::Cell => Some(winit::window::CursorIcon::Cell),
        egui::CursorIcon::ContextMenu => Some(winit::window::CursorIcon::ContextMenu),
        egui::CursorIcon::Copy => Some(winit::window::CursorIcon::Copy),
        egui::CursorIcon::Crosshair => Some(winit::window::CursorIcon::Crosshair),
        egui::CursorIcon::Default => Some(winit::window::CursorIcon::Default),
        egui::CursorIcon::Grab => Some(winit::window::CursorIcon::Grab),
        egui::CursorIcon::Grabbing => Some(winit::window::CursorIcon::Grabbing),
        egui::CursorIcon::Help => Some(winit::window::CursorIcon::Help),
        egui::CursorIcon::Move => Some(winit::window::CursorIcon::Move),
        egui::CursorIcon::NoDrop => Some(winit::window::CursorIcon::NoDrop),
        egui::CursorIcon::NotAllowed => Some(winit::window::CursorIcon::NotAllowed),
        egui::CursorIcon::PointingHand => Some(winit::window::CursorIcon::Pointer),
        egui::CursorIcon::Progress => Some(winit::window::CursorIcon::Progress),

        egui::CursorIcon::ResizeHorizontal => Some(winit::window::CursorIcon::EwResize),
        egui::CursorIcon::ResizeNeSw => Some(winit::window::CursorIcon::NeswResize),
        egui::CursorIcon::ResizeNwSe => Some(winit::window::CursorIcon::NwseResize),
        egui::CursorIcon::ResizeVertical => Some(winit::window::CursorIcon::NsResize),

        egui::CursorIcon::ResizeEast => Some(winit::window::CursorIcon::EResize),
        egui::CursorIcon::ResizeSouthEast => Some(winit::window::CursorIcon::SeResize),
        egui::CursorIcon::ResizeSouth => Some(winit::window::CursorIcon::SResize),
        egui::CursorIcon::ResizeSouthWest => Some(winit::window::CursorIcon::SwResize),
        egui::CursorIcon::ResizeWest => Some(winit::window::CursorIcon::WResize),
        egui::CursorIcon::ResizeNorthWest => Some(winit::window::CursorIcon::NwResize),
        egui::CursorIcon::ResizeNorth => Some(winit::window::CursorIcon::NResize),
        egui::CursorIcon::ResizeNorthEast => Some(winit::window::CursorIcon::NeResize),
        egui::CursorIcon::ResizeColumn => Some(winit::window::CursorIcon::ColResize),
        egui::CursorIcon::ResizeRow => Some(winit::window::CursorIcon::RowResize),

        egui::CursorIcon::Text => Some(winit::window::CursorIcon::Text),
        egui::CursorIcon::VerticalText => Some(winit::window::CursorIcon::VerticalText),
        egui::CursorIcon::Wait => Some(winit::window::CursorIcon::Wait),
        egui::CursorIcon::ZoomIn => Some(winit::window::CursorIcon::ZoomIn),
        egui::CursorIcon::ZoomOut => Some(winit::window::CursorIcon::ZoomOut),
    }
}
