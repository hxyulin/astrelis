//! Backend-neutral text clipboard access.

use std::{fmt, sync::Arc};

use crate::{PlatformError, backend};

/// Cloneable handle to the operating-system text clipboard.
#[derive(Clone)]
pub struct Clipboard {
    inner: Arc<dyn backend::Clipboard>,
}

impl Clipboard {
    /// Wraps a backend clipboard implementation.
    pub fn from_backend(inner: Arc<dyn backend::Clipboard>) -> Self {
        Self { inner }
    }

    /// Reads text from the clipboard.
    pub fn read_text(&self) -> Result<Option<String>, PlatformError> {
        self.inner.read_text()
    }

    /// Replaces the clipboard contents with text.
    pub fn write_text(&self, text: impl Into<String>) -> Result<(), PlatformError> {
        self.inner.write_text(text.into())
    }
}

impl fmt::Debug for Clipboard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Clipboard").finish_non_exhaustive()
    }
}
