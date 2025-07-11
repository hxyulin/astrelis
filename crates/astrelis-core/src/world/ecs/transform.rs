use glam::{Mat4, Quat, Vec3};

use crate::world::Component;

/// A basic component object
/// Every in-scene object should have this
/// This also depends on the [`GlobalTransform`] component
#[derive(Debug, Clone, Copy)]
pub struct Transform {
    position: Vec3,
    rotation: Quat,
    scale: Vec3,
}

impl Component for Transform {}

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct GlobalTransform {
    model: Mat4,
}

impl Component for GlobalTransform {}
