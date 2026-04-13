//! Sprite drawing options and types.

use astrelis_core::color::Color;
use astrelis_core::math::Vec2;

/// Options for drawing a sprite.
#[derive(Debug, Clone)]
pub struct SpriteOptions {
    /// Color tint multiplied with the texture color.
    pub tint: Color,
    /// Flip the sprite horizontally.
    pub flip_x: bool,
    /// Flip the sprite vertically.
    pub flip_y: bool,
    /// Origin point for rotation and positioning, relative to the sprite
    /// size. (0,0) = top-left, (0.5,0.5) = center (default).
    pub origin: Vec2,
    /// Scale factor.
    pub scale: Vec2,
    /// Rotation in radians.
    pub rotation: f32,
}

impl Default for SpriteOptions {
    fn default() -> Self {
        Self {
            tint: Color::WHITE,
            flip_x: false,
            flip_y: false,
            origin: Vec2::new(0.5, 0.5),
            scale: Vec2::ONE,
            rotation: 0.0,
        }
    }
}

/// A rectangular sub-region of a texture, in pixel coordinates.
#[derive(Debug, Clone, Copy)]
pub struct SpriteRegion {
    /// X offset in pixels from the texture's top-left.
    pub x: f32,
    /// Y offset in pixels from the texture's top-left.
    pub y: f32,
    /// Width in pixels.
    pub width: f32,
    /// Height in pixels.
    pub height: f32,
}
