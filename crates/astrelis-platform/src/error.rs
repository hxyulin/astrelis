//! Platform error types.

use std::{error::Error, fmt};

/// An error reported by a platform backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformError {
    message: String,
}

impl PlatformError {
    /// Creates an error with a human-readable message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for PlatformError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for PlatformError {}

/// The result of sending an event to a closed event loop.
#[derive(Debug)]
pub struct EventLoopClosed<T>(pub T);

impl<T> fmt::Display for EventLoopClosed<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the event loop is closed")
    }
}

impl<T: fmt::Debug> Error for EventLoopClosed<T> {}
