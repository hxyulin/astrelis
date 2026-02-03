//! Dirty tracking system for efficient incremental UI updates.
//!
//! This module provides fine-grained dirty flags, O(1) dirty counters,
//! style change guards, versioned values, and dirty range tracking.

mod flags;
pub mod counters;
pub mod guard;
pub mod ranges;
pub mod versioned;

pub use flags::DirtyFlags;
pub use counters::{DirtyCounters, DirtySummary};
pub use guard::StyleGuard;
pub use ranges::{DirtyRangeStats, DirtyRanges};
pub use versioned::Versioned;
