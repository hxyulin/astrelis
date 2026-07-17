//! Theme tokens and per-widget visual overrides.

use astrelis_core::color::Color;
use astrelis_text::FontFamily;

use crate::layout::Insets;

/// Optional direct visual overrides for one widget.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WidgetStyle {
    /// Foreground/text color override.
    pub foreground: Option<Color>,
    /// Background color override.
    pub background: Option<Color>,
}

/// Typed visual style for a checkbox.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CheckboxStyle {
    /// Box background.
    pub background: Color,
    /// Checked indicator.
    pub indicator: Color,
    /// Corner radius.
    pub radius: f32,
}

/// Typed visual style for a horizontal slider.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderStyle {
    /// Track color.
    pub track: Color,
    /// Thumb color.
    pub thumb: Color,
    /// Thumb diameter.
    pub thumb_size: f32,
}

/// Typed visual style for a vertical scroll view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollViewStyle {
    /// Scrollbar track color.
    pub track: Color,
    /// Scrollbar thumb color.
    pub thumb: Color,
    /// Scrollbar width.
    pub width: f32,
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

/// Typed visual tokens used by the built-in widgets.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    /// Window/background color.
    pub background: Color,
    /// Primary foreground color.
    pub foreground: Color,
    /// Muted foreground color.
    pub muted_foreground: Color,
    /// Text selection color.
    pub selection: Color,
    /// Caret and focus-ring color.
    pub accent: Color,
    /// Button state colors.
    pub button: ControlColors,
    /// Text-field background.
    pub field_background: Color,
    /// Default logical font size.
    pub font_size: f32,
    /// Ordered font families used by built-in widget text.
    pub font_families: Vec<FontFamily>,
    /// Default inter-widget gap.
    pub gap: f32,
    /// Default control padding.
    pub control_padding: Insets,
    /// Default corner radius.
    pub corner_radius: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::new(0.035, 0.045, 0.075, 1.0),
            foreground: Color::new(0.92, 0.94, 1.0, 1.0),
            muted_foreground: Color::new(0.58, 0.64, 0.75, 1.0),
            selection: Color::new(0.12, 0.38, 0.68, 0.85),
            accent: Color::new(0.25, 0.75, 1.0, 1.0),
            button: ControlColors {
                normal: Color::new(0.12, 0.16, 0.26, 1.0),
                hovered: Color::new(0.16, 0.22, 0.35, 1.0),
                pressed: Color::new(0.09, 0.35, 0.5, 1.0),
                disabled: Color::new(0.08, 0.09, 0.12, 1.0),
            },
            field_background: Color::new(0.065, 0.08, 0.13, 1.0),
            font_size: 16.0,
            font_families: vec![FontFamily::SansSerif],
            gap: 10.0,
            control_padding: Insets {
                left: 12.0,
                top: 8.0,
                right: 12.0,
                bottom: 8.0,
            },
            corner_radius: 6.0,
        }
    }
}
