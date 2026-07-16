use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use astrelis_core::geometry::{Logical, Point, Size};
use astrelis_platform::{
    DeviceEvent, DeviceId, ElementState, ImeEvent, Key, KeyCode, KeyLocation, KeyboardInput,
    Modifiers, NamedKey, NativeKey, NativeKeyCode, PhysicalKey, PointerButton, ScrollDelta,
    StartCause, Theme, Touch, TouchForce, TouchPhase, WindowEvent,
};

pub(crate) fn hash_id(value: impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

pub(crate) fn start_cause(value: winit::event::StartCause) -> StartCause {
    match value {
        winit::event::StartCause::Init => StartCause::Init,
        winit::event::StartCause::Poll => StartCause::Poll,
        winit::event::StartCause::ResumeTimeReached {
            start,
            requested_resume,
        } => StartCause::ResumeTimeReached {
            start,
            requested_resume,
        },
        winit::event::StartCause::WaitCancelled {
            start,
            requested_resume,
        } => StartCause::WaitCancelled {
            start,
            requested_resume,
        },
    }
}

pub(crate) fn monitor(value: winit::monitor::MonitorHandle) -> astrelis_platform::Monitor {
    let position = value.position();
    let size = value.size();
    astrelis_platform::Monitor {
        id: astrelis_platform::MonitorId(hash_id((
            value.name(),
            position.x,
            position.y,
            size.width,
            size.height,
        ))),
        name: value.name(),
        position: Point::new(position.x, position.y),
        size: Size::new(size.width, size.height),
        scale_factor: value.scale_factor(),
    }
}

pub(crate) fn window_event(
    value: winit::event::WindowEvent,
    current_size: Option<winit::dpi::PhysicalSize<u32>>,
) -> Option<WindowEvent> {
    use winit::event::WindowEvent as W;
    Some(match value {
        W::CloseRequested => WindowEvent::CloseRequested,
        W::Destroyed => WindowEvent::Destroyed,
        W::Resized(size) => WindowEvent::Resized(Size::new(size.width, size.height)),
        W::Moved(position) => WindowEvent::Moved(Point::new(position.x, position.y)),
        W::Focused(value) => WindowEvent::Focused(value),
        W::Occluded(value) => WindowEvent::Occluded(value),
        W::ThemeChanged(value) => WindowEvent::ThemeChanged(theme(value)),
        W::RedrawRequested => WindowEvent::RedrawRequested,
        W::ScaleFactorChanged { scale_factor, .. } => {
            let size = current_size.unwrap_or_default();
            WindowEvent::ScaleFactorChanged {
                scale_factor,
                inner_size: Size::new(size.width, size.height),
            }
        }
        W::KeyboardInput {
            device_id,
            event,
            is_synthetic,
        } => WindowEvent::KeyboardInput(KeyboardInput {
            device_id: DeviceId(hash_id(device_id)),
            physical_key: physical_key(event.physical_key),
            logical_key: logical_key(event.logical_key),
            text: event.text.map(|text| text.to_string()),
            location: location(event.location),
            state: state(event.state),
            repeat: event.repeat,
            synthetic: is_synthetic,
        }),
        W::ModifiersChanged(value) => {
            let value = value.state();
            WindowEvent::ModifiersChanged(Modifiers {
                shift: value.shift_key(),
                control: value.control_key(),
                alt: value.alt_key(),
                super_key: value.super_key(),
            })
        }
        W::Ime(value) => WindowEvent::Ime(match value {
            winit::event::Ime::Enabled => ImeEvent::Enabled,
            winit::event::Ime::Preedit(text, range) => ImeEvent::Preedit(text, range),
            winit::event::Ime::Commit(text) => ImeEvent::Commit(text),
            winit::event::Ime::Disabled => ImeEvent::Disabled,
        }),
        W::CursorEntered { device_id } => WindowEvent::PointerEntered {
            device_id: DeviceId(hash_id(device_id)),
        },
        W::CursorLeft { device_id } => WindowEvent::PointerLeft {
            device_id: DeviceId(hash_id(device_id)),
        },
        W::CursorMoved {
            device_id,
            position,
        } => WindowEvent::PointerMoved {
            device_id: DeviceId(hash_id(device_id)),
            position: Point::new(position.x, position.y),
        },
        W::MouseInput {
            device_id,
            state: value,
            button: value_button,
        } => WindowEvent::PointerButton {
            device_id: DeviceId(hash_id(device_id)),
            button: button(value_button),
            state: state(value),
        },
        W::MouseWheel {
            device_id,
            delta,
            phase: value_phase,
        } => WindowEvent::PointerWheel {
            device_id: DeviceId(hash_id(device_id)),
            delta: scroll(delta),
            phase: phase(value_phase),
        },
        W::Touch(value) => WindowEvent::Touch(touch(value)),
        W::PinchGesture {
            device_id,
            delta,
            phase: value,
        } => WindowEvent::PinchGesture {
            device_id: DeviceId(hash_id(device_id)),
            delta,
            phase: phase(value),
        },
        W::PanGesture {
            device_id,
            delta,
            phase: value,
        } => WindowEvent::PanGesture {
            device_id: DeviceId(hash_id(device_id)),
            delta: Point::<Logical, f64>::new(f64::from(delta.x), f64::from(delta.y)),
            phase: phase(value),
        },
        W::RotationGesture {
            device_id,
            delta,
            phase: value,
        } => WindowEvent::RotationGesture {
            device_id: DeviceId(hash_id(device_id)),
            delta_degrees: delta,
            phase: phase(value),
        },
        W::DoubleTapGesture { device_id } => WindowEvent::DoubleTapGesture {
            device_id: DeviceId(hash_id(device_id)),
        },
        W::HoveredFile(path) => WindowEvent::HoveredFile(path),
        W::HoveredFileCancelled => WindowEvent::HoveredFileCancelled,
        W::DroppedFile(path) => WindowEvent::DroppedFile(path),
        _ => return None,
    })
}

pub(crate) fn device_event(value: winit::event::DeviceEvent) -> Option<DeviceEvent> {
    Some(match value {
        winit::event::DeviceEvent::Added => DeviceEvent::Added,
        winit::event::DeviceEvent::Removed => DeviceEvent::Removed,
        winit::event::DeviceEvent::MouseMotion { delta } => DeviceEvent::MouseMotion { delta },
        winit::event::DeviceEvent::MouseWheel { delta } => DeviceEvent::MouseWheel(scroll(delta)),
        winit::event::DeviceEvent::Motion { axis, value } => DeviceEvent::Motion { axis, value },
        winit::event::DeviceEvent::Button {
            button,
            state: value,
        } => DeviceEvent::Button {
            button,
            state: state(value),
        },
        winit::event::DeviceEvent::Key(value) => DeviceEvent::Key {
            physical_key: physical_key(value.physical_key),
            state: state(value.state),
            repeat: false,
        },
    })
}

fn state(value: winit::event::ElementState) -> ElementState {
    match value {
        winit::event::ElementState::Pressed => ElementState::Pressed,
        winit::event::ElementState::Released => ElementState::Released,
    }
}
fn phase(value: winit::event::TouchPhase) -> TouchPhase {
    match value {
        winit::event::TouchPhase::Started => TouchPhase::Started,
        winit::event::TouchPhase::Moved => TouchPhase::Moved,
        winit::event::TouchPhase::Ended => TouchPhase::Ended,
        winit::event::TouchPhase::Cancelled => TouchPhase::Cancelled,
    }
}
fn button(value: winit::event::MouseButton) -> PointerButton {
    match value {
        winit::event::MouseButton::Left => PointerButton::Primary,
        winit::event::MouseButton::Right => PointerButton::Secondary,
        winit::event::MouseButton::Middle => PointerButton::Middle,
        winit::event::MouseButton::Back => PointerButton::Back,
        winit::event::MouseButton::Forward => PointerButton::Forward,
        winit::event::MouseButton::Other(value) => PointerButton::Other(value),
    }
}
fn scroll(value: winit::event::MouseScrollDelta) -> ScrollDelta {
    match value {
        winit::event::MouseScrollDelta::LineDelta(x, y) => ScrollDelta::Lines { x, y },
        winit::event::MouseScrollDelta::PixelDelta(value) => {
            ScrollDelta::Pixels(Point::new(value.x, value.y))
        }
    }
}
fn theme(value: winit::window::Theme) -> Theme {
    match value {
        winit::window::Theme::Light => Theme::Light,
        winit::window::Theme::Dark => Theme::Dark,
    }
}
fn location(value: winit::keyboard::KeyLocation) -> KeyLocation {
    match value {
        winit::keyboard::KeyLocation::Standard => KeyLocation::Standard,
        winit::keyboard::KeyLocation::Left => KeyLocation::Left,
        winit::keyboard::KeyLocation::Right => KeyLocation::Right,
        winit::keyboard::KeyLocation::Numpad => KeyLocation::Numpad,
    }
}
fn physical_key(value: winit::keyboard::PhysicalKey) -> PhysicalKey {
    match value {
        winit::keyboard::PhysicalKey::Code(code) => PhysicalKey::Code(key_code(code)),
        winit::keyboard::PhysicalKey::Unidentified(value) => match native_code(value) {
            Some(value) => PhysicalKey::Native(value),
            None => PhysicalKey::Unidentified,
        },
    }
}
fn native_code(value: winit::keyboard::NativeKeyCode) -> Option<NativeKeyCode> {
    match value {
        winit::keyboard::NativeKeyCode::Unidentified => None,
        winit::keyboard::NativeKeyCode::Android(v) => Some(NativeKeyCode::Android(v)),
        winit::keyboard::NativeKeyCode::MacOS(v) => Some(NativeKeyCode::MacOS(v)),
        winit::keyboard::NativeKeyCode::Windows(v) => Some(NativeKeyCode::Windows(v)),
        winit::keyboard::NativeKeyCode::Xkb(v) => Some(NativeKeyCode::Xkb(v)),
    }
}
fn key_code(value: winit::keyboard::KeyCode) -> KeyCode {
    use winit::keyboard::KeyCode as W;
    match value {
        W::KeyA => KeyCode::KeyA,
        W::KeyB => KeyCode::KeyB,
        W::KeyC => KeyCode::KeyC,
        W::KeyD => KeyCode::KeyD,
        W::KeyE => KeyCode::KeyE,
        W::KeyF => KeyCode::KeyF,
        W::KeyG => KeyCode::KeyG,
        W::KeyH => KeyCode::KeyH,
        W::KeyI => KeyCode::KeyI,
        W::KeyJ => KeyCode::KeyJ,
        W::KeyK => KeyCode::KeyK,
        W::KeyL => KeyCode::KeyL,
        W::KeyM => KeyCode::KeyM,
        W::KeyN => KeyCode::KeyN,
        W::KeyO => KeyCode::KeyO,
        W::KeyP => KeyCode::KeyP,
        W::KeyQ => KeyCode::KeyQ,
        W::KeyR => KeyCode::KeyR,
        W::KeyS => KeyCode::KeyS,
        W::KeyT => KeyCode::KeyT,
        W::KeyU => KeyCode::KeyU,
        W::KeyV => KeyCode::KeyV,
        W::KeyW => KeyCode::KeyW,
        W::KeyX => KeyCode::KeyX,
        W::KeyY => KeyCode::KeyY,
        W::KeyZ => KeyCode::KeyZ,
        W::Escape => KeyCode::Escape,
        W::Enter => KeyCode::Enter,
        W::Space => KeyCode::Space,
        W::Tab => KeyCode::Tab,
        W::Backspace => KeyCode::Backspace,
        value => KeyCode::Other(format!("{value:?}")),
    }
}
fn logical_key(value: winit::keyboard::Key) -> Key {
    match value {
        winit::keyboard::Key::Character(value) => Key::Character(value.to_string()),
        winit::keyboard::Key::Named(value) => Key::Named(match value {
            winit::keyboard::NamedKey::Alt => NamedKey::Alt,
            winit::keyboard::NamedKey::Control => NamedKey::Control,
            winit::keyboard::NamedKey::Shift => NamedKey::Shift,
            winit::keyboard::NamedKey::Super => NamedKey::Super,
            winit::keyboard::NamedKey::Enter => NamedKey::Enter,
            winit::keyboard::NamedKey::Escape => NamedKey::Escape,
            winit::keyboard::NamedKey::Space => NamedKey::Space,
            winit::keyboard::NamedKey::Tab => NamedKey::Tab,
            winit::keyboard::NamedKey::Backspace => NamedKey::Backspace,
            value => NamedKey::Other(format!("{value:?}")),
        }),
        winit::keyboard::Key::Unidentified(value) => match value {
            winit::keyboard::NativeKey::Unidentified => Key::Unidentified,
            winit::keyboard::NativeKey::Android(v) => Key::Native(NativeKey::Android(v)),
            winit::keyboard::NativeKey::MacOS(v) => Key::Native(NativeKey::MacOS(v)),
            winit::keyboard::NativeKey::Windows(v) => Key::Native(NativeKey::Windows(v)),
            winit::keyboard::NativeKey::Xkb(v) => Key::Native(NativeKey::Xkb(v)),
            winit::keyboard::NativeKey::Web(v) => Key::Native(NativeKey::Web(v.to_string())),
        },
        winit::keyboard::Key::Dead(value) => {
            Key::Named(NamedKey::Other(format!("Dead({value:?})")))
        }
    }
}
fn touch(value: winit::event::Touch) -> Touch {
    Touch {
        device_id: DeviceId(hash_id(value.device_id)),
        phase: phase(value.phase),
        position: Point::new(value.location.x, value.location.y),
        id: value.id,
        force: value.force.map(|force| match force {
            winit::event::Force::Calibrated {
                force,
                max_possible_force,
                altitude_angle,
            } => TouchForce::Calibrated {
                force,
                max_possible_force,
                altitude_angle,
            },
            winit::event::Force::Normalized(value) => TouchForce::Normalized(value),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_native_physical_keys() {
        assert_eq!(
            physical_key(winit::keyboard::PhysicalKey::Unidentified(
                winit::keyboard::NativeKeyCode::Xkb(271)
            )),
            PhysicalKey::Native(NativeKeyCode::Xkb(271))
        );
    }

    #[test]
    fn maps_pointer_and_scroll_values() {
        assert_eq!(
            button(winit::event::MouseButton::Other(8)),
            PointerButton::Other(8)
        );
        assert_eq!(
            scroll(winit::event::MouseScrollDelta::LineDelta(1.0, -2.0)),
            ScrollDelta::Lines { x: 1.0, y: -2.0 }
        );
    }
}
