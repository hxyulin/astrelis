//! Asset events for change detection.

use std::any::TypeId;

use crate::handle::{HandleId, UntypedHandle};

/// Events emitted by the asset system.
#[derive(Debug, Clone)]
pub enum AssetEvent {
    /// An asset was created (first load completed).
    Created {
        /// The handle to the created asset.
        handle: UntypedHandle,
        /// The type of the asset.
        type_id: TypeId,
        /// The version of the asset.
        version: u32,
    },

    /// An asset was modified (reloaded).
    Modified {
        /// The handle to the modified asset.
        handle: UntypedHandle,
        /// The type of the asset.
        type_id: TypeId,
        /// The new version of the asset.
        version: u32,
    },

    /// An asset was removed/unloaded.
    Removed {
        /// The handle ID of the removed asset.
        handle_id: HandleId,
        /// The type of the asset.
        type_id: TypeId,
    },

    /// An asset failed to load.
    LoadFailed {
        /// The handle to the failed asset.
        handle: UntypedHandle,
        /// The type of the asset.
        type_id: TypeId,
        /// Error message.
        error: String,
    },
}

impl AssetEvent {
    /// Get the type ID of the asset this event relates to.
    pub fn type_id(&self) -> TypeId {
        match self {
            AssetEvent::Created { type_id, .. } => *type_id,
            AssetEvent::Modified { type_id, .. } => *type_id,
            AssetEvent::Removed { type_id, .. } => *type_id,
            AssetEvent::LoadFailed { type_id, .. } => *type_id,
        }
    }

    /// Get the handle ID if available.
    pub fn handle_id(&self) -> HandleId {
        match self {
            AssetEvent::Created { handle, .. } => handle.id(),
            AssetEvent::Modified { handle, .. } => handle.id(),
            AssetEvent::Removed { handle_id, .. } => *handle_id,
            AssetEvent::LoadFailed { handle, .. } => handle.id(),
        }
    }

    /// Check if this is a creation event.
    pub fn is_created(&self) -> bool {
        matches!(self, AssetEvent::Created { .. })
    }

    /// Check if this is a modification event.
    pub fn is_modified(&self) -> bool {
        matches!(self, AssetEvent::Modified { .. })
    }

    /// Check if this is a removal event.
    pub fn is_removed(&self) -> bool {
        matches!(self, AssetEvent::Removed { .. })
    }

    /// Check if this is a failure event.
    pub fn is_failed(&self) -> bool {
        matches!(self, AssetEvent::LoadFailed { .. })
    }
}

/// A buffer of asset events that can be drained each frame.
#[derive(Debug, Default)]
pub struct AssetEventBuffer {
    events: Vec<AssetEvent>,
}

impl AssetEventBuffer {
    /// Create a new empty event buffer.
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Push an event to the buffer.
    pub fn push(&mut self, event: AssetEvent) {
        self.events.push(event);
    }

    /// Drain all events from the buffer.
    pub fn drain(&mut self) -> impl Iterator<Item = AssetEvent> + '_ {
        self.events.drain(..)
    }

    /// Get an iterator over events without draining.
    pub fn iter(&self) -> impl Iterator<Item = &AssetEvent> {
        self.events.iter()
    }

    /// Check if there are any events.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Get the number of events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Clear all events.
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

/// Filter for typed asset events.
pub struct TypedEventFilter<T> {
    type_id: TypeId,
    _marker: std::marker::PhantomData<T>,
}

impl<T: 'static> TypedEventFilter<T> {
    /// Create a new typed event filter.
    pub fn new() -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Check if an event matches this filter.
    pub fn matches(&self, event: &AssetEvent) -> bool {
        event.type_id() == self.type_id
    }

    /// Filter events by type.
    pub fn filter<'a>(
        &self,
        events: impl Iterator<Item = &'a AssetEvent>,
    ) -> impl Iterator<Item = &'a AssetEvent> {
        let type_id = self.type_id;
        events.filter(move |e| e.type_id() == type_id)
    }
}

impl<T: 'static> Default for TypedEventFilter<T> {
    fn default() -> Self {
        Self::new()
    }
}
