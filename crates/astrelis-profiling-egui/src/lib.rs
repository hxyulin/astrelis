//! egui viewer widgets for the Astrelis in-engine profiler.
//!
//! This crate reads from [`astrelis_profiling`]'s global timeline
//! and renders it inside an egui UI. Stage 1 exposes a simple
//! [`LastFrameFlameGraph`] widget that draws the most recent
//! frame's CPU scopes as a nested flame graph. Stage 2 will replace
//! this with a scrollable multi-frame timeline.

#![warn(missing_docs)]

mod flame_graph;

pub use flame_graph::LastFrameFlameGraph;
