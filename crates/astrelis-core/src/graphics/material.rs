use glam::Vec4;

pub struct Color(Vec4);

impl Color {
    pub const TRANSPARENT: Color = Self::new(0.0, 0.0, 0.0, 0.0);
    pub const BLACK: Color = Self::new(0.0, 0.0, 0.0, 1.0);
    pub const WHITE: Color = Self::new(1.0, 1.0, 1.0, 1.0);
    pub const RED: Color = Self::new(1.0, 0.0, 0.0, 1.0);
    pub const GREEN: Color = Self::new(0.0, 1.0, 0.0, 1.0);
    pub const BLUE: Color = Self::new(0.0, 0.0, 1.0, 1.0);

    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Color(Vec4::new(r, g, b, a))
    }
}

impl Into<Vec4> for Color {
    fn into(self) -> Vec4 {
        self.0
    }
}

pub struct Material {
    diffuse_color: Color,
}
