//! Theme tokens and per-widget visual overrides.

use astrelis_core::color::Color;
use astrelis_core::math::Vec2;
use astrelis_text::FontFamily;

use crate::layout::Insets;

/// Optional direct visual overrides for one widget.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WidgetStyle {
    /// Foreground/text color override.
    pub foreground: Option<Color>,
    /// Background color override.
    pub background: Option<Color>,
    /// Font-size override in logical pixels; unset resolves to
    /// `theme.type_scale.body`.
    pub font_size: Option<f32>,
}

/// Optional visual overrides for a checkbox.
///
/// Every field defaults to `None`, meaning "resolve from the active theme when
/// painting." Overrides therefore stay live across `set_theme`: an unset field
/// always tracks the current theme rather than snapshotting it at creation.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CheckboxStyle {
    /// Box background; unset falls back to `theme.button.normal`.
    pub background: Option<Color>,
    /// Checked indicator; unset falls back to `theme.accent`.
    pub indicator: Option<Color>,
    /// Corner radius; unset falls back to `theme.radii.md`.
    pub radius: Option<f32>,
}

/// Optional visual overrides for a horizontal slider.
///
/// Unset fields resolve from the active theme at paint time; see
/// [`CheckboxStyle`] for the rationale.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SliderStyle {
    /// Track color; unset falls back to `theme.button.normal`.
    pub track: Option<Color>,
    /// Thumb color; unset falls back to `theme.accent`.
    pub thumb: Option<Color>,
    /// Thumb diameter; unset falls back to `theme.metrics.slider_thumb`.
    pub thumb_size: Option<f32>,
}

/// Optional visual overrides for a vertical scroll view.
///
/// Unset fields resolve from the active theme at paint time; see
/// [`CheckboxStyle`] for the rationale.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollViewStyle {
    /// Scrollbar track color; unset falls back to `theme.button.normal`.
    pub track: Option<Color>,
    /// Scrollbar thumb color; unset falls back to `theme.accent`.
    pub thumb: Option<Color>,
    /// Scrollbar width; unset falls back to `theme.metrics.scrollbar_width`.
    pub width: Option<f32>,
}

/// Interaction state of a control, used to resolve state-dependent colors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlState {
    /// Whether the control accepts input.
    pub enabled: bool,
    /// Whether the pointer is over the control.
    pub hovered: bool,
    /// Whether the control is actively pressed.
    pub pressed: bool,
}

/// Visual state colors for an interactive control.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlColors {
    /// Normal background.
    pub normal: Color,
    /// Hovered background.
    pub hovered: Color,
    /// Pressed background.
    pub pressed: Color,
    /// Disabled background.
    pub disabled: Color,
}

impl ControlColors {
    /// Selects the background color for a given interaction state.
    ///
    /// Precedence is disabled, then pressed, then hovered, then normal. This is
    /// the single resolution path shared by every built-in control and by
    /// application widgets (such as docking tabs) that map their own states
    /// onto a [`ControlState`].
    pub fn resolve(&self, state: ControlState) -> Color {
        if !state.enabled {
            self.disabled
        } else if state.pressed {
            self.pressed
        } else if state.hovered {
            self.hovered
        } else {
            self.normal
        }
    }
}

/// A corner-radius scale in logical pixels.
///
/// A three-step scale keeps widgets from doing arithmetic on a single radius
/// token (the old `corner_radius * 1.5` idiom): pick the step that matches the
/// element's visual weight instead.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Radii {
    /// Tight radius for indicators and small chips.
    pub sm: f32,
    /// Default control radius.
    pub md: f32,
    /// Emphasized radius for cards, render views, and drop targets.
    pub lg: f32,
}

/// A spacing scale in logical pixels for gaps, insets, and padding.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spacing {
    /// Extra-small step.
    pub xs: f32,
    /// Small step.
    pub sm: f32,
    /// Medium step.
    pub md: f32,
    /// Large step.
    pub lg: f32,
    /// Extra-large step.
    pub xl: f32,
}

/// A font-size scale in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeScale {
    /// Section-heading size.
    pub heading: f32,
    /// Default body size.
    pub body: f32,
    /// Secondary caption size.
    pub caption: f32,
}

/// Fixed geometry for the built-in controls, in logical pixels.
///
/// These are the theme-level defaults; per-element typed styles (such as
/// [`SliderStyle::thumb_size`]) override them where set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlMetrics {
    /// Focus-ring bar thickness.
    pub focus_ring: f32,
    /// Inset of a checkbox's checked indicator from its box edge.
    pub checkbox_inset: f32,
    /// Slider track height.
    pub slider_track: f32,
    /// Slider thumb diameter (fallback for [`SliderStyle::thumb_size`]).
    pub slider_thumb: f32,
    /// Scrollbar width (fallback for [`ScrollViewStyle::width`]).
    pub scrollbar_width: f32,
    /// Minimum scrollbar-thumb length.
    pub scrollbar_min_thumb: f32,
}

/// A drop-shadow / elevation token.
///
/// `astrelis-paint` has no gaussian-blur primitive, so the built-in widgets
/// approximate this with a short stack of translucent offset rounded rects.
/// `blur` controls how far that stack spreads outward, not a true blur radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    /// Shadow color, including its alpha.
    pub color: Color,
    /// Offset of the shadow from the casting rect.
    pub offset: Vec2,
    /// Spread distance approximating a blur radius.
    pub blur: f32,
}

/// Typed visual tokens used by the built-in widgets.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    /// Window/background color.
    pub background: Color,
    /// Raised-surface color for panels, cards, and popovers.
    pub surface: Color,
    /// Primary foreground color.
    pub foreground: Color,
    /// Muted foreground color.
    pub muted_foreground: Color,
    /// Foreground color for disabled controls.
    pub disabled_foreground: Color,
    /// Text selection color.
    pub selection: Color,
    /// Caret and focus-ring color.
    pub accent: Color,
    /// Destructive/error status color.
    pub danger: Color,
    /// Positive/confirmation status color.
    pub success: Color,
    /// Caution status color.
    pub warning: Color,
    /// Button state colors.
    pub button: ControlColors,
    /// Text-field background.
    pub field_background: Color,
    /// Default border/outline color.
    pub border: Color,
    /// Default border/outline width.
    pub border_width: f32,
    /// Corner-radius scale.
    pub radii: Radii,
    /// Spacing scale.
    pub spacing: Spacing,
    /// Font-size scale.
    pub type_scale: TypeScale,
    /// Built-in control geometry.
    pub metrics: ControlMetrics,
    /// Elevation/drop-shadow token.
    pub shadow: Shadow,
    /// Ordered font families used by built-in widget text.
    pub font_families: Vec<FontFamily>,
    /// Default inter-widget gap.
    pub gap: f32,
    /// Default control padding.
    pub control_padding: Insets,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    /// The default dark theme.
    pub fn dark() -> Self {
        Self {
            background: Color::new(0.027, 0.030, 0.038, 1.0),
            surface: Color::new(0.074, 0.080, 0.094, 1.0),
            foreground: Color::new(0.94, 0.95, 0.97, 1.0),
            muted_foreground: Color::new(0.55, 0.58, 0.65, 1.0),
            disabled_foreground: Color::new(0.34, 0.36, 0.42, 1.0),
            selection: Color::new(0.16, 0.40, 0.70, 0.75),
            accent: Color::new(0.32, 0.64, 1.0, 1.0),
            danger: Color::new(0.92, 0.34, 0.38, 1.0),
            success: Color::new(0.32, 0.78, 0.48, 1.0),
            warning: Color::new(0.96, 0.72, 0.28, 1.0),
            button: ControlColors {
                normal: Color::new(0.11, 0.12, 0.15, 1.0),
                hovered: Color::new(0.16, 0.175, 0.21, 1.0),
                pressed: Color::new(0.16, 0.36, 0.60, 1.0),
                disabled: Color::new(0.075, 0.08, 0.095, 1.0),
            },
            field_background: Color::new(0.050, 0.055, 0.068, 1.0),
            border: Color::new(0.17, 0.185, 0.225, 1.0),
            border_width: 1.5,
            radii: Radii {
                sm: 3.0,
                md: 6.0,
                lg: 9.0,
            },
            spacing: Spacing {
                xs: 4.0,
                sm: 8.0,
                md: 12.0,
                lg: 16.0,
                xl: 24.0,
            },
            type_scale: TypeScale {
                heading: 22.0,
                body: 16.0,
                caption: 13.0,
            },
            metrics: ControlMetrics {
                focus_ring: 2.0,
                checkbox_inset: 6.0,
                slider_track: 4.0,
                slider_thumb: 16.0,
                scrollbar_width: 8.0,
                scrollbar_min_thumb: 24.0,
            },
            shadow: Shadow {
                color: Color::new(0.0, 0.0, 0.0, 0.40),
                offset: Vec2::new(0.0, 4.0),
                blur: 13.0,
            },
            font_families: vec![FontFamily::SansSerif],
            gap: 10.0,
            control_padding: Insets {
                left: 12.0,
                top: 8.0,
                right: 12.0,
                bottom: 8.0,
            },
        }
    }

    /// A light theme sharing the dark theme's geometry.
    ///
    /// Only the color tokens and the shadow differ; the spacing, radius, type,
    /// and control-metric scales are identical, so the two themes lay out and
    /// measure the same content identically.
    pub fn light() -> Self {
        Self {
            background: Color::new(0.90, 0.91, 0.94, 1.0),
            surface: Color::new(1.0, 1.0, 1.0, 1.0),
            foreground: Color::new(0.10, 0.12, 0.16, 1.0),
            muted_foreground: Color::new(0.40, 0.44, 0.50, 1.0),
            disabled_foreground: Color::new(0.64, 0.68, 0.74, 1.0),
            selection: Color::new(0.40, 0.62, 0.95, 0.40),
            accent: Color::new(0.10, 0.48, 0.92, 1.0),
            danger: Color::new(0.80, 0.20, 0.24, 1.0),
            success: Color::new(0.16, 0.58, 0.30, 1.0),
            warning: Color::new(0.78, 0.54, 0.10, 1.0),
            button: ControlColors {
                normal: Color::new(1.0, 1.0, 1.0, 1.0),
                hovered: Color::new(0.93, 0.94, 0.97, 1.0),
                pressed: Color::new(0.85, 0.89, 0.98, 1.0),
                disabled: Color::new(0.94, 0.95, 0.96, 1.0),
            },
            field_background: Color::new(1.0, 1.0, 1.0, 1.0),
            border: Color::new(0.76, 0.79, 0.85, 1.0),
            shadow: Shadow {
                color: Color::new(0.10, 0.12, 0.18, 0.18),
                offset: Vec2::new(0.0, 4.0),
                blur: 14.0,
            },
            ..Self::dark()
        }
    }
}
