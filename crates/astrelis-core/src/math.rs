/// Fast mathematical operations using SIMD-accelerated `glam` types.
///
///This module re-exports all types and functions from the [`glam`] crate, which provides
/// high-performance vector and matrix mathematics using SIMD instructions when available.
///
/// # Common Types
///
/// - [`Vec2`]: 2D vector (x, y)
/// - [`Vec3`]: 3D vector (x, y, z)
/// - [`Vec4`]: 4D vector (x, y, z, w)
/// - [`Mat2`], [`Mat3`], [`Mat4`]: 2x2, 3x3, and 4x4 matrices
/// - [`Quat`]: Quaternion for 3D rotations
///
/// # Examples
///
/// ```
/// use astrelis_core::math::{Vec2, Vec3, Mat4};
///
/// // 2D vectors for positions, velocities, etc.
/// let position = Vec2::new(10.0, 20.0);
/// let velocity = Vec2::new(1.0, 0.5);
/// let new_position = position + velocity * 0.016;
///
/// // 3D transformations
/// let translation = Vec3::new(0.0, 0.0, -5.0);
/// let transform = Mat4::from_translation(translation);
/// ```
///
/// # Performance
///
/// `glam` types use SIMD (Single Instruction, Multiple Data) instructions on supported
/// platforms (x86/x86_64 with SSE, ARM with NEON) for optimal performance. Operations
/// like vector addition, dot products, and matrix multiplication are highly optimized.
///
/// [`glam`]: https://docs.rs/glam
pub mod fast {
    pub use glam::*;
}

/// Packed vector types for GPU buffer uploads and interoperability.
///
/// This module provides `#[repr(C)]` vector types that can be safely cast to byte slices
/// for GPU buffer uploads using [`bytemuck`]. These types are guaranteed to have a specific
/// memory layout compatible with WGSL shaders and other low-level APIs.
///
/// # When to Use
///
/// Use packed types when:
/// - Uploading vertex or instance data to GPU buffers
/// - Interfacing with C/C++ libraries
/// - Need guaranteed memory layout (`#[repr(C)]`)
///
/// Use [`fast`] module types (re-exported as [`Vec2`], [`Vec3`], [`Vec4`]) for:
/// - CPU-side math calculations (SIMD-accelerated)
/// - Game logic (positions, velocities, etc.)
///
/// # Examples
///
/// ```
/// use astrelis_core::math::PackedVec2;
/// use bytemuck::cast_slice;
///
/// // Create packed vertices for GPU upload
/// let vertices = vec![
///     PackedVec2 { x: -1.0, y: -1.0 },
///     PackedVec2 { x:  1.0, y: -1.0 },
///     PackedVec2 { x:  0.0, y:  1.0 },
/// ];
///
/// // Safely cast to bytes for buffer upload
/// let bytes: &[u8] = cast_slice(&vertices);
/// // ... upload bytes to GPU buffer
/// ```
///
/// # Conversions
///
/// Convert between packed and fast types as needed:
///
/// ```
/// use astrelis_core::math::{Vec2, PackedVec2};
///
/// // Fast type for calculations
/// let velocity = Vec2::new(1.0, 0.5);
///
/// // Convert to packed for GPU upload
/// let packed = PackedVec2 {
///     x: velocity.x,
///     y: velocity.y,
/// };
/// ```
///
/// [`bytemuck`]: https://docs.rs/bytemuck
pub mod packed {
    use bytemuck::{Pod, Zeroable};

    /// A 2D vector with guaranteed `#[repr(C)]` layout for GPU uploads.
    ///
    /// This type is [`Pod`] (Plain Old Data) and can be safely cast to bytes.
    ///
    /// # Memory Layout
    ///
    /// ```text
    /// Offset | Field | Size
    /// -------|-------|------
    /// 0      | x     | 4 bytes (f32)
    /// 4      | y     | 4 bytes (f32)
    /// Total: 8 bytes
    /// ```
    #[repr(C)]
    #[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
    pub struct Vec2 {
        pub x: f32,
        pub y: f32,
    }

    /// A 3D vector with guaranteed `#[repr(C)]` layout for GPU uploads.
    ///
    /// This type is [`Pod`] (Plain Old Data) and can be safely cast to bytes.
    ///
    /// # Memory Layout
    ///
    /// ```text
    /// Offset | Field | Size
    /// -------|-------|------
    /// 0      | x     | 4 bytes (f32)
    /// 4      | y     | 4 bytes (f32)
    /// 8      | z     | 4 bytes (f32)
    /// Total: 12 bytes
    /// ```
    ///
    /// **Note**: GPU shaders often expect 16-byte alignment. Consider using [`Vec4`]
    /// with `w` as padding if you encounter alignment issues.
    #[repr(C)]
    #[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
    pub struct Vec3 {
        pub x: f32,
        pub y: f32,
        pub z: f32,
    }

    /// A 4D vector with guaranteed `#[repr(C)]` layout for GPU uploads.
    ///
    /// This type is [`Pod`] (Plain Old Data) and can be safely cast to bytes.
    ///
    /// # Memory Layout
    ///
    /// ```text
    /// Offset | Field | Size
    /// -------|-------|------
    /// 0      | x     | 4 bytes (f32)
    /// 4      | y     | 4 bytes (f32)
    /// 8      | z     | 4 bytes (f32)
    /// 12     | w     | 4 bytes (f32)
    /// Total: 16 bytes
    /// ```
    ///
    /// **Note**: This type has natural 16-byte alignment, making it ideal for GPU buffers.
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
pub use packed::{Vec2 as PackedVec2, Vec3 as PackedVec3, Vec4 as PackedVec4};
