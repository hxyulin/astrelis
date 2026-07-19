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
    /// Font-weight override (CSS-style, 100-900); unset resolves to
    /// `theme.type_scale.body_weight`.
    pub font_weight: Option<f32>,
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

/// A font-size and font-weight scale.
///
/// Sizes are logical pixels; weights use the CSS convention (400 regular,
/// 600 semibold). In a desktop tool the heading barely exceeds the body size —
/// hierarchy comes from `heading_weight`, not from a large point jump.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeScale {
    /// Section-heading size.
    pub heading: f32,
    /// Default body size.
    pub body: f32,
    /// Secondary caption size.
    pub caption: f32,
    /// Section-heading font weight.
    pub heading_weight: f32,
    /// Default body font weight.
    pub body_weight: f32,
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
/// Rendered as a true gaussian shadow by `astrelis-paint`'s analytic
/// rounded-rect shadow primitive ([`astrelis_paint::ShadowStyle`]); `blur` is a
/// real blur radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    /// Shadow color, including its alpha.
    pub color: Color,
    /// Offset of the shadow from the casting rect.
    pub offset: Vec2,
    /// Gaussian blur radius in logical pixels.
    pub blur: f32,
    /// Outward expansion of the casting rect before blurring (may be
    /// negative to tuck the shadow under its surface).
    pub spread: f32,
}

impl Default for Shadow {
    fn default() -> Self {
        Theme::dark().shadow
    }
}

/// Typed visual tokens used by the built-in widgets.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    /// Window/background color.
    pub background: Color,
    /// Raised-surface color for panels and cards.
    pub surface: Color,
    /// Floating-surface color for menus, popovers, dialogs, and toasts — one
    /// elevation step above [`Theme::surface`].
    pub overlay: Color,
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
    /// Foreground for text and icons painted on accent-filled surfaces.
    pub accent_foreground: Color,
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
    /// The default dark theme: a hue-neutral zinc ramp with a restrained blue
    /// accent and hairline borders.
    ///
    /// Colors are authored as sRGB hex via [`Color::from_hex`], so the values
    /// here are exactly what reaches the screen.
    pub fn dark() -> Self {
        Self {
            background: Color::from_hex(0x131316),
            surface: Color::from_hex(0x1b1b1f),
            overlay: Color::from_hex(0x212126),
            foreground: Color::from_hex(0xededef),
            muted_foreground: Color::from_hex(0x9d9da6),
            disabled_foreground: Color::from_hex(0x62626b),
            selection: Color::from_hex(0x4c8dff).with_alpha(0.35),
            accent: Color::from_hex(0x4c8dff),
            accent_foreground: Color::from_hex(0xffffff),
            danger: Color::from_hex(0xf2555a),
            success: Color::from_hex(0x3fb950),
            warning: Color::from_hex(0xd29922),
            // Hover lifts a step; pressed drops below normal for an inset
            // feel instead of an accent flash.
            button: ControlColors {
                normal: Color::from_hex(0x232328),
                hovered: Color::from_hex(0x2b2b31),
                pressed: Color::from_hex(0x1e1e23),
                disabled: Color::from_hex(0x1c1c20),
            },
            field_background: Color::from_hex(0x101013),
            border: Color::from_hex(0x2a2a30),
            border_width: 1.0,
            radii: Radii {
                sm: 3.0,
                md: 5.0,
                lg: 8.0,
            },
            spacing: Spacing {
                xs: 4.0,
                sm: 8.0,
                md: 12.0,
                lg: 16.0,
                xl: 24.0,
            },
            type_scale: TypeScale {
                heading: 15.0,
                body: 13.0,
                caption: 11.0,
                heading_weight: 600.0,
                body_weight: 400.0,
            },
            metrics: ControlMetrics {
                focus_ring: 2.0,
                checkbox_inset: 4.0,
                slider_track: 4.0,
                slider_thumb: 14.0,
                scrollbar_width: 6.0,
                scrollbar_min_thumb: 24.0,
            },
            shadow: Shadow {
                color: Color::new(0.0, 0.0, 0.0, 0.45),
                offset: Vec2::new(0.0, 6.0),
                blur: 20.0,
                spread: -2.0,
            },
            font_families: vec![FontFamily::SansSerif],
            gap: 8.0,
            control_padding: Insets {
                left: 10.0,
                top: 6.0,
                right: 10.0,
                bottom: 6.0,
            },
        }
    }

    /// A light theme sharing the dark theme's geometry.
    ///
    /// Only the color tokens and the shadow differ; the spacing, radius, type,
    /// and control-metric scales are identical, so the two themes lay out and
    /// measure the same content identically. Elevation in light mode comes
    /// from borders and a fainter shadow rather than surface lightness.
    pub fn light() -> Self {
        Self {
            background: Color::from_hex(0xf7f7f8),
            surface: Color::from_hex(0xffffff),
            overlay: Color::from_hex(0xffffff),
            foreground: Color::from_hex(0x202024),
            muted_foreground: Color::from_hex(0x6e6e78),
            disabled_foreground: Color::from_hex(0xa9a9b2),
            selection: Color::from_hex(0x2563eb).with_alpha(0.25),
            accent: Color::from_hex(0x2563eb),
            accent_foreground: Color::from_hex(0xffffff),
            danger: Color::from_hex(0xd92d2d),
            success: Color::from_hex(0x1a7f37),
            warning: Color::from_hex(0x9a6700),
            button: ControlColors {
                normal: Color::from_hex(0xffffff),
                hovered: Color::from_hex(0xf2f2f5),
                pressed: Color::from_hex(0xe9e9ee),
                disabled: Color::from_hex(0xf6f6f8),
            },
            field_background: Color::from_hex(0xffffff),
            border: Color::from_hex(0xe1e1e6),
            shadow: Shadow {
                color: Color::from_hex(0x16181d).with_alpha(0.14),
                offset: Vec2::new(0.0, 4.0),
                blur: 14.0,
                spread: 0.0,
            },
            ..Self::dark()
        }
    }
}
