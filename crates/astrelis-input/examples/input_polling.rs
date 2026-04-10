//! Demonstrates the `InputState` polling API with synthetic events (no windowing backend needed).

use astrelis_core::geometry::{Physical, Point};
use astrelis_input::InputState;
use astrelis_window::event::{DeviceEvent, ElementState, KeyEvent, WindowEvent};
use astrelis_window::keyboard::{Key, KeyCode, KeyLocation, NamedKey};
use astrelis_window::mouse::{MouseButton, MouseScrollDelta};

fn key_event(code: KeyCode, state: ElementState) -> WindowEvent {
    WindowEvent::KeyboardInput(KeyEvent {
        key_code: code,
        key: Key::Named(NamedKey::Space),
        state,
        location: KeyLocation::Standard,
        repeat: false,
    })
}

fn main() {
    let mut input = InputState::new();

    // --- Frame 1: press W, move cursor, click left mouse, scroll, raw mouse motion ---
    input.begin_frame();
    input.handle_event(&key_event(KeyCode::KeyW, ElementState::Pressed));
    input.handle_event(&WindowEvent::CursorMoved(Point::<Physical>::new(320.0, 240.0)));
    input.handle_event(&WindowEvent::MouseButtonInput {
        button: MouseButton::Left,
        state: ElementState::Pressed,
    });
    input.handle_event(&WindowEvent::MouseWheel(MouseScrollDelta::LineDelta { x: 0.0, y: 3.0 }));
    input.handle_device_event(&DeviceEvent::MouseMotion { delta_x: 5.5, delta_y: -2.0 });

    println!("=== Frame 1 ===");
    println!("W just pressed: {}", input.is_key_just_pressed(KeyCode::KeyW));
    println!("W pressed:      {}", input.is_key_pressed(KeyCode::KeyW));
    println!("Mouse pos:      {:?}", input.mouse_position());
    println!("Left btn:       {}", input.is_mouse_button_pressed(MouseButton::Left));
    println!("Scroll delta:   {:?}", input.scroll_delta());
    println!("Mouse delta:    {:?}", input.mouse_delta());

    // --- Frame 2: hold continues, release left mouse ---
    input.begin_frame();
    input.handle_event(&WindowEvent::MouseButtonInput {
        button: MouseButton::Left,
        state: ElementState::Released,
    });

    println!("\n=== Frame 2 ===");
    println!("W just pressed: {}", input.is_key_just_pressed(KeyCode::KeyW));
    println!("W still held:   {}", input.is_key_pressed(KeyCode::KeyW));
    println!("Left released:  {}", input.is_mouse_button_just_released(MouseButton::Left));
    println!("Scroll (reset): {:?}", input.scroll_delta());
}
