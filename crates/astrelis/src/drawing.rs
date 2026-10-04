use crate::Error;

/// Units used by 2D geometry before its affine transform.
///
/// Both spaces start at the current viewport's top-left, with positive X right
/// and positive Y down. Viewport origin is applied by GPU rasterization. Astrelis
/// does not apply window DPI: applications can scale logical coordinates explicitly.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DrawSpace {
    /// Physical pixels relative to the viewport, the default.
    #[default]
    Pixels,
    /// Viewport fractions: `(1, 1)` is the bottom-right. Axes scale independently.
    Normalized,
}

/// An affine transform `[xx, yx, xy, yy, tx, ty]` in the draw's selected units.
///
/// Maps `(x, y)` to `(xx*x + xy*y + tx, yx*x + yy*y + ty)`. Geometry, stroke
/// width, and caps are transformed together, before viewport conversion. Scaling
/// normalized coordinates therefore can be anisotropic on nonsquare viewports.
/// Draw calls reject nonfinite transforms and overflowing transformed geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform2D(pub [f32; 6]);
impl Default for Transform2D {
    fn default() -> Self {
        Self::IDENTITY
    }
}
impl From<[f32; 6]> for Transform2D {
    fn from(value: [f32; 6]) -> Self {
        Self(value)
    }
}
impl Transform2D {
    /// Identity transform.
    pub const IDENTITY: Self = Self([1., 0., 0., 1., 0., 0.]);
    /// A translation in the selected drawing units.
    pub const fn translation(x: f32, y: f32) -> Self {
        Self([1., 0., 0., 1., x, y])
    }
    /// Independent scaling around the origin.
    pub const fn scale(x: f32, y: f32) -> Self {
        Self([x, 0., 0., y, 0., 0.])
    }
    /// Clockwise rotation in radians around the origin in the Y-down coordinate system.
    pub fn rotation(radians: f32) -> Self {
        let (s, c) = radians.sin_cos();
        Self([c, s, -s, c, 0., 0.])
    }
    /// Applies this transform, then `next`; translation is affected by `next`.
    pub fn then(self, next: Self) -> Self {
        let [a, b, c, d, x, y] = self.0;
        let [e, f, g, h, u, v] = next.0;
        Self([
            e * a + g * b,
            f * a + h * b,
            e * c + g * d,
            f * c + h * d,
            e * x + g * y + u,
            f * x + h * y + v,
        ])
    }
    /// Maps one point without validation. Drawing validates the resulting geometry.
    pub fn transform_point(self, point: [f32; 2]) -> [f32; 2] {
        let [a, b, c, d, x, y] = self.0;
        [
            a * point[0] + c * point[1] + x,
            b * point[0] + d * point[1] + y,
        ]
    }
    /// Returns the array accepted by existing texture placement APIs.
    pub const fn to_array(self) -> [f32; 6] {
        self.0
    }
    pub(crate) fn valid(self) -> bool {
        self.0.iter().all(|v| v.is_finite())
    }
}

/// A top-left rectangle in explicitly selected destination units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    /// Left coordinate.
    pub x: f32,
    /// Top coordinate.
    pub y: f32,
    /// Nonnegative width.
    pub width: f32,
    /// Nonnegative height.
    pub height: f32,
}
impl Rect {
    /// Creates a rectangle; drawing validates finite values and nonnegative extents.
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
    pub(crate) fn array(self) -> [f32; 4] {
        [self.x, self.y, self.width, self.height]
    }
    pub(crate) fn valid(self) -> bool {
        self.array().iter().all(|v| v.is_finite()) && self.width >= 0. && self.height >= 0.
    }
}

/// Edge coverage for solid 2D primitives, independent of target MSAA.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EdgeAntialiasing {
    /// Approximate one-pixel analytic edge coverage, including on single-sampled targets.
    #[default]
    Coverage,
    /// Hard shader edges. Target MSAA remains controlled by the application.
    None,
}

/// Filled primitive geometry. Paths and shape outlines are separate future capabilities.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// Axis-aligned rectangle before transformation.
    Rectangle,
    /// Uniform circular corners, clamped to half the shorter rectangle dimension.
    RoundedRectangle {
        /// Nonnegative radius in the draw's units, before transformation.
        radius: f32,
    },
    /// Ellipse inscribed in the rectangle, before transformation.
    Ellipse,
}

/// One filled solid primitive, independent of GPU resources.
///
/// Color is **linear, straight RGBA**: RGB is finite (HDR values are allowed),
/// alpha is in `0..=1`. Renderers output premultiplied color and use source-over
/// blending. Zero extents or singular transforms produce no area. Drawing order
/// is explicit; batching preserves input order and does not sort by primitive.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeDraw {
    /// Bounding rectangle before transformation.
    pub rect: Rect,
    /// Filled geometry.
    pub shape: Shape,
    /// Linear, straight RGBA.
    pub color: [f32; 4],
    /// Geometry units.
    pub space: DrawSpace,
    /// Affine transform in those units.
    pub transform: Transform2D,
    /// Shader edge coverage.
    pub antialiasing: EdgeAntialiasing,
}
impl ShapeDraw {
    /// A filled rectangle in physical pixels with identity transform and edge coverage.
    pub const fn rect(rect: Rect, color: [f32; 4]) -> Self {
        Self {
            rect,
            shape: Shape::Rectangle,
            color,
            space: DrawSpace::Pixels,
            transform: Transform2D::IDENTITY,
            antialiasing: EdgeAntialiasing::Coverage,
        }
    }
    /// A filled uniformly rounded rectangle. Drawing validates the radius.
    pub const fn rounded_rect(rect: Rect, radius: f32, color: [f32; 4]) -> Self {
        Self {
            shape: Shape::RoundedRectangle { radius },
            ..Self::rect(rect, color)
        }
    }
    /// A filled ellipse inscribed in `rect`.
    pub const fn ellipse(rect: Rect, color: [f32; 4]) -> Self {
        Self {
            shape: Shape::Ellipse,
            ..Self::rect(rect, color)
        }
    }
    /// Selects viewport fractions or pixels for geometry and transform.
    pub const fn space(mut self, space: DrawSpace) -> Self {
        self.space = space;
        self
    }
    /// Selects an affine transform, also accepting the texture API's six-element array.
    pub fn transform(mut self, transform: impl Into<Transform2D>) -> Self {
        self.transform = transform.into();
        self
    }
    /// Selects shader edge coverage; does not change target MSAA.
    pub const fn antialiasing(mut self, value: EdgeAntialiasing) -> Self {
        self.antialiasing = value;
        self
    }
    /// Checks finite input values, nonnegative geometry, and straight alpha.
    /// Rendering also checks transformed bounds against the current viewport.
    pub fn validate(self) -> Result<(), Error> {
        if !self.rect.valid()
            || !color_valid(self.color)
            || !self.transform.valid()
            || matches!(self.shape, Shape::RoundedRectangle { radius } if !radius.is_finite() || radius < 0.)
        {
            return Err(Error::InvalidShapeDraw);
        }
        Ok(())
    }
}

/// End caps for an independent line segment; no connected-path joins are implied.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineCap {
    /// Ends exactly at the endpoints.
    #[default]
    Butt,
    /// Extends each endpoint by half the width.
    Square,
    /// Semicircular caps centered on the endpoints.
    Round,
}

/// One solid line segment expanded to triangles by the built-in renderer.
///
/// Endpoints and width use the same selected units before transformation. The
/// default is one physical pixel and butt caps. Zero width draws nothing. A
/// zero-length round/square segment is a disk/square; a zero-length butt segment
/// draws nothing. Color and blending follow [`ShapeDraw`]. This type represents
/// independent segments; connected polylines and stroke joins are not supported.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineDraw {
    /// Start point.
    pub start: [f32; 2],
    /// End point.
    pub end: [f32; 2],
    /// Nonnegative width before transformation.
    pub width: f32,
    /// End cap behavior.
    pub cap: LineCap,
    /// Linear, straight RGBA.
    pub color: [f32; 4],
    /// Endpoint and width units.
    pub space: DrawSpace,
    /// Affine transform in those units.
    pub transform: Transform2D,
    /// Shader edge coverage.
    pub antialiasing: EdgeAntialiasing,
}
impl LineDraw {
    /// Creates a one-pixel segment with butt caps and analytic edge coverage.
    pub const fn new(start: [f32; 2], end: [f32; 2], color: [f32; 4]) -> Self {
        Self {
            start,
            end,
            color,
            width: 1.,
            cap: LineCap::Butt,
            space: DrawSpace::Pixels,
            transform: Transform2D::IDENTITY,
            antialiasing: EdgeAntialiasing::Coverage,
        }
    }
    /// Selects width in the draw's units before transformation.
    pub const fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
    /// Selects independent segment caps.
    pub const fn cap(mut self, cap: LineCap) -> Self {
        self.cap = cap;
        self
    }
    /// Selects the units of endpoints, width, and transform.
    pub const fn space(mut self, space: DrawSpace) -> Self {
        self.space = space;
        self
    }
    /// Selects an affine transform, also accepting a six-element array.
    pub fn transform(mut self, transform: impl Into<Transform2D>) -> Self {
        self.transform = transform.into();
        self
    }
    /// Selects shader edge coverage; does not change target MSAA.
    pub const fn antialiasing(mut self, value: EdgeAntialiasing) -> Self {
        self.antialiasing = value;
        self
    }
    /// Checks finite input values, nonnegative geometry, and straight alpha.
    /// Rendering also checks transformed bounds against the current viewport.
    pub fn validate(self) -> Result<(), Error> {
        if !self.start.iter().chain(&self.end).all(|v| v.is_finite())
            || !self.width.is_finite()
            || self.width < 0.
            || !color_valid(self.color)
            || !self.transform.valid()
        {
            return Err(Error::InvalidLineDraw);
        }
        Ok(())
    }
}
pub(crate) fn color_valid(color: [f32; 4]) -> bool {
    color.iter().all(|v| v.is_finite()) && (0. ..=1.).contains(&color[3])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transform_order_and_y_down_rotation() {
        let t = Transform2D::translation(2., 3.).then(Transform2D::scale(2., 4.));
        assert_eq!(t.transform_point([1., 1.]), [6., 16.]);
        let p = Transform2D::rotation(std::f32::consts::FRAC_PI_2).transform_point([1., 0.]);
        assert!(p[0].abs() < 1e-6 && (p[1] - 1.).abs() < 1e-6);
    }
    #[test]
    fn invalid_geometry_and_colors_are_rejected() {
        let rect = Rect::new(0., 0., 10., 10.);
        assert!(
            ShapeDraw::rounded_rect(rect, -1., [1.; 4])
                .validate()
                .is_err()
        );
        assert!(ShapeDraw::rect(rect, [1., 1., 1., 2.]).validate().is_err());
        assert!(
            ShapeDraw::ellipse(rect, [4., 2., 1., 0.5])
                .validate()
                .is_ok()
        );
        assert!(
            LineDraw::new([f32::NAN, 0.], [0., 0.], [1.; 4])
                .validate()
                .is_err()
        );
        assert!(
            LineDraw::new([0.; 2], [1.; 2], [1.; 4])
                .width(-1.)
                .validate()
                .is_err()
        );
        assert!(
            LineDraw::new([0.; 2], [1.; 2], [1.; 4])
                .transform([f32::INFINITY; 6])
                .validate()
                .is_err()
        );
    }
}
