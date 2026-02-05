//! Dirty tracking system for efficient incremental UI updates.
//!
//! This module provides fine-grained dirty flags, O(1) dirty counters,
//! style change guards, versioned values, and dirty range tracking.

pub mod counters;
mod flags;
pub mod guard;
pub mod ranges;
pub mod versioned;

pub use counters::{DirtyCounters, DirtySummary};
pub use flags::DirtyFlags;
pub use guard::StyleGuard;
pub use ranges::{DirtyRangeStats, DirtyRanges};
pub use versioned::Versioned;
