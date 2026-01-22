//! Viewport context for resolving viewport-relative units.
//!
//! Provides context information needed to resolve viewport-relative length units
//! (vw, vh, vmin, vmax) to absolute pixel values.

use astrelis_core::math::Vec2;

/// Context for resolving viewport-relative units.
///
/// Stores the viewport dimensions needed to convert viewport-relative length units
/// (vw, vh, vmin, vmax) into absolute pixel values.
///
/// # Examples
/// ```
/// use astrelis_ui::viewport_context::ViewportContext;
/// use astrelis_core::math::Vec2;
///
/// let ctx = ViewportContext::new(Vec2::new(1280.0, 720.0));
/// assert_eq!(ctx.viewport_size().x, 1280.0);
/// assert_eq!(ctx.viewport_size().y, 720.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportContext {
    /// Viewport dimensions (width, height) in pixels
    viewport_size: Vec2,
}

impl ViewportContext {
    /// Create a new viewport context.
    ///
    /// # Arguments
    /// * `viewport_size` - The viewport dimensions (width, height) in pixels
    ///
    /// # Examples
    /// ```
    /// use astrelis_ui::viewport_context::ViewportContext;
    /// use astrelis_core::math::Vec2;
    ///
    /// let ctx = ViewportContext::new(Vec2::new(1920.0, 1080.0));
    /// ```
    pub fn new(viewport_size: Vec2) -> Self {
        Self { viewport_size }
    }

    /// Get the viewport size.
    pub fn viewport_size(&self) -> Vec2 {
        self.viewport_size
    }

    /// Get the viewport width.
    pub fn width(&self) -> f32 {
        self.viewport_size.x
    }

    /// Get the viewport height.
    pub fn height(&self) -> f32 {
        self.viewport_size.y
    }

    /// Get the smaller viewport dimension.
    pub fn min_dimension(&self) -> f32 {
        self.viewport_size.x.min(self.viewport_size.y)
    }

    /// Get the larger viewport dimension.
    pub fn max_dimension(&self) -> f32 {
        self.viewport_size.x.max(self.viewport_size.y)
    }

    /// Get the aspect ratio (width / height).
    pub fn aspect_ratio(&self) -> f32 {
        if self.viewport_size.y != 0.0 {
            self.viewport_size.x / self.viewport_size.y
        } else {
            1.0
        }
    }

    /// Create a viewport context from window dimensions.
    ///
    /// Convenience constructor for common use cases.
    pub fn from_window_size(width: f32, height: f32) -> Self {
        Self::new(Vec2::new(width, height))
    }
}

impl Default for ViewportContext {
    /// Default viewport context with 1280x720 dimensions.
    fn default() -> Self {
        Self::new(Vec2::new(1280.0, 720.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_viewport_context_new() {
        let ctx = ViewportContext::new(Vec2::new(1920.0, 1080.0));
        assert_eq!(ctx.viewport_size(), Vec2::new(1920.0, 1080.0));
        assert_eq!(ctx.width(), 1920.0);
        assert_eq!(ctx.height(), 1080.0);
    }

    #[test]
    fn test_viewport_context_dimensions() {
        let ctx = ViewportContext::new(Vec2::new(1280.0, 720.0));
        assert_eq!(ctx.min_dimension(), 720.0);
        assert_eq!(ctx.max_dimension(), 1280.0);
        assert!((ctx.aspect_ratio() - 1.777777).abs() < 0.001);
    }

    #[test]
    fn test_viewport_context_square() {
        let ctx = ViewportContext::new(Vec2::new(800.0, 800.0));
        assert_eq!(ctx.min_dimension(), 800.0);
        assert_eq!(ctx.max_dimension(), 800.0);
        assert_eq!(ctx.aspect_ratio(), 1.0);
    }

    #[test]
    fn test_viewport_context_from_window_size() {
        let ctx = ViewportContext::from_window_size(1024.0, 768.0);
        assert_eq!(ctx.width(), 1024.0);
        assert_eq!(ctx.height(), 768.0);
    }

    #[test]
    fn test_viewport_context_default() {
        let ctx = ViewportContext::default();
        assert_eq!(ctx.width(), 1280.0);
        assert_eq!(ctx.height(), 720.0);
    }

    #[test]
    fn test_aspect_ratio_zero_height() {
        let ctx = ViewportContext::new(Vec2::new(1920.0, 0.0));
        assert_eq!(ctx.aspect_ratio(), 1.0); // Should not panic
    }
}
