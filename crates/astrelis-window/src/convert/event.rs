//! Window event conversions from winit to astrelis.

use astrelis_core::geometry::{Physical, Point};

use crate::event::{
    ElementState, ImeEvent, KeyEvent, TouchEvent, TouchId, TouchPhase, WindowEvent,
};
use crate::types::{InnerSize, OuterPosition};

use super::keyboard;
use super::mouse;

/// Converts a winit ElementState to astrelis.
fn convert_element_state(state: winit::event::ElementState) -> ElementState {
    match state {
        winit::event::ElementState::Pressed => ElementState::Pressed,
        winit::event::ElementState::Released => ElementState::Released,
    }
}

/// Converts a winit WindowEvent to an astrelis WindowEvent.
///
/// Returns `None` for events that have no astrelis equivalent.
pub(crate) fn convert_window_event(
    event: winit::event::WindowEvent,
) -> Option<WindowEvent> {
    astrelis_profiling::profile_function!();
    Some(match event {
        winit::event::WindowEvent::CloseRequested => WindowEvent::CloseRequested,

        winit::event::WindowEvent::Destroyed => WindowEvent::Destroyed,

        winit::event::WindowEvent::Resized(size) => {
            WindowEvent::Resized(InnerSize::new(size.width as f32, size.height as f32))
        }

        winit::event::WindowEvent::Moved(pos) => {
            WindowEvent::Moved(OuterPosition::new(pos.x as f32, pos.y as f32))
        }

        winit::event::WindowEvent::ScaleFactorChanged {
            scale_factor,
            inner_size_writer: _,
        } => {
            // winit 0.30 doesn't give us the new size directly in the event;
            // the actual new size comes in a subsequent Resized event.
            // We emit ScaleFactorChanged with a zero size; the app should
            // rely on Resized for the actual new dimensions.
            WindowEvent::ScaleFactorChanged {
                scale_factor: scale_factor as f32,
                new_inner_size: InnerSize::new(0.0, 0.0),
            }
        }

        winit::event::WindowEvent::ThemeChanged(theme) => {
            let t = match theme {
                winit::window::Theme::Light => crate::theme::Theme::Light,
                winit::window::Theme::Dark => crate::theme::Theme::Dark,
            };
            WindowEvent::ThemeChanged(t)
        }

        winit::event::WindowEvent::RedrawRequested => WindowEvent::RedrawRequested,

        winit::event::WindowEvent::Focused(focused) => WindowEvent::Focused(focused),

        winit::event::WindowEvent::Occluded(occluded) => WindowEvent::Occluded(occluded),

        winit::event::WindowEvent::KeyboardInput {
            event,
            is_synthetic: _,
            ..
        } => {
            let key_event = KeyEvent {
                key_code: keyboard::convert_key_code(event.physical_key),
                key: keyboard::convert_key(&event.logical_key),
                state: convert_element_state(event.state),
                location: keyboard::convert_key_location(event.location),
                repeat: event.repeat,
            };
            WindowEvent::KeyboardInput(key_event)
        }

        winit::event::WindowEvent::ModifiersChanged(mods) => {
            WindowEvent::ModifiersChanged(keyboard::convert_modifiers(&mods))
        }

        winit::event::WindowEvent::Ime(ime) => {
            let ime_event = match ime {
                winit::event::Ime::Enabled => ImeEvent::Enabled,
                winit::event::Ime::Preedit(text, cursor) => ImeEvent::Preedit(text, cursor),
                winit::event::Ime::Commit(text) => ImeEvent::Commit(text),
                winit::event::Ime::Disabled => ImeEvent::Disabled,
            };
            WindowEvent::Ime(ime_event)
        }

        winit::event::WindowEvent::CursorEntered { .. } => WindowEvent::CursorEntered,

        winit::event::WindowEvent::CursorLeft { .. } => WindowEvent::CursorLeft,

        winit::event::WindowEvent::CursorMoved { position, .. } => {
            WindowEvent::CursorMoved(Point::<Physical>::new(
                position.x as f32,
                position.y as f32,
            ))
        }

        winit::event::WindowEvent::MouseInput { button, state, .. } => {
            WindowEvent::MouseButtonInput {
                button: mouse::convert_mouse_button(button),
                state: convert_element_state(state),
            }
        }

        winit::event::WindowEvent::MouseWheel { delta, .. } => {
            WindowEvent::MouseWheel(mouse::convert_scroll_delta(delta))
        }

        winit::event::WindowEvent::Touch(touch) => {
            let phase = match touch.phase {
                winit::event::TouchPhase::Started => TouchPhase::Started,
                winit::event::TouchPhase::Moved => TouchPhase::Moved,
                winit::event::TouchPhase::Ended => TouchPhase::Ended,
                winit::event::TouchPhase::Cancelled => TouchPhase::Cancelled,
            };
            WindowEvent::Touch(TouchEvent {
                id: TouchId(touch.id),
                phase,
                position: Point::<Physical>::new(
                    touch.location.x as f32,
                    touch.location.y as f32,
                ),
            })
        }

        winit::event::WindowEvent::DroppedFile(path) => {
            WindowEvent::DroppedFile(vec![path])
        }

        winit::event::WindowEvent::HoveredFile(path) => {
            WindowEvent::DroppedFileHovered(vec![path])
        }

        winit::event::WindowEvent::HoveredFileCancelled => WindowEvent::DroppedFileCancelled,

        // Events we don't map
        _ => return None,
    })
}
