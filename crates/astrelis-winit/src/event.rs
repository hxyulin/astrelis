pub use winit::dpi::{PhysicalPosition, PhysicalSize};
pub use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent as WinitEvent};
pub use winit::keyboard::*;

use std::collections::VecDeque;

/// Event queue with batching and deduplication
pub struct EventQueue {
    /// Pending events for this frame
    pending: VecDeque<Event>,
    
    /// High-priority events (processed first)
    priority: VecDeque<Event>,
    
    /// Deduplicated events (only last value kept)
    latest_mouse_pos: Option<PhysicalPosition<f64>>,
    latest_scale_factor: Option<f64>,
    
    /// Statistics
    stats: EventStats,
}

impl EventQueue {
    pub fn new() -> Self {
        Self {
            pending: VecDeque::with_capacity(64),
            priority: VecDeque::with_capacity(8),
            latest_mouse_pos: None,
            latest_scale_factor: None,
            stats: EventStats::default(),
        }
    }
    
    /// Push event to queue (called from winit handler)
    pub fn push(&mut self, event: Event) {
        self.stats.events_received += 1;
        
        match event {
            // High priority - process immediately
            Event::CloseRequested | Event::WindowResized(_) | Event::Focused(_) => {
                self.priority.push_back(event);
            }
            
            // Deduplicate - only keep latest
            Event::MouseMoved(pos) => {
                self.latest_mouse_pos = Some(pos);
            }
            Event::ScaleFactorChanged(scale) => {
                self.latest_scale_factor = Some(scale);
            }
            
            // Normal priority
            _ => {
                self.pending.push_back(event);
            }
        }
    }
    
    /// Process all events and return batch
    pub fn drain(&mut self) -> EventBatch {
        let mut events = Vec::with_capacity(
            self.priority.len() + self.pending.len() + 2
        );
        
        // Priority events first
        events.extend(self.priority.drain(..));
        
        // Deduplicated events
        if let Some(pos) = self.latest_mouse_pos.take() {
            events.push(Event::MouseMoved(pos));
        }
        if let Some(scale) = self.latest_scale_factor.take() {
            events.push(Event::ScaleFactorChanged(scale));
        }
        
        // Regular events
        events.extend(self.pending.drain(..));
        
        self.stats.events_processed += events.len();
        self.stats.events_dropped = self.stats.events_received - self.stats.events_processed;
        
        EventBatch { events }
    }
    
    pub fn stats(&self) -> &EventStats {
        &self.stats
    }
    
    pub fn reset_stats(&mut self) {
        self.stats = EventStats::default();
    }
}

pub struct EventBatch {
    events: Vec<Event>,
}

impl EventBatch {
    pub fn iter(&self) -> impl Iterator<Item = &Event> {
        self.events.iter()
    }
    
    pub fn len(&self) -> usize {
        self.events.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn dispatch<H>(&mut self, mut handler: H)
    where
        H: FnMut(&Event) -> HandleStatus,
    {
        self.events.retain(|event| {
            let status = handler(event);
            !status.consumed
        });
    }
}

#[derive(Default, Debug, Clone)]
pub struct EventStats {
    pub events_received: usize,
    pub events_processed: usize,
    pub events_dropped: usize,
}

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
