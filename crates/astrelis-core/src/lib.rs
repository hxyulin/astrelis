//! Core types and math for the Astrelis engine.
//!
//! This crate provides the foundational types used throughout the engine:
//! - [`math`] — Linear algebra types (re-exported from `glam`) and GPU-ready packed types
//! - [`color`] — RGBA color type with named constants and conversions
//! - [`geometry`] — Coordinate-space-aware geometric primitives (points, sizes, rects)
//! - [`id`] — Type-safe generic ID handles

pub mod color;
pub mod geometry;
pub mod id;
#[cfg(feature = "tracing-init")]
pub mod logging;
pub mod math;
