use crate::window::{PhysicalPosition, PhysicalSize};
pub use winit::event::WindowEvent as WinitEvent;
pub use winit::event::{ElementState, MouseButton, MouseScrollDelta};
pub use winit::keyboard::*;

#[derive(Debug, Clone)]
pub enum Event {
    WindowMoved(PhysicalPosition<i32>),
    WindowResized(PhysicalSize<u32>),
    ScaleFactorChanged(f64),
    Focused(bool),
    CloseRequested,
    MouseButtonDown(MouseButton),
    MouseButtonUp(MouseButton),
    MouseScrolled(MouseScrollDelta),
    MouseMoved(PhysicalPosition<f64>),
    MouseEntered,
    MouseLeft,
    KeyInput(KeyEvent),
}

#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub physical_key: PhysicalKey,
    pub logical_key: Key,
    pub text: Option<SmolStr>,
    pub location: KeyLocation,
    pub state: ElementState,
    pub repeat: bool,
    pub is_synthetic: bool,
}

pub struct HandleStatus {
    pub handled: bool,
    pub consumed: bool,
}

impl HandleStatus {
    pub const fn consumed() -> Self {
        Self {
            handled: true,
            consumed: true,
        }
    }

    pub const fn handled() -> Self {
        Self {
            handled: true,
            consumed: false,
        }
    }

    pub const fn ignored() -> Self {
        Self {
            handled: false,
            consumed: false,
        }
    }
}

impl Event {
    pub(crate) fn from_winit(event: winit::event::WindowEvent) -> Option<Self> {
        match event {
            WinitEvent::Moved(pos) => Some(Event::WindowMoved(pos)),
            WinitEvent::Resized(size) => Some(Event::WindowResized(size)),
            WinitEvent::ScaleFactorChanged {
                scale_factor,
                inner_size_writer: _,
            } => Some(Event::ScaleFactorChanged(scale_factor)),
            WinitEvent::Focused(focus) => Some(Event::Focused(focus)),
            WinitEvent::CloseRequested => Some(Event::CloseRequested),
            WinitEvent::MouseInput {
                device_id: _,
                state,
                button,
            } => match state {
                ElementState::Pressed => Some(Event::MouseButtonDown(button)),
                ElementState::Released => Some(Event::MouseButtonUp(button)),
            },
            WinitEvent::MouseWheel {
                device_id: _,
                delta,
                phase: _,
            } => Some(Event::MouseScrolled(delta)),
            WinitEvent::CursorMoved {
                device_id: _,
                position,
            } => Some(Event::MouseMoved(position)),
            WinitEvent::CursorEntered { device_id: _ } => Some(Event::MouseEntered),
            WinitEvent::CursorLeft { device_id: _ } => Some(Event::MouseLeft),
            WinitEvent::KeyboardInput {
                device_id: _,
                event,
                is_synthetic,
            } => Some(Event::KeyInput(KeyEvent {
                physical_key: event.physical_key,
                logical_key: event.logical_key,
                location: event.location,
                repeat: event.repeat,
                text: event.text,
                state: event.state,

                is_synthetic,
            })),
            unknown => {
                tracing::warn!("unhandled window event: {:?}", unknown);
                None
            }
        }
    }
}
