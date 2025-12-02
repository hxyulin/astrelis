/// A module that re-exports the `glam` crate for fast mathematical operations.
pub mod fast {
    pub use glam::*;
}

pub mod packed {
    use bytemuck::{Pod, Zeroable};

    #[repr(C)]
    #[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
    pub struct Vec2 {
        pub x: f32,
        pub y: f32,
    }

    #[repr(C)]
    #[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
    pub struct Vec3 {
        pub x: f32,
        pub y: f32,
        pub z: f32,
    }

    #[repr(C)]
    #[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
    pub struct Vec4 {
        pub x: f32,
        pub y: f32,
        pub z: f32,
        pub w: f32,
    }
}

pub use fast::*;
pub use packed::{
    Vec2 as PackedVec2,
    Vec3 as PackedVec3,
    Vec4 as PackedVec4,
};
