//! Linear algebra types and GPU-ready packed representations.
//!
//! This module re-exports commonly used types from [`glam`] and provides
//! `#[repr(C)]` packed variants suitable for direct GPU buffer upload via
//! [`bytemuck`].

// Re-export glam types at module root for convenience.
pub use glam::{
    Affine2, Affine3A, Mat2, Mat3, Mat3A, Mat4, Quat, Vec2, Vec3, Vec3A, Vec4,
};
pub use glam::{IVec2, IVec3, IVec4, UVec2, UVec3, UVec4};

/// Packed `#[repr(C)]` types for direct GPU buffer upload.
///
/// These mirror the corresponding `glam` types but are guaranteed to have
/// a C-compatible memory layout and implement [`bytemuck::Pod`], making them
/// safe to cast to byte slices for GPU upload.
pub mod packed {
    use bytemuck::{Pod, Zeroable};

    /// A packed 2-component float vector.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
    #[repr(C)]
    pub struct Vec2 {
        /// X component.
        pub x: f32,
        /// Y component.
        pub y: f32,
    }

    /// A packed 3-component float vector.
    ///
    /// Note: this is 12 bytes, **not** 16-byte aligned. For GPU uniforms that
    /// require 16-byte alignment, use [`Vec4`] and ignore the `w` component.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
    #[repr(C)]
    pub struct Vec3 {
        /// X component.
        pub x: f32,
        /// Y component.
        pub y: f32,
        /// Z component.
        pub z: f32,
    }

    /// A packed 4-component float vector.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
    #[repr(C)]
    pub struct Vec4 {
        /// X component.
        pub x: f32,
        /// Y component.
        pub y: f32,
        /// Z component.
        pub z: f32,
        /// W component.
        pub w: f32,
    }

    /// A packed 4x4 float matrix stored in column-major order.
    #[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
    #[repr(C)]
    pub struct Mat4 {
        /// The 16 matrix elements in column-major order.
        pub cols: [f32; 16],
    }

    // --- Conversions from glam types ---

    impl From<glam::Vec2> for Vec2 {
        fn from(v: glam::Vec2) -> Self {
            Self { x: v.x, y: v.y }
        }
    }

    impl From<Vec2> for glam::Vec2 {
        fn from(v: Vec2) -> Self {
            Self::new(v.x, v.y)
        }
    }

    impl From<glam::Vec3> for Vec3 {
        fn from(v: glam::Vec3) -> Self {
            Self {
                x: v.x,
                y: v.y,
                z: v.z,
            }
        }
    }

    impl From<Vec3> for glam::Vec3 {
        fn from(v: Vec3) -> Self {
            Self::new(v.x, v.y, v.z)
        }
    }

    impl From<glam::Vec4> for Vec4 {
        fn from(v: glam::Vec4) -> Self {
            Self {
                x: v.x,
                y: v.y,
                z: v.z,
                w: v.w,
            }
        }
    }

    impl From<Vec4> for glam::Vec4 {
        fn from(v: Vec4) -> Self {
            Self::new(v.x, v.y, v.z, v.w)
        }
    }

    impl From<glam::Mat4> for Mat4 {
        fn from(m: glam::Mat4) -> Self {
            Self {
                cols: m.to_cols_array(),
            }
        }
    }

    impl From<Mat4> for glam::Mat4 {
        fn from(m: Mat4) -> Self {
            Self::from_cols_array(&m.cols)
        }
    }

    impl Default for Mat4 {
        fn default() -> Self {
            glam::Mat4::IDENTITY.into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_vec2_roundtrip() {
        let v = Vec2::new(1.0, 2.0);
        let packed: packed::Vec2 = v.into();
        let back: Vec2 = packed.into();
        assert_eq!(v, back);
    }

    #[test]
    fn packed_mat4_roundtrip() {
        let m = Mat4::from_scale(Vec3::new(2.0, 3.0, 4.0));
        let packed: packed::Mat4 = m.into();
        let back: Mat4 = packed.into();
        assert_eq!(m, back);
    }

    #[test]
    fn packed_types_are_pod() {
        // Verify we can cast to bytes (this is the whole point).
        let v = packed::Vec4 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
            w: 4.0,
        };
        let bytes = bytemuck::bytes_of(&v);
        assert_eq!(bytes.len(), 16);
    }
}
