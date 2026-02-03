use astrelis_core::profiling::profile_function;
use astrelis_render::RenderableWindow;
use astrelis_winit::event::{ElementState, Event, KeyEvent, MouseButton, MouseScrollDelta};

#[derive(Clone, Copy, Debug, Default)]
pub struct EventResponse {
    /// If true, egui consumed this event, i.e. wants exclusive use of this event
    /// (e.g. a mouse click on an egui window, or entering text into a text field).
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
    /// Cursor icon state (planned for future cursor handling)
    #[allow(dead_code)]
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
            focused: false,
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

    pub fn take_input(&mut self, window: &RenderableWindow) -> egui::RawInput {
        profile_function!();
        let screen_size_in_pixels = screen_size_in_pixels(window);
        let screen_size_in_points = screen_size_in_pixels / pixels_per_point(&self.context, window);

        self.input.screen_rect = (screen_size_in_points.x > 0.0 && screen_size_in_points.y > 0.0)
            .then(|| egui::Rect::from_min_size(egui::Pos2::ZERO, screen_size_in_points));

        self.input.viewport_id = self.viewport_id;

        self.input
            .viewports
            .entry(self.viewport_id)
            .or_default()
            .native_pixels_per_point = Some(window.window().window.scale_factor() as f32);

        self.input.take()
    }

    pub fn handle_platform_output(
        &mut self,
        _window: &RenderableWindow,
        _output: egui::PlatformOutput,
    ) {
        // TODO: Handle cursor changes, clipboard copies, opening links in browsers
    }

    pub fn on_event(&mut self, window: &RenderableWindow, event: &Event) -> EventResponse {
        profile_function!();
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
                if event.is_synthetic && event.state == ElementState::Pressed {
                    EventResponse {
                        repaint: true,
                        consumed: false,
                    }
                } else {
                    self.on_keyboard_input(event);

                    let consumed = self.context.wants_keyboard_input()
                        || matches!(
                            event.logical_key,
                            astrelis_winit::event::Key::Named(astrelis_winit::event::NamedKey::Tab)
                        );
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
        if let Some(pos) = self.pointer_pos_in_points
            && let Some(button) = translate_mouse_button(button) {
                let pressed = state == ElementState::Pressed;

                self.input.events.push(egui::Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers: self.input.modifiers,
                });

                self.any_pointer_button_down = pressed;
            }
    }

    fn on_cursor_moved(
        &mut self,
        window: &RenderableWindow,
        pos_in_logical: astrelis_core::geometry::LogicalPosition<f64>,
    ) {
        // Input is in logical pixels, convert to physical pixels first
        let scale_factor = window.window().window.scale_factor() as f32;
        let pos_in_physical = egui::pos2(
            pos_in_logical.x as f32 * scale_factor,
            pos_in_logical.y as f32 * scale_factor,
        );
        
        // Then convert physical pixels to egui points
        let pixels_per_point = pixels_per_point(&self.context, window);
        let pos_in_points = egui::pos2(
            pos_in_physical.x / pixels_per_point,
            pos_in_physical.y / pixels_per_point,
        );
        self.pointer_pos_in_points = Some(pos_in_points);

        self.input
            .events
            .push(egui::Event::PointerMoved(pos_in_points));
    }

    fn on_mouse_wheel(&mut self, window: &RenderableWindow, delta: MouseScrollDelta) {
        let pixels_per_point = pixels_per_point(&self.context, window);

        let (unit, delta) = match delta {
            MouseScrollDelta::LineDelta(x, y) => (egui::MouseWheelUnit::Line, egui::vec2(x, y)),
            MouseScrollDelta::PixelDelta(pos) => (
                egui::MouseWheelUnit::Point,
                egui::vec2(pos.x as f32, pos.y as f32) / pixels_per_point,
            ),
        };
        let modifiers = self.input.modifiers;
        self.input.events.push(egui::Event::MouseWheel {
            unit,
            delta,
            modifiers,
        });
    }

    fn on_keyboard_input(&mut self, event: &KeyEvent) {
        let KeyEvent {
            physical_key,
            logical_key,
            text,
            state,
            location: _,
            repeat: _,
            ..
        } = event;

        let pressed = *state == ElementState::Pressed;

        let physical_key = if let astrelis_winit::event::PhysicalKey::Code(keycode) = *physical_key
        {
            key_from_key_code(keycode)
        } else {
            None
        };

        let logical_key = key_from_winit_key(logical_key);

        if let Some(active_key) = logical_key.or(physical_key) {
            if pressed {
                if is_cut_command(self.input.modifiers, active_key) {
                    self.input.events.push(egui::Event::Cut);
                    return;
                } else if is_copy_command(self.input.modifiers, active_key) {
                    self.input.events.push(egui::Event::Copy);
                    return;
                } else if is_paste_command(self.input.modifiers, active_key) {
                    // TODO: Support clipboard
                    return;
                }
            }

            self.input.events.push(egui::Event::Key {
                key: active_key,
                physical_key,
                pressed,
                repeat: false,
                modifiers: self.input.modifiers,
            });
        }

        if let Some(text) = &text
            && !text.is_empty() && text.chars().all(is_printable_char) {
                let is_cmd = self.input.modifiers.ctrl
                    || self.input.modifiers.command
                    || self.input.modifiers.mac_cmd;
                if pressed && !is_cmd {
                    self.input.events.push(egui::Event::Text(text.to_string()));
                }
            }
    }
}

pub fn screen_size_in_pixels(window: &RenderableWindow) -> egui::Vec2 {
    let size = window.window().window.inner_size();
    egui::vec2(size.width as f32, size.height as f32)
}

pub fn pixels_per_point(context: &egui::Context, window: &RenderableWindow) -> f32 {
    let native_pixels_per_point = window.window().window.scale_factor() as f32;
    let egui_zoom_factor = context.zoom_factor();
    egui_zoom_factor * native_pixels_per_point
}

fn is_printable_char(chr: char) -> bool {
    let is_in_private_use_area = ('\u{e000}'..='\u{f8ff}').contains(&chr)
        || ('\u{f0000}'..='\u{ffffd}').contains(&chr)
        || ('\u{100000}'..='\u{10fffd}').contains(&chr);

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

fn translate_mouse_button(button: MouseButton) -> Option<egui::PointerButton> {
    match button {
        MouseButton::Left => Some(egui::PointerButton::Primary),
        MouseButton::Right => Some(egui::PointerButton::Secondary),
        MouseButton::Middle => Some(egui::PointerButton::Middle),
        MouseButton::Back => Some(egui::PointerButton::Extra1),
        MouseButton::Forward => Some(egui::PointerButton::Extra2),
        MouseButton::Other(_) => None,
    }
}

fn key_from_winit_key(key: &astrelis_winit::event::Key) -> Option<egui::Key> {
    use astrelis_winit::event::Key;

    match key {
        Key::Named(named_key) => key_from_named_key(*named_key),
        Key::Character(str) => egui::Key::from_name(str.as_str()),
        Key::Unidentified(_) | Key::Dead(_) => None,
    }
}

fn key_from_named_key(named_key: astrelis_winit::event::NamedKey) -> Option<egui::Key> {
    use astrelis_winit::event::NamedKey;
    use egui::Key;

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
            tracing::trace!("Unknown key: {named_key:?}");
            return None;
        }
    })
}

fn key_from_key_code(key: astrelis_winit::event::KeyCode) -> Option<egui::Key> {
    use astrelis_winit::event::KeyCode;
    use egui::Key;

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
        KeyCode::Space => Key::Space,
        KeyCode::Comma => Key::Comma,
        KeyCode::Period => Key::Period,
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
        _ => return None,
    })
}
