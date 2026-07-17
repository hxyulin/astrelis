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
    /// Corner radius; unset falls back to `theme.corner_radius`.
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
    /// Thumb diameter; unset falls back to the built-in default.
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
    /// Scrollbar width; unset falls back to the built-in default.
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
