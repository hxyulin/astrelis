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
    astrelis_profiling::init();
    astrelis_profiling::set_thread_name("main");
    astrelis_core::logging::init_default();

    let mut input = InputState::new();

    // --- Frame 1: press W, move cursor, click left mouse, scroll, raw mouse motion ---
    astrelis_profiling::profile_scope!("frame_1");
    input.begin_frame();
    input.handle_event(&key_event(KeyCode::KeyW, ElementState::Pressed));
    input.handle_event(&WindowEvent::CursorMoved(Point::<Physical>::new(320.0, 240.0)));
    input.handle_event(&WindowEvent::MouseButtonInput {
        button: MouseButton::Left,
        state: ElementState::Pressed,
    });
    input.handle_event(&WindowEvent::MouseWheel(MouseScrollDelta::LineDelta { x: 0.0, y: 3.0 }));
    input.handle_device_event(&DeviceEvent::MouseMotion { delta_x: 5.5, delta_y: -2.0 });

    tracing::info!("=== Frame 1 ===");
    tracing::info!("W just pressed: {}", input.is_key_just_pressed(KeyCode::KeyW));
    tracing::info!("W pressed:      {}", input.is_key_pressed(KeyCode::KeyW));
    tracing::info!("Mouse pos:      {:?}", input.mouse_position());
    tracing::info!("Left btn:       {}", input.is_mouse_button_pressed(MouseButton::Left));
    tracing::info!("Scroll delta:   {:?}", input.scroll_delta());
    tracing::info!("Mouse delta:    {:?}", input.mouse_delta());

    astrelis_profiling::new_frame();

    // --- Frame 2: hold continues, release left mouse ---
    astrelis_profiling::profile_scope!("frame_2");
    input.begin_frame();
    input.handle_event(&WindowEvent::MouseButtonInput {
        button: MouseButton::Left,
        state: ElementState::Released,
    });

    tracing::info!("=== Frame 2 ===");
    tracing::info!("W just pressed: {}", input.is_key_just_pressed(KeyCode::KeyW));
    tracing::info!("W still held:   {}", input.is_key_pressed(KeyCode::KeyW));
    tracing::info!("Left released:  {}", input.is_mouse_button_just_released(MouseButton::Left));
    tracing::info!("Scroll (reset): {:?}", input.scroll_delta());

    astrelis_profiling::new_frame();
    astrelis_profiling::finish();
}
