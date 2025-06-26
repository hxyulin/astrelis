use crate::window::{PhysicalPosition, PhysicalSize};
pub use winit::event::WindowEvent as WinitEvent;
pub use winit::event::{ElementState, MouseButton, MouseScrollDelta};
pub use winit::keyboard::*;

#[derive(Debug, Clone)]
pub enum Event {
    Moved(PhysicalPosition<i32>),
    Resized(PhysicalSize<u32>),
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

impl From<winit::event::KeyEvent> for KeyEvent {
    fn from(event: winit::event::KeyEvent) -> Self {
        Self {
            physical_key: event.physical_key,
            logical_key: event.logical_key,
            text: event.text,
            location: event.location,
            state: event.state,
            repeat: event.repeat,
            is_synthetic: false,
        }
    }
}

impl Event {
    pub(crate) fn from_winit(event: winit::event::WindowEvent) -> Option<Self> {
        match event {
            WinitEvent::CloseRequested => Some(Event::CloseRequested),
            unknown => {
                log::warn!("unhandled window event: {:?}", unknown);
                None
            }
        }
    }
}
