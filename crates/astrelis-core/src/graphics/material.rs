use bytemuck::Zeroable;
use glam::Vec4;

use crate::{
    alloc::{IndexSlot, SparseSet},
    graphics::shader::ShaderHandle,
    world::Component,
};

#[repr(transparent)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Zeroable, bytemuck::NoUninit)]
pub struct Color([f32; 4]);

impl Color {
    pub const TRANSPARENT: Color = Self::new(0.0, 0.0, 0.0, 0.0);
    pub const BLACK: Color = Self::new(0.0, 0.0, 0.0, 1.0);
    pub const WHITE: Color = Self::new(1.0, 1.0, 1.0, 1.0);
    pub const RED: Color = Self::new(1.0, 0.0, 0.0, 1.0);
    pub const GREEN: Color = Self::new(0.0, 1.0, 0.0, 1.0);
    pub const BLUE: Color = Self::new(0.0, 0.0, 1.0, 1.0);

    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Color([r, g, b, a])
    }
}

impl Into<Vec4> for Color {
    fn into(self) -> Vec4 {
        Vec4::from_array(self.0)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::NoUninit)]
pub struct Material {
    pub diffuse_color: Color,
    pub shader: ShaderHandle,
}

#[derive(Debug, Clone, Copy)]
pub struct MaterialComponent(pub MatHandle);

impl Component for MaterialComponent {}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct MatHandle(IndexSlot);

pub struct MaterialManager {
    mats: SparseSet<Material>,
}

impl MaterialManager {
    pub fn new() -> Self {
        Self {
            mats: SparseSet::new(),
        }
    }

    pub fn create_mat(&mut self, mat: Material) -> MatHandle {
        MatHandle(self.mats.push(mat))
    }

    pub fn get_mat(&self, handle: MatHandle) -> &Material {
        self.mats.get(handle.0)
    }

    pub fn remove_mesh(&mut self, handle: MatHandle) -> Material {
        self.mats.remove(handle.0)
    }
}
