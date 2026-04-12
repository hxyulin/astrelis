//! egui viewer widgets for the Astrelis in-engine profiler.
//!
//! This crate reads from [`astrelis_profiling`]'s global timeline
//! and renders it inside an egui UI. [`LastFrameFlameGraph`] draws
//! the most recent frame's CPU scopes as a nested flame graph;
//! [`ProfilerWindow`] provides a scrollable multi-frame timeline.

#![warn(missing_docs)]

mod flame_graph;
mod layout;
mod timeline_view;

pub use flame_graph::LastFrameFlameGraph;
pub use timeline_view::ProfilerWindow;
