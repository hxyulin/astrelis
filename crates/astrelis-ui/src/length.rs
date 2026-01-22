//! Type-safe length and dimension types for UI styling.
//!
//! Provides enum-based alternatives to raw floats for better type safety
//! and clearer intent in style declarations.

use astrelis_core::math::Vec2;
use std::fmt;

/// Length value for UI dimensions.
///
/// Represents different ways to specify sizes in the UI system.
///
/// # Examples
/// ```
/// use astrelis_ui::Length;
///
/// let fixed = Length::Px(100.0);
/// let relative = Length::Percent(50.0);
/// let auto_size = Length::Auto;
/// let viewport_width = Length::Vw(80.0); // 80% of viewport width
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Length {
    /// Fixed pixel value
    Px(f32),
    /// Percentage of parent size (0.0 - 100.0)
    Percent(f32),
    /// Automatic sizing based on content
    Auto,
    /// Percentage of viewport width (0.0 - 100.0)
    Vw(f32),
    /// Percentage of viewport height (0.0 - 100.0)
    Vh(f32),
    /// Percentage of smaller viewport dimension (0.0 - 100.0)
    Vmin(f32),
    /// Percentage of larger viewport dimension (0.0 - 100.0)
    Vmax(f32),
}

impl Length {
    /// Create a pixel length.
    pub fn px(value: f32) -> Self {
        Self::Px(value)
    }

    /// Create a percentage length.
    pub fn percent(value: f32) -> Self {
        Self::Percent(value)
    }

    /// Create an auto length.
    pub fn auto() -> Self {
        Self::Auto
    }

    /// Create a viewport width length (1vw = 1% of viewport width).
    pub fn vw(value: f32) -> Self {
        Self::Vw(value)
    }

    /// Create a viewport height length (1vh = 1% of viewport height).
    pub fn vh(value: f32) -> Self {
        Self::Vh(value)
    }

    /// Create a viewport minimum length (1vmin = 1% of smaller viewport dimension).
    pub fn vmin(value: f32) -> Self {
        Self::Vmin(value)
    }

    /// Create a viewport maximum length (1vmax = 1% of larger viewport dimension).
    pub fn vmax(value: f32) -> Self {
        Self::Vmax(value)
    }

    /// Resolve viewport-relative units to pixels.
    ///
    /// Converts vw/vh/vmin/vmax units to absolute pixel values based on the viewport size.
    /// Other unit types (Px, Percent, Auto) are returned unchanged.
    ///
    /// # Arguments
    /// * `viewport_size` - The viewport dimensions (width, height) in pixels
    ///
    /// # Examples
    /// ```
    /// use astrelis_ui::Length;
    /// use astrelis_core::math::Vec2;
    ///
    /// let viewport = Vec2::new(1280.0, 720.0);
    ///
    /// let width = Length::Vw(50.0).resolve(viewport);
    /// assert_eq!(width, Length::Px(640.0)); // 50% of 1280px
    ///
    /// let height = Length::Vh(100.0).resolve(viewport);
    /// assert_eq!(height, Length::Px(720.0)); // 100% of 720px
    /// ```
    pub fn resolve(self, viewport_size: Vec2) -> Self {
        match self {
            Length::Vw(v) => Length::Px(v * viewport_size.x / 100.0),
            Length::Vh(v) => Length::Px(v * viewport_size.y / 100.0),
            Length::Vmin(v) => {
                let min = viewport_size.x.min(viewport_size.y);
                Length::Px(v * min / 100.0)
            }
            Length::Vmax(v) => {
                let max = viewport_size.x.max(viewport_size.y);
                Length::Px(v * max / 100.0)
            }
            other => other, // Px/Percent/Auto unchanged
        }
    }

    /// Convert to Taffy Dimension.
    ///
    /// # Panics
    /// Panics if called on viewport-relative units (Vw/Vh/Vmin/Vmax).
    /// These must be resolved to pixels first using [`Length::resolve`].
    pub fn to_dimension(self) -> taffy::Dimension {
        match self {
            Length::Px(v) => taffy::Dimension::Length(v),
            Length::Percent(v) => taffy::Dimension::Percent(v / 100.0),
            Length::Auto => taffy::Dimension::Auto,
            Length::Vw(_) | Length::Vh(_) | Length::Vmin(_) | Length::Vmax(_) => {
                panic!(
                    "Viewport-relative units must be resolved to pixels before converting to Taffy dimension. \
                     Call .resolve(viewport_size) first."
                );
            }
        }
    }

    /// Convert from Taffy Dimension.
    pub fn from_dimension(dim: taffy::Dimension) -> Self {
        match dim {
            taffy::Dimension::Length(v) => Length::Px(v),
            taffy::Dimension::Percent(v) => Length::Percent(v * 100.0),
            taffy::Dimension::Auto => Length::Auto,
        }
    }

    /// Check if this is a fixed pixel value.
    pub fn is_px(&self) -> bool {
        matches!(self, Length::Px(_))
    }

    /// Check if this is a percentage value.
    pub fn is_percent(&self) -> bool {
        matches!(self, Length::Percent(_))
    }

    /// Check if this is auto.
    pub fn is_auto(&self) -> bool {
        matches!(self, Length::Auto)
    }

    /// Check if this is a viewport-relative unit.
    pub fn is_viewport(&self) -> bool {
        matches!(
            self,
            Length::Vw(_) | Length::Vh(_) | Length::Vmin(_) | Length::Vmax(_)
        )
    }
}

impl From<f32> for Length {
    fn from(value: f32) -> Self {
        Length::Px(value)
    }
}

impl From<Length> for taffy::Dimension {
    fn from(length: Length) -> Self {
        length.to_dimension()
    }
}

impl fmt::Display for Length {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Length::Px(v) => write!(f, "{}px", v),
            Length::Percent(v) => write!(f, "{}%", v),
            Length::Auto => write!(f, "auto"),
            Length::Vw(v) => write!(f, "{}vw", v),
            Length::Vh(v) => write!(f, "{}vh", v),
            Length::Vmin(v) => write!(f, "{}vmin", v),
            Length::Vmax(v) => write!(f, "{}vmax", v),
        }
    }
}

/// Length value that can also be Auto (for margins/insets).
///
/// Similar to Length but allows automatic positioning.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LengthAuto {
    /// Fixed pixel value
    Px(f32),
    /// Percentage of parent size (0.0 - 100.0)
    Percent(f32),
    /// Automatic positioning
    Auto,
    /// Percentage of viewport width (0.0 - 100.0)
    Vw(f32),
    /// Percentage of viewport height (0.0 - 100.0)
    Vh(f32),
    /// Percentage of smaller viewport dimension (0.0 - 100.0)
    Vmin(f32),
    /// Percentage of larger viewport dimension (0.0 - 100.0)
    Vmax(f32),
}

impl LengthAuto {
    /// Create a pixel length.
    pub fn px(value: f32) -> Self {
        Self::Px(value)
    }

    /// Create a percentage length.
    pub fn percent(value: f32) -> Self {
        Self::Percent(value)
    }

    /// Create an auto length.
    pub fn auto() -> Self {
        Self::Auto
    }

    /// Create a viewport width length (1vw = 1% of viewport width).
    pub fn vw(value: f32) -> Self {
        Self::Vw(value)
    }

    /// Create a viewport height length (1vh = 1% of viewport height).
    pub fn vh(value: f32) -> Self {
        Self::Vh(value)
    }

    /// Create a viewport minimum length (1vmin = 1% of smaller viewport dimension).
    pub fn vmin(value: f32) -> Self {
        Self::Vmin(value)
    }

    /// Create a viewport maximum length (1vmax = 1% of larger viewport dimension).
    pub fn vmax(value: f32) -> Self {
        Self::Vmax(value)
    }

    /// Resolve viewport-relative units to pixels.
    pub fn resolve(self, viewport_size: Vec2) -> Self {
        match self {
            LengthAuto::Vw(v) => LengthAuto::Px(v * viewport_size.x / 100.0),
            LengthAuto::Vh(v) => LengthAuto::Px(v * viewport_size.y / 100.0),
            LengthAuto::Vmin(v) => {
                let min = viewport_size.x.min(viewport_size.y);
                LengthAuto::Px(v * min / 100.0)
            }
            LengthAuto::Vmax(v) => {
                let max = viewport_size.x.max(viewport_size.y);
                LengthAuto::Px(v * max / 100.0)
            }
            other => other,
        }
    }

    /// Convert to Taffy LengthPercentageAuto.
    ///
    /// # Panics
    /// Panics if called on viewport-relative units (Vw/Vh/Vmin/Vmax).
    /// These must be resolved to pixels first using [`LengthAuto::resolve`].
    pub fn to_length_percentage_auto(self) -> taffy::LengthPercentageAuto {
        match self {
            LengthAuto::Px(v) => taffy::LengthPercentageAuto::Length(v),
            LengthAuto::Percent(v) => taffy::LengthPercentageAuto::Percent(v / 100.0),
            LengthAuto::Auto => taffy::LengthPercentageAuto::Auto,
            LengthAuto::Vw(_) | LengthAuto::Vh(_) | LengthAuto::Vmin(_) | LengthAuto::Vmax(_) => {
                panic!(
                    "Viewport-relative units must be resolved to pixels before converting to Taffy dimension. \
                     Call .resolve(viewport_size) first."
                );
            }
        }
    }

    /// Convert from Taffy LengthPercentageAuto.
    pub fn from_length_percentage_auto(lpa: taffy::LengthPercentageAuto) -> Self {
        match lpa {
            taffy::LengthPercentageAuto::Length(v) => LengthAuto::Px(v),
            taffy::LengthPercentageAuto::Percent(v) => LengthAuto::Percent(v * 100.0),
            taffy::LengthPercentageAuto::Auto => LengthAuto::Auto,
        }
    }
}

impl From<f32> for LengthAuto {
    fn from(value: f32) -> Self {
        LengthAuto::Px(value)
    }
}

impl From<Length> for LengthAuto {
    fn from(length: Length) -> Self {
        match length {
            Length::Px(v) => LengthAuto::Px(v),
            Length::Percent(v) => LengthAuto::Percent(v),
            Length::Auto => LengthAuto::Auto,
            Length::Vw(v) => LengthAuto::Vw(v),
            Length::Vh(v) => LengthAuto::Vh(v),
            Length::Vmin(v) => LengthAuto::Vmin(v),
            Length::Vmax(v) => LengthAuto::Vmax(v),
        }
    }
}

impl From<LengthAuto> for taffy::LengthPercentageAuto {
    fn from(length: LengthAuto) -> Self {
        length.to_length_percentage_auto()
    }
}

impl fmt::Display for LengthAuto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LengthAuto::Px(v) => write!(f, "{}px", v),
            LengthAuto::Percent(v) => write!(f, "{}%", v),
            LengthAuto::Auto => write!(f, "auto"),
            LengthAuto::Vw(v) => write!(f, "{}vw", v),
            LengthAuto::Vh(v) => write!(f, "{}vh", v),
            LengthAuto::Vmin(v) => write!(f, "{}vmin", v),
            LengthAuto::Vmax(v) => write!(f, "{}vmax", v),
        }
    }
}

/// Length value without Auto (for padding/border).
///
/// Similar to Length but doesn't allow Auto sizing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LengthPercentage {
    /// Fixed pixel value
    Px(f32),
    /// Percentage of parent size (0.0 - 100.0)
    Percent(f32),
    /// Percentage of viewport width (0.0 - 100.0)
    Vw(f32),
    /// Percentage of viewport height (0.0 - 100.0)
    Vh(f32),
    /// Percentage of smaller viewport dimension (0.0 - 100.0)
    Vmin(f32),
    /// Percentage of larger viewport dimension (0.0 - 100.0)
    Vmax(f32),
}

impl LengthPercentage {
    /// Create a pixel length.
    pub fn px(value: f32) -> Self {
        Self::Px(value)
    }

    /// Create a percentage length.
    pub fn percent(value: f32) -> Self {
        Self::Percent(value)
    }

    /// Create a viewport width length (1vw = 1% of viewport width).
    pub fn vw(value: f32) -> Self {
        Self::Vw(value)
    }

    /// Create a viewport height length (1vh = 1% of viewport height).
    pub fn vh(value: f32) -> Self {
        Self::Vh(value)
    }

    /// Create a viewport minimum length (1vmin = 1% of smaller viewport dimension).
    pub fn vmin(value: f32) -> Self {
        Self::Vmin(value)
    }

    /// Create a viewport maximum length (1vmax = 1% of larger viewport dimension).
    pub fn vmax(value: f32) -> Self {
        Self::Vmax(value)
    }

    /// Resolve viewport-relative units to pixels.
    pub fn resolve(self, viewport_size: Vec2) -> Self {
        match self {
            LengthPercentage::Vw(v) => LengthPercentage::Px(v * viewport_size.x / 100.0),
            LengthPercentage::Vh(v) => LengthPercentage::Px(v * viewport_size.y / 100.0),
            LengthPercentage::Vmin(v) => {
                let min = viewport_size.x.min(viewport_size.y);
                LengthPercentage::Px(v * min / 100.0)
            }
            LengthPercentage::Vmax(v) => {
                let max = viewport_size.x.max(viewport_size.y);
                LengthPercentage::Px(v * max / 100.0)
            }
            other => other,
        }
    }

    /// Convert to Taffy LengthPercentage.
    ///
    /// # Panics
    /// Panics if called on viewport-relative units (Vw/Vh/Vmin/Vmax).
    /// These must be resolved to pixels first using [`LengthPercentage::resolve`].
    pub fn to_length_percentage(self) -> taffy::LengthPercentage {
        match self {
            LengthPercentage::Px(v) => taffy::LengthPercentage::Length(v),
            LengthPercentage::Percent(v) => taffy::LengthPercentage::Percent(v / 100.0),
            LengthPercentage::Vw(_)
            | LengthPercentage::Vh(_)
            | LengthPercentage::Vmin(_)
            | LengthPercentage::Vmax(_) => {
                panic!(
                    "Viewport-relative units must be resolved to pixels before converting to Taffy dimension. \
                     Call .resolve(viewport_size) first."
                );
            }
        }
    }

    /// Convert from Taffy LengthPercentage.
    pub fn from_length_percentage(lp: taffy::LengthPercentage) -> Self {
        match lp {
            taffy::LengthPercentage::Length(v) => LengthPercentage::Px(v),
            taffy::LengthPercentage::Percent(v) => LengthPercentage::Percent(v * 100.0),
        }
    }
}

impl From<f32> for LengthPercentage {
    fn from(value: f32) -> Self {
        LengthPercentage::Px(value)
    }
}

impl From<LengthPercentage> for taffy::LengthPercentage {
    fn from(length: LengthPercentage) -> Self {
        length.to_length_percentage()
    }
}

impl fmt::Display for LengthPercentage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LengthPercentage::Px(v) => write!(f, "{}px", v),
            LengthPercentage::Percent(v) => write!(f, "{}%", v),
            LengthPercentage::Vw(v) => write!(f, "{}vw", v),
            LengthPercentage::Vh(v) => write!(f, "{}vh", v),
            LengthPercentage::Vmin(v) => write!(f, "{}vmin", v),
            LengthPercentage::Vmax(v) => write!(f, "{}vmax", v),
        }
    }
}

/// Helper function to create a Taffy length dimension.
pub fn length(value: f32) -> taffy::Dimension {
    taffy::Dimension::Length(value)
}

/// Helper function to create a Taffy percent dimension.
pub fn percent(value: f32) -> taffy::Dimension {
    taffy::Dimension::Percent(value / 100.0)
}

/// Helper function to create auto dimension.
pub fn auto() -> taffy::Dimension {
    taffy::Dimension::Auto
}

/// Helper function to create a viewport width length.
///
/// # Examples
/// ```
/// use astrelis_ui::vw;
///
/// let width = vw(80.0); // 80% of viewport width
/// ```
pub fn vw(value: f32) -> Length {
    Length::Vw(value)
}

/// Helper function to create a viewport height length.
///
/// # Examples
/// ```
/// use astrelis_ui::vh;
///
/// let height = vh(100.0); // 100% of viewport height
/// ```
pub fn vh(value: f32) -> Length {
    Length::Vh(value)
}

/// Helper function to create a viewport minimum length.
///
/// # Examples
/// ```
/// use astrelis_ui::vmin;
///
/// let size = vmin(50.0); // 50% of smaller viewport dimension
/// ```
pub fn vmin(value: f32) -> Length {
    Length::Vmin(value)
}

/// Helper function to create a viewport maximum length.
///
/// # Examples
/// ```
/// use astrelis_ui::vmax;
///
/// let size = vmax(50.0); // 50% of larger viewport dimension
/// ```
pub fn vmax(value: f32) -> Length {
    Length::Vmax(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_length_px() {
        let len = Length::Px(100.0);
        assert!(len.is_px());
        assert!(!len.is_percent());
        assert!(!len.is_auto());
        assert_eq!(len.to_string(), "100px");
    }

    #[test]
    fn test_length_percent() {
        let len = Length::Percent(50.0);
        assert!(!len.is_px());
        assert!(len.is_percent());
        assert!(!len.is_auto());
        assert_eq!(len.to_string(), "50%");
    }

    #[test]
    fn test_length_auto() {
        let len = Length::Auto;
        assert!(!len.is_px());
        assert!(!len.is_percent());
        assert!(len.is_auto());
        assert_eq!(len.to_string(), "auto");
    }

    #[test]
    fn test_length_from_f32() {
        let len: Length = 100.0.into();
        assert_eq!(len, Length::Px(100.0));
    }

    #[test]
    fn test_length_to_dimension() {
        let len = Length::Px(100.0);
        let dim = len.to_dimension();
        assert!(matches!(dim, taffy::Dimension::Length(100.0)));

        let len = Length::Percent(50.0);
        let dim = len.to_dimension();
        assert!(matches!(dim, taffy::Dimension::Percent(0.5)));

        let len = Length::Auto;
        let dim = len.to_dimension();
        assert!(matches!(dim, taffy::Dimension::Auto));
    }

    #[test]
    fn test_length_percentage() {
        let lp = LengthPercentage::Px(100.0);
        assert_eq!(lp.to_string(), "100px");

        let lp = LengthPercentage::Percent(50.0);
        assert_eq!(lp.to_string(), "50%");
    }

    #[test]
    fn test_length_auto_conversion() {
        let la = LengthAuto::Px(100.0);
        assert_eq!(la.to_string(), "100px");

        let len: LengthAuto = Length::Percent(50.0).into();
        assert_eq!(len, LengthAuto::Percent(50.0));
    }

    #[test]
    fn test_helper_functions() {
        let dim = length(100.0);
        assert!(matches!(dim, taffy::Dimension::Length(100.0)));

        let dim = percent(50.0);
        assert!(matches!(dim, taffy::Dimension::Percent(0.5)));

        let dim = auto();
        assert!(matches!(dim, taffy::Dimension::Auto));
    }
}
