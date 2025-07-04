use glam::{Quat, Vec3};

use crate::world::Component;

#[derive(Debug, Clone, Copy)]
pub struct Transform {
    position: Vec3,
    rotation: Quat,
    scale: Vec3,
}

impl Component for Transform {}
