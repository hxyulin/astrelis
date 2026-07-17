//! Error type for retained UI operations.

use std::{error::Error, fmt};

/// Error produced by tree, layout, text, or paint operations.
#[derive(Debug)]
pub struct UiError(String);

impl UiError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }

    /// Creates an error reported by an application-defined widget.
    pub fn from_message(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for UiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for UiError {}
