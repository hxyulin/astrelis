//! RGBA color type with named constants and format conversions.

use bytemuck::{Pod, Zeroable};

/// An RGBA color with 32-bit floating-point components.
///
/// Components are stored in linear color space in the range `[0.0, 1.0]`.
/// The type is `#[repr(C)]` and implements [`Pod`], so it can be directly
/// uploaded to GPU buffers.
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct Color {
    /// Red channel.
    pub r: f32,
    /// Green channel.
    pub g: f32,
    /// Blue channel.
    pub b: f32,
    /// Alpha channel (1.0 = fully opaque).
    pub a: f32,
}

/// An RGBA color with 8-bit unsigned components.
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct Rgba8 {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha channel (255 = fully opaque).
    pub a: u8,
}

impl Color {
    /// Transparent black.
    pub const TRANSPARENT: Self = Self::new(0.0, 0.0, 0.0, 0.0);
    /// Opaque black.
    pub const BLACK: Self = Self::new(0.0, 0.0, 0.0, 1.0);
    /// Opaque white.
    pub const WHITE: Self = Self::new(1.0, 1.0, 1.0, 1.0);
    /// Opaque red.
    pub const RED: Self = Self::new(1.0, 0.0, 0.0, 1.0);
    /// Opaque green.
    pub const GREEN: Self = Self::new(0.0, 1.0, 0.0, 1.0);
    /// Opaque blue.
    pub const BLUE: Self = Self::new(0.0, 0.0, 1.0, 1.0);
    /// Opaque yellow.
    pub const YELLOW: Self = Self::new(1.0, 1.0, 0.0, 1.0);
    /// Opaque cyan.
    pub const CYAN: Self = Self::new(0.0, 1.0, 1.0, 1.0);
    /// Opaque magenta.
    pub const MAGENTA: Self = Self::new(1.0, 0.0, 1.0, 1.0);

    /// Creates a new color from RGBA components.
    #[inline]
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Creates a fully opaque color from RGB components.
    #[inline]
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self::new(r, g, b, 1.0)
    }

    /// Creates a color from 8-bit RGBA components (0-255).
    #[inline]
    pub fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::new(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        )
    }

    /// Creates a color from a packed `0xRRGGBBAA` u32.
    #[inline]
    pub fn from_u32(rgba: u32) -> Self {
        Self::from_rgba8(
            (rgba >> 24) as u8,
            (rgba >> 16) as u8,
            (rgba >> 8) as u8,
            rgba as u8,
        )
    }

    /// Packs the color into a `0xRRGGBBAA` u32.
    #[inline]
    pub fn to_u32(self) -> u32 {
        let r = (self.r.clamp(0.0, 1.0) * 255.0) as u32;
        let g = (self.g.clamp(0.0, 1.0) * 255.0) as u32;
        let b = (self.b.clamp(0.0, 1.0) * 255.0) as u32;
        let a = (self.a.clamp(0.0, 1.0) * 255.0) as u32;
        (r << 24) | (g << 16) | (b << 8) | a
    }

    /// Returns the color with a different alpha value.
    #[inline]
    pub const fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::WHITE
    }
}

impl From<Color> for [f32; 4] {
    fn from(c: Color) -> Self {
        [c.r, c.g, c.b, c.a]
    }
}

impl From<[f32; 4]> for Color {
    fn from([r, g, b, a]: [f32; 4]) -> Self {
        Self { r, g, b, a }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u32_roundtrip() {
        let c = Color::from_u32(0xFF8040FF);
        let packed = c.to_u32();
        // Allow +-1 due to float rounding.
        assert!((packed as i64 - 0xFF8040FF_i64).unsigned_abs() <= 0x01010101);
    }

    #[test]
    fn rgba8_white() {
        let c = Color::from_rgba8(255, 255, 255, 255);
        assert_eq!(c, Color::WHITE);
    }

    #[test]
    fn array_conversion() {
        let arr: [f32; 4] = Color::RED.into();
        assert_eq!(arr, [1.0, 0.0, 0.0, 1.0]);

        let c: Color = [0.5, 0.5, 0.5, 1.0].into();
        assert_eq!(c.r, 0.5);
    }
}
