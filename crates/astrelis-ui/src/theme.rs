//! Theme system for consistent UI styling.
//!
//! Provides centralized color palettes, typography, spacing, and shape definitions
//! for building cohesive user interfaces.
//!
//! # Example
//!
//! ```ignore
//! use astrelis_ui::*;
//!
//! // Use built-in dark theme
//! let theme = Theme::dark();
//!
//! // Or create a custom theme
//! let custom = Theme::builder()
//!     .primary(Color::from_rgb_u8(60, 120, 200))
//!     .secondary(Color::from_rgb_u8(100, 180, 100))
//!     .build();
//!
//! // Apply theme to UI system
//! ui_system.set_theme(custom);
//!
//! // Widgets automatically use theme colors
//! let button = Button::new("Click").color_role(ColorRole::Primary);
//! ```

use astrelis_render::Color;

/// Color role for semantic color assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorRole {
    /// Primary brand color
    Primary,
    /// Secondary brand color
    Secondary,
    /// Background color
    Background,
    /// Surface color (cards, panels)
    Surface,
    /// Error/danger color
    Error,
    /// Warning color
    Warning,
    /// Success color
    Success,
    /// Info color
    Info,
    /// Primary text color
    TextPrimary,
    /// Secondary/muted text color
    TextSecondary,
    /// Disabled text color
    TextDisabled,
    /// Border color
    Border,
    /// Divider color
    Divider,
}

/// Color palette for a theme.
#[derive(Debug, Clone)]
pub struct ColorPalette {
    /// Primary brand color
    pub primary: Color,
    /// Secondary brand color
    pub secondary: Color,
    /// Background color
    pub background: Color,
    /// Surface color (cards, panels, elevated elements)
    pub surface: Color,
    /// Error/danger color
    pub error: Color,
    /// Warning color
    pub warning: Color,
    /// Success color
    pub success: Color,
    /// Info color
    pub info: Color,
    /// Primary text color
    pub text_primary: Color,
    /// Secondary/muted text color
    pub text_secondary: Color,
    /// Disabled text color
    pub text_disabled: Color,
    /// Border color
    pub border: Color,
    /// Divider color
    pub divider: Color,
    /// Hover overlay color (applied on top of elements)
    pub hover_overlay: Color,
    /// Active/pressed overlay color
    pub active_overlay: Color,
}

impl ColorPalette {
    /// Get a color by its role.
    pub fn get(&self, role: ColorRole) -> Color {
        match role {
            ColorRole::Primary => self.primary,
            ColorRole::Secondary => self.secondary,
            ColorRole::Background => self.background,
            ColorRole::Surface => self.surface,
            ColorRole::Error => self.error,
            ColorRole::Warning => self.warning,
            ColorRole::Success => self.success,
            ColorRole::Info => self.info,
            ColorRole::TextPrimary => self.text_primary,
            ColorRole::TextSecondary => self.text_secondary,
            ColorRole::TextDisabled => self.text_disabled,
            ColorRole::Border => self.border,
            ColorRole::Divider => self.divider,
        }
    }

    /// Create a dark color palette.
    pub fn dark() -> Self {
        Self {
            primary: Color::from_rgb_u8(60, 120, 200),
            secondary: Color::from_rgb_u8(100, 180, 100),
            background: Color::from_rgb_u8(18, 18, 18),
            surface: Color::from_rgb_u8(30, 30, 30),
            error: Color::from_rgb_u8(220, 60, 60),
            warning: Color::from_rgb_u8(255, 180, 60),
            success: Color::from_rgb_u8(80, 200, 120),
            info: Color::from_rgb_u8(100, 180, 255),
            text_primary: Color::from_rgb_u8(255, 255, 255),
            text_secondary: Color::from_rgb_u8(180, 180, 180),
            text_disabled: Color::from_rgb_u8(100, 100, 100),
            border: Color::from_rgb_u8(60, 60, 60),
            divider: Color::from_rgb_u8(40, 40, 40),
            hover_overlay: Color::from_rgba_u8(255, 255, 255, 20),
            active_overlay: Color::from_rgba_u8(255, 255, 255, 40),
        }
    }

    /// Create a light color palette.
    pub fn light() -> Self {
        Self {
            primary: Color::from_rgb_u8(50, 100, 200),
            secondary: Color::from_rgb_u8(80, 160, 80),
            background: Color::from_rgb_u8(250, 250, 250),
            surface: Color::from_rgb_u8(255, 255, 255),
            error: Color::from_rgb_u8(200, 50, 50),
            warning: Color::from_rgb_u8(220, 150, 50),
            success: Color::from_rgb_u8(60, 180, 100),
            info: Color::from_rgb_u8(80, 150, 220),
            text_primary: Color::from_rgb_u8(0, 0, 0),
            text_secondary: Color::from_rgb_u8(100, 100, 100),
            text_disabled: Color::from_rgb_u8(180, 180, 180),
            border: Color::from_rgb_u8(200, 200, 200),
            divider: Color::from_rgb_u8(230, 230, 230),
            hover_overlay: Color::from_rgba_u8(0, 0, 0, 15),
            active_overlay: Color::from_rgba_u8(0, 0, 0, 30),
        }
    }
}

impl Default for ColorPalette {
    fn default() -> Self {
        Self::dark()
    }
}

/// Typography settings for a theme.
#[derive(Debug, Clone)]
pub struct Typography {
    /// Default font family
    pub font_family: String,
    /// Heading font sizes (h1-h6)
    pub heading_sizes: [f32; 6],
    /// Body text size
    pub body_size: f32,
    /// Small text size (captions, labels)
    pub small_size: f32,
    /// Tiny text size (hints, footnotes)
    pub tiny_size: f32,
    /// Line height multiplier
    pub line_height: f32,
}

impl Typography {
    /// Create default typography settings.
    pub fn new() -> Self {
        Self {
            font_family: String::new(), // System default
            heading_sizes: [48.0, 40.0, 32.0, 24.0, 20.0, 16.0],
            body_size: 14.0,
            small_size: 12.0,
            tiny_size: 10.0,
            line_height: 1.5,
        }
    }

    /// Get a heading size by level (1-6).
    pub fn heading_size(&self, level: usize) -> f32 {
        if level == 0 || level > 6 {
            self.body_size
        } else {
            self.heading_sizes[level - 1]
        }
    }
}

impl Default for Typography {
    fn default() -> Self {
        Self::new()
    }
}

/// Spacing scale for consistent layout.
#[derive(Debug, Clone, Copy)]
pub struct Spacing {
    /// Extra small spacing (2px)
    pub xs: f32,
    /// Small spacing (4px)
    pub sm: f32,
    /// Medium spacing (8px)
    pub md: f32,
    /// Large spacing (16px)
    pub lg: f32,
    /// Extra large spacing (24px)
    pub xl: f32,
    /// Extra extra large spacing (32px)
    pub xxl: f32,
}

impl Spacing {
    /// Create default spacing scale.
    pub fn new() -> Self {
        Self {
            xs: 2.0,
            sm: 4.0,
            md: 8.0,
            lg: 16.0,
            xl: 24.0,
            xxl: 32.0,
        }
    }

    /// Get spacing by name.
    pub fn get(&self, name: &str) -> f32 {
        match name {
            "xs" => self.xs,
            "sm" => self.sm,
            "md" => self.md,
            "lg" => self.lg,
            "xl" => self.xl,
            "xxl" => self.xxl,
            _ => self.md,
        }
    }
}

impl Default for Spacing {
    fn default() -> Self {
        Self::new()
    }
}

/// Shape definitions for consistent rounded corners.
#[derive(Debug, Clone, Copy)]
pub struct Shapes {
    /// No rounding
    pub none: f32,
    /// Small border radius (2px)
    pub sm: f32,
    /// Medium border radius (4px)
    pub md: f32,
    /// Large border radius (8px)
    pub lg: f32,
    /// Extra large border radius (16px)
    pub xl: f32,
    /// Fully rounded (pill shape)
    pub full: f32,
}

impl Shapes {
    /// Create default shape definitions.
    pub fn new() -> Self {
        Self {
            none: 0.0,
            sm: 2.0,
            md: 4.0,
            lg: 8.0,
            xl: 16.0,
            full: 9999.0, // Large value for fully rounded
        }
    }

    /// Get shape by name.
    pub fn get(&self, name: &str) -> f32 {
        match name {
            "none" => self.none,
            "sm" => self.sm,
            "md" => self.md,
            "lg" => self.lg,
            "xl" => self.xl,
            "full" => self.full,
            _ => self.md,
        }
    }
}

impl Default for Shapes {
    fn default() -> Self {
        Self::new()
    }
}

/// A complete theme definition.
#[derive(Debug, Clone)]
pub struct Theme {
    /// Color palette
    pub colors: ColorPalette,
    /// Typography settings
    pub typography: Typography,
    /// Spacing scale
    pub spacing: Spacing,
    /// Shape definitions
    pub shapes: Shapes,
}

impl Theme {
    /// Create a new theme with default settings.
    pub fn new() -> Self {
        Self {
            colors: ColorPalette::default(),
            typography: Typography::default(),
            spacing: Spacing::default(),
            shapes: Shapes::default(),
        }
    }

    /// Create a dark theme.
    pub fn dark() -> Self {
        Self {
            colors: ColorPalette::dark(),
            typography: Typography::new(),
            spacing: Spacing::new(),
            shapes: Shapes::new(),
        }
    }

    /// Create a light theme.
    pub fn light() -> Self {
        Self {
            colors: ColorPalette::light(),
            typography: Typography::new(),
            spacing: Spacing::new(),
            shapes: Shapes::new(),
        }
    }

    /// Create a theme builder.
    pub fn builder() -> ThemeBuilder {
        ThemeBuilder::new()
    }

    /// Get a color by role.
    pub fn color(&self, role: ColorRole) -> Color {
        self.colors.get(role)
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

/// Builder for creating custom themes.
pub struct ThemeBuilder {
    theme: Theme,
}

impl ThemeBuilder {
    /// Create a new theme builder.
    pub fn new() -> Self {
        Self {
            theme: Theme::default(),
        }
    }

    /// Start with a dark theme.
    pub fn dark() -> Self {
        Self {
            theme: Theme::dark(),
        }
    }

    /// Start with a light theme.
    pub fn light() -> Self {
        Self {
            theme: Theme::light(),
        }
    }

    /// Set the primary color.
    pub fn primary(mut self, color: Color) -> Self {
        self.theme.colors.primary = color;
        self
    }

    /// Set the secondary color.
    pub fn secondary(mut self, color: Color) -> Self {
        self.theme.colors.secondary = color;
        self
    }

    /// Set the background color.
    pub fn background(mut self, color: Color) -> Self {
        self.theme.colors.background = color;
        self
    }

    /// Set the surface color.
    pub fn surface(mut self, color: Color) -> Self {
        self.theme.colors.surface = color;
        self
    }

    /// Set the error color.
    pub fn error(mut self, color: Color) -> Self {
        self.theme.colors.error = color;
        self
    }

    /// Set the font family.
    pub fn font_family(mut self, family: impl Into<String>) -> Self {
        self.theme.typography.font_family = family.into();
        self
    }

    /// Set the body font size.
    pub fn body_size(mut self, size: f32) -> Self {
        self.theme.typography.body_size = size;
        self
    }

    /// Set a custom color palette.
    pub fn colors(mut self, colors: ColorPalette) -> Self {
        self.theme.colors = colors;
        self
    }

    /// Set custom typography.
    pub fn typography(mut self, typography: Typography) -> Self {
        self.theme.typography = typography;
        self
    }

    /// Set custom spacing.
    pub fn spacing(mut self, spacing: Spacing) -> Self {
        self.theme.spacing = spacing;
        self
    }

    /// Set custom shapes.
    pub fn shapes(mut self, shapes: Shapes) -> Self {
        self.theme.shapes = shapes;
        self
    }

    /// Build the theme.
    pub fn build(self) -> Theme {
        self.theme
    }
}

impl Default for ThemeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dark_theme() {
        let theme = Theme::dark();
        assert_eq!(theme.colors.primary, Color::from_rgb_u8(60, 120, 200));
        assert_eq!(theme.typography.body_size, 14.0);
    }

    #[test]
    fn test_light_theme() {
        let theme = Theme::light();
        assert_eq!(theme.colors.background, Color::from_rgb_u8(250, 250, 250));
    }

    #[test]
    fn test_theme_builder() {
        let theme = Theme::builder()
            .primary(Color::RED)
            .secondary(Color::BLUE)
            .body_size(16.0)
            .build();

        assert_eq!(theme.colors.primary, Color::RED);
        assert_eq!(theme.colors.secondary, Color::BLUE);
        assert_eq!(theme.typography.body_size, 16.0);
    }

    #[test]
    fn test_color_roles() {
        let theme = Theme::dark();
        let primary = theme.color(ColorRole::Primary);
        assert_eq!(primary, theme.colors.primary);
    }

    #[test]
    fn test_spacing() {
        let spacing = Spacing::new();
        assert_eq!(spacing.xs, 2.0);
        assert_eq!(spacing.lg, 16.0);
        assert_eq!(spacing.get("md"), 8.0);
    }

    #[test]
    fn test_shapes() {
        let shapes = Shapes::new();
        assert_eq!(shapes.sm, 2.0);
        assert_eq!(shapes.lg, 8.0);
        assert_eq!(shapes.get("md"), 4.0);
    }

    #[test]
    fn test_typography_heading_sizes() {
        let typography = Typography::new();
        assert_eq!(typography.heading_size(1), 48.0);
        assert_eq!(typography.heading_size(6), 16.0);
        assert_eq!(typography.heading_size(0), typography.body_size);
        assert_eq!(typography.heading_size(7), typography.body_size);
    }
}
