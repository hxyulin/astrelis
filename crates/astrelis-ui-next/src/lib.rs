//! Experimental incremental retained UI core.
//!
//! This crate deliberately does not replace `astrelis-ui-core`. It is a
//! proving ground for node-local invalidation, custom layout, cached paint
//! fragments, accessibility deltas, and typed mutation guards.

#![warn(missing_docs)]

mod builtins;
mod element;
mod semantics;
mod text_field;
mod tree;

pub use builtins::*;
pub use element::*;
pub use semantics::*;
pub use text_field::*;
pub use tree::*;
