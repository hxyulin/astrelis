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
    ///
    /// Performs no sRGB decode: the bytes are treated as already-linear
    /// values. For 8-bit sRGB values (web/design-tool colors) use
    /// [`Color::from_srgb8`] or [`Color::from_hex`] instead.
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
    ///
    /// Performs no sRGB decode; see [`Color::from_rgba8`]. For packed sRGB
    /// values use [`Color::from_hex`].
    #[inline]
    pub fn from_u32(rgba: u32) -> Self {
        Self::from_rgba8(
            (rgba >> 24) as u8,
            (rgba >> 16) as u8,
            (rgba >> 8) as u8,
            rgba as u8,
        )
    }

    /// Creates a color from 8-bit sRGB-encoded components, decoding them to
    /// the linear color space this type stores.
    ///
    /// Alpha is coverage, not a color channel, and is not gamma-encoded; it
    /// is divided by 255 unchanged.
    #[inline]
    pub fn from_srgb8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::new(
            srgb_to_linear(r as f32 / 255.0),
            srgb_to_linear(g as f32 / 255.0),
            srgb_to_linear(b as f32 / 255.0),
            a as f32 / 255.0,
        )
    }

    /// Creates an opaque color from a `0xRRGGBB` sRGB hex value, decoding it
    /// to linear space.
    ///
    /// This is the constructor for colors authored as web/design-tool hex
    /// values: `Color::from_hex(0x4c8dff)` renders exactly as `#4c8dff`.
    #[inline]
    pub fn from_hex(rgb: u32) -> Self {
        Self::from_srgb8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255)
    }

    /// Creates a color from a `0xRRGGBBAA` sRGB hex value, decoding the color
    /// channels to linear space. Alpha is linear coverage.
    #[inline]
    pub fn from_hex_alpha(rgba: u32) -> Self {
        Self::from_srgb8(
            (rgba >> 24) as u8,
            (rgba >> 16) as u8,
            (rgba >> 8) as u8,
            rgba as u8,
        )
    }

    /// Encodes the color to 8-bit sRGB components.
    ///
    /// Inverse of [`Color::from_srgb8`]; channels are clamped to `[0, 1]`
    /// before encoding.
    #[inline]
    pub fn to_srgb8(self) -> Rgba8 {
        let encode = |c: f32| (linear_to_srgb(c.clamp(0.0, 1.0)) * 255.0).round() as u8;
        Rgba8 {
            r: encode(self.r),
            g: encode(self.g),
            b: encode(self.b),
            a: (self.a.clamp(0.0, 1.0) * 255.0).round() as u8,
        }
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

/// Decodes one sRGB-encoded channel value in `[0, 1]` to linear space.
#[inline]
pub fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Encodes one linear channel value in `[0, 1]` to sRGB space.
#[inline]
pub fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
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
    fn srgb_white_and_black_roundtrip() {
        assert_eq!(Color::from_srgb8(255, 255, 255, 255), Color::WHITE);
        assert_eq!(Color::from_srgb8(0, 0, 0, 255), Color::BLACK);
        assert_eq!(Color::from_hex(0xffffff), Color::WHITE);
    }

    #[test]
    fn srgb_midpoint_decodes_correctly() {
        // sRGB #808080 (128/255 = 0.50196) decodes to ~0.2159 linear.
        let c = Color::from_hex(0x808080);
        assert!((c.r - 0.2159).abs() < 1e-3, "got {}", c.r);
        assert_eq!(c.r, c.g);
        assert_eq!(c.g, c.b);
        assert_eq!(c.a, 1.0);
    }

    #[test]
    fn srgb_linear_roundtrip() {
        for i in 0..=20 {
            let x = i as f32 / 20.0;
            assert!((linear_to_srgb(srgb_to_linear(x)) - x).abs() < 1e-5);
        }
    }

    #[test]
    fn hex_alpha_and_to_srgb8_roundtrip() {
        let c = Color::from_hex_alpha(0x4c8dff80);
        let back = c.to_srgb8();
        assert_eq!((back.r, back.g, back.b, back.a), (0x4c, 0x8d, 0xff, 0x80));
    }

    #[test]
    fn array_conversion() {
        let arr: [f32; 4] = Color::RED.into();
        assert_eq!(arr, [1.0, 0.0, 0.0, 1.0]);

        let c: Color = [0.5, 0.5, 0.5, 1.0].into();
        assert_eq!(c.r, 0.5);
    }
}
