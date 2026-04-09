//! Event loop control flow types.

use std::time::Duration;

/// Controls how the event loop behaves between events.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ControlFlow {
    /// Continuously poll for events, returning immediately if none are
    /// pending. Suitable for games that render every frame.
    Poll,
    /// Block the thread until an event arrives. Suitable for applications
    /// that only redraw in response to user input.
    Wait,
    /// Block the thread until an event arrives or the timeout expires,
    /// whichever comes first. Useful for periodic updates (cursor blink,
    /// animations) without full frame-rate rendering.
    WaitUntil(Duration),
}
