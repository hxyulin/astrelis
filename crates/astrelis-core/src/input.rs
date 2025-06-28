use std::collections::HashSet;

use winit::event::ElementState;

use crate::event::{Event, KeyCode, PhysicalKey};

#[derive(Debug)]
pub struct InputSystem {
    keys_pressed: HashSet<KeyCode>,
}

impl InputSystem {
    pub fn new() -> Self {
        Self {
            keys_pressed: HashSet::new(),
        }
    }

    pub fn new_frame(&mut self) {}

    pub fn on_event(&mut self, event: &Event) {
        match event {
            Event::KeyInput(event) if !event.repeat => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => self.keys_pressed.insert(code),
                        ElementState::Released => self.keys_pressed.remove(&code),
                    };
                }
            }
            _ => {}
        }
    }

    pub fn keys_pressed(&self) -> &HashSet<KeyCode> {
        &self.keys_pressed
    }

    pub fn is_key_pressed(&self, code: &KeyCode) -> bool {
        self.keys_pressed.contains(code)
    }
}
