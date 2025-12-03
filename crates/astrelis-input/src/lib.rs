use astrelis_winit::event::{EventBatch, HandleStatus};

pub struct InputState {

}

pub struct InputSystem {
    state: InputState,
}

impl InputState {
    pub fn new() -> Self {
        InputState {}
    }

    pub fn handle_events(&mut self, events: &mut EventBatch) {
        events.dispatch(|event| {
            HandleStatus::ignored()
        });
    }
}
