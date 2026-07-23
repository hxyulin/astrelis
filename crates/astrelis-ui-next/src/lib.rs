//! Experimental incremental retained UI core.
//!
//! This crate deliberately does not replace `astrelis-ui-core`. It is a
//! proving ground for node-local invalidation, custom layout, cached paint
//! fragments, accessibility deltas, and typed mutation guards.

#![warn(missing_docs)]

mod builtins;
mod controls;
mod element;
mod media;
mod mutation;
mod scroll;
mod semantics;
mod text_field;
mod tree;

pub use builtins::*;
pub use controls::*;
pub use element::*;
pub use media::*;
pub use mutation::*;
pub use scroll::*;
pub use semantics::*;
pub use text_field::*;
pub use tree::*;
