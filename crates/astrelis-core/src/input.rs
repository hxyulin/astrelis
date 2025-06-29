use std::collections::HashSet;

use glam::Vec2;
use winit::event::{ElementState, MouseScrollDelta};

use crate::event::{Event, KeyCode, PhysicalKey};

#[derive(Debug)]
pub struct InputSystem {
    keys_pressed: HashSet<KeyCode>,
    scroll_delta: Vec2,
    mouse_pos: Vec2,
    mouse_delta: Vec2,
}

impl InputSystem {
    pub fn new() -> Self {
        Self {
            keys_pressed: HashSet::new(),
            scroll_delta: Vec2::ZERO,
            mouse_pos: Vec2::ZERO,
            mouse_delta: Vec2::ZERO,
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
            Event::MouseScrolled(delta) => match delta {
                MouseScrollDelta::LineDelta(x_delta, y_delta) => {
                    const LINE_SCROLL_DELTA: f32 = 10.0;
                    self.scroll_delta += Vec2::new(*x_delta, *y_delta) * LINE_SCROLL_DELTA
                }

                MouseScrollDelta::PixelDelta(delta) => {
                    self.scroll_delta += Vec2::new(delta.x as f32, delta.y as f32)
                }
            },
            Event::MouseMoved(pos) => {
                let new_pos = Vec2::new(pos.x as f32, pos.y as f32);
                self.mouse_delta = new_pos - self.mouse_pos;
                self.mouse_pos = new_pos;
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

    pub fn scroll_delta(&self) -> Vec2 {
        self.scroll_delta
    }

    pub fn mouse_delta(&self) -> Vec2 {
        self.mouse_delta
    }

    pub fn mouse_pos(&self) -> Vec2 {
        self.mouse_pos
    }
}
