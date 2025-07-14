use astrelis_core_macros::ShaderBufferCompatible;
use glam::{Mat4, Quat, Vec3, Vec4};

use crate::{
    graphics::shader::{BufferLayout, ShaderBufferCompatible},
    world::Component,
};

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

// TODO: We currently don't support Mat4
impl ShaderBufferCompatible for GlobalTransform {
    fn buffer_layout(base_location: u32) -> BufferLayout {
        let attributes = vec![
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: base_location,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: size_of::<Vec4>() as u64,
                shader_location: base_location + 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: (2 * size_of::<Vec4>()) as u64,
                shader_location: base_location + 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: (3 * size_of::<Vec4>()) as u64,
                shader_location: base_location + 3,
            },
        ];

        BufferLayout {
            attributes,
            size: size_of::<Self>() as u64,
        }
    }
}

impl Component for GlobalTransform {}
