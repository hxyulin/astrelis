use glam::{Mat4, Quat, Vec3};

use crate::world::Component;

/// A basic component object
/// Every in-scene object should have this
/// This also depends on the [`GlobalTransform`] component
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Component for Transform {}

#[repr(transparent)]
#[derive(Debug, Clone, Default, Copy, bytemuck::NoUninit)]
pub struct GlobalTransform {
    pub model: Mat4,
}

impl GlobalTransform {
    pub fn from_transform(transform: &Transform) -> Self {
        let mut sel = Self::default();
        sel.update(transform);
        sel
    }

    pub fn update(&mut self, transform: &Transform) {
        self.model = glam::Mat4::from_scale_rotation_translation(
            transform.scale,
            transform.rotation,
            transform.position,
        );
    }
}

impl Component for GlobalTransform {}
