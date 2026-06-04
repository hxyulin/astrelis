//! Scene tree for the Astrelis engine.
//!
//! A [`Scene`] owns a forest of nodes in an arena, addressed by
//! generational [`NodeId`] handles. Each node has a name, a local
//! [`Transform`], a visibility flag, and parent/children links.
//! Arbitrary data attaches to nodes as [`Component`]s, stored in
//! per-type columns so queries iterate only nodes that have the
//! component.
//!
//! Nodes are pure data: game logic lives in ordinary `astrelis-app`
//! systems that query the scene. [`ScenePlugin`] inserts a [`Scene`]
//! resource and runs one transform/visibility propagation pass per
//! frame in `Phase::PostUpdate` — mutate the scene in `Update`, read
//! world transforms in `Render`.
//!
//! Mutating nodes while iterating a component column would alias the
//! scene borrow, so collect the target ids first:
//!
//! ```ignore
//! let spinners: Vec<(NodeId, f32)> =
//!     scene.iter::<Spin>().map(|(id, s)| (id, s.speed)).collect();
//! for (id, speed) in spinners {
//!     scene.set_rotation(id, Quat::from_rotation_z(angle * speed));
//! }
//! ```
//!
//! This crate has no renderer or GPU dependencies. Rendering glue
//! (e.g. a sprite component plus a `Render`-phase system that calls a
//! renderer) lives downstream.

#![warn(missing_docs)]

pub mod component;
pub mod node;
pub mod plugin;
pub mod scene;
pub mod transform;

pub use component::Component;
pub use node::NodeId;
pub use plugin::ScenePlugin;
pub use scene::{NodeBuilder, Scene, SceneError};
pub use transform::Transform;
