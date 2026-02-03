use astrelis_core::geometry::{LogicalPosition, LogicalSize, PhysicalPosition};
use astrelis_core::profiling::profile_function;
pub use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent as WinitEvent};
pub use winit::keyboard::*;

use std::collections::VecDeque;

/// Touch phase for gesture state tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TouchPhase {
    /// Touch or gesture started.
    Started,
    /// Touch or gesture moved/updated.
    Moved,
    /// Touch or gesture ended.
    Ended,
    /// Touch or gesture was cancelled.
    Cancelled,
}

impl From<winit::event::TouchPhase> for TouchPhase {
    fn from(phase: winit::event::TouchPhase) -> Self {
        match phase {
            winit::event::TouchPhase::Started => TouchPhase::Started,
            winit::event::TouchPhase::Moved => TouchPhase::Moved,
            winit::event::TouchPhase::Ended => TouchPhase::Ended,
            winit::event::TouchPhase::Cancelled => TouchPhase::Cancelled,
        }
    }
}

/// Individual touch point event.
#[derive(Debug, Clone)]
pub struct TouchEvent {
    /// Device ID that generated this touch.
    pub device_id: u64,
    /// Unique identifier for this touch point.
    pub id: u64,
    /// Current phase of the touch.
    pub phase: TouchPhase,
    /// Position of the touch in logical coordinates.
    pub position: LogicalPosition<f64>,
    /// Force of the touch (normalized 0.0 to 1.0), if available.
    pub force: Option<f32>,
}

/// Pinch gesture for zoom operations.
#[derive(Debug, Clone)]
pub struct PinchGesture {
    /// Scale delta: positive = magnify, negative = shrink.
    pub delta: f64,
    /// Current phase of the gesture.
    pub phase: TouchPhase,
}

/// Rotation gesture for rotation operations.
#[derive(Debug, Clone)]
pub struct RotationGesture {
    /// Rotation delta in radians: positive = counter-clockwise, negative = clockwise.
    pub delta: f64,
    /// Current phase of the gesture.
    pub phase: TouchPhase,
}

/// Pan gesture for two-finger scrolling/panning.
#[derive(Debug, Clone)]
pub struct PanGesture {
    /// Pan delta in logical coordinates.
    pub delta: LogicalPosition<f64>,
    /// Current phase of the gesture.
    pub phase: TouchPhase,
}

/// Event queue with batching and deduplication
pub struct EventQueue {
    /// Pending events for this frame
    pending: VecDeque<Event>,

    /// High-priority events (processed first)
    priority: VecDeque<Event>,

    /// Deduplicated events (only last value kept)
    latest_mouse_pos: Option<LogicalPosition<f64>>,
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
            Event::CloseRequested
            | Event::WindowResized(_)
            | Event::Focused(_)
            | Event::ThemeChanged(_) => {
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
        let mut events = Vec::with_capacity(self.priority.len() + self.pending.len() + 2);

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

impl Default for EventQueue {
    fn default() -> Self {
        Self::new()
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
        profile_function!();
        self.events.retain(|event| {
            let status = handler(event);
            !status.is_consumed()
        });
    }
}

#[derive(Default, Debug, Clone)]
pub struct EventStats {
    pub events_received: usize,
    pub events_processed: usize,
    pub events_dropped: usize,
}

/// System theme preference (light or dark).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SystemTheme {
    /// Light appearance.
    Light,
    /// Dark appearance.
    Dark,
}

#[derive(Debug, Clone)]
pub enum Event {
    /// Window moved to a new physical position.
    WindowMoved(PhysicalPosition<i32>),
    /// Window resized to a new logical size.
    WindowResized(LogicalSize<u32>),
    /// Scale factor changed.
    ScaleFactorChanged(f64),
    /// Window focus changed.
    Focused(bool),
    /// Window close requested.
    CloseRequested,
    /// The system theme (light/dark) changed.
    ThemeChanged(SystemTheme),
    /// Mouse button pressed.
    MouseButtonDown(MouseButton),
    /// Mouse button released.
    MouseButtonUp(MouseButton),
    /// Mouse wheel scrolled.
    MouseScrolled(MouseScrollDelta),
    /// Mouse cursor moved (logical coordinates).
    MouseMoved(LogicalPosition<f64>),
    /// Mouse cursor entered the window.
    MouseEntered,
    /// Mouse cursor left the window.
    MouseLeft,
    /// Keyboard input event.
    KeyInput(KeyEvent),
    /// Touch event (touchscreen or trackpad).
    Touch(TouchEvent),
    /// Pinch gesture (zoom).
    PinchGesture(PinchGesture),
    /// Rotation gesture.
    RotationGesture(RotationGesture),
    /// Pan gesture (two-finger scroll).
    PanGesture(PanGesture),
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

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct HandleStatus: u8 {
        const HANDLED = 0b00000001;
        const CONSUMED = 0b00000010;
    }
}

impl HandleStatus {
    pub const fn is_consumed(&self) -> bool {
        self.contains(Self::CONSUMED)
    }

    pub const fn is_handled(&self) -> bool {
        self.contains(Self::HANDLED)
    }

    pub const fn consumed() -> Self {
        Self::from_bits_truncate(Self::HANDLED.bits() | Self::CONSUMED.bits())
    }

    pub const fn handled() -> Self {
        Self::from_bits_truncate(Self::HANDLED.bits())
    }

    pub const fn ignored() -> Self {
        Self::empty()
    }
}

impl Event {
    pub(crate) fn from_winit(event: winit::event::WindowEvent, scale_factor: f64) -> Option<Self> {
        match event {
            WinitEvent::Moved(pos) => Some(Event::WindowMoved(pos.into())),
            WinitEvent::Resized(size) => Some(Event::WindowResized(LogicalSize::new(
                (size.width as f64 / scale_factor) as u32,
                (size.height as f64 / scale_factor) as u32,
            ))),
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
            } => Some(Event::MouseMoved(LogicalPosition::new(
                position.x / scale_factor,
                position.y / scale_factor,
            ))),
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
            WinitEvent::Touch(touch) => {
                let force = match touch.force {
                    Some(winit::event::Force::Normalized(f)) => Some(f as f32),
                    Some(winit::event::Force::Calibrated { force, max_possible_force, .. }) => {
                        Some((force / max_possible_force) as f32)
                    }
                    None => None,
                };
                // DeviceId doesn't expose its inner value, so we use a hash for uniqueness
                let device_id = {
                    use std::hash::{Hash, Hasher};
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    touch.device_id.hash(&mut hasher);
                    hasher.finish()
                };
                Some(Event::Touch(TouchEvent {
                    device_id,
                    id: touch.id,
                    phase: touch.phase.into(),
                    position: LogicalPosition::new(
                        touch.location.x / scale_factor,
                        touch.location.y / scale_factor,
                    ),
                    force,
                }))
            }
            WinitEvent::PinchGesture { delta, phase, .. } => {
                Some(Event::PinchGesture(PinchGesture {
                    delta,
                    phase: phase.into(),
                }))
            }
            WinitEvent::RotationGesture { delta, phase, .. } => {
                Some(Event::RotationGesture(RotationGesture {
                    delta: delta as f64,
                    phase: phase.into(),
                }))
            }
            WinitEvent::PanGesture { delta, phase, .. } => {
                Some(Event::PanGesture(PanGesture {
                    delta: LogicalPosition::new(
                        delta.x as f64 / scale_factor,
                        delta.y as f64 / scale_factor,
                    ),
                    phase: phase.into(),
                }))
            }
            WinitEvent::ThemeChanged(theme) => Some(Event::ThemeChanged(match theme {
                winit::window::Theme::Light => SystemTheme::Light,
                winit::window::Theme::Dark => SystemTheme::Dark,
            })),
            // We explicitly ignore touchpad pressure (deprecated in favor of Force in Touch)
            WinitEvent::TouchpadPressure { .. } => None,
            // DoubleTapGesture is macOS-specific and we don't have a use case for it yet
            WinitEvent::DoubleTapGesture { .. } => None,
            unknown => {
                tracing::warn!("unhandled window event: {:?}", unknown);
                None
            }
        }
    }
}
