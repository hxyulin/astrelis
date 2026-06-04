//! Unlit/debug 3D rendering for the Astrelis engine.
//!
//! World convention: right-handed, +Y up, −Z forward (glTF-aligned).
//! Depth: reverse-Z with an infinite far plane (`Depth32Float`,
//! compare `GreaterEqual`, cleared to 0.0) for near-uniform float
//! precision over the whole range.
//!
//! Frame flow mirrors `astrelis-render-2d`: upload meshes once with
//! [`Renderer3D::create_mesh`], then per frame call
//! [`Renderer3D::begin`], any number of `draw_*` calls, and
//! [`Renderer3D::end`]. The renderer owns its depth texture; clearing
//! the color target stays the caller's job (compose passes by
//! ordering: clear → 3D → 2D HUD on top).
//!
//! This crate has no scene or app dependencies; scene→renderer glue
//! lives downstream.

#![warn(missing_docs)]

pub mod camera;
pub mod mesh;
pub mod primitives;

pub use camera::Camera3D;
pub use mesh::{MeshData, MeshHandle, Vertex};
