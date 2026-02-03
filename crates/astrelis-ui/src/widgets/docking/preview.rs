//! Drop preview rendering utilities for cross-container dragging.

use super::types::DockZone;
use crate::tree::LayoutRect;
use astrelis_render::Color;

/// Speed of the fade-in animation (1.0 / FADE_SPEED = seconds to full opacity).
pub const FADE_SPEED: f32 = 5.0;

/// Default preview color (semi-transparent blue).
pub fn default_preview_color() -> Color {
    Color::from_rgba_u8(100, 150, 255, 80)
}

/// Default preview border color (brighter blue).
pub fn default_preview_border_color() -> Color {
    Color::from_rgba_u8(100, 150, 255, 180)
}

/// Drop preview configuration.
#[derive(Debug, Clone)]
pub struct DropPreviewStyle {
    /// Fill color for the preview rectangle.
    pub fill_color: Color,
    /// Border color for the preview rectangle.
    pub border_color: Color,
    /// Border width in pixels.
    pub border_width: f32,
    /// Corner radius for rounded corners.
    pub corner_radius: f32,
}

impl Default for DropPreviewStyle {
    fn default() -> Self {
        Self {
            fill_color: default_preview_color(),
            border_color: default_preview_border_color(),
            border_width: 2.0,
            corner_radius: 4.0,
        }
    }
}

impl DropPreviewStyle {
    /// Create a new preview style with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the fill color.
    pub fn fill_color(mut self, color: Color) -> Self {
        self.fill_color = color;
        self
    }

    /// Set the border color.
    pub fn border_color(mut self, color: Color) -> Self {
        self.border_color = color;
        self
    }

    /// Set the border width.
    pub fn border_width(mut self, width: f32) -> Self {
        self.border_width = width;
        self
    }

    /// Set the corner radius.
    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
    }
}

/// Active drop preview state.
#[derive(Debug, Clone)]
pub struct DropPreview {
    /// The zone being previewed.
    pub zone: DockZone,
    /// The bounds where the preview is displayed.
    pub bounds: LayoutRect,
    /// Preview style.
    pub style: DropPreviewStyle,
    /// Fade-in animation progress (0.0-1.0).
    pub fade_progress: f32,
}

impl DropPreview {
    /// Create a new drop preview.
    pub fn new(zone: DockZone, bounds: LayoutRect) -> Self {
        Self {
            zone,
            bounds,
            style: DropPreviewStyle::default(),
            fade_progress: 0.0,
        }
    }

    /// Create with custom style.
    pub fn with_style(mut self, style: DropPreviewStyle) -> Self {
        self.style = style;
        self
    }

    /// Update fade-in animation.
    ///
    /// Call this every frame to animate the preview appearance.
    /// `delta_time` is in seconds.
    pub fn update_animation(&mut self, delta_time: f32) {
        self.fade_progress = (self.fade_progress + delta_time * FADE_SPEED).min(1.0);
    }

    /// Get the current fill color with fade applied.
    pub fn current_fill_color(&self) -> Color {
        let mut color = self.style.fill_color;
        color.a *= self.fade_progress;
        color
    }

    /// Get the current border color with fade applied.
    pub fn current_border_color(&self) -> Color {
        let mut color = self.style.border_color;
        color.a *= self.fade_progress;
        color
    }

    /// Check if animation is complete.
    pub fn is_fully_visible(&self) -> bool {
        self.fade_progress >= 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preview_animation() {
        let bounds = LayoutRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        };
        let mut preview = DropPreview::new(DockZone::Left, bounds);

        assert_eq!(preview.fade_progress, 0.0);
        assert!(!preview.is_fully_visible());

        // Update animation
        preview.update_animation(0.1); // 0.1 seconds
        assert!(preview.fade_progress > 0.0);
        assert!(preview.fade_progress < 1.0);

        // Complete animation
        preview.update_animation(0.5); // More than enough time
        assert!(preview.is_fully_visible());
        assert_eq!(preview.fade_progress, 1.0);
    }

    #[test]
    fn test_fade_color() {
        let bounds = LayoutRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        };
        let mut preview = DropPreview::new(DockZone::Center, bounds);
        preview.fade_progress = 0.5;

        let color = preview.current_fill_color();
        let original_alpha = preview.style.fill_color.a;
        assert!((color.a - (original_alpha * 0.5)).abs() < 0.001);
    }

    #[test]
    fn test_custom_style() {
        let bounds = LayoutRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        };

        let custom_style = DropPreviewStyle::new()
            .fill_color(Color::from_rgb_u8(255, 0, 0))
            .border_width(4.0);

        let preview = DropPreview::new(DockZone::Top, bounds).with_style(custom_style);

        assert!((preview.style.fill_color.r - 1.0).abs() < 0.001); // 255/255.0 = 1.0
        assert!((preview.style.border_width - 4.0).abs() < 0.001);
    }
}
